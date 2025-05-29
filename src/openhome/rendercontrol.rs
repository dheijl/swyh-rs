///
/// rendercontrol.rs
///
/// controller for avmedia renderers (audio only) using `OpenHome` and `AVTransport` protocol
///
///
use crate::{
    enums::streaming::StreamingFormat,
    globals::statics::{APP_VERSION, get_config},
};
use bitflags::bitflags;
use hashbrown::HashMap;
use log::{debug, error, info};
use std::collections::HashMap as StdHashMap;
use std::{
    net::{IpAddr, SocketAddr, UdpSocket},
    time::{Duration, Instant},
};
use strfmt::strfmt;
use url::Url;
use xml::reader::{EventReader, XmlEvent};

/// OH insert playlist template
static OH_INSERT_PL_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" \
xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:Insert xmlns:u=\"urn:av-openhome-org:service:Playlist:1\">\
<AfterId>0</AfterId>\
<Uri>{server_uri}</Uri>\
<Metadata>{didl_data}</Metadata>\
</u:Insert>\
</s:Body>\
</s:Envelope>";

/// AV `SetTransportURI` template
static AV_SET_TRANSPORT_URI_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" \
s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
<s:Body>\
<u:SetAVTransportURI xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\">\
<InstanceID>0</InstanceID>\
<CurrentURI>{server_uri}</CurrentURI>\
<CurrentURIMetaData>{didl_data}</CurrentURIMetaData>\
</u:SetAVTransportURI>\
</s:Body>\
</s:Envelope>";

/// didl protocolinfo
/// rf64 seems to work with L16, do we need a specific one?
static L16_PROT_INFO: &str = "http-get:*:audio/L16;rate={sample_rate};channels=2:DLNA.ORG_PN=LPCM";
static L24_PROT_INFO: &str = "http-get:*:audio/L24;rate={sample_rate};channels=2:DLNA.ORG_PN=LPCM";
static WAV_PROT_INFO: &str = "http-get:*:audio/wav:DLNA.ORG_PN=WAV;DLNA.ORG_OP=01;DLNA.ORG_CI=0;\
    DLNA.ORG_FLAGS=03700000000000000000000000000000";
static FLAC_PROT_INFO: &str = "http-get:*:audio/flac:DLNA.ORG_PN=FLAC;DLNA.ORG_OP=01;DLNA.ORG_CI=0;\
    DLNA.ORG_FLAGS=01700000000000000000000000000000";

/// didl metadata template
static DIDL_TEMPLATE: &str = "\
<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" \
xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\">\
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
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" \
xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
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
static AV_STOP_PLAY_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" \
xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:Stop xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\">\
<InstanceID>0</InstanceID>\
</u:Stop>\
</s:Body>\
</s:Envelope>";

/// OH get volume template, uses Volume service
static OH_GET_VOL_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" \
xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:Volume xmlns:u=\"urn:av-openhome-org:service:Volume:1\">\
</u:Volume>\
</s:Body>\
</s:Envelope>";

/// OH set volume template, uses Volume service
static OH_SET_VOL_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" \
xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:SetVolume xmlns:u=\"urn:av-openhome-org:service:SetVolume:1\">\
<Value>{volume}</Value>\
</u:SetVolume>\
</s:Body>\
</s:Envelope>";

/// AV get Volume template, uses `RenderingControl` service
static AV_GET_VOL_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" \
xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:GetVolume xmlns:u=\"urn:schemas-upnp-org:service:RenderingControl:1\">\
<InstanceID>0</InstanceID>\
<Channel>Master</Channel>\
</u:GetVolume>\
</s:Body>\
</s:Envelope>";

/// AV set Volume template, uses `RenderingControl` service
static AV_SET_VOL_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" \
xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:SetVolume xmlns:u=\"urn:schemas-upnp-org:service:RenderingControl:1\">\
<InstanceID>0</InstanceID>\
<Channel>Master</Channel>\
<DesiredVolume>{volume}</DesiredVolume>\
</u:SetVolume>\
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

#[derive(Debug, Clone, Copy)]
pub struct StreamInfo {
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub streaming_format: StreamingFormat,
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
#[derive(Debug, Clone, Copy)]
pub struct SupportedProtocols: u32 {
        const NONE        = 0b0000;
        const OPENHOME    = 0b0001;
        const AVTRANSPORT = 0b0010;
        const ALL = Self::OPENHOME.bits() | Self::AVTRANSPORT.bits();
    }
}

