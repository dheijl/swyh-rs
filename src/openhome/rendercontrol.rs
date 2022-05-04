///
/// rendercontrol.rs
///
/// controller for avmedia renderers (audio only) using OpenHome protocol
///
/// Only tested with Volumio streamers (https://volumio.org/)
///
///
use crate::CONFIG;
use log::{debug, error, info};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};
use strfmt::strfmt;
use stringreader::StringReader;
use url::Url;
use xml::reader::{EventReader, XmlEvent};

/// OH insert playlist template
static OH_INSERT_PL_TEMPLATE: &str = "\
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

/// didl protocolinfo
static L16_PROT_INFO: &str = "http-get:*:audio/L16;rate={sample_rate};channels=2:DLNA.ORG_PN=LPCM";
static L24_PROT_INFO: &str = "http-get:*:audio/L24;rate={sample_rate};channels=2:DLNA.ORG_PN=LPCM";
static WAV_PROT_INFO: &str = "http-get:*:audio/wav:DLNA.ORG_PN=WAV;DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=03700000000000000000000000000000";

/// didl metadata template
static DIDL_TEMPLATE: &str = "\
<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\">\
<item id=\"1\" parentID=\"0\" restricted=\"0\">\
<dc:title>swyh-rs</dc:title>\
<res bitsPerSample=\"{bits_per_sample}\" \
nrAudioChannels=\"2\" \
sampleFrequency=\"{sample_rate}\" \
protocolInfo=\"{didl_prot_info}\" \
duration=\"{duration}\" >{server_uri}</res>\
<upnp:class>object.item.audioItem.musicTrack</upnp:class>\
</item>\
</DIDL-Lite>";

/// OH play playlist template
static OH_PLAY_PL_TEMPLATE: &str = "\
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
static OH_DELETE_PL_TEMPLATE: &str = "\
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

/// Bad XML template error
static BAD_TEMPL: &str = "Bad xml template (strfmt)";

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

