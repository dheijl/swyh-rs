//! [`Renderer`]'s full `impl` block: construction/setup (`new`, `parse_url`)
//! plus SOAP-based play/stop/volume control, driving both the OpenHome
//! Playlist and UPnP AVTransport protocols.

#[cfg(feature = "gui")]
use super::types::RendUI;
use super::types::{Controller, Renderer, StreamInfo, SupportedProtocols};
use crate::{
    enums::{
        messages::MessageType,
        streaming::{BitDepth, StreamingFormat},
    },
    globals::statics::{APP_VERSION, THREAD_STACK, get_msgchannel},
    utils::ui_logger::{LogCategory, ui_log},
};
use ecow::EcoString;
use figura::{Context, Template, Value};
#[cfg(feature = "gui")]
use fltk::app;
use fluent_uri::Uri;
use log::{debug, error};
use std::{
    net::IpAddr,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use xml::reader::{EventReader, XmlEvent};

/// a Figura Template with Curly Braces as delimiter
type CbTemplate = Template<'{', '}'>;

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
static BAD_TEMPL: &str = "Error parsing/formatting XML template.";

/// Compiled figura templates, shared across threads.
/// `Template` is `Send + Sync` as of figura 3.0, so these can live in one
/// process-wide static instead of being recompiled per thread-local.
struct CompiledTemplates {
    flac_prot: CbTemplate,
    wav_prot: CbTemplate,
    l16_prot: CbTemplate,
    l24_prot: CbTemplate,
    didl: CbTemplate,
    oh_insert_pl: CbTemplate,
    av_set_transport_uri: CbTemplate,
}

/// Compiled once, on first use, and shared by all threads.
static TEMPLATES: LazyLock<CompiledTemplates> = LazyLock::new(|| {
    debug!("Compiling figura HTTP templates");
    CompiledTemplates {
        flac_prot: CbTemplate::compile(htmlescape::encode_minimal(FLAC_PROT_INFO))
            .expect("static FLAC prot info template is invalid"),
        wav_prot: CbTemplate::compile(htmlescape::encode_minimal(WAV_PROT_INFO))
            .expect("static WAV prot info template is invalid"),
        l16_prot: CbTemplate::compile(htmlescape::encode_minimal(L16_PROT_INFO))
            .expect("static L16 prot info template is invalid"),
        l24_prot: CbTemplate::compile(htmlescape::encode_minimal(L24_PROT_INFO))
            .expect("static L24 prot info template is invalid"),
        didl: CbTemplate::compile(htmlescape::encode_minimal(DIDL_TEMPLATE))
            .expect("static DIDL template is invalid"),
        oh_insert_pl: CbTemplate::compile(OH_INSERT_PL_TEMPLATE)
            .expect("static OH insert playlist template is invalid"),
        av_set_transport_uri: CbTemplate::compile(AV_SET_TRANSPORT_URI_TEMPLATE)
            .expect("static AV set transport URI template is invalid"),
    }
});

/// `soap_request` - send a SOAP message to a renderer over `agent`
///
/// Free function (not a `Renderer`/`Controller` method) since it only ever
/// needs the `ureq::Agent`, and both types need to call it.
fn soap_request(agent: &ureq::Agent, url: &str, soap_action: &str, body: &str) -> Option<String> {
    debug!("url: {url},\r\n=>SOAP Action: {soap_action},\r\n=>SOAP xml: \r\n{body}");
    match agent
        .post(url)
        .header("User-Agent", format!("swyh-rs/{APP_VERSION}"))
        .header("Accept", "*/*")
        .header("SOAPAction", format!("\"{soap_action}\""))
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .send(body)
    {
        Ok(mut resp) => {
            let xml = resp.body_mut().read_to_string().unwrap_or_default();
            debug!("<=SOAP response: {xml}\r\n");
            Some(xml)
        }
        Err(e) => {
            error!("<= SOAP POST error: {e}\r\n");
            None
        }
    }
}

impl Renderer {
    pub(super) fn new(agent: &ureq::Agent) -> Renderer {
        Renderer {
            player_index: 0,
            dev_model: String::new(),
            dev_url: String::new(),
            dev_type: String::new(),
            oh_control_url: String::new(),
            av_control_url: String::new(),
            oh_volume_url: String::new(),
            av_volume_url: String::new(),
            volume: -1,
            location: String::new(),
            services: Vec::with_capacity(8),
            playing: false,
            #[cfg(feature = "gui")]
            rend_ui: RendUI::default(),
            controller: Arc::new(Controller {
                dev_name: String::new(),
                host: String::new(),
                port: 0,
                remote_addr: EcoString::new(),
                oh_control_full_url: String::new(),
                av_control_full_url: String::new(),
                oh_volume_full_url: String::new(),
                av_volume_full_url: String::new(),
                supported_protocols: SupportedProtocols::NONE,
                agent: agent.clone(),
            }),
            play_pending: Arc::new(AtomicBool::new(false)),
        }
    }

    /// extract host and port from device url
    pub(super) fn parse_url(&mut self) {
        let host: String;
        let port: u16;
        match Uri::parse(self.dev_url.as_str()) {
            Ok(url) => {
                if let Some(auth) = url.authority() {
                    host = auth.host().to_string();
                    port = auth
                        .port()
                        .and_then(|p| p.as_str().parse::<u16>().ok())
                        .unwrap_or(0);
                } else {
                    host = "0.0.0.0".to_string();
                    port = 0;
                }
            }
            Err(e) => {
                ui_log(
                    LogCategory::Info,
                    &format!(
                        "parse_url(): Error '{e}' while parsing base url '{}'",
                        self.dev_url
                    ),
                );
                host = "0.0.0.0".to_string();
                port = 0;
            }
        }
        // path fields (oh/av control/volume urls) are already set by the service
        // discovery XML parsing that runs before parse_url(), so it's safe to
        // compose and cache the absolute URLs here, once, instead of re-formatting
        // them on every play/stop/volume call
        let oh_control_full_url = format!("http://{host}:{port}{}", self.oh_control_url);
        let av_control_full_url = format!("http://{host}:{port}{}", self.av_control_url);
        let oh_volume_full_url = format!("http://{host}:{port}{}", self.oh_volume_url);
        let av_volume_full_url = format!("http://{host}:{port}{}", self.av_volume_url);
        let c = Arc::make_mut(&mut self.controller);
        c.oh_control_full_url = oh_control_full_url;
        c.av_control_full_url = av_control_full_url;
        c.oh_volume_full_url = oh_volume_full_url;
        c.av_volume_full_url = av_volume_full_url;
        c.host = host;
        c.port = port;
    }

    /// get volume
    pub fn get_volume(&mut self) -> i32 {
        self.volume = self.controller.get_volume();
        self.volume
    }

    /// Runs synchronously on the calling thread and blocks on the SOAP
    /// round-trip; use [`Renderer::spawn_set_volume`] to run this off the
    /// caller's thread (e.g. the FLTK UI thread) instead.
    pub fn set_volume(&mut self, vol: i32) {
        self.volume = vol;
        self.controller.set_volume(vol);
    }

    /// Set the volume on this renderer on a background thread, mirroring
    /// [`Renderer::spawn_play`]/[`Renderer::spawn_stop_play`] so an
    /// interactive volume-slider drag (an FLTK callback) never blocks its
    /// caller on the SOAP round-trip either. Fire-and-forget: `ui_log`
    /// inside `Controller` already forwards success/failure to the UI over
    /// the message channel, so there's no result to deliver back here.
    pub fn spawn_set_volume(&mut self, vol: i32) {
        self.volume = vol;
        let handler = self.controller.clone();
        let spawned = thread::Builder::new()
            .name("renderer_set_volume".into())
            .stack_size(THREAD_STACK)
            .spawn(move || handler.set_volume(vol));
        if let Err(e) = spawned {
            ui_log(
                LogCategory::Error,
                &format!("set_volume: failed to spawn volume thread: {e}"),
            );
        }
    }

    /// play - start play on this renderer, using Openhome if present, else `AvTransport` (if present)
    ///
    /// Runs synchronously on the calling thread and blocks on the SOAP
    /// round-trips; use [`Renderer::spawn_play`] to run this off the caller's
    /// thread (e.g. the FLTK UI thread) instead.
    pub fn play(
        &mut self,
        local_addr: &IpAddr,
        streaminfo: StreamInfo,
    ) -> Result<(), &'static str> {
        self.controller.play(local_addr, streaminfo)
    }

    /// `stop_play` - stop playing on this renderer (`OpenHome` or `AvTransport`)
    pub fn stop_play(&mut self) {
        self.controller.stop_play();
    }

    /// Start playing on this renderer on a background thread, so the caller
    /// is never blocked on the renderer's SOAP round-trips. A play already in
    /// flight for this renderer (tracked via `play_pending`, shared across
    /// every clone made from the same discovered instance) makes this a
    /// no-op, so overlapping calls (e.g. a double click racing an
    /// auto-resume) can't interleave `stop`/`SetTransportURI`/`Play` requests
    /// against the same physical device. The outcome is delivered back on
    /// the UI thread as `MessageType::PlayResult`.
    pub fn spawn_play(&self, local_addr: IpAddr, streaminfo: StreamInfo) {
        if self
            .play_pending
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            ui_log(
                LogCategory::Info,
                &format!(
                    "play: {} already starting, ignoring",
                    self.controller.dev_name
                ),
            );
            return;
        }
        let handler = self.controller.clone();
        let pending = self.play_pending.clone();
        let spawned = thread::Builder::new()
            .name("renderer_play".into())
            .stack_size(THREAD_STACK)
            .spawn(move || {
                let result = handler.play(&local_addr, streaminfo);
                pending.store(false, Ordering::Release);
                let _ = get_msgchannel()
                    .0
                    .send(MessageType::PlayResult(PlayOutcome {
                        remote_addr: handler.remote_addr.clone(),
                        result,
                    }));
                #[cfg(feature = "gui")]
                app::awake();
            });
        if let Err(e) = spawned {
            self.play_pending.store(false, Ordering::Release);
            ui_log(
                LogCategory::Error,
                &format!("play: failed to spawn play thread: {e}"),
            );
        }
    }

    /// Stop playing on this renderer on a background thread, mirroring
    /// [`Renderer::spawn_play`] so an interactive stop (e.g. the FLTK button
    /// callback) never blocks its caller on the SOAP round-trips either.
    /// Shares `play_pending` with `spawn_play`, so a stop can't race a play
    /// already in flight for this renderer (or vice versa) and is ignored
    /// (as a no-op) if one is.
    ///
    /// For shutdown paths that must guarantee the stop was actually sent
    /// before the process exits, use the synchronous [`Renderer::stop_play`]
    /// instead.
    pub fn spawn_stop_play(&self) {
        if self
            .play_pending
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            ui_log(
                LogCategory::Info,
                &format!("stop_play: {} busy, ignoring", self.controller.dev_name),
            );
            return;
        }
        let handler = self.controller.clone();
        let pending = self.play_pending.clone();
        let spawned = thread::Builder::new()
            .name("renderer_stop".into())
            .stack_size(THREAD_STACK)
            .spawn(move || {
                handler.stop_play();
                pending.store(false, Ordering::Release);
            });
        if let Err(e) = spawned {
            self.play_pending.store(false, Ordering::Release);
            ui_log(
                LogCategory::Error,
                &format!("stop_play: failed to spawn stop thread: {e}"),
            );
        }
    }
}