/// Renderer struct describers a media renderer,
/// info is collected from the GetDescription.xml
#[derive(Debug, Clone)]
pub struct Renderer {
    pub dev_name: String,
    pub dev_model: String,
    pub dev_type: String,
    pub dev_url: String,
    pub oh_control_url: String,
    pub av_control_url: String,
    pub oh_volume_url: String,
    pub av_volume_url: String,
    pub volume: i32,
    pub supported_protocols: SupportedProtocols,
    pub remote_addr: String,
    pub location: String,
    pub services: Vec<AvService>,
    pub playing: bool,
    host: String,
    port: u16,
    agent: ureq::Agent,
}

impl Renderer {
    fn new(agent: &ureq::Agent) -> Renderer {
        Renderer {
            dev_name: String::new(),
            dev_model: String::new(),
            dev_url: String::new(),
            dev_type: String::new(),
            oh_control_url: String::new(),
            av_control_url: String::new(),
            oh_volume_url: String::new(),
            av_volume_url: String::new(),
            volume: -1,
            supported_protocols: SupportedProtocols::NONE,
            remote_addr: String::new(),
            location: String::new(),
            services: Vec::with_capacity(8),
            playing: false,
            host: String::new(),
            port: 0,
            agent: agent.clone(),
        }
    }

    /// extract host and port from dev_url
    fn parse_url(&mut self, log: &dyn Fn(&str)) {
        let host: String;
        let port: u16;
        match Url::parse(&self.dev_url) {
            Ok(url) => {
                host = url.host_str().unwrap().to_string();
                port = url.port_or_known_default().unwrap();
            }
            Err(e) => {
                log(&format!(
                    "parse_url(): Error '{e}' while parsing base url '{}'",
                    self.dev_url
                ));
                host = "0.0.0.0".to_string();
                port = 0;
            }
        }
        self.host = host;
        self.port = port;
    }

    /// `oh_soap_request` - send an `OpenHome` SOAP message to a renderer
    fn soap_request(&mut self, url: &str, soap_action: &str, body: &str) -> Option<String> {
        debug!(
            "url: {},\r\n=>SOAP Action: {},\r\n=>SOAP xml: \r\n{}",
            url, soap_action, body
        );
        match self
            .agent
            .post(url)
            .header("User-Agent", format!("swyh-rs/{APP_VERSION}"))
            .header("Accept", "*/*")
            .header("SOAPAction", format!("\"{soap_action}\""))
            .header("Content-Type", "text/xml; charset=\"utf-8\"")
            .send(body)
        {
            Ok(mut resp) => {
                let xml = resp.body_mut().read_to_string().unwrap_or_default();
                debug!("<=SOAP response: {}\r\n", xml);
                Some(xml)
            }
            Err(e) => {
                error!("<= SOAP POST error: {}\r\n", e);
                None
            }
        }
    }

    /// get volume
    pub fn get_volume(&mut self, log: &dyn Fn(&str)) -> i32 {
        if self
            .supported_protocols
            .contains(SupportedProtocols::OPENHOME)
        {
            return self.oh_get_volume(log);
        } else if self
            .supported_protocols
            .contains(SupportedProtocols::AVTRANSPORT)
        {
            return self.av_get_volume(log);
        }
        -1
    }

    pub fn set_volume(&mut self, log: &dyn Fn(&str), vol: i32) {
        self.volume = vol;
        if self
            .supported_protocols
            .contains(SupportedProtocols::OPENHOME)
        {
            self.oh_set_volume(log);
        } else if self
            .supported_protocols
            .contains(SupportedProtocols::AVTRANSPORT)
        {
            self.av_set_volume(log);
        }
    }