bitflags! {
/// supported UPNP/DLNA protocols
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
                    "parse_url(): Error '{e}' while parsing base url '{dev_url}'"
                ));
                host = "0.0.0.0".to_string();
                port = 0;
            }
        }
        (host, port)
    }

    /// oh_soap_request - send an OpenHome SOAP message to a renderer
    fn soap_request(&self, url: &str, soap_action: &str, body: &str) -> Option<String> {
        debug!(
            "url: {},\r\n=>SOAP Action: {},\r\n=>SOAP xml: \r\n{}",
            url.to_string(),
            soap_action,
            body
        );
        match ureq::post(url)
            .set("Connection", "close")
            .set("User-Agent", "swyh-rs-Rust/0.x")
            .set("Accept", "*/*")
            .set("SOAPAction", &format!("\"{soap_action}\""))
            .set("Content-Type", "text/xml; charset=\"utf-8\"")
            .send_string(body)
        {
            Ok(resp) => {
                let xml = resp.into_string().unwrap();
                debug!("<=SOAP response: {}\r\n", xml);
                Some(xml)
            }
            Err(e) => {
                error!("<= SOAP POST error: {}\r\n", e);
                None
            }
        }
    }

    /// play - start play on this renderer, using Openhome if present, else AvTransport (if present)
    pub fn play(
        &self,
        local_addr: &IpAddr,
        server_port: u16,
        wd: &WavData,
        log: &dyn Fn(String),
        use_wav_format: bool,
        bits_per_sample: u16,
    ) -> Result<(), &str> {
        // build the hashmap with the formatting vars for the OH and AV play templates
        let mut fmt_vars = HashMap::new();
        let (host, port) = self.parse_url(&self.dev_url, log);
        let addr = format!("{local_addr}:{server_port}");
        let local_url = format!("http://{addr}/stream/swyh.wav");
        fmt_vars.insert("server_uri".to_string(), local_url);
        fmt_vars.insert("bits_per_sample".to_string(), bits_per_sample.to_string());
        fmt_vars.insert("sample_rate".to_string(), wd.sample_rate.0.to_string());
        fmt_vars.insert("duration".to_string(), "00:00:00".to_string());
        let mut didl_prot: String;
        if use_wav_format {
            didl_prot = htmlescape::encode_minimal(WAV_PROT_INFO);
        } else {
            if bits_per_sample == 16 {
                didl_prot = htmlescape::encode_minimal(L16_PROT_INFO);
            } else {
                didl_prot = htmlescape::encode_minimal(L24_PROT_INFO);
            }
            match strfmt(&didl_prot, &fmt_vars) {
                Ok(s) => didl_prot = s,
                Err(e) => {
                    didl_prot = format!("oh_play: error {e} formatting didl_prot");
                    log(didl_prot.clone());
                    return Err(BAD_TEMPL);
                }
            }
        }
        fmt_vars.insert("didl_prot_info".to_string(), didl_prot);
        let mut didl_data = htmlescape::encode_minimal(DIDL_TEMPLATE);
        match strfmt(&didl_data, &fmt_vars) {
            Ok(s) => didl_data = s,
            Err(e) => {
                didl_data = format!("oh_play: error {e} formatting didl_data xml");
                log(didl_data.clone());
                return Err(BAD_TEMPL);
            }
        }
        fmt_vars.insert("didl_data".to_string(), didl_data);
        // now send the start playing commands
        if self
            .supported_protocols
            .contains(SupportedProtocols::OPENHOME)
        {
            let url = format!("http://{host}:{port}{}", self.oh_control_url);
            log(format!(
            "OH Start playing on {} host={host} port={port} from {local_addr} using OpenHome Playlist",
            self.dev_name));
            return self.oh_play(log, &url, &fmt_vars);
        } else if self
            .supported_protocols
            .contains(SupportedProtocols::AVTRANSPORT)
        {
            let url = format!("http://{host}:{port}{}", self.av_control_url);
            log(format!(
            "AV Start playing on {} host={host} port={port} from {local_addr} using AvTransport Play",
            self.dev_name));
            return self.av_play(log, &url, &fmt_vars);
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
        log: &dyn Fn(String),
        url: &str,
        fmt_vars: &HashMap<String, String>,
    ) -> Result<(), &str> {
        // stop anything currently playing first, Moode needs it
        self.oh_stop_play(log);
        // Send the InsertPlayList command with metadate(DIDL-Lite)
        let xmlbody = match strfmt(OH_INSERT_PL_TEMPLATE, fmt_vars) {
            Ok(s) => s,
            Err(e) => {
                let errmsg = format!("oh_play: error {e} formatting oh playlist xml");
                log(errmsg);
                return Err(BAD_TEMPL);
            }
        };
        let _resp = self
            .soap_request(
                url,
                "urn:av-openhome-org:service:Playlist:1#Insert",
                &xmlbody,
            )
            .unwrap_or_default();
        // send the Play command
        let _resp = self
            .soap_request(
                url,
                "urn:av-openhome-org:service:Playlist:1#Play",
                OH_PLAY_PL_TEMPLATE,
            )
            .unwrap_or_default();
        Ok(())
    }

    /// av_play - send the AVTransport URI to the player and tell it to play
    ///
    /// the renderer will then try to get the audio from our built-in webserver
    /// at http://{_my_ip_}:{server_port}/stream/swyh.wav  
    fn av_play(
        &self,
        log: &dyn Fn(String),
        url: &str,
        fmt_vars: &HashMap<String, String>,
    ) -> Result<(), &str> {
        // to prevent error 705 (transport locked) on some devices
        // it's necessary to send a stop play request first
        self.av_stop_play(log);
        // now send SetAVTransportURI with metadate(DIDL-Lite) and play requests
        let xmlbody = match strfmt(AV_SET_TRANSPORT_URI_TEMPLATE, fmt_vars) {
            Ok(s) => s,
            Err(e) => {
                let errmsg = format!("av_play: error {e} formatting set transport uri");
                log(errmsg);
                return Err(BAD_TEMPL);
            }
        };
        let _resp = self
            .soap_request(
                url,
                "urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI",
                &xmlbody,
            )
            .unwrap_or_default();
        // the renderer will now send a head request first, so wait a bit
        std::thread::sleep(Duration::from_millis(100));
        // send play command
        let _resp = self
            .soap_request(
                url,
                "urn:schemas-upnp-org:service:AVTransport:1#Play",
                AV_PLAY_TEMPLATE,
            )
            .unwrap_or_default();
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
            "OH Stop playing on {} host={host} port={port}",
            self.dev_name
        ));
        let url = format!("http://{host}:{port}{}", self.oh_control_url);

        // delete current playlist
        let _resp = self
            .soap_request(
                &url,
                "urn:av-openhome-org:service:Playlist:1#DeleteAll",
                OH_DELETE_PL_TEMPLATE,
            )
            .unwrap_or_default();
    }

    /// av_stop_play - stop playing on the AV renderer
    fn av_stop_play(&self, log: &dyn Fn(String)) {
        let (host, port) = self.parse_url(&self.dev_url, log);
        log(format!(
            "AV Stop playing on {} host={host} port={port}",
            self.dev_name
        ));
        let url = format!("http://{host}:{port}{}", self.av_control_url);

        // delete current playlist
        let _resp = self
            .soap_request(
                &url,
                "urn:schemas-upnp-org:service:AVTransport:1#Stop",
                AV_STOP_PLAY_TEMPLATE,
            )
            .unwrap_or_default();
    }
}

