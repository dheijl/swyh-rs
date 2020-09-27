// #![windows_subsystem = "windows"]  // enable to suppress console println!

use ascii::AsciiString;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{unbounded, Receiver as OtherReceiver, Sender as OtherSender};
use fltk::{app, button::*, frame::*, text::*, window::*};
use futures::prelude::*;
use lazy_static::*;
use rupnp::ssdp::{SearchTarget, URN};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use url::Url;
mod utils;
use strfmt::strfmt;
use stringreader::StringReader;
use utils::rwstream::ChannelStream;
use xml::reader::{EventReader, XmlEvent};

#[derive(Debug, Clone, Copy)]
pub enum Message {
    Increment,
    Decrement,
}

#[derive(Debug, Clone)]
struct Renderer {
    dev_name: String,
    dev_model: String,
    dev_type: String,
    dev_url: String,
    svc_type: String,
    svc_id: String,
    pl_control_url: String,
    ovh_control_url: String,
}

#[derive(Debug, Clone, Copy)]
struct WavData {
    sample_format: cpal::SampleFormat,
    sample_rate: cpal::SampleRate,
    channels: u16,
}

macro_rules! DEBUG {
    ($x:stmt) => {
        if cfg!(debug_assertions) {
            $x
        }
    };
}

lazy_static! {
    static ref CLIENTS: Mutex<HashMap<String, ChannelStream>> = Mutex::new(HashMap::new());
    static ref LOGCHANNEL: Mutex<(OtherSender<String>, OtherReceiver<String>)> =
        Mutex::new(unbounded());
}

const PORT: i32 = 5901;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // first initialize cpal audio to prevent COM reinitialize panic on Windows
    let audio_output_device = get_audio_device();
    let audio_cfg = &audio_output_device
        .default_output_config()
        .expect("No default output config found");

    let _app = app::App::default().with_scheme(app::Scheme::Gleam);
    let (sw, sh) = app::screen_size();
    let mut wind = Window::default()
        .with_size((sw / 2.5) as i32, (sh / 2.0) as i32)
        .with_label("UPNP/DLNA Renderers");
    wind.handle(Box::new(move |_ev| {
        //eprintln!("{:?}", app::event());
        let ev = app::event();
        match ev {
            Event::Close => {
                _app.quit();
                std::process::exit(0);
            }
            _ => false,
        }
    }));

    let fw = (sw as i32) / 3;
    let fx = ((wind.width() - 30) / 2) - (fw / 2);
    let mut frame = Frame::new(fx, 5, fw, 25, "").with_align(Align::Center);
    frame.set_frame(FrameType::BorderBox);
    let buf = TextBuffer::default();
    let tb = Arc::from(Mutex::from(
        TextDisplay::new(2, wind.height() - 154, wind.width() - 4, 150, "").with_align(Align::Left),
    ));
    let mut _tb = tb.lock().unwrap();
    _tb.set_buffer(Some(buf));
    drop(_tb);

    let local_addr = get_local_addr().expect("Could not obtain local address.");
    frame.set_label(&format!(
        "Scanning {} for UPNP rendering devices",
        local_addr
    ));
    wind.make_resizable(true);
    wind.end();
    wind.show();
    for _ in 1..100 {
        app::wait_for(0.00001)?
    }

    // setup logger thread that updates text display
    //let (msg_s, msg_r): (Sender<String>, Receiver<String>) = channel();
    let _ = std::thread::spawn(move || log_reader(tb));

    for _ in 1..100 {
        app::wait_for(0.001)?
    }

    // build a list with renderers descovered on the network
    let renderers = discover().await?;
    // now create a button for each discovered renderer
    let mut buttons: Vec<LightButton> = Vec::new();
    // button dimensions and starting position
    let bwidth = frame.width() / 2; // button width
    let bheight = frame.height(); // button height
    let bx = ((wind.width() - 30) / 2) - (bwidth / 2); // button x offset
    let mut by = frame.y() + frame.height() + 10; // button y offset
                                                  // create the buttons
    let mut bi = 0; // button index
    match renderers {
        Some(rends) => {
            let rs = rends;
            for renderer in rs.iter() {
                let mut but = LightButton::default() // create the button
                    .with_size(bwidth, bheight)
                    .with_pos(bx, by)
                    .with_align(Align::Center)
                    .with_label(&format!("{} {}", renderer.dev_model, renderer.dev_name));
                let rs_c = rs.clone();
                let but_c = but.clone();
                but.handle(Box::new(move |ev| {
                    let but_cc = but_c.clone();
                    let renderer = &rs_c[bi as usize];
                    match ev {
                        Event::Push => {
                            DEBUG!(eprintln!(
                                "Pushed renderer #{} {} {}, state = {}",
                                bi,
                                renderer.dev_model,
                                renderer.dev_name,
                                if but_cc.is_on() { "ON" } else { "OFF" }
                            ));
                            if but_cc.is_on() {
                                let _ = play(&renderer, &local_addr);
                            } else {
                                let _ = stop(&renderer);
                            }
                            true
                        }
                        _ => true,
                    }
                }));
                wind.add(&but); // add the button to the window
                buttons.push(but); // and keep a reference to it
                bi += 1; // bump the button index
                by += bheight + 10; // and the button y offset
            }
        }
        None => {}
    }
    frame.set_label("Rendering Devices");
    wind.redraw();
    for _ in 1..100 {
        app::wait_for(0.00001)?
    }

    // capture system audio
    let stream = capture_output_audio();
    stream.play().expect("Could not play audio capture stream");

    // start webserver
    let wd = WavData {
        sample_format: audio_cfg.sample_format(),
        sample_rate: audio_cfg.sample_rate(),
        channels: audio_cfg.channels(),
    };
    let _ = std::thread::spawn(move || run_server(&local_addr, wd));
    std::thread::yield_now();

    // run GUI, _app.wait() and _app.run() somehow block the logger channel
    // from receiving messages
    loop {
        app::wait_for(0.00001).unwrap();
        std::thread::sleep(std::time::Duration::new(0, 100000));
        if app::should_program_quit() {
            break;
        }
    }
    Ok(())
}