    /// play - start play on this renderer, using Openhome if present, else `AvTransport` (if present)
    pub fn play(
        &mut self,
        local_addr: &IpAddr,
        server_port: u16,
        log: &dyn Fn(&str),
        streaminfo: StreamInfo,
    ) -> Result<(), &str> {
        // build the hashmap with the formatting vars for the OH and AV play templates
        let mut fmt_vars = StdHashMap::new();
        let addr = format!("{local_addr}:{server_port}");

        let local_url = match streaminfo.streaming_format {
            StreamingFormat::Wav => format!("http://{addr}/stream/swyh.wav"),
            StreamingFormat::Lpcm => format!("http://{addr}/stream/swyh.raw"),
            StreamingFormat::Flac => format!("http://{addr}/stream/swyh.flac"),
            StreamingFormat::Rf64 => format!("http://{addr}/stream/swyh.rf64"),
        };
        fmt_vars.insert("server_uri".to_string(), local_url);
        fmt_vars.insert(
            "bits_per_sample".to_string(),
            streaminfo.bits_per_sample.to_string(),
        );
        fmt_vars.insert(
            "sample_rate".to_string(),
            streaminfo.sample_rate.to_string(),
        );
        fmt_vars.insert("duration".to_string(), "00:00:00".to_string());
        let mut didl_prot: String;
        if streaminfo.streaming_format == StreamingFormat::Flac {
            didl_prot = htmlescape::encode_minimal(FLAC_PROT_INFO);
        } else if streaminfo.streaming_format == StreamingFormat::Wav
            || streaminfo.streaming_format == StreamingFormat::Rf64
        {
            didl_prot = htmlescape::encode_minimal(WAV_PROT_INFO);
        } else if streaminfo.bits_per_sample == 16 {
            didl_prot = htmlescape::encode_minimal(L16_PROT_INFO);
        } else {
            didl_prot = htmlescape::encode_minimal(L24_PROT_INFO);
        }
        match strfmt(&didl_prot, &fmt_vars) {
            Ok(s) => didl_prot = s,
            Err(e) => {
                didl_prot = format!("oh_play: error {e} formatting didl_prot");
                log(&didl_prot);
                return Err(BAD_TEMPL);
            }
        }
        fmt_vars.insert("didl_prot_info".to_string(), didl_prot);
        let mut didl_data = htmlescape::encode_minimal(DIDL_TEMPLATE);
        match strfmt(&didl_data, &fmt_vars) {
            Ok(s) => didl_data = s,
            Err(e) => {
                didl_data = format!("oh_play: error {e} formatting didl_data xml");
                log(&didl_data);
                return Err(BAD_TEMPL);
            }
        }
        fmt_vars.insert("didl_data".to_string(), didl_data);
        // now send the start playing commands
        if self
            .supported_protocols
            .contains(SupportedProtocols::OPENHOME)
        {
            log(&format!(
                "OH Start playing on {} host={} port={} from {local_addr} using OH Playlist",
                self.dev_name, self.host, self.port
            ));
            return self.oh_play(log, &fmt_vars);
        } else if self
            .supported_protocols
            .contains(SupportedProtocols::AVTRANSPORT)
        {
            log(&format!(
                "AV Start playing on {} host={} port={} from {local_addr} using AV Play",
                self.dev_name, self.host, self.port
            ));
            return self.av_play(log, &fmt_vars);
        }
        log("ERROR: play: no supported renderer protocol found");
        Ok(())
    }

    /// `oh_play` - set up a playlist on this `OpenHome` renderer and tell it to play it
    ///
    /// the renderer will then try to get the audio from our built-in webserver
    /// at http://{_`my_ip`_}:`{server_port}/stream/swyh.wav`
    fn oh_play(
        &mut self,
        log: &dyn Fn(&str),
        fmt_vars: &StdHashMap<String, String>,
    ) -> Result<(), &str> {
        // stop anything currently playing first, Moode needs it
        let url = format!("http://{}:{}{}", self.host, self.port, self.oh_control_url);
        self.oh_stop_play(&url, log);
        // Send the InsertPlayList command with metadate(DIDL-Lite)
        log(&format!(
            "OH Inserting new playlist on {} host={} port={}",
            self.dev_name, self.host, self.port
        ));
        let xmlbody = match strfmt(OH_INSERT_PL_TEMPLATE, fmt_vars) {
            Ok(s) => s,
            Err(e) => {
                log(&format!("oh_play: error {e} formatting oh playlist xml"));
                return Err(BAD_TEMPL);
            }
        };
        let _resp = self
            .soap_request(
                &url,
                "urn:av-openhome-org:service:Playlist:1#Insert",
                &xmlbody,
            )
            .unwrap_or_default();
        // send the Play command
        log(&format!(
            "OH Play on {} host={} port={}",
            self.dev_name, self.host, self.port
        ));
        let _resp = self
            .soap_request(
                &url,
                "urn:av-openhome-org:service:Playlist:1#Play",
                OH_PLAY_PL_TEMPLATE,
            )
            .unwrap_or_default();
        Ok(())
    }

