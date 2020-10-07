///
/// avmedia.rs
///
/// controller for avmedia renderers (audio only) using OpenHome protocol
///
/// Only tested with Volumio streamers (https://volumio.org/)
///
///
/*
MIT License

Copyright (c) 2020 dheijl

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/
use std::collections::HashMap;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::time::{Duration, Instant};
use strfmt::strfmt;
use stringreader::StringReader;
use url::Url;
use xml::reader::{EventReader, XmlEvent};

macro_rules! DEBUG {
    ($x:stmt) => {
        if cfg!(debug_assertions) {
            $x
        }
    };
}

// some audio config info
#[derive(Debug, Clone, Copy)]
pub struct WavData {
    pub sample_format: cpal::SampleFormat,
    pub sample_rate: cpal::SampleRate,
    pub channels: u16,
}

/// An UPNP/DLNA service desciption
#[derive(Debug, Clone)]
pub struct AvService {
    service_id: String,
    service_type: String,
    control_url: String,
}

impl AvService {
    fn new() -> AvService {
        AvService {
            service_id: String::new(),
            service_type: String::new(),
            control_url: String::new(),
        }
    }
}

/// OH insert playlist template
static INSERT_PL_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:Insert xmlns:u=\"urn:av-openhome-org:service:Playlist:1\">\
<AfterId>0</AfterId>\
<Uri>{server_uri}</Uri>\
<Metadata>{didl_data}</Metadata>\
</u:Insert>\
</s:Body>\
</s:Envelope>";

/// AV SetTransportURI template
static AV_SET_TRANSPORT_URI_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
<s:Body>\
<u:SetAVTransportURI xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\">\
<InstanceID>0</InstanceID>\
<CurrentURI>{server_uri}</CurrentURI>\
<CurrentURIMetaData>{didl_data}</CurrentURIMetaData>\
</u:SetAVTransportURI>\
</s:Body>\
</s:Envelope>";

/// didl metadata template
static DIDL_TEMPLATE: &str = "\
<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\">\
<item id=\"1\" parentID=\"0\" restricted=\"0\">\
<dc:title>swyh-rs</dc:title>\
<res bitsPerSample=\"16\" \
nrAudioChannels=\"2\" \
protocolInfo=\"http-get:*:audio/l16;rate={sample_rate};channels=2:DLNA.ORG_PN=LPCM\" \
sampleFrequency=\"{sample_rate}\">{server_uri}</res>\
<upnp:class>object.item.audioItem.musicTrack</upnp:class>\
</item>\
</DIDL-Lite>";

/// OH seek id templete
static SEEKID_PL_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" \
xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:SeekId xmlns:u=\"urn:av-openhome-org:service:Playlist:1\">\
<Value>{seek_id}</Value>\
</u:SeekId>\
</s:Body>\
</s:Envelope>";

/// OH play playlist template
static PLAY_PL_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" \
xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:Play xmlns:u=\"urn:av-openhome-org:service:Playlist:1\"/>\
</s:Body>\
</s:Envelope>";

/// AV Play template
static AV_PLAY_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:Play xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\">\
<InstanceID>0</InstanceID>\
<Speed>1</Speed>\
</u:Play>\
</s:Body>\
</s:Envelope>";

/// OH delete playlist template
static DELETE_PL_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" \
xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:DeleteAll xmlns:u=\"urn:av-openhome-org:service:Playlist:1\"/>\
</s:Body>\
</s:Envelope>";

/// AV Stop play template
static AV_STOP_PLAY_TEMPLATE: &str ="\
<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:Stop xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\">\
<InstanceID>0</InstanceID>\
</u:Stop>\
</s:Body>\
</s:Envelope>";

bitflags! {
    pub struct SupportedProtocols: u32 {
        const NONE        = 0b0000;
        const OPENHOME    = 0b0001;
        const AVTRANSPORT = 0b0010;
        const ALL = Self::OPENHOME.bits | Self::AVTRANSPORT.bits;
    }
}

/// Renderer struct describers a media renderer, info is collected from GetDescription.xml
#[derive(Debug, Clone)]
pub struct Renderer {
    pub dev_name: String,
    pub dev_model: String,
    pub dev_type: String,
    pub dev_url: String,
    pub oh_control_url: String,
    pub av_control_url: String,
    pub supported_protocols: SupportedProtocols,
    pub remote_addr: String,
    pub services: Vec<AvService>,
}

impl Renderer {
    fn new() -> Renderer {
        Renderer {
            dev_name: String::new(),
            dev_url: String::new(),
            dev_model: String::new(),
            dev_type: String::new(),
            av_control_url: String::new(),
            oh_control_url: String::new(),
            supported_protocols: SupportedProtocols::NONE,
            remote_addr: String::new(),
            services: Vec::new(),
        }
    }

    fn parse_url(&self, dev_url: &str, log: &dyn Fn(String)) -> (String, u16) {
        let host: String;
        let port: u16;
        match Url::parse(dev_url) {
            Ok(url) => {
                host = url.host_str().unwrap().to_string();
                port = url.port_or_known_default().unwrap();
            }
            Err(e) => {
                log(format!(
                    "parse_url(): Error '{}' while parsing base url '{}'",
                    e, dev_url
                ));
                host = "0.0.0.0".to_string();
                port = 0;
            }
        }
        (host, port)
    }

    /// oh_soap_request - send an OpenHome SOAP message to a renderer
    fn oh_soap_request(&self, url: &str, soap_action: &str, body: &str) -> Option<String> {
        DEBUG!(eprintln!(
            "url: {},\r\n=>SOAP Action: {},\r\n=>SOAP xml: \r\n{}",
            url.to_string(),
            soap_action,
            body
        ));
        let resp = ureq::post(url)
            .set("Connection", "close")
            .set("User-Agent", "swyh-rs-Rust/0.x")
            .set("Accept", "*/*")
            .set("SOAPAction", &format!("\"{}\"", soap_action))
            .set("Content-Type", "text/xml; charset=\"utf-8\"")
            .send_string(body);
        let xml = resp.into_string().unwrap();
        DEBUG!(eprintln!("<=SOAP response: {}\r\n", xml));

        Some(xml)
    }

    /// play - start play on this renderer, using Openhome if present, else AvTransport (if present)
    pub fn play(
        &self,
        local_addr: &IpAddr,
        server_port: u16,
        wd: &WavData,
        log: &dyn Fn(String),
    ) -> Result<(), ureq::Error> {
        if self
            .supported_protocols
            .contains(SupportedProtocols::OPENHOME)
        {
            return self.oh_play(local_addr, server_port, wd, log);
        } else if self
            .supported_protocols
            .contains(SupportedProtocols::AVTRANSPORT)
        {
            return self.av_play(local_addr, server_port, wd, log);
        } else {
            log("ERROR: play: no supported renderer protocol found".to_string());
        }
        Ok(())
    }

    /// oh_play - set up a playlist on this OpenHome renderer and tell it to play it
    ///
    /// the renderer will then try to get the audio from our built-in webserver
    /// at http://{_my_ip_}:{server_port}/stream/swyh.wav  
    fn oh_play(
        &self,
        local_addr: &IpAddr,
        server_port: u16,
        wd: &WavData,
        log: &dyn Fn(String),
    ) -> Result<(), ureq::Error> {
        let (host, port) = self.parse_url(&self.dev_url, log);
        log(format!(
            "OH Start playing on {} host={} port={} from {} using OpenHome Playlist",
            self.dev_name, host, port, local_addr
        ));
        let url = format!("http://{}:{}{}", host, port, self.oh_control_url);
        let addr = format!("{}:{}", local_addr, server_port);
        let local_url = format!("http://{}/stream/swyh.wav", addr);
        DEBUG!(eprintln!("OHPlaylist server URL: {}", local_url));
        // delete current playlist
        let _resp = self
            .oh_soap_request(
                &url,
                &"urn:av-openhome-org:service:Playlist:1#DeleteAll".to_string(),
                &DELETE_PL_TEMPLATE.to_string(),
            )
            .unwrap();
        // create new playlist
        let mut vars = HashMap::new();
        vars.insert("server_uri".to_string(), local_url);
        vars.insert("sample_rate".to_string(), wd.sample_rate.0.to_string());
        let mut didl_data = htmlescape::encode_minimal(DIDL_TEMPLATE);
        match strfmt(&didl_data, &vars) {
            Ok(s) => didl_data = s,
            Err(e) => {
                didl_data = format!("oh_play: error {} formatting didl_data xml", e);
                log(didl_data.clone());
                return Err(ureq::Error::BadUrl("bad xml".to_string()));
            }
        }
        vars.insert("didl_data".to_string(), didl_data);
        let mut xmlbody: String;
        match strfmt(INSERT_PL_TEMPLATE, &vars) {
            Ok(s) => xmlbody = s,
            Err(e) => {
                xmlbody = format!("oh_play: error {} formatting oh playlist xml", e);
                log(xmlbody);
                return Err(ureq::Error::BadUrl("bad xml".to_string()));
            }
        }
        let resp = self
            .oh_soap_request(
                &url,
                &"urn:av-openhome-org:service:Playlist:1#Insert".to_string(),
                &xmlbody,
            )
            .unwrap();
        // extract new seek id
        let mut seek_id = String::new();
        if resp.contains("NewId") {
            let s = resp.find("<NewId>").unwrap();
            let e = resp.find("</NewId>").unwrap();
            seek_id = resp.as_str()[s + 7..e].to_string();
        }
        DEBUG!(eprintln!("SeekId: {}", seek_id));
        // send seek_id
        vars.insert("seek_id".to_string(), seek_id);
        match strfmt(SEEKID_PL_TEMPLATE, &vars) {
            Ok(s) => xmlbody = s,
            Err(e) => {
                xmlbody = format!("oh_play: error {} formatting seekid xml", e);
                log(xmlbody);
                return Err(ureq::Error::BadUrl("bad xml".to_string()));
            }
        }
        let _resp = self
            .oh_soap_request(
                &url,
                &"urn:av-openhome-org:service:Playlist:1#SeekId".to_string(),
                &xmlbody,
            )
            .unwrap();
        // send play command
        let _resp = self
            .oh_soap_request(
                &url,
                &"urn:av-openhome-org:service:Playlist:1#Play".to_string(),
                &PLAY_PL_TEMPLATE.to_string(),
            )
            .unwrap();
        Ok(())
    }

    /// av_play - send the AVTransport URI to the player and tell it to play
    ///
    /// the renderer will then try to get the audio from our built-in webserver
    /// at http://{_my_ip_}:{server_port}/stream/swyh.wav  
    fn av_play(
        &self,
        local_addr: &IpAddr,
        server_port: u16,
        wd: &WavData,
        log: &dyn Fn(String),
    ) -> Result<(), ureq::Error> {
        // to prevent error 705 (transport locked) on some devices
        // it's necessary to send a stop play request first
        self.av_stop_play(log);
        // now send AVTransportURI with metadate(DIDL-Lite) and play requests
        let (host, port) = self.parse_url(&self.dev_url, log);
        log(format!(
            "AV Start playing on {} host={} port={} from {} using AvTransport Play",
            self.dev_name, host, port, local_addr
        ));
        let url = format!("http://{}:{}{}", host, port, self.av_control_url);
        let addr = format!("{}:{}", local_addr, server_port);
        let local_url = format!("http://{}/stream/swyh.wav", addr);
        DEBUG!(eprintln!("AvTransport server URL: {}", local_url));
        // set AVTransportURI
        let mut vars = HashMap::new();
        vars.insert("server_uri".to_string(), local_url);
        vars.insert("sample_rate".to_string(), wd.sample_rate.0.to_string());
        let mut didl_data = htmlescape::encode_minimal(DIDL_TEMPLATE);
        match strfmt(&didl_data, &vars) {
            Ok(s) => didl_data = s,
            Err(e) => {
                didl_data = format!("av_play: error {} formatting didl_data", e);
                log(didl_data.clone());
                return Err(ureq::Error::BadUrl("bad xml".to_string()));
            }
        }
        vars.insert("didl_data".to_string(), didl_data);
        let xmlbody: String;
        match strfmt(AV_SET_TRANSPORT_URI_TEMPLATE, &vars) {
            Ok(s) => xmlbody = s,
            Err(e) => {
                xmlbody = format!("av_play: error {} formatting set transport uri", e);
                log(xmlbody);
                return Err(ureq::Error::BadUrl("bad xml".to_string()));
            }
        }
        let _resp = self
            .oh_soap_request(
                &url,
                &"urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI".to_string(),
                &xmlbody,
            )
            .unwrap();
        // send play command
        let _resp = self
            .oh_soap_request(
                &url,
                &"urn:schemas-upnp-org:service:AVTransport:1#Play".to_string(),
                &AV_PLAY_TEMPLATE.to_string(),
            )
            .unwrap();
        Ok(())
    }

    /// stop_play - stop playing on this renderer (OpenHome or AvTransport)
    pub fn stop_play(&self, log: &dyn Fn(String)) {
        if self
            .supported_protocols
            .contains(SupportedProtocols::OPENHOME)
        {
            self.oh_stop_play(log)
        } else if self
            .supported_protocols
            .contains(SupportedProtocols::AVTRANSPORT)
        {
            self.av_stop_play(log)
        } else {
            log("ERROR: stop_play: no supported renderer protocol found".to_string());
        }
    }

    /// oh_stop_play - delete the playlist on the OpenHome renderer, so that it stops playing
    fn oh_stop_play(&self, log: &dyn Fn(String)) {
        let (host, port) = self.parse_url(&self.dev_url, log);
        log(format!(
            "OH Stop playing on {} host={} port={}",
            self.dev_name, host, port
        ));
        let url = format!("http://{}:{}{}", host, port, self.oh_control_url);

        // delete current playlist
        let _resp = self
            .oh_soap_request(
                &url,
                &"urn:av-openhome-org:service:Playlist:1#DeleteAll".to_string(),
                &DELETE_PL_TEMPLATE.to_string(),
            )
            .unwrap();
    }

    /// av_stop_play - stop playing on the AV renderer
    fn av_stop_play(&self, log: &dyn Fn(String)) {
        let (host, port) = self.parse_url(&self.dev_url, log);
        log(format!(
            "AV Stop playing on {} host={} port={}",
            self.dev_name, host, port
        ));
        let url = format!("http://{}:{}{}", host, port, self.av_control_url);

        // delete current playlist
        let _resp = self
            .oh_soap_request(
                &url,
                &"urn:schemas-upnp-org:service:AVTransport:1#Stop".to_string(),
                &AV_STOP_PLAY_TEMPLATE.to_string(),
            )
            .unwrap();
    }
}