fn oh_soap_request(url: &String, soap_action: &String, body: &String) -> Option<String> {
    DEBUG!(eprintln!(
        "url: {}, SOAP Action: {}, SOAP xml body \r\n{}",
        url.clone(), soap_action, body
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

static PLAY_PL_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" \
xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:Play xmlns:u=\"urn:av-openhome-org:service:Playlist:1\"/>\
</s:Body>\
</s:Envelope>";

static DELETE_PL_TEMPLATE: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<s:Envelope s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\" \
xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">\
<s:Body>\
<u:DeleteAll xmlns:u=\"urn:av-openhome-org:service:Playlist:1\"/>\
</s:Body>\
</s:Envelope>";

fn play(renderer: &Renderer, local_addr: &IpAddr) -> Result<(), ureq::Error> {
    let url = renderer.dev_url.clone();
    let (host, port) = parse_url(url);
    log(format!(
        "Start playing on {} host={} port={} from {}",
        renderer.dev_name, host, port, local_addr
    ));

    let url = format!("http://{}:{}{}", host, port, renderer.pl_control_url);
    let addr = format!("{}:{}", local_addr, PORT);
    let local_url = format!("http://{}/stream/swyh.wav", addr);
    DEBUG!(eprintln!("OHPlaylist server URL: {}", local_url.clone()));

    // delete current playlist
    let _resp = oh_soap_request(
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
    let resp = oh_soap_request(
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
        seek_id = resp.as_str()[s + 7 .. e].to_string();
    }
    DEBUG!(eprintln!("SeekId: {}", seek_id.clone()));

    // send seek_id
    vars.insert("seek_id".to_string(), seek_id);
    let xmlbody = strfmt(&SEEKID_PL_TEMPLATE, &vars).unwrap();
    let _resp = oh_soap_request(
        &url,
        &"urn:av-openhome-org:service:Playlist:1#SeekId".to_string(),
        &xmlbody,
    )
    .unwrap();

    // send play command
    let _resp = oh_soap_request(
        &url,
        &"urn:av-openhome-org:service:Playlist:1#Play".to_string(),
        &PLAY_PL_TEMPLATE.to_string(),
    )
    .unwrap();

    Ok(())
}

fn stop(renderer: &Renderer) {
    let url = renderer.dev_url.clone();
    let (host, port) = parse_url(url);
    log(format!(
        "Stop playing on {} host={} port={}",
        renderer.dev_name, host, port
    ));
    let url = format!("http://{}:{}{}", host, port, renderer.pl_control_url);

    // delete current playlist
    let _resp = oh_soap_request(
        &url,
        &"urn:av-openhome-org:service:Playlist:1#DeleteAll".to_string(),
        &DELETE_PL_TEMPLATE.to_string(),
    )
    .unwrap();
}

fn parse_url(dev_url: String) -> (String, u16) {
    let host: String;
    let port: u16;
    match Url::parse(&dev_url) {
        Ok(url) => {
            host = url.host_str().unwrap().to_string();
            port = url.port().unwrap();
        }
        Err(e) => {
            log(format!("Error {} parsing url {}", e, dev_url));
            host = "0.0.0.0".to_string();
            port = 0;
        }
    }
    (host, port)
}

fn log_reader(tb: Arc<Mutex<TextDisplay>>) {
    let logreader: OtherReceiver<String>;
    {
        let ch = &LOGCHANNEL.lock().unwrap();
        logreader = ch.1.clone();
    }
    loop {
        let msg = logreader.recv().unwrap();
        eprintln!("LOG: {}", msg);
        let mut _tb = tb.lock().unwrap();
        _tb.buffer().unwrap().append(&msg);
        _tb.buffer().unwrap().append("\n");
        let buflen = _tb.buffer().unwrap().length();
        _tb.set_insert_position(buflen);
        let buflines = _tb.count_lines(0, buflen, true);
        _tb.scroll(buflines, 0);
        _tb.redraw();
        drop(_tb);
    }
}

fn log(s: String) {
    let logger: OtherSender<String>;
    {
        let ch = &LOGCHANNEL.lock().unwrap();
        logger = ch.0.clone();
    }
    let d = s.clone();
    logger.send(s).unwrap();
    DEBUG!(eprintln!("{}", d));
}

fn run_server(local_addr: &IpAddr, wd: WavData) -> () {
    let addr = format!("{}:{}", local_addr, PORT);
    let logmsg = format!("Serving on {}", addr);
    log(logmsg);
    let server = Arc::new(tiny_http::Server::http(addr).unwrap());
    let mut handles = Vec::new();
    for _ in 0..8 {
        let server = server.clone();

        handles.push(thread::spawn(move || {
            for rq in server.incoming_requests() {
                log(format!(
                    "Received request {} from {}",
                    rq.url(),
                    rq.remote_addr()
                ));
                let remote_addr = format!("{}", rq.remote_addr());
                let (tx, rx): (OtherSender<i16>, OtherReceiver<i16>) = unbounded();
                let channel_stream = ChannelStream {
                    s: tx.clone(),
                    r: rx.clone(),
                };
                let mut clients = CLIENTS.lock().unwrap();
                clients.insert(remote_addr.clone(), channel_stream);
                drop(clients);
                std::thread::yield_now();
                let channel_stream = ChannelStream {
                    s: tx.clone(),
                    r: rx.clone(),
                };
                let ct_text = format!("audio/L16;rate={};channels=2", wd.sample_rate.0.to_string());
                let ct_hdr = tiny_http::Header {
                    field: "Content-Type".parse().unwrap(),
                    value: AsciiString::from_ascii(ct_text).unwrap(),
                };
                let response = tiny_http::Response::empty(200)
                    .with_header(ct_hdr)
                    .with_data(channel_stream, None)
                    .with_chunked_threshold(8192);
                let _ = rq.respond(response);
                let mut clients = CLIENTS.lock().unwrap();
                clients.remove(&remote_addr);
                drop(clients);
                log(format!("End of response to {}", remote_addr));
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

fn capture_output_audio() -> cpal::Stream {
    // first initialize cpal audio to prevent COM reinitialize panic on Windows
    let audio_output_device = get_audio_device();
    log(format!(
        "Default audio output device: {}",
        audio_output_device
            .name()
            .expect("Could not get default audio device name")
    ));
    let audio_cfg = &audio_output_device
        .default_output_config()
        .expect("No default output config found");
    log(format!("Default config {:?}", audio_cfg));
    let stream = match audio_cfg.sample_format() {
        cpal::SampleFormat::F32 => {
            let s = audio_output_device
                .build_input_stream(
                    &audio_cfg.config(),
                    move |data, _: &_| wave_reader::<f32>(data),
                    err_fn,
                )
                .expect("Could not capture f32 stream format");
            s
        }
        cpal::SampleFormat::I16 => {
            let s = audio_output_device
                .build_input_stream(
                    &audio_cfg.config(),
                    move |data, _: &_| wave_reader::<i16>(data),
                    err_fn,
                )
                .expect("Could not capture i16 stream format");
            s
        }
        cpal::SampleFormat::U16 => {
            let s = audio_output_device
                .build_input_stream(
                    &audio_cfg.config(),
                    move |data, _: &_| wave_reader::<u16>(data),
                    err_fn,
                )
                .expect("Could not capture u16 stream format");
            s
        }
    };
    stream
}

fn err_fn(err: cpal::StreamError) {
    log(format!("Error {} building audio input stream", err));
}

fn wave_reader<T>(samples: &[T])
where
    T: cpal::Sample,
{
    static mut ONETIME_SW: bool = false;
    unsafe {
        if !ONETIME_SW {
            log(format!("The wave_reader is receiving samples"));
            ONETIME_SW = true;
        }
    }

    let i16_samples: Vec<i16> = samples.into_iter().map(|x| x.to_i16()).collect();
    let clients = CLIENTS.lock().unwrap();
    for (_, v) in clients.iter() {
        v.write(&i16_samples);
    }
}

///
/// discover the available (audio) renderers on the network
///  
async fn discover() -> Result<Option<Vec<Renderer>>, rupnp::Error> {
    const RENDERING_CONTROL: URN = URN::service("schemas-upnp-org", "RenderingControl", 1);

    log(format!("Starting SSDP renderer discovery"));

    let mut renderers: Vec<Renderer> = Vec::new();
    let search_target = SearchTarget::URN(RENDERING_CONTROL);
    match rupnp::discover(&search_target, Duration::from_secs(3)).await {
        Ok(d) => {
            pin_utils::pin_mut!(d);
            loop {
                if let Some(device) = d.try_next().await? {
                    if device.services().len() > 0 {
                        if let Some(service) = device.find_service(&RENDERING_CONTROL) {
                            print_renderer(&device, &service);
                            let mut renderer = Renderer {
                                dev_name: device.friendly_name().to_string(),
                                dev_model: device.model_name().to_string(),
                                dev_type: device.device_type().to_string(),
                                dev_url: device.url().to_string(),
                                svc_id: service.service_type().to_string(),
                                svc_type: service.service_type().to_string(),
                                pl_control_url: String::new(),
                                ovh_control_url: String::new(),
                            };
                            let xml = get_service_description(&renderer).unwrap();
                            renderer.pl_control_url = get_control_url(
                                &xml,
                                &"Playlist:1".to_string(),
                                &"Playlist".to_string(),
                            )
                            .unwrap();
                            renderer.ovh_control_url = get_control_url(
                                &xml,
                                &"Volume:1".to_string(),
                                &"Volume".to_string(),
                            )
                            .unwrap();
                            renderers.push(renderer);
                        }
                    } else {
                        DEBUG!(eprintln!(
                            "*No services* type={}, manufacturer={}, name={}, model={}, at url= {}",
                            device.device_type(),
                            device.manufacturer(),
                            device.friendly_name(),
                            device.model_name(),
                            device.url()
                        ));
                    }
                } else {
                    log(format!("End of SSDP devices discovery"));
                    break;
                }
            }
        }
        Err(e) => {
            log(format!("Error {} running SSDP discover", e));
        }
    }

    Ok(Some(renderers))
}

fn _indent(size: usize) -> String {
    const INDENT: &'static str = "    ";
    (0..size)
        .map(|_| INDENT)
        .fold(String::with_capacity(size * INDENT.len()), |r, s| r + s)
}

fn get_service_description(renderer: &Renderer) -> Option<String> {
    // get the description, need the renderer control url
    let url = renderer.dev_url.clone();
    let resp = ureq::get(url.as_str())
        .set("User-Agent", "swyh-rs-Rust")
        .set("Content-Type", "text/xml")
        .send_string("");
    let xml = resp.into_string().unwrap();
    DEBUG!(eprintln!("resp: {}", xml));
    Some(xml)
}

fn get_control_url(xml: &String, service_type: &String, service_id: &String) -> Option<String> {
    struct AvService {
        service_id: String,
        service_type: String,
        control_url: String,
    }
    let xmlstream = StringReader::new(&xml);
    let parser = EventReader::new(xmlstream);
    let mut _depth = 0;
    let mut cur_elem = String::new();
    let mut service = AvService {
        service_id: String::new(),
        service_type: String::new(),
        control_url: String::new(),
    };
    for e in parser {
        match e {
            Ok(XmlEvent::StartElement { name, .. }) => {
                //DEBUG!(eprintln!("{}+{}", _indent(depth), name));
                _depth += 1;
                cur_elem = name.local_name;
            }
            Ok(XmlEvent::EndElement { name }) => {
                _depth -= 1;
                let _ = name;
                //DEBUG!(eprintln!("{}-{}", _indent(depth), name));
            }
            Ok(XmlEvent::Characters(value)) => {
                //DEBUG!(eprintln!("{}*{}={}", _indent(depth), cur_elem, value));
                if cur_elem.contains("serviceType") {
                    service.service_type = value;
                } else if cur_elem.contains("serviceId") {
                    service.service_id = value;
                } else if cur_elem.contains("controlURL")
                    && service.service_type.contains(service_type)
                    && service.service_id.contains(service_id)
                {
                    service.control_url = value;
                    break;
                }
            }
            Err(e) => {
                DEBUG!(eprintln!("Error: {}", e));
                break;
            }
            _ => {}
        }
    }
    log(format!(
        "{}/{}={}",
        service.service_type, service.service_id, service.control_url
    ));
    Some(service.control_url)
}

///
/// print the information for a renderer
///
fn print_renderer(device: &rupnp::Device, service: &rupnp::Service) {
    log(format!(
        "Found renderer type={}, manufacturer={}, name={}, model={}, at url= {}",
        device.device_type(),
        device.manufacturer(),
        device.friendly_name(),
        device.model_name(),
        device.url()
    ));
    log(format!(
        "  Service type: {}, id:   {}",
        service.service_type(),
        service.service_id()
    ));
}

///
/// return the default output audio device
///
fn get_audio_device() -> cpal::Device {
    // audio hosts
    let _available_hosts = cpal::available_hosts();
    let default_host = cpal::default_host();
    let default_device = default_host
        .default_output_device()
        .expect("Failed to get the default audio output device");
    default_device
}

use std::net::{IpAddr, UdpSocket};

/// get the local ip address, return an `Option<String>`. when it fails, return `None`.
fn get_local_addr() -> Option<IpAddr> {
    // bind to IN_ADDR_ANY, can be multiple interfaces/addresses
    let socket = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return None,
    };
    // try to connect to Google DNS so that we bind to an interface connected to the internet
    match socket.connect("8.8.8.8:80") {
        Ok(()) => (),
        Err(_) => return None,
    };
    // now we can return the IP address of this interface
    match socket.local_addr() {
        Ok(addr) => return Some(addr.ip()),
        Err(_) => return None,
    };
}