    /// `av_play` - send the `AVTransport` URI to the player and tell it to play
    ///
    /// the renderer will then try to get the audio from our built-in webserver
    /// at http://{_`my_ip`_}:`{server_port}/stream/swyh.wav`
    fn av_play(
        &mut self,
        log: &dyn Fn(&str),
        fmt_vars: &StdHashMap<String, String>,
    ) -> Result<(), &str> {
        let url = format!("http://{}:{}{}", self.host, self.port, self.av_control_url);
        // to prevent error 705 (transport locked) on some devices
        // it's necessary to send a stop play request first
        self.av_stop_play(&url, log);
        // now send SetAVTransportURI with metadate(DIDL-Lite) and play requests
        let xmlbody = match strfmt(AV_SET_TRANSPORT_URI_TEMPLATE, fmt_vars) {
            Ok(s) => s,
            Err(e) => {
                log(&format!("av_play: error {e} formatting set transport uri"));
                return Err(BAD_TEMPL);
            }
        };
        let _resp = self
            .soap_request(
                &url,
                "urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI",
                &xmlbody,
            )
            .unwrap_or_default();
        // the renderer will now send a head request first, so wait a bit
        std::thread::sleep(Duration::from_millis(100));
        // send play command
        let _resp = self
            .soap_request(
                &url,
                "urn:schemas-upnp-org:service:AVTransport:1#Play",
                AV_PLAY_TEMPLATE,
            )
            .unwrap_or_default();
        Ok(())
    }

    /// `stop_play` - stop playing on this renderer (`OpenHome` or `AvTransport`)
    pub fn stop_play(&mut self, log: &dyn Fn(&str)) {
        let url = format!("http://{}:{}{}", self.host, self.port, self.oh_control_url);
        if self
            .supported_protocols
            .contains(SupportedProtocols::OPENHOME)
        {
            self.oh_stop_play(&url, log);
        } else if self
            .supported_protocols
            .contains(SupportedProtocols::AVTRANSPORT)
        {
            self.av_stop_play(&url, log);
        } else {
            log("ERROR: stop_play: no supported renderer protocol found");
        }
    }

    /// `oh_stop_play` - delete the playlist on the `OpenHome` renderer, so that it stops playing
    fn oh_stop_play(&mut self, url: &str, log: &dyn Fn(&str)) {
        log(&format!(
            "OH Delete playlist on {} => {}",
            self.dev_name, self.remote_addr
        ));

        // delete current playlist
        let _resp = self
            .soap_request(
                url,
                "urn:av-openhome-org:service:Playlist:1#DeleteAll",
                OH_DELETE_PL_TEMPLATE,
            )
            .unwrap_or_default();
    }

    /// `av_stop_play` - stop playing on the AV renderer
    fn av_stop_play(&mut self, url: &str, log: &dyn Fn(&str)) {
        log(&format!(
            "AV Stop playing on {} => {}",
            self.dev_name, self.remote_addr
        ));

        // delete current playlist
        let _resp = self
            .soap_request(
                url,
                "urn:schemas-upnp-org:service:AVTransport:1#Stop",
                AV_STOP_PLAY_TEMPLATE,
            )
            .unwrap_or_default();
    }

    fn oh_get_volume(&mut self, log: &dyn Fn(&str)) -> i32 {
        let url = format!("http://{}:{}{}", self.host, self.port, self.oh_volume_url);

        // get current volume
        let vol_xml = self
            .soap_request(
                &url,
                "urn:av-openhome-org:service:Volume:1#Volume",
                OH_GET_VOL_TEMPLATE,
            )
            .unwrap_or("<Error/>".to_string());
        // parse response to extract volume
        debug!("oh_get_volume response: {vol_xml}");
        let parser = EventReader::new(vol_xml.as_bytes());
        let mut cur_elem = String::new();
        let mut have_vol_response = false;
        let mut str_volume = "-1".to_string();
        for e in parser {
            match e {
                Ok(XmlEvent::StartElement { name, .. }) => {
                    cur_elem = name.local_name;
                    if cur_elem.contains("VolumeResponse") {
                        have_vol_response = true;
                    }
                }
                Ok(XmlEvent::Characters(value)) => {
                    if cur_elem.contains("Value") && have_vol_response {
                        str_volume = value;
                    }
                }
                Err(e) => {
                    error!("OH Volume XML parse error: {e}");
                }
                _ => {}
            }
        }
        self.volume = str_volume.parse::<i32>().unwrap_or(-1);
        log(&format!(
            "OH Get Volume on {} host={} port={} = {}%",
            self.dev_name, self.host, self.port, self.volume,
        ));
        self.volume
    }