// SSDP UDP search message for media renderers with a 3.0 second MX response time
static SSDP_DISCOVER_MSG: &str = "M-SEARCH * HTTP/1.1\r\n\
Host: 239.255.255.250:1900\r\n\
Man: \"ssdp:discover\"\r\n\
ST: {device_type}\r\n\
MX: 3\r\n\r\n";

//
// SSDP UPNP service discovery
//
// returns a list of all AVTransport DLNA and Openhome rendering devices
//
pub fn discover(
    rmap: &HashMap<String, Renderer>,
    logger: &dyn Fn(String),
) -> Option<Vec<Renderer>> {
    debug!("SSDP discovery started");

    const OH_DEVICE: &str = "urn:av-openhome-org:service:Product:1";
    const AV_DEVICE: &str = "urn:schemas-upnp-org:service:RenderingControl:1";

    // get the address of the selected interface
    let local_addr = CONFIG.read().last_network.parse().unwrap();
    let bind_addr = SocketAddr::new(local_addr, 0);
    let socket = UdpSocket::bind(&bind_addr).unwrap();
    let _ = socket.set_broadcast(true).unwrap();

    // broadcast the M-SEARCH message (MX is 3 secs) and collect responses
    let mut oh_devices: Vec<(String, SocketAddr)> = Vec::new();
    let mut av_devices: Vec<(String, SocketAddr)> = Vec::new();
    let mut devices: Vec<(String, SocketAddr)> = Vec::new();
    //  SSDP UDP broadcast address
    let broadcast_address: SocketAddr = ([239, 255, 255, 250], 1900).into();
    let msg = SSDP_DISCOVER_MSG.replace("{device_type}", OH_DEVICE);
    socket.send_to(msg.as_bytes(), &broadcast_address).unwrap();
    let msg = SSDP_DISCOVER_MSG.replace("{device_type}", AV_DEVICE);
    socket.send_to(msg.as_bytes(), &broadcast_address).unwrap();
    // collect the responses and remeber all new renderers
    let start = Instant::now();
    loop {
        let duration = start.elapsed().as_millis() as u64;
        // keep capturing responses for 3.1 seconds
        if duration >= 3100 {
            break;
        }
        let max_wait_time = 3100 - duration;
        let _ = socket
            .set_read_timeout(Some(Duration::from_millis(max_wait_time)))
            .unwrap();
        let mut buf: [u8; 2048] = [0; 2048];
        let resp: String;
        match socket.recv_from(&mut buf) {
            Ok((received, from)) => {
                resp = std::str::from_utf8(&buf[0..received]).unwrap().to_string();
                debug!(
                    "UDP response at {} from {}: \r\n{}",
                    start.elapsed().as_millis(),
                    from,
                    resp
                );
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
                    let mut dev_url: String = String::new();
                    let mut oh_device = false;
                    let mut av_device = false;
                    for (header, value) in iter {
                        if header.to_ascii_uppercase() == "LOCATION" {
                            dev_url = value.to_string();
                        } else if header.to_ascii_uppercase() == "ST" {
                            if value.contains("urn:schemas-upnp-org:service:RenderingControl:1") {
                                av_device = true;
                            } else if value.contains("urn:av-openhome-org:service:Product:1") {
                                oh_device = true;
                            }
                        }
                    }
                    if oh_device {
                        oh_devices.push((dev_url.clone(), from));
                        debug!("SSDP Discovery: OH renderer: {}", dev_url);
                    }
                    if av_device {
                        av_devices.push((dev_url.clone(), from));
                        debug!("SSDP Discovery: AV renderer: {}", dev_url);
                    }
                }
            }
            Err(e) => {
                // ignore socket read timeout on Windows or EAGAIN on Linux
                if !(e.to_string().contains("10060") || e.to_string().contains("os error 11")) {
                    logger(format!("*E*E>Error reading SSDP M-SEARCH response: {e}"));
                }
            }
        }
    }

    // only keep OH devices and AV devices that are not OH capable
    let mut usable_devices: Vec<(String, SocketAddr)> = Vec::new();
    for (oh_url, sa) in oh_devices.iter() {
        usable_devices.push((oh_url.to_string(), *sa));
    }
    for (av_url, sa) in av_devices.iter() {
        if !usable_devices.iter().any(|d| d.0 == *av_url) {
            usable_devices.push((av_url.to_string(), *sa));
        } else {
            debug!(
                "SSDP Discovery: skipping AV renderer {} as it is also OH",
                av_url
            );
        }
    }
    // now filter out devices we already know about
    for (url, sa) in usable_devices.iter() {
        if !rmap.iter().any(|m| url.contains(&m.1.dev_url)) {
            info!("SSDP discovery: new Renderer found at : {}", url);
            devices.push((url.to_string(), *sa));
        } else {
            info!("SSDP discovery: Skipping known Renderer at {}", url);
        }
    }

    // now get the new renderers description xml
    debug!("Getting new renderer descriptions");
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
                    rend.dev_url = format!("http://{url_base}/");
                }
                renderers.push(rend);
            }
        }
    }

    for r in renderers.iter() {
        debug!(
            "Renderer {} {} ip {} at urlbase {} has {} services",
            r.dev_name,
            r.dev_model,
            r.remote_addr,
            r.dev_url,
            r.services.len()
        );
        debug!(
            "  => OpenHome Playlist control url: '{}', AvTransport url: '{}'",
            r.oh_control_url, r.av_control_url
        );
        for s in r.services.iter() {
            debug!(".. {} {} {}", s.service_type, s.service_id, s.control_url);
        }
    }
    debug!("SSDP discovery complete");
    Some(renderers)
}