// SSDP UDP search message for media renderers with a 3.0 second MX response time
static SSDP_DISCOVER_MSG: &str = "M-SEARCH * HTTP/1.1\r\n\
Host: 239.255.255.250:1900\r\n\
Man: \"ssdp:discover\"\r\n\
ST: urn:schemas-upnp-org:service:RenderingControl:1\r\n\
MX: 3\r\n\r\n";

pub fn discover(logger: &dyn Fn(String)) -> Option<Vec<Renderer>> {
    logger("SSDP discovery started".to_string());

    // get the address of the internet connected interface
    let any: SocketAddr = ([0, 0, 0, 0], 0).into();
    let socket = UdpSocket::bind(any).expect("Could not bind the UDP socket to INADDR_ANY");
    let googledns: SocketAddr = ([8, 8, 8, 8], 80).into();
    socket.connect(googledns).expect("No network connectivity");
    let bind_addr = socket
        .local_addr()
        .expect("Could not obtain local ip address for udp broadcast socket");
    let bind_addr = SocketAddr::new(bind_addr.ip(), 0);
    let socket = UdpSocket::bind(&bind_addr).unwrap();
    let _ = socket
        .set_read_timeout(Some(Duration::from_millis(250)))
        .unwrap();
    let _ = socket.set_broadcast(true).unwrap();

    // broadcast the M-SEARCH message (MX is 3 secs)
    //  SSDP UDP broadcast address
    let broadcast_address: SocketAddr = ([239, 255, 255, 250], 1900).into();
    socket
        .send_to(SSDP_DISCOVER_MSG.as_bytes(), &broadcast_address)
        .unwrap();

    // collect the responses and remeber all renderers
    let mut devices: Vec<(String, SocketAddr)> = Vec::new();
    let start = Instant::now();
    loop {
        let duration = start.elapsed();
        // keep capturing responses for 3.1 seconds
        if duration > Duration::from_millis(3100) {
            break;
        }
        let mut buf: [u8; 2048] = [0; 2048];
        let resp: String;
        match socket.recv_from(&mut buf) {
            Ok((received, from)) => {
                resp = std::str::from_utf8(&buf[0..received]).unwrap().to_string();
                DEBUG!(eprintln!(
                    "UDP response at {} from {}: \r\n{}",
                    start.elapsed().as_millis(),
                    from,
                    resp
                ));
                let response: Vec<&str> = resp.split("\r\n").collect();
                if !response.is_empty() {
                    let status_code = response[0]
                        .trim_start_matches("HTTP/1.1 ")
                        .chars()
                        .take_while(|x| x.is_numeric())
                        .collect::<String>()
                        .parse::<u32>()
                        .unwrap_or(0);

                    if status_code != 200 {
                        continue; // ignore
                    }

                    let iter = response.iter().filter_map(|l| {
                        let mut split = l.splitn(2, ':');
                        match (split.next(), split.next()) {
                            (Some(header), Some(value)) => Some((header, value.trim())),
                            _ => None,
                        }
                    });
                    let mut is_renderer = false;
                    let mut dev_url: String = String::new();
                    for (header, value) in iter {
                        if header == "LOCATION" {
                            dev_url = value.to_string();
                        } else if header == "ST" && value.contains("RenderingControl") {
                            is_renderer = true;
                        }
                    }
                    if is_renderer {
                        logger(format!("Renderer at : {}", dev_url.clone()));
                        devices.push((dev_url, from));
                    }
                } else {
                    continue;
                }
            }
            Err(_e) => {}
        }
    }

    logger("Getting renderer descriptions".to_string());
    let mut renderers: Vec<Renderer> = Vec::new();

    for (dev, from) in devices {
        if let Some(xml) = get_service_description(&dev) {
            if let Some(mut rend) = get_renderer(&xml) {
                let mut s = from.to_string();
                if let Some(i) = s.find(':') {
                    s.truncate(i);
                }
                rend.remote_addr = s;
                // check for an absent URLBase in the description
                if rend.dev_url.is_empty() {
                    let mut url_base = dev;
                    if url_base.contains("http://") {
                        url_base = url_base["http://".to_string().len()..].to_string();
                        let pos = url_base.find('/').unwrap_or_default();
                        if pos > 0 {
                            url_base = url_base[0..pos].to_string();
                        }
                    }
                    rend.dev_url = format!("http://{}/", url_base);
                }
                renderers.push(rend);
            }
        }
    }

    for r in renderers.iter() {
        logger(format!(
            "Renderer {} {} ip {} at urlbase {} has {} services",
            r.dev_name,
            r.dev_model,
            r.remote_addr,
            r.dev_url,
            r.services.len()
        ));
        logger(format!(
            "  => OpenHome Playlist control url: '{}', AvTransport url: '{}'",
            r.oh_control_url, r.av_control_url
        ));
        for s in r.services.iter() {
            DEBUG!(eprintln!(
                ".. {} {} {}",
                s.service_type, s.service_id, s.control_url
            ));
        }
    }
    logger("SSDP discovery complete".to_string());
    Some(renderers)
}