    fn av_get_volume(&mut self, log: &dyn Fn(&str)) -> i32 {
        let url = format!("http://{}:{}{}", self.host, self.port, self.av_volume_url);

        // get current volume
        let vol_xml = self
            .soap_request(
                &url,
                "urn:schemas-upnp-org:service:RenderingControl:1#GetVolume",
                AV_GET_VOL_TEMPLATE,
            )
            .unwrap_or("<Error/>".to_string());
        debug!("av_get_volume response: {vol_xml}");
        let parser = EventReader::new(vol_xml.as_bytes());
        let mut cur_elem = String::new();
        let mut have_vol_response = false;
        let mut str_volume = "-1".to_string();
        for e in parser {
            match e {
                Ok(XmlEvent::StartElement { name, .. }) => {
                    cur_elem = name.local_name;
                    if cur_elem.contains("GetVolumeResponse") {
                        have_vol_response = true;
                    }
                }
                Ok(XmlEvent::Characters(value)) => {
                    if cur_elem.contains("CurrentVolume") && have_vol_response {
                        str_volume = value;
                    }
                }
                Err(e) => {
                    error!("AV Volume XML parse error: {e}");
                }
                _ => {}
            }
        }
        self.volume = str_volume.parse::<i32>().unwrap_or(-1);
        log(&format!(
            "AV Get Volume on {} host={} port={} = {}%",
            self.dev_name, self.host, self.port, self.volume,
        ));
        self.volume
    }

    fn oh_set_volume(&mut self, log: &dyn Fn(&str)) {
        let vol = self.volume;
        let tmpl = OH_SET_VOL_TEMPLATE.replace("{volume}", &vol.to_string());
        let url = format!("http://{}:{}{}", self.host, self.port, self.oh_volume_url);
        log(&format!(
            "OH Set New Volume on {} host={} port={}: {vol}%",
            self.dev_name, self.host, self.port
        ));
        // set new volume
        let vol_xml = self
            .soap_request(
                &url,
                "urn:av-openhome-org:service:Volume:1#SetVolume",
                &tmpl,
            )
            .unwrap_or("<Error/>".to_string());
        debug!("oh_set_volume response: {vol_xml}");
    }