/// get_service_description - get the upnp service description xml for a media renderer
fn get_service_description(dev_url: &str) -> Option<String> {
    debug!("Get service description for {}", dev_url.to_string());
    let url = dev_url.to_string();
    match ureq::get(url.as_str())
        .set("User-Agent", "swyh-rs-Rust")
        .set("Content-Type", "text/xml")
        .send_string("")
    {
        Ok(resp) => {
            let descr_xml = resp.into_string().unwrap_or_default();
            debug!("Service description:");
            debug!("{}", descr_xml);
            if !descr_xml.is_empty() {
                Some(descr_xml)
            } else {
                None
            }
        }
        Err(e) => {
            error!("Error {} getting service description for {}", e, url);
            None
        }
    }
}

/// build a renderer struct by parsing the GetDescription.xml
fn get_renderer(xml: &str) -> Option<Renderer> {
    let xmlstream = StringReader::new(xml);
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
                    if !service.control_url.is_empty() && !service.control_url.starts_with('/') {
                        service.control_url.insert(0, '/');
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
                error!("SSDP Get Renderer Description Error: {}", e);
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
        if !url.is_empty() && !url.starts_with('/') {
            url.insert(0, '/');
        }
        assert_eq!(url, "/Avcontrol.url");
        url = "/Avcontrol.url".to_string();
        if !url.is_empty() && !url.starts_with('/') {
            url.insert(0, '/');
        }
        assert_eq!(url, "/Avcontrol.url");
        url = "".to_string();
        if !url.is_empty() && !url.starts_with('/') {
            url.insert(0, '/');
        }
        assert_eq!(url, "");
        url = "A/.url".to_string();
        if !url.is_empty() && !url.starts_with('/') {
            url.insert(0, '/');
        }
        assert_eq!(url, "/A/.url");
    }
}