/// get_service_description - get the upnp service description xml for a media renderer
fn get_service_description(dev_url: &str) -> Option<String> {
    DEBUG!(eprintln!(
        "Get service description for {}",
        dev_url.to_string()
    ));
    let url = dev_url.to_string();
    let resp = ureq::get(url.as_str())
        .set("User-Agent", "swyh-rs-Rust")
        .set("Content-Type", "text/xml")
        .send_string("");
    if resp.error() {
        return None;
    }
    let descr_xml = resp.into_string().unwrap_or_default();
    DEBUG!(eprintln!("Service description:"));
    DEBUG!(eprintln!("{}", descr_xml));
    if !descr_xml.is_empty() {
        Some(descr_xml)
    } else {
        None
    }
}

/// build a renderer struct by parsing the GetDescription.xml
fn get_renderer(xml: &str) -> Option<Renderer> {
    let xmlstream = StringReader::new(&xml);
    let parser = EventReader::new(xmlstream);
    let mut cur_elem = String::new();
    let mut service = AvService::new();
    let mut renderer = Renderer::new();
    for e in parser {
        match e {
            Ok(XmlEvent::StartElement { name, .. }) => {
                cur_elem = name.local_name;
            }
            Ok(XmlEvent::EndElement { name }) => {
                let end_elem = name.local_name;
                if end_elem == "service" {
                    if service.service_id.contains("Playlist") {
                        renderer.oh_control_url = service.control_url.clone();
                        renderer.supported_protocols |= SupportedProtocols::OPENHOME;
                    } else if service.service_id.contains("AVTransport") {
                        renderer.av_control_url = service.control_url.clone();
                        renderer.supported_protocols |= SupportedProtocols::AVTRANSPORT;
                    }
                    renderer.services.push(service);
                    service = AvService::new();
                }
            }
            Ok(XmlEvent::Characters(value)) => {
                if cur_elem.contains("serviceType") {
                    service.service_type = value;
                } else if cur_elem.contains("serviceId") {
                    service.service_id = value;
                } else if cur_elem.contains("controlURL") {
                    service.control_url = value;
                    // sometimes the control url is not prefixed with a '/'
                    if !service.control_url.is_empty() {
                        let vchars: Vec<char> = service.control_url.chars().collect();
                        if vchars[0] != '/' {
                            service.control_url.insert(0, '/');
                        }
                    }
                } else if cur_elem.contains("modelName") {
                    renderer.dev_model = value;
                } else if cur_elem.contains("friendlyName") {
                    renderer.dev_name = value;
                } else if cur_elem.contains("deviceType") {
                    renderer.dev_type = value;
                } else if cur_elem.contains("URLBase") {
                    renderer.dev_url = value;
                }
            }
            Err(e) => {
                DEBUG!(eprintln!("Error: {}", e));
                return None;
            }
            _ => {}
        }
    }

    Some(renderer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(_s: String) {}

    #[test]
    fn renderer() {
        let renderer = Renderer::new();
        let (host, port) = renderer.parse_url("http://192.168.1.26:80/", &log);
        assert_eq!(host, "192.168.1.26");
        assert_eq!(port, 80); // default port
        let (host, port) = renderer.parse_url("http://192.168.1.26:12345/", &log);
        assert_eq!(host, "192.168.1.26");
        assert_eq!(port, 12345); // other port
    }

    #[test]
    fn control_url_harman_kardon() {
        let mut url = "Avcontrol.url".to_string();
        if url.len() > 0 {
            let vchars: Vec<char> = url.chars().collect();
            if vchars[0] != '/' {
                url.insert(0, '/');
            }
        }
        assert_eq!(url, "/Avcontrol.url");
    }
}