    fn av_set_volume(&mut self, log: &dyn Fn(&str)) {
        let vol = self.volume;
        let tmpl = AV_SET_VOL_TEMPLATE.replace("{volume}", &vol.to_string());
        let url = format!("http://{}:{}{}", self.host, self.port, self.av_volume_url);
        log(&format!(
            "AV Set New Volume on {} host={} port={}: {vol}%",
            self.dev_name, self.host, self.port
        ));
        // set new volume
        let vol_xml = self
            .soap_request(
                &url,
                "urn:schemas-upnp-org:service:RenderingControl:1#SetVolume",
                &tmpl,
            )
            .unwrap_or("<Error/>".to_string());
        debug!("av_set_volume response: {vol_xml}");
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
    agent: ureq::Agent,
    rmap: &HashMap<String, Renderer>,
    logger: &dyn Fn(&str),
) -> Option<Vec<Renderer>> {
    const OH_DEVICE: &str = "urn:av-openhome-org:service:Product:1";
    const AV_DEVICE: &str = "urn:schemas-upnp-org:service:RenderingControl:1";
    const DEFAULT_SEARCH_TTL: u32 = 2;

    debug!("SSDP discovery started");

    // get the address of the selected interface
    let ip = get_config().last_network.as_ref().unwrap().clone();
    info!("running SSDP on {ip}");
    let local_addr: IpAddr = ip.parse().unwrap();
    let bind_addr = SocketAddr::new(local_addr, 0);
    let socket = UdpSocket::bind(bind_addr).unwrap();
    socket.set_broadcast(true).unwrap();
    socket.set_multicast_ttl_v4(DEFAULT_SEARCH_TTL).unwrap();

    // broadcast the M-SEARCH message (MX is 3 secs) and collect responses
    let mut oh_devices: Vec<(String, SocketAddr)> = Vec::new();
    let mut av_devices: Vec<(String, SocketAddr)> = Vec::new();
    let mut devices: Vec<(String, SocketAddr)> = Vec::new();
    //  SSDP UDP broadcast address
    let broadcast_address: SocketAddr = ([239, 255, 255, 250], 1900).into();
    let msg = SSDP_DISCOVER_MSG.replace("{device_type}", OH_DEVICE);
    socket.send_to(msg.as_bytes(), broadcast_address).unwrap();
    let msg = SSDP_DISCOVER_MSG.replace("{device_type}", AV_DEVICE);
    socket.send_to(msg.as_bytes(), broadcast_address).unwrap();
    // collect the responses and remeber all new renderers
    let start = Instant::now();
    loop {
        let duration = start.elapsed().as_millis() as u64;
        // keep capturing responses for 3.1 seconds
        if duration >= 3100 {
            break;
        }
        let max_wait_time = 3100 - duration;
        socket
            .set_read_timeout(Some(Duration::from_millis(max_wait_time)))
            .unwrap();
        let mut buf: [u8; 2048] = [0; 2048];
        let resp: String;
        match socket.recv_from(&mut buf) {
            Ok((received, from)) => {
                resp = std::str::from_utf8(&buf[0..received]).unwrap().to_string();
                debug!(
                    "SSDP: HTTP response at {} from {}: \r\n{}",
                    start.elapsed().as_millis(),
                    from,
                    resp
                );
                let response: Vec<&str> = resp.split("\r\n").collect();
                if !response.is_empty() {
                    let status_code = response[0]
                        .trim_start_matches("HTTP/1.1 ")
                        .chars()
                        .take_while(|x| x.is_ascii_digit())
                        .collect::<String>()
                        .parse::<u32>()
                        .unwrap_or(0);

                    if status_code != 200 {
                        error!("SSDP: HTTP error response status={status_code}");
                        continue; // ignore
                    }

                    let mut dev_location = String::new();
                    let mut oh_device = false;
                    let mut av_device = false;
                    response
                        .iter()
                        .filter_map(|l| {
                            let mut split = l.splitn(2, ':');
                            match (split.next(), split.next()) {
                                (Some(header), Some(value)) => Some((header, value.trim())),
                                _ => None,
                            }
                        })
                        .for_each(|hv_pair| match hv_pair.0.to_ascii_uppercase().as_str() {
                            "LOCATION" => dev_location = hv_pair.1.to_string(),
                            "ST" => match hv_pair.1 {
                                schema
                                    if schema.contains(
                                        "urn:schemas-upnp-org:service:RenderingControl:1",
                                    ) =>
                                {
                                    av_device = true;
                                }
                                schema
                                    if schema.contains("urn:av-openhome-org:service:Product:1") =>
                                {
                                    oh_device = true;
                                }
                                _ => (),
                            },
                            _ => (),
                        });
                    if !dev_location.is_empty() {
                        if av_device {
                            av_devices.push((dev_location.clone(), from));
                            debug!("SSDP Discovery: AV renderer: {dev_location}");
                        } else if oh_device {
                            oh_devices.push((dev_location.clone(), from));
                            debug!("SSDP Discovery: OH renderer: {dev_location}");
                        }
                    }
                }
            }
            Err(e) => {
                // ignore socket read timeout on Windows or EAGAIN/EWOULBLOCK on Linux/Unix/MacOS
                let error_text = e.to_string();
                let to_ignore = ["10060", "os error 11", "os error 35"]
                    .iter()
                    .any(|s| error_text.contains(*s));
                if !to_ignore {
                    logger(&format!("*E*E>Error reading SSDP M-SEARCH response: {e}"));
                }
            }
        }
    }

    // only keep OH devices and AV devices that are not OH capable
    let mut usable_devices: Vec<(String, SocketAddr)> =
        Vec::with_capacity(oh_devices.len() + av_devices.len());
    for (oh_location, sa) in &oh_devices {
        usable_devices.push((oh_location.to_string(), *sa));
    }
    for (av_location, sa) in &av_devices {
        if usable_devices.iter().any(|d| d.0 == *av_location) {
            debug!("SSDP Discovery: skipping AV renderer {av_location} as it is also OH");
        } else {
            usable_devices.push((av_location.to_string(), *sa));
        }
    }
    // now filter out devices we already know about
    for (location, sa) in &usable_devices {
        if rmap.iter().any(|m| *location == m.1.location) {
            info!("SSDP discovery: Skipping known Renderer at {location}");
        } else {
            info!("SSDP discovery: new Renderer found at : {}", location);
            devices.push((location.to_string(), *sa));
        }
    }

    // now get the new renderers description xml
    debug!("Getting new renderer descriptions");
    let mut renderers: Vec<Renderer> = Vec::with_capacity(devices.len());

    for (location, from) in devices {
        if let Some(xml) = get_service_description(&agent, &location) {
            if let Some(mut rend) = get_renderer(&agent, &xml) {
                rend.location = location.clone();
                let mut s = from.to_string();
                if let Some(i) = s.find(':') {
                    s.truncate(i);
                }
                rend.remote_addr = s;
                // check for an absent URLBase in the description
                // or devices like Yamaha WXAD-10 with bad URLBase port number
                if rend.dev_url.is_empty() || !location.contains(&rend.dev_url) {
                    let mut url_base = location;
                    if url_base.contains("http://") {
                        url_base = url_base["http://".to_string().len()..].to_string();
                        let pos = url_base.find('/').unwrap_or_default();
                        if pos > 0 {
                            url_base = url_base[0..pos].to_string();
                        }
                    }
                    rend.dev_url = format!("http://{url_base}/");
                }
                rend.parse_url(logger);
                renderers.push(rend);
            }
        }
    }

    for r in &renderers {
        debug!(
            "Renderer {} {} ip {} at location {} has {} services",
            r.dev_name,
            r.dev_model,
            r.remote_addr,
            r.location,
            r.services.len()
        );
        debug!(
            "  => OpenHome Playlist control url: '{}', AvTransport url: '{}'",
            r.oh_control_url, r.av_control_url
        );
        for s in &r.services {
            debug!(".. {} {} {}", s.service_type, s.service_id, s.control_url);
        }
    }
    debug!("SSDP discovery complete");
    Some(renderers)
}

/// `get_service_description` - get the upnp service description xml for a media renderer
fn get_service_description(agent: &ureq::Agent, location: &str) -> Option<String> {
    debug!("Get service description for {location}");
    match agent
        .get(location)
        .header("User-Agent", format!("swyh-rs/{APP_VERSION}"))
        .header("Content-Type", "text/xml")
        .call()
    {
        Ok(mut resp) => {
            let descr_xml = resp.body_mut().read_to_string().unwrap_or_default();
            debug!("Service description:");
            debug!("{}", descr_xml);
            if descr_xml.is_empty() {
                None
            } else {
                Some(descr_xml)
            }
        }
        Err(e) => {
            error!("Error {e} getting service description for {location}");
            None
        }
    }
}

/// build a renderer struct by (roughly) parsing the GetDescription.xml
fn get_renderer(agent: &ureq::Agent, xml: &str) -> Option<Renderer> {
    let parser = EventReader::new(xml.as_bytes());
    let mut cur_elem = String::new();
    let mut service = AvService::new();
    let mut renderer = Renderer::new(agent);
    for e in parser {
        match e {
            Ok(XmlEvent::StartElement { name, .. }) => {
                cur_elem = name.local_name;
            }
            Ok(XmlEvent::EndElement { name }) => {
                let end_elem = name.local_name;
                if end_elem == "service" {
                    match service.service_id {
                        ref id
                            if ["Playlist", "urn:av-openhome-org:service"]
                                .iter()
                                .all(|&p| id.contains(p)) =>
                        {
                            renderer.oh_control_url.clone_from(&service.control_url);
                            renderer.supported_protocols |= SupportedProtocols::OPENHOME;
                        }
                        ref id
                            if ["Volume", "urn:av-openhome-org:service"]
                                .iter()
                                .all(|&p| id.contains(p)) =>
                        {
                            renderer.oh_volume_url.clone_from(&service.control_url);
                        }
                        ref id if id.contains(":AVTransport") => {
                            renderer.av_control_url.clone_from(&service.control_url);
                            renderer.supported_protocols |= SupportedProtocols::AVTRANSPORT;
                        }
                        ref id if id.contains(":RenderingControl") => {
                            renderer.av_volume_url.clone_from(&service.control_url);
                        }
                        _ => (),
                    }
                    renderer.services.push(service);
                    service = AvService::new();
                }
            }
            Ok(XmlEvent::Characters(value)) => match cur_elem {
                // these values come from various tags, ignoring xml hierarchy
                ref el if el.contains("serviceType") => service.service_type = value,
                ref el if el.contains("serviceId") => service.service_id = value,
                ref el if el.contains("modelName") => renderer.dev_model = value,
                ref el if el.contains("friendlyName") => renderer.dev_name = value,
                ref el if el.contains("deviceType") => renderer.dev_type = value,
                ref el if el.contains("URLBase") => renderer.dev_url = value,
                ref el if el.contains("controlURL") => service.control_url = normalize_url(&value),
                _ => (),
            },
            Err(e) => {
                error!("SSDP Get Renderer Description Error: {e}");
                return None;
            }
            _ => {}
        }
    }

    Some(renderer)
}

/// sometimes the control url is not prefixed with a '/'
fn normalize_url(value: &str) -> String {
    if value.is_empty() || value.starts_with('/') {
        value.to_owned()
    } else {
        '/'.to_string() + value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log(_s: &str) {}

    #[test]
    fn renderer() {
        let mut rend = Renderer::new(&ureq::agent());
        rend.dev_url = "http://192.168.1.26:80/".to_string();
        rend.parse_url(&log);
        assert_eq!(rend.host, "192.168.1.26");
        assert_eq!(rend.port, 80); // default port
        rend.dev_url = "http://192.168.1.26:12345/".to_string();
        rend.parse_url(&log);
        assert_eq!(rend.host, "192.168.1.26");
        assert_eq!(rend.port, 12345); // other port
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

    #[test]
    fn test_contains() {
        let ok_errors = ["10060", "os error 11", "os error 35"];
        let mut e = "bla bla os error 11 bla bla";
        let to_ignore = ok_errors.iter().any(|s| e.contains(*s));
        assert!(to_ignore);
        e = "bla bla os error 12 bla bla";
        let to_ignore = ok_errors.iter().any(|s| e.contains(*s));
        assert!(!to_ignore);
    }

    #[test]
    fn test_format() {
        let bps = 24;
        let format = StreamingFormat::Flac;
        let url = "http://192.168.0.135:5901/Stream/Swyh.raw".to_lowercase();
        let (req_bps, req_format) = {
            if let Some(format_start) = url.find("/stream/swyh.") {
                match url.get(format_start + 13..) {
                    Some("flac") => (24, StreamingFormat::Flac),
                    Some("wav") => (16, StreamingFormat::Wav),
                    Some("rf64") => (16, StreamingFormat::Rf64),
                    Some("raw") => (16, StreamingFormat::Lpcm),
                    None | Some(&_) => (bps, format),
                }
            } else {
                (bps, format)
            }
        };
        assert!(req_format == StreamingFormat::Lpcm);
        assert!(req_bps == 16);
    }

    #[test]
    fn test_normalize() {
        let mut url = "/ctl".to_string();
        assert!(normalize_url(&url) == *"/ctl");
        url = "ctl".to_string();
        assert!(normalize_url(&url) == *"/ctl");
        url = String::new();
        assert!(normalize_url(&url) == url);
    }

    #[test]
    fn test_bubble() {
        static BUBBLE_SSDP: &str = "HTTP/1.1 200 OK
Ext:
St: urn:schemas-upnp-org:service:RenderingControl:1
Server: Linux/6.8.4-3-pve UPnP/1.0 BubbleUPnPServer/0.9-update49
Usn: uuid:e8dbf26b-de8f-4c96-0000-0000002ea642::urn:schemas-upnp-org:service:RenderingControl:1
Cache-control: max-age=1800\r\n
Location: http://192.168.1.181:33065/dev/e8dbf26b-de8f-4c96-0000-0000002ea642/desc.xml
";
        let response: Vec<&str> = BUBBLE_SSDP.split("\n").collect();
        if !response.is_empty() {
            let status_code = response[0]
                .trim_start_matches("HTTP/1.1 ")
                .chars()
                .take_while(|x| x.is_ascii_digit())
                .collect::<String>()
                .parse::<u32>()
                .unwrap_or(0);

            assert!(status_code == 200);

            let mut dev_url = String::new();
            let mut oh_device = false;
            let mut av_device = false;
            response
                .iter()
                .filter_map(|l| {
                    let mut split = l.splitn(2, ':');
                    match (split.next(), split.next()) {
                        (Some(header), Some(value)) => Some((header, value.trim())),
                        _ => None,
                    }
                })
                .for_each(|hv_pair| match hv_pair.0.to_ascii_uppercase().as_str() {
                    "LOCATION" => dev_url = hv_pair.1.to_string(),
                    "ST" => match hv_pair.1 {
                        schema
                            if schema
                                .contains("urn:schemas-upnp-org:service:RenderingControl:1") =>
                        {
                            av_device = true;
                        }
                        schema if schema.contains("urn:av-openhome-org:service:Product:1") => {
                            oh_device = true;
                        }
                        _ => (),
                    },
                    _ => eprintln!("{} = {}", hv_pair.0, hv_pair.1),
                });
            eprintln!("{dev_url}");
            eprintln!("{oh_device}");
            eprintln!("{av_device}");
            assert!(!dev_url.is_empty());
            assert!(av_device);
            assert!(!oh_device);
        }
    }
}