/// Outcome of a `play()` attempt kicked off on a background thread by
/// [`Renderer::spawn_play`], delivered back to the UI thread via
/// `MessageType::PlayResult` once the SOAP round-trips finish.
#[derive(Debug, Clone)]
pub struct PlayOutcome {
    pub remote_addr: EcoString,
    pub result: Result<(), &'static str>,
}

/// [`Controller`]'s `impl` block: `Renderer::play`/`Renderer::stop_play`/
/// `Renderer::get_volume`/`Renderer::set_volume` all delegate to the methods
/// here (directly, or via [`Renderer::spawn_play`],
/// [`Renderer::spawn_stop_play`], [`Renderer::spawn_set_volume`] on a
/// background thread), so there's a single implementation of the
/// play/stop/volume logic for both the synchronous and backgrounded paths.
impl Controller {
    /// play - start play on this renderer, using Openhome if present, else `AvTransport` (if present)
    fn play(&self, local_addr: &IpAddr, streaminfo: StreamInfo) -> Result<(), &'static str> {
        // do we support this protocol?
        if !self.supported_protocols.is_valid() {
            ui_log(
                LogCategory::Error,
                "play: no supported renderer protocol found",
            );
            return Err("Invalid UPNP/DLNA protocol");
        }
        // build the hashmap with the formatting vars for the OH and AV play templates
        let mut fmt_vars = Context::new();
        let addr = format!("{local_addr}:{}", streaminfo.server_port);
        let streaming_url = format!("http://{addr}/stream/swyh.{}", streaminfo.streaming_format);
        fmt_vars.insert("server_uri", Value::owned_str(streaming_url));
        fmt_vars.insert(
            "bits_per_sample",
            Value::Int(streaminfo.bits_per_sample as i64),
        );
        fmt_vars.insert("sample_rate", Value::Int(streaminfo.sample_rate.into()));
        fmt_vars.insert("duration", Value::static_str("00:00:00"));
        let didl_tmpl = match streaminfo.streaming_format {
            StreamingFormat::Flac => TEMPLATES.flac_prot.format(&fmt_vars),
            StreamingFormat::Rf64 | StreamingFormat::Wav => TEMPLATES.wav_prot.format(&fmt_vars),
            StreamingFormat::Lpcm => match streaminfo.bits_per_sample {
                BitDepth::Bits16 => TEMPLATES.l16_prot.format(&fmt_vars),
                BitDepth::Bits24 => TEMPLATES.l24_prot.format(&fmt_vars),
            },
        };
        let Ok(didl_prot) = didl_tmpl.inspect_err(|e| {
            ui_log(
                LogCategory::Error,
                &format!("Error {e} formatting DIDL template."),
            );
        }) else {
            return Err(BAD_TEMPL);
        };
        fmt_vars.insert("didl_prot_info", Value::owned_str(didl_prot));
        let Ok(formatted_didl) = TEMPLATES.didl.format(&fmt_vars).inspect_err(|e| {
            ui_log(
                LogCategory::Error,
                &format!("Error {e} formatting didl_data xml"),
            );
        }) else {
            return Err(BAD_TEMPL);
        };
        fmt_vars.insert("didl_data", Value::owned_str(formatted_didl));
        // now send the start playing commands
        if self
            .supported_protocols
            .contains(SupportedProtocols::OPENHOME)
        {
            ui_log(
                LogCategory::Info,
                &format!(
                    "OH Start playing on {} host={} port={} from {local_addr} using OH Playlist",
                    self.dev_name, self.host, self.port
                ),
            );
            self.oh_play(&fmt_vars)
        } else if self
            .supported_protocols
            .contains(SupportedProtocols::AVTRANSPORT)
        {
            ui_log(
                LogCategory::Info,
                &format!(
                    "AV Start playing on {} host={} port={} from {local_addr} using AV Play",
                    self.dev_name, self.host, self.port
                ),
            );
            self.av_play(&fmt_vars)
        } else {
            unreachable!(
                "SupportedProtocol passed IsValid() but contains neither OPENHOME nor AVTRANSPORT"
            );
        }
    }

    /// `oh_play` - set up a playlist on this `OpenHome` renderer and tell it to play it
    ///
    /// the renderer will then try to get the audio from our built-in webserver
    /// at http://{_`my_ip`_}:`{server_port}/stream/swyh.wav`
    fn oh_play(&self, fmt_vars: &Context) -> Result<(), &'static str> {
        // stop anything currently playing first, Moode needs it
        let url = &self.oh_control_full_url;
        self.oh_stop_play(url);
        // Send the InsertPlayList command with metadate(DIDL-Lite)
        ui_log(
            LogCategory::Info,
            &format!(
                "OH Inserting new playlist on {} host={} port={}",
                self.dev_name, self.host, self.port
            ),
        );
        let Ok(xmlbody) = TEMPLATES.oh_insert_pl.format(fmt_vars).inspect_err(|e| {
            ui_log(
                LogCategory::Error,
                &format!("oh_play: error {e} formatting oh playlist xml"),
            );
        }) else {
            return Err(BAD_TEMPL);
        };
        let _resp = soap_request(
            &self.agent,
            url,
            "urn:av-openhome-org:service:Playlist:1#Insert",
            &xmlbody,
        )
        .unwrap_or_default();
        // send the Play command
        ui_log(
            LogCategory::Info,
            &format!(
                "OH Play on {} host={} port={}",
                self.dev_name, self.host, self.port
            ),
        );
        let _resp = soap_request(
            &self.agent,
            url,
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
    fn av_play(&self, fmt_vars: &Context) -> Result<(), &'static str> {
        let url = &self.av_control_full_url;
        // to prevent error 705 (transport locked) on some devices
        // it's necessary to send a stop play request first
        self.av_stop_play(url);
        // now send SetAVTransportURI with metadate(DIDL-Lite) and play requests
        let Ok(xmlbody) = TEMPLATES
            .av_set_transport_uri
            .format(fmt_vars)
            .inspect_err(|e| {
                ui_log(
                    LogCategory::Error,
                    &format!("av_play: error {e} formatting set transport uri"),
                );
            })
        else {
            return Err(BAD_TEMPL);
        };
        let _resp = soap_request(
            &self.agent,
            url,
            "urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI",
            &xmlbody,
        )
        .unwrap_or_default();
        // the renderer will now send a head request first, so wait a bit
        thread::sleep(Duration::from_millis(100));
        // send play command
        let _resp = soap_request(
            &self.agent,
            url,
            "urn:schemas-upnp-org:service:AVTransport:1#Play",
            AV_PLAY_TEMPLATE,
        )
        .unwrap_or_default();
        Ok(())
    }

    /// `stop_play` - stop playing on this renderer (`OpenHome` or `AvTransport`)
    fn stop_play(&self) {
        if self
            .supported_protocols
            .contains(SupportedProtocols::OPENHOME)
        {
            self.oh_stop_play(&self.oh_control_full_url);
        } else if self
            .supported_protocols
            .contains(SupportedProtocols::AVTRANSPORT)
        {
            self.av_stop_play(&self.av_control_full_url);
        } else {
            ui_log(
                LogCategory::Error,
                "ERROR: stop_play: no supported renderer protocol found",
            );
        }
    }

    /// `oh_stop_play` - delete the playlist on the `OpenHome` renderer, so that it stops playing
    fn oh_stop_play(&self, url: &str) {
        ui_log(
            LogCategory::Info,
            &format!(
                "OH Delete playlist on {} => {}",
                self.dev_name, self.remote_addr
            ),
        );

        // delete current playlist
        let _resp = soap_request(
            &self.agent,
            url,
            "urn:av-openhome-org:service:Playlist:1#DeleteAll",
            OH_DELETE_PL_TEMPLATE,
        )
        .unwrap_or_default();
    }

    /// `av_stop_play` - stop playing on the AV renderer
    fn av_stop_play(&self, url: &str) {
        ui_log(
            LogCategory::Info,
            &format!(
                "AV Stop playing on {} => {}",
                self.dev_name, self.remote_addr
            ),
        );

        // Stop play
        let _resp = soap_request(
            &self.agent,
            url,
            "urn:schemas-upnp-org:service:AVTransport:1#Stop",
            AV_STOP_PLAY_TEMPLATE,
        )
        .unwrap_or_default();
    }

    /// get volume, using Openhome if present, else `AvTransport` (if present)
    fn get_volume(&self) -> i32 {
        if self
            .supported_protocols
            .contains(SupportedProtocols::OPENHOME)
        {
            self.oh_get_volume()
        } else if self
            .supported_protocols
            .contains(SupportedProtocols::AVTRANSPORT)
        {
            self.av_get_volume()
        } else {
            -1
        }
    }

    /// get OpenHome Volume
    fn oh_get_volume(&self) -> i32 {
        // get current volume
        let vol_xml = soap_request(
            &self.agent,
            &self.oh_volume_full_url,
            "urn:av-openhome-org:service:Volume:1#Volume",
            OH_GET_VOL_TEMPLATE,
        )
        .unwrap_or_else(|| "<Error/>".to_string());
        // parse response to extract volume
        debug!("oh_get_volume response: {vol_xml}");
        let parser = EventReader::new(vol_xml.as_bytes());
        let mut cur_elem = EcoString::new();
        let mut have_vol_response = false;
        let mut str_volume = EcoString::from("-1".to_string());
        for e in parser {
            match e {
                Ok(XmlEvent::StartElement { name, .. }) => {
                    cur_elem = EcoString::from(&name.local_name);
                    if cur_elem == "VolumeResponse" {
                        have_vol_response = true;
                    }
                }
                Ok(XmlEvent::Characters(value)) if cur_elem == "Value" && have_vol_response => {
                    str_volume = EcoString::from(value);
                }
                Err(e) => {
                    error!("OH Volume XML parse error: {e}");
                }
                _ => {}
            }
        }
        let volume = str_volume.parse::<i32>().unwrap_or(-1);
        if volume >= 0 {
            ui_log(
                LogCategory::Info,
                &format!(
                    "OH Get Volume on {} host={} port={} = {volume}%",
                    self.dev_name, self.host, self.port,
                ),
            );
        } else {
            ui_log(
                LogCategory::Info,
                &format!("OH Get Volume not available for {}.", self.dev_name),
            );
        }
        volume
    }

    /// get AV Volume
    fn av_get_volume(&self) -> i32 {
        // get current volume
        let vol_xml = soap_request(
            &self.agent,
            &self.av_volume_full_url,
            "urn:schemas-upnp-org:service:RenderingControl:1#GetVolume",
            AV_GET_VOL_TEMPLATE,
        )
        .unwrap_or_else(|| "<Error/>".to_string());
        debug!("av_get_volume response: {vol_xml}");
        let parser = EventReader::new(vol_xml.as_bytes());
        let mut cur_elem = EcoString::new();
        let mut have_vol_response = false;
        let mut str_volume = "-1".to_string();
        for e in parser {
            match e {
                Ok(XmlEvent::StartElement { name, .. }) => {
                    cur_elem = EcoString::from(name.local_name);
                    if cur_elem == "GetVolumeResponse" {
                        have_vol_response = true;
                    }
                }
                Ok(XmlEvent::Characters(value))
                    if cur_elem == "CurrentVolume" && have_vol_response =>
                {
                    str_volume = value;
                }
                Err(e) => {
                    error!("AV Volume XML parse error: {e}");
                }
                _ => {}
            }
        }
        let volume = str_volume.parse::<i32>().unwrap_or(-1);
        if volume >= 0 {
            ui_log(
                LogCategory::Info,
                &format!(
                    "AV Get Volume on {} host={} port={} = {volume}%",
                    self.dev_name, self.host, self.port,
                ),
            );
        } else {
            ui_log(
                LogCategory::Info,
                &format!("AV Get Volume not available for {}.", self.dev_name),
            );
        }
        volume
    }

    /// set volume, using Openhome if present, else `AvTransport` (if present)
    fn set_volume(&self, vol: i32) {
        if self
            .supported_protocols
            .contains(SupportedProtocols::OPENHOME)
        {
            self.oh_set_volume(vol);
        } else if self
            .supported_protocols
            .contains(SupportedProtocols::AVTRANSPORT)
        {
            self.av_set_volume(vol);
        }
    }

    /// set Openhome Volume
    fn oh_set_volume(&self, vol: i32) {
        let tmpl = OH_SET_VOL_TEMPLATE.replace("{volume}", &vol.to_string());
        ui_log(
            LogCategory::Info,
            &format!(
                "OH Set New Volume on {} host={} port={} to {vol}%",
                self.dev_name, self.host, self.port
            ),
        );
        // set new volume
        let vol_xml = soap_request(
            &self.agent,
            &self.oh_volume_full_url,
            "urn:av-openhome-org:service:Volume:1#SetVolume",
            &tmpl,
        )
        .unwrap_or("<Error/>".to_string());
        debug!("oh_set_volume response: {vol_xml}");
    }

    /// set AV Volume
    fn av_set_volume(&self, vol: i32) {
        let tmpl = AV_SET_VOL_TEMPLATE.replace("{volume}", &vol.to_string());
        ui_log(
            LogCategory::Info,
            &format!(
                "AV Set New Volume on {} host={} port={} to {vol}%",
                self.dev_name, self.host, self.port
            ),
        );
        // set new volume
        let vol_xml = soap_request(
            &self.agent,
            &self.av_volume_full_url,
            "urn:schemas-upnp-org:service:RenderingControl:1#SetVolume",
            &tmpl,
        )
        .unwrap_or("<Error/>".to_string());
        debug!("av_set_volume response: {vol_xml}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url_ip_with_port() {
        let mut rend = Renderer::new(&ureq::agent());
        rend.dev_url = "http://192.168.1.26:80/".to_string();
        rend.parse_url();
        assert_eq!(rend.controller.host, "192.168.1.26");
        assert_eq!(rend.controller.port, 80);
        rend.dev_url = "http://192.168.1.26:12345/".to_string();
        rend.parse_url();
        assert_eq!(rend.controller.host, "192.168.1.26");
        assert_eq!(rend.controller.port, 12345);
    }

    #[test]
    fn parse_url_no_port() {
        let mut rend = Renderer::new(&ureq::agent());
        rend.dev_url = "http://192.168.1.26/".to_string();
        rend.parse_url();
        assert_eq!(rend.controller.host, "192.168.1.26");
        assert_eq!(rend.controller.port, 0);
    }

    #[test]
    fn parse_url_hostname_with_port() {
        let mut rend = Renderer::new(&ureq::agent());
        rend.dev_url = "http://myrenderer.local:8080/desc.xml".to_string();
        rend.parse_url();
        assert_eq!(rend.controller.host, "myrenderer.local");
        assert_eq!(rend.controller.port, 8080);
    }

    #[test]
    fn parse_url_hostname_no_port() {
        let mut rend = Renderer::new(&ureq::agent());
        rend.dev_url = "http://myrenderer.local/desc.xml".to_string();
        rend.parse_url();
        assert_eq!(rend.controller.host, "myrenderer.local");
        assert_eq!(rend.controller.port, 0);
    }

    #[test]
    fn parse_url_with_path() {
        let mut rend = Renderer::new(&ureq::agent());
        rend.dev_url = "http://192.168.0.1:1234/some/path/desc.xml".to_string();
        rend.parse_url();
        assert_eq!(rend.controller.host, "192.168.0.1");
        assert_eq!(rend.controller.port, 1234);
    }

    #[test]
    fn parse_url_invalid_url() {
        let mut rend = Renderer::new(&ureq::agent());
        rend.dev_url = "not a url at all".to_string();
        rend.parse_url();
        assert_eq!(rend.controller.host, "0.0.0.0");
        assert_eq!(rend.controller.port, 0);
    }

    #[test]
    fn parse_url_no_authority() {
        let mut rend = Renderer::new(&ureq::agent());
        // relative URL has no authority
        rend.dev_url = "/just/a/path".to_string();
        rend.parse_url();
        assert_eq!(rend.controller.host, "0.0.0.0");
        assert_eq!(rend.controller.port, 0);
    }

    #[test]
    fn renderer() {
        let mut rend = Renderer::new(&ureq::agent());
        rend.dev_url = "http://192.168.1.26:80/".to_string();
        rend.parse_url();
        assert_eq!(rend.controller.host, "192.168.1.26");
        assert_eq!(rend.controller.port, 80);
        rend.dev_url = "http://192.168.1.26:12345/".to_string();
        rend.parse_url();
        assert_eq!(rend.controller.host, "192.168.1.26");
        assert_eq!(rend.controller.port, 12345);
    }
}
