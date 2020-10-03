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
static _AV_SET_TRANSPORT_URI_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
<s:Body>\
<u:SetAVTransportURI xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\">\
<InstanceID>0</InstanceID>\
<CurrentURI>{server_uri}</CurrentURI>\
<CurrentURIMetaData>{didl_data}</CurrentURIMetaData>\
</u:SetAVTransportURI>
</s:Body>\
</s:Envelope>";


/// didl metadata template
static DIDL_TEMPLATE: &str = "\
<DIDL-Lite>\
<item>\
<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\">\
<item id=\"1\" parentID=\"0\" restricted=\"0\">\
<dc:title>swyh-rs</dc:title>\
<res bitsPerSample=\"16\" \
nrAudioChannels=\"2\" \
protocolInfo=\"http-get:*:audio/wav:*\" \
sampleFrequency=\"44100\">{server_uri}</res>\
<upnp:class>object.item.audioItem.musicTrack</upnp:class>\
</item>\
</DIDL-Lite>\
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
static _AV_PLAY_TEMPLATE: &str = "\
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
static _AV_STOP_PLAY_TEMPLATE: &str ="\
<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:Stop xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\">\
<InstanceID>0</InstanceID>\
</u:Stop>\
</s:Body>\
</s:Envelope>";

/// Renderer struct describers a media renderer, info is collected from GetDescription.xml
#[derive(Debug, Clone)]
pub struct Renderer {
    pub dev_name: String,
    pub dev_model: String,
    pub dev_type: String,
    pub dev_url: String,
    pub pl_control_url: String,
    pub vol_control_url: String,
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
            vol_control_url: String::new(),
            pl_control_url: String::new(),
            remote_addr: String::new(),
            services: Vec::new(),
        }
    }

    fn parse_url(&self, dev_url: String, log: &dyn Fn(String)) -> (String, u16) {
        let host: String;
        let port: u16;
        match Url::parse(&dev_url) {
            Ok(url) => {
                host = url.host_str().unwrap().to_string();
                port = url.port().unwrap();
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
    fn oh_soap_request(&self, url: &String, soap_action: &String, body: &String) -> Option<String> {
        DEBUG!(eprintln!(
            "url: {}, SOAP Action: {}, SOAP xml body \r\n{}",
            url.clone(),
            soap_action,
            body
        ));
        let resp = ureq::post(url.as_str())
            .set("Connection", "close")
            .set("User-Agent", "swyh-rs-Rust/0.x")
            .set("Accept", "*/*")
            .set("SOAPAction", &format!("\"{}\"", soap_action))
            .set("Content-Type", "text/xml; charset=\"utf-8\"")
            .send_string(body);
        let xml = resp.into_string().unwrap();
        DEBUG!(eprintln!("resp: {}", xml));

        Some(xml)
    }

    /// oh_play - set up a playlist on this OpenHome renderer and tell it to play it
    ///
    /// the renderer will then try to get the audio from our built-in webserver
    /// at http://{_my_ip_}:{server_port}/stream/swyh.wav  

    pub fn oh_play(
        &self,
        local_addr: &IpAddr,
        server_port: u16,
        log: &dyn Fn(String),
    ) -> Result<(), ureq::Error> {
        let url = self.dev_url.clone();
        let (host, port) = self.parse_url(url, log);
        log(format!(
            "Start playing on {} host={} port={} from {}",
            self.dev_name, host, port, local_addr
        ));
        let url = format!("http://{}:{}{}", host, port, self.pl_control_url);
        let addr = format!("{}:{}", local_addr, server_port);
        let local_url = format!("http://{}/stream/swyh.wav", addr);
        DEBUG!(eprintln!("OHPlaylist server URL: {}", local_url.clone()));
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
        vars.insert("server_uri".to_string(), local_url.clone());
        let mut didl_data = htmlescape::encode_minimal(&DIDL_TEMPLATE);
        didl_data = strfmt(&didl_data, &vars).unwrap();
        vars.insert("didl_data".to_string(), didl_data);
        let xmlbody = strfmt(&INSERT_PL_TEMPLATE, &vars).unwrap();
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
        DEBUG!(eprintln!("SeekId: {}", seek_id.clone()));
        // send seek_id
        vars.insert("seek_id".to_string(), seek_id);
        let xmlbody = strfmt(&SEEKID_PL_TEMPLATE, &vars).unwrap();
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

    /// oh_stop_play - delete the playlist on the OpenHome renderer, so that it stops playing
    pub fn oh_stop_play(&self, log: &dyn Fn(String)) {
        let url = self.dev_url.clone();
        let (host, port) = self.parse_url(url, log);
        log(format!(
            "Stop playing on {} host={} port={}",
            self.dev_name, host, port
        ));
        let url = format!("http://{}:{}{}", host, port, self.pl_control_url);

        // delete current playlist
        let _resp = self
            .oh_soap_request(
                &url,
                &"urn:av-openhome-org:service:Playlist:1#DeleteAll".to_string(),
                &DELETE_PL_TEMPLATE.to_string(),
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
    logger(format!("SSDP discovery started"));

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
                if response.len() > 0 {
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

    logger(format!("Getting renderer descriptions"));
    let mut renderers: Vec<Renderer> = Vec::new();

    for (dev, from) in devices {
        match get_service_description(&dev) {
            Some(xml) => match get_renderer(&xml) {
                Some(mut rend) => {
                    let mut s = from.to_string();
                    match s.find(':') {
                        Some(i) => {
                            s.truncate(i);
                        }
                        None => {}
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
                None => {}
            },
            None => {}
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
        for s in r.services.iter() {
            DEBUG!(eprintln!(
                ".. {} {} {}",
                s.service_type, s.service_id, s.control_url
            ));
        }
    }
    logger(format!("SSDP discovery complete"));
    Some(renderers)
}

/// get_service_description - get the upnp service description xml for a media renderer
fn get_service_description(dev_url: &String) -> Option<String> {
    DEBUG!(eprintln!("Get service description for {}", dev_url.clone()));
    let url = dev_url.clone();
    let resp = ureq::get(url.as_str())
        .set("User-Agent", "swyh-rs-Rust")
        .set("Content-Type", "text/xml")
        .send_string("");
    if resp.error() {
        return None;
    }
    let descr_xml = resp.into_string().unwrap_or_default();
    DEBUG!(eprintln!("Service description: \r\n{}", descr_xml));
    if !descr_xml.is_empty() {
        Some(descr_xml)
    } else {
        None
    }
}

/// build a renderer struct by parsing the GetDescription.xml
fn get_renderer(xml: &String) -> Option<Renderer> {
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
                        renderer.pl_control_url = service.control_url.clone();
                    } else if service.service_id.contains("Volume") {
                        renderer.vol_control_url = service.control_url.clone();
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
