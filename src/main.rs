#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // to suppress console with debug output for release builds
///
/// swyh-rs
///
/// Basic SWYH (https://www.streamwhatyouhear.com/, source repo https://github.com/StreamWhatYouHear/SWYH) clone entirely written in rust.
///
/// I wrote this because I a) wanted to learn Rust and b) SWYH did not work on Linux and did not work well with Volumio (push streaming does not work).
///
/// For the moment all music is streamed in wav-format (audio/l16) with the sample rate of the music source (the default audio device, I use HiFi Cable Input).
///
/// Tested on Windows 10 and on Ubuntu 20.04 with Raspberry Pi based Volumio DLNA renderers and with a Harman-Kardon AVR DLNA device.
/// I don't have access to a Mac, so I don't know if this would also work.
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

#[macro_use]
extern crate bitflags;

mod openhome;
mod utils;

use crate::openhome::avmedia::{discover, Renderer, WavData};
use crate::utils::audiodevices::{get_default_audio_output_device, get_output_audio_devices};
use crate::utils::configuration::Configuration;
use crate::utils::escape::FwSlashPipeEscape;
use crate::utils::local_ip_address::get_local_addr;
use crate::utils::priority::raise_priority;
use crate::utils::rwstream::ChannelStream;
use cpal::traits::{DeviceTrait, StreamTrait};
use crossbeam_channel::{unbounded, Receiver, Sender};
use fltk::{
    app,
    button::{
        Align, ButtonExt, CheckButton, Color, DisplayExt, Event, FrameType, GroupExt, LightButton,
        MenuExt, ValuatorExt, WidgetBase, WidgetExt, WindowExt,
    },
    dialog,
    frame::Frame,
    group::{Pack, PackType},
    menu::MenuButton,
    text::{TextBuffer, TextDisplay},
    valuator::Counter,
    window::DoubleWindow,
};
use lazy_static::lazy_static;
use log::{debug, error, info, log, warn, LevelFilter};
use parking_lot::{Mutex, Once};
use simplelog::{CombinedLogger, Config, TermLogger, WriteLogger};
use std::cell::Cell;
use std::collections::HashMap;
use std::fs::File;
use std::net::IpAddr;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tiny_http::{Header, Method, Response, Server};

/// app version
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// the HTTP server port
pub const SERVER_PORT: u16 = 5901;

/// streaming state
#[derive(Debug, Clone, Copy)]
enum StreamingState {
    Started,
    Ended,
}

impl PartialEq for StreamingState {
    fn eq(&self, other: &Self) -> bool {
        self == other
    }
}

/// streaming state feedback for a client
#[derive(Debug, Clone, PartialEq)]
struct StreamerFeedBack {
    remote_ip: String,
    streaming_state: StreamingState,
}

lazy_static! {
    // streaming clients of the webserver
    static ref CLIENTS: Mutex<HashMap<String, ChannelStream>> = Mutex::new(HashMap::new());
    // the global GUI logger textbox channel used by all threads
    static ref LOGCHANNEL: Mutex<(Sender<String>, Receiver<String>)> = Mutex::new(unbounded());
    // the global configuration state
    static ref CONFIG: Mutex<Configuration> = Mutex::new(Configuration::read_config());
}

/// swyh-rs
///
/// - set up the fltk GUI
/// - setup and start audio capture
/// - start the streaming webserver
/// - start ssdp discovery of media renderers thread
/// - run the GUI, and show any renderers found in the GUI as buttons (to start/stop playing)
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // first initialize cpal audio to prevent COM reinitialize panic on Windows
    let mut audio_output_device =
        get_default_audio_output_device().expect("No default audio device");

    let app = app::App::default().with_scheme(app::Scheme::Gtk);
    app::background(247, 247, 247);
    let ww = 660;
    let wh = 660;
    let mut wind = DoubleWindow::default()
        .with_size(ww, wh)
        .with_label(&format!(
            "swyh-rs UPNP/DLNA Media Renderers V{}",
            APP_VERSION
        ));
    wind.handle(move |_ev| {
        //eprintln!("{:?}", app::event());
        let ev = app::event();
        match ev {
            Event::Close => {
                app.quit();
                std::process::exit(0);
            }
            _ => false,
        }
    });

    wind.make_resizable(true);
    wind.size_range(ww, wh * 2 / 3, 0, 0);
    wind.end();
    wind.show();

    let gw = 600;
    let fw = 600;
    let xpos = 30;
    let ypos = 5;

    let mut vpack = Pack::new(xpos, ypos, gw, wh - 10, "");
    vpack.make_resizable(false);
    vpack.set_spacing(15);
    vpack.end();
    wind.add(&vpack);

    // title frame
    let mut p1 = Pack::new(0, 0, gw, 25, "");
    p1.end();
    let mut opt_frame = Frame::new(0, 0, 0, 25, "").with_align(Align::Center);
    opt_frame.set_frame(FrameType::BorderBox);
    opt_frame.set_label("Options");
    opt_frame.set_color(Color::Light2);
    p1.add(&opt_frame);
    vpack.add(&p1);

    // initialize config
    let mut config: Configuration;
    {
        let mut conf = CONFIG.lock();
        if conf.sound_source == "None" {
            conf.sound_source = audio_output_device.name().unwrap();
            let _ = conf.update_config();
        }
        config = conf.clone();
    }
    log(format!("{:?}", config));
    if cfg!(debug_assertions) {
        config.log_level = LevelFilter::Debug;
    }

    let config_changed: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    // configure simplelogger
    let loglevel = config.log_level;
    let logfile = Path::new(&config.config_dir()).join("log.txt");
    let _ = CombinedLogger::init(vec![
        TermLogger::new(loglevel, Config::default(), simplelog::TerminalMode::Stderr),
        WriteLogger::new(loglevel, Config::default(), File::create(logfile).unwrap()),
    ]);
    info!("swyh-rs Logging started.");
    if cfg!(debug_assertions) {
        log("*W*W*>Running DEBUG build => log level set to DEBUG!".to_string());
    }

    // show config option widgets
    let mut p2 = Pack::new(0, 0, gw, 25, "");
    p2.set_spacing(10);
    p2.set_type(PackType::Horizontal);
    p2.end();

    // auto_resume button for AVTransport autoresume play
    let mut auto_resume = CheckButton::new(0, 0, 0, 0, "Autoresume play");
    if config.auto_resume {
        auto_resume.set(true);
    }
    auto_resume.set_callback2(move |b| {
        let mut conf = CONFIG.lock();
        if b.is_set() {
            conf.auto_resume = true;
        } else {
            conf.auto_resume = false;
        }
        let _ = conf.update_config();
    });
    p2.add(&auto_resume);

    // AutoReconnect to last renderer on startup button
    let mut auto_reconnect = CheckButton::new(0, 0, 0, 0, "Autoreconnect");
    if config.auto_reconnect {
        auto_reconnect.set(true);
    }
    auto_reconnect.set_callback2(move |b| {
        let mut conf = CONFIG.lock();
        if b.is_set() {
            conf.auto_reconnect = true;
        } else {
            conf.auto_reconnect = false;
        }
        let _ = conf.update_config();
    });
    p2.add(&auto_reconnect);

    // SSDP interval counter
    let mut ssdp_interval = Counter::new(0, 0, 0, 0, "SSDP Interval (in minutes)");
    ssdp_interval.set_value(config.ssdp_interval_mins);
    let config_ch_flag = config_changed.clone();
    ssdp_interval.handle2(move |b, ev| match ev {
        Event::Leave => {
            let mut conf = CONFIG.lock();
            if b.value() < 0.5 {
                b.set_value(0.5);
            }
            if (conf.ssdp_interval_mins - b.value()).abs() > 0.09 {
                conf.ssdp_interval_mins = b.value();
                log(format!(
                    "*W*W*> ssdp interval changed to {} minutes, restart required!!",
                    conf.ssdp_interval_mins
                ));
                let _ = conf.update_config();
                config_ch_flag.set(true);
                app::awake();
            }
            true
        }
        _ => false,
    });
    p2.add(&ssdp_interval);

    // show log level choice
    let ll = format!("Log Level: {}", config.log_level.to_string());
    let mut log_level_choice = MenuButton::new(0, 0, 0, 0, &ll);
    let log_levels = vec!["Info", "Debug"];
    for ll in log_levels.iter() {
        log_level_choice.add_choice(ll);
    }
    // apparently this event can recurse on very fast machines
    // probably because it takes some time doing the file I/O, hence recursion lock
    let rlock = Mutex::new(0);
    let config_ch_flag = config_changed.clone();
    log_level_choice.set_callback2(move |b| {
        let mut recursion = rlock.lock();
        if *recursion > 0 {
            return;
        }
        *recursion += 1;
        let mut conf = CONFIG.lock();
        let i = b.value();
        if i < 0 {
            return;
        }
        let level = log_levels[i as usize];
        log(format!(
            "*W*W*> Log level changed to {}, restart required!!",
            level
        ));
        conf.log_level = level.parse().unwrap_or(LevelFilter::Info);
        let _ = conf.update_config();
        config_ch_flag.set(true);
        let ll = format!("Log Level: {}", conf.log_level.to_string());
        b.set_label(&ll);
        app::awake();
        *recursion -= 1;
    });
    p2.add(&log_level_choice);
    p2.auto_layout();
    p2.make_resizable(false);
    vpack.add(&p2);

    let mut p2b = Pack::new(0, 0, gw, 25, "");
    p2b.set_spacing(10);
    p2b.set_type(PackType::Horizontal);
    p2b.end();

    // disable chunked transfer (for AVTransport renderers that can't handle chunkeed transfer)
    let mut disable_chunked = CheckButton::new(0, 0, 0, 0, "Disable Chunked TransferEncoding");
    if config.disable_chunked {
        disable_chunked.set(true);
    }
    disable_chunked.set_callback2(move |b| {
        let mut conf = CONFIG.lock();
        if b.is_set() {
            conf.disable_chunked = true;
        } else {
            conf.disable_chunked = false;
        }
        let _ = conf.update_config();
    });
    p2b.add(&disable_chunked);
    let mut use_wma = CheckButton::new(0, 0, 0, 0, "Use WMA/WAV format");
    if config.use_wave_format {
        use_wma.set(true);
    }
    use_wma.set_callback2(move |b| {
        let mut conf = CONFIG.lock();
        if b.is_set() {
            conf.use_wave_format = true;
        } else {
            conf.use_wave_format = false;
        }
        let _ = conf.update_config();
    });
    p2b.add(&use_wma);
    p2b.auto_layout();
    p2b.make_resizable(false);
    vpack.add(&p2b);

    // get the output device from the config and get all available audio source names
    let audio_devices = get_output_audio_devices().unwrap();
    let mut source_names: Vec<String> = Vec::new();
    for adev in audio_devices {
        let devname = adev.name().unwrap();
        if devname == config.sound_source {
            audio_output_device = adev;
            info!("Selected audio source: {}", devname);
        }
        source_names.push(devname);
    }
    // we need to pass some audio config data to the play function
    let audio_cfg = &audio_output_device
        .default_output_config()
        .expect("No default output config found");
    let wd = WavData {
        sample_format: audio_cfg.sample_format(),
        sample_rate: audio_cfg.sample_rate(),
        channels: audio_cfg.channels(),
    };

    // setup audio source choice
    let mut p3 = Pack::new(0, 0, gw, 25, "");
    p3.end();
    let cur_audio_src = format!("Source: {}", config.sound_source);
    log("Setup audio sources".to_string());
    let mut choose_audio_source_but = MenuButton::new(0, 0, 0, 25, &cur_audio_src);
    for name in source_names.iter() {
        choose_audio_source_but.add_choice(&name.fw_slash_pipe_escape());
    }
    let rlock = Mutex::new(0);
    let config_ch_flag = config_changed.clone();
    choose_audio_source_but.set_callback2(move |b| {
        let mut recursion = rlock.lock();
        if *recursion > 0 {
            return;
        }
        *recursion += 1;
        let mut conf = CONFIG.lock();
        let mut i = b.value();
        if i < 0 {
            return;
        }
        if i as usize >= source_names.len() {
            i = (source_names.len() - 1) as i32;
        }
        let name = source_names[i as usize].clone();
        log(format!(
            "*W*W*> Audio source changed to {}, restart required!!",
            name
        ));
        conf.sound_source = name;
        let _ = conf.update_config();
        b.set_label(&format!("New Source: {}", conf.sound_source));
        config_ch_flag.set(true);
        app::awake();
        *recursion -= 1;
    });
    p3.add(&choose_audio_source_but);
    vpack.add(&p3);

    // raise process priority a bit to prevent audio stuttering under cpu load
    raise_priority();

    // set the last renderer used (for autoreconnect)
    let last_renderer = config.last_renderer;

    // capture system audio
    debug!("Try capturing system audio");
    let stream: cpal::Stream;
    match capture_output_audio(&audio_output_device) {
        Some(s) => {
            stream = s;
            stream.play().unwrap();
        }
        None => {
            log("*E*E*> Could not capture audio ...Please check configuration.".to_string());
        }
    }

    // show renderer buttons title with our local ip address
    let local_addr = get_local_addr().expect("Could not obtain local address.");
    let mut p4 = Pack::new(0, 0, gw, 25, "");
    p4.end();
    let mut frame = Frame::new(0, 0, fw, 25, "").with_align(Align::Center);
    frame.set_frame(FrameType::BorderBox);
    frame.set_label(&format!("UPNP rendering devices on network {}", local_addr));
    frame.set_color(Color::Light2);
    p4.add(&frame);
    vpack.add(&p4);

    // setup feedback textbox at the bottom
    let mut p5 = Pack::new(0, 0, gw, 156, "");
    p5.end();
    let buf = TextBuffer::default();
    let mut tb = TextDisplay::new(0, 0, 0, 150, "").with_align(Align::Left);
    tb.set_buffer(Some(buf));
    p5.add(&tb);
    p5.resizable(&tb);
    vpack.add(&p5);
    vpack.resizable(&p5);

    // create a hashmap for a button for each discovered renderer
    let mut buttons: HashMap<String, LightButton> = HashMap::new();
    // the discovered renderers will be kept in this list
    let mut renderers: Vec<Renderer> = Vec::new();
    // now start the SSDP discovery update thread with a Crossbeam channel for renderer updates
    let (ssdp_tx, ssdp_rx): (Sender<Renderer>, Receiver<Renderer>) = unbounded();
    log("Starting SSDP discovery".to_string());
    let conf = CONFIG.lock().clone();
    let _ = std::thread::Builder::new()
        .name("ssdp_updater".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(move || run_ssdp_updater(ssdp_tx, conf.ssdp_interval_mins))
        .unwrap();

    // start a webserver on the local address,
    // with a Crossbeam feedback channel for connection accept/drop
    let (feedback_tx, feedback_rx): (Sender<StreamerFeedBack>, Receiver<StreamerFeedBack>) =
        unbounded();
    let _ = std::thread::Builder::new()
        .name("swyh_rs_webserver".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(move || run_server(&local_addr, wd, feedback_tx.clone()))
        .unwrap();
    std::thread::yield_now();

    // (SSDP) new renderer button dimensions and starting position
    let bwidth = frame.width();
    let bheight = frame.height();
    let binsert: u32 = 5;

    // get the logreader channel
    let logreader: Receiver<String>;
    {
        let ch = &LOGCHANNEL.lock();
        logreader = ch.1.clone();
    }

    // now run the GUI event loop, app::awake() is used by the various threads to
    // trigger updates when something has changed, some threads use Crossbeam channels
    // to signal what has changed
    while app::wait() {
        if app::should_program_quit() {
            break;
        }
        // a configuration change that needs an app restart to take effect
        if config_changed.get() {
            let c = dialog::choice(
                wind.width() as i32 / 2 - 100,
                wind.height() as i32 / 2 - 50,
                "Configuration value changed!",
                "Restart",
                "Cancel",
                "",
            );
            if c == 0 {
                std::process::Command::new(std::env::current_exe().unwrap().into_os_string())
                    .spawn()
                    .expect("Unable to spawn myself!");
                std::process::exit(0);
            } else {
                config_changed.set(false);
            }
        }
        // check if the streaming webserver has closed a connection not caused by
        // pushing a renderer button
        // in that case we turn the button off as a visual feedback for the user
        // but if auto_resume is set, we restart playing instead
        while let Ok(streamer_feedback) = feedback_rx.try_recv() {
            if let Some(button) = buttons.get_mut(&streamer_feedback.remote_ip) {
                match streamer_feedback.streaming_state {
                    StreamingState::Started => {
                        if !button.is_set() {
                            button.set(true);
                        }
                    }
                    StreamingState::Ended => {
                        // first check if the renderer has actually not started streaming again
                        // as this can happen with Bubble/Nest Audio Openhome
                        let mut still_streaming = false;
                        {
                            let clients = CLIENTS.lock();
                            for (_, c) in clients.iter() {
                                if c.remote_ip == streamer_feedback.remote_ip {
                                    still_streaming = true;
                                    break;
                                }
                            }
                        }
                        if !still_streaming {
                            if auto_resume.is_set() && button.is_set() {
                                for r in renderers.iter() {
                                    if streamer_feedback.remote_ip == r.remote_addr {
                                        let _ = r.play(&local_addr, SERVER_PORT, &wd, &dummy_log);
                                        break;
                                    }
                                }
                            } else if button.is_set() {
                                button.set(false);
                            }
                        }
                    }
                }
            }
        }
        // check the ssdp discovery thread channel for newly discovered renderers
        // add a new button below the last one for each discovered renderer
        while let Ok(newr) = ssdp_rx.try_recv() {
            let mut but = LightButton::default() // create the button
                .with_size(bwidth, bheight)
                .with_pos(0, 0)
                .with_align(Align::Center)
                .with_label(&format!("{} {}", newr.dev_model, newr.dev_name));
            renderers.push(newr.clone());
            // prepare for event handler closure
            let newr_c = newr.clone();
            let bi = buttons.len();
            but.set_callback2(move |b| {
                debug!(
                    "Pushed renderer #{} {} {}, state = {}",
                    bi,
                    newr_c.dev_model,
                    newr_c.dev_name,
                    if b.is_set() { "ON" } else { "OFF" }
                );
                if b.is_set() {
                    let _ = newr_c.play(&local_addr, SERVER_PORT, &wd, &log);
                    {
                        let mut conf = CONFIG.lock();
                        conf.last_renderer = b.label();
                        let _ = conf.update_config();
                    }
                } else {
                    let _ = newr_c.stop_play(&log);
                }
            });
            // the pack for the new button
            let mut pbutton = Pack::new(0, 0, bwidth, bheight, "");
            pbutton.end();
            pbutton.add(&but); // add the button to the window
            vpack.insert(&pbutton, binsert);
            buttons.insert(newr.remote_addr.clone(), but.clone()); // and keep a reference to it for bookkeeping
            app::redraw();
            // check if autoreconnect is set for this renderer
            if auto_reconnect.is_set() && but.label() == *last_renderer {
                but.turn_on(true);
                but.do_callback();
            }
        }
        // check the logchannel for new log messages to show in the logger textbox
        while let Ok(msg) = logreader.try_recv() {
            tb.buffer().unwrap().append(&msg);
            tb.buffer().unwrap().append("\n");
            let buflen = tb.buffer().unwrap().length();
            tb.set_insert_position(buflen);
            let buflines = tb.count_lines(0, buflen, true);
            tb.scroll(buflines, 0);
        }
    } // while app::wait()

    Ok(())
}

/// log - send a logmessage to the textbox on the Crossbeam LOGCHANNEL
fn log(s: String) {
    let cat: &str = &s[..2];
    match cat {
        "*W" => warn!("tb_log: {}", s),
        "*E" => error!("tb_log: {}", s),
        _ => info!("tb_log: {}", s),
    };
    let logger: Sender<String>;
    {
        let ch = &LOGCHANNEL.lock();
        logger = ch.0.clone();
    }
    logger.send(s).unwrap();
    app::awake();
}

/// a dummy_log is used during AV transport autoresume
fn dummy_log(s: String) {
    debug!("Autoresume: {}", s);
}

/// run_server - run a tiny-http webserver to serve streaming requests from renderers
///
/// all music is sent in audio/l16 PCM format (i16) with the sample rate of the source
/// the samples are read from a crossbeam channel fed by the wave_reader
/// a ChannelStream is created for this purpose, and inserted in the array of active
/// "clients" for the wave_reader
fn run_server(local_addr: &IpAddr, wd: WavData, feedback_tx: Sender<StreamerFeedBack>) {
    let addr = format!("{}:{}", local_addr, SERVER_PORT);
    let logmsg = format!(
        "The streaming server is listening on http://{}/stream/swyh.wav",
        addr,
    );
    log(logmsg);
    let logmsg = format!(
        "Sample rate: {}, sample format: audio/l16 (PCM)",
        wd.sample_rate.0.to_string(),
    );
    log(logmsg);
    let server = Arc::new(Server::http(addr).unwrap());
    let mut handles = Vec::new();
    for _ in 0..8 {
        let server = server.clone();
        let feedback_tx_c = feedback_tx.clone();
        handles.push(std::thread::spawn(move || {
            for rq in server.incoming_requests() {
                // get remote ip
                let remote_addr = format!("{}", rq.remote_addr());
                let mut remote_ip = remote_addr.clone();
                if let Some(i) = remote_ip.find(':') {
                    remote_ip.truncate(i);
                }
                // default headers
                let srvr_hdr =
                    Header::from_bytes(&b"Server"[..], &b"UPnP/1.0 DLNADOC/1.50 LAB/1.0"[..])
                        .unwrap();
                let nm_hdr = Header::from_bytes(&b"icy-name"[..], &b"swyh-rs"[..]).unwrap();
                let cc_hdr = Header::from_bytes(&b"Connection"[..], &b"close"[..]).unwrap();
                // check url
                if rq.url() != "/stream/swyh.wav" {
                    log(format!(
                        "Unrecognized request '{}' from {}'",
                        rq.url(),
                        rq.remote_addr()
                    ));
                    let response = Response::empty(404)
                        .with_header(cc_hdr)
                        .with_header(srvr_hdr)
                        .with_header(nm_hdr);
                    if let Err(e) = rq.respond(response) {
                        log(format!(
                            "=>Http POST connection with {} terminated [{}]",
                            remote_addr, e
                        ));
                    }
                    continue;
                }
                // get remote ip
                let remote_addr = format!("{}", rq.remote_addr());
                let mut remote_ip = remote_addr.clone();
                if let Some(i) = remote_ip.find(':') {
                    remote_ip.truncate(i);
                }
                // prpare streaming headers
                let conf = CONFIG.lock().clone();
                let ct_text = if conf.use_wave_format {
                    "audio/vnd.wave;codec=1".to_string()
                } else {
                    format!("audio/L16;rate={};channels=2", wd.sample_rate.0.to_string())
                };
                let ct_hdr = Header::from_bytes(&b"Content-Type"[..], ct_text.as_bytes()).unwrap();
                let tm_hdr =
                    Header::from_bytes(&b"TransferMode.DLNA.ORG"[..], &b"Streaming"[..]).unwrap();
                // handle response, streaming if GET, headers only otherwise
                if matches!(rq.method(), Method::Get) {
                    log(format!(
                        "Received request {} from {}",
                        rq.url(),
                        rq.remote_addr()
                    ));
                    // set transfer encoding chunked unless disabled
                    let (streamsize, chunked_threshold) = {
                        if conf.disable_chunked {
                            (Some(usize::MAX - 1), usize::MAX)
                        } else {
                            (None, 8192)
                        }
                    };
                    let (tx, rx): (Sender<Vec<i16>>, Receiver<Vec<i16>>) = unbounded();
                    let channel_stream = ChannelStream::new(
                        tx.clone(),
                        rx.clone(),
                        remote_ip.clone(),
                        conf.use_wave_format,
                        wd.sample_rate.0,
                    );
                    {
                        let mut clients = CLIENTS.lock();
                        clients.insert(remote_addr.clone(), channel_stream);
                        debug!("Now have {} streaming clients", clients.len());
                    }
                    feedback_tx_c
                        .send(StreamerFeedBack {
                            remote_ip: remote_ip.clone(),
                            streaming_state: StreamingState::Started,
                        })
                        .unwrap();
                    std::thread::yield_now();
                    let mut channel_stream = ChannelStream::new(
                        tx.clone(),
                        rx.clone(),
                        remote_ip.clone(),
                        conf.use_wave_format,
                        wd.sample_rate.0,
                    );
                    channel_stream.create_silence(wd.sample_rate.0);
                    let response = Response::empty(200)
                        .with_data(channel_stream, streamsize)
                        .with_chunked_threshold(chunked_threshold)
                        .with_header(cc_hdr)
                        .with_header(ct_hdr)
                        .with_header(tm_hdr)
                        .with_header(srvr_hdr)
                        .with_header(nm_hdr);
                    if let Err(e) = rq.respond(response) {
                        log(format!(
                            "=>Http connection with {} terminated [{}]",
                            remote_addr, e
                        ));
                    }
                    {
                        let mut clients = CLIENTS.lock();
                        clients.remove(&remote_addr.clone());
                        debug!("Now have {} streaming clients left", clients.len());
                    }
                    log(format!("Streaming to {} has ended", remote_addr));
                    // inform the main thread that this renderer has finished receiving
                    // necessary if the connection close was not caused by our own GUI
                    // so that we can update the corresponding button state
                    feedback_tx_c
                        .send(StreamerFeedBack {
                            remote_ip,
                            streaming_state: StreamingState::Ended,
                        })
                        .unwrap();
                    app::awake();
                    std::thread::yield_now();
                } else if matches!(rq.method(), Method::Head) {
                    debug!("HEAD rq from {}", remote_addr);
                    let response = Response::empty(200)
                        .with_header(cc_hdr)
                        .with_header(ct_hdr)
                        .with_header(tm_hdr)
                        .with_header(srvr_hdr)
                        .with_header(nm_hdr);
                    if let Err(e) = rq.respond(response) {
                        log(format!(
                            "=>Http HEAD connection with {} terminated [{}]",
                            remote_addr, e
                        ));
                    }
                } else if matches!(rq.method(), Method::Post) {
                    debug!("POST rq from {}", remote_addr);
                    let response = Response::empty(200)
                        .with_header(cc_hdr)
                        .with_header(srvr_hdr)
                        .with_header(nm_hdr);
                    if let Err(e) = rq.respond(response) {
                        log(format!(
                            "=>Http POST connection with {} terminated [{}]",
                            remote_addr, e
                        ));
                    }
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

/// run the ssdp_updater - thread that periodically run ssdp discovery
/// and detect new renderers
/// send any new renderers to te main thread on the Crossbeam ssdp channel
fn run_ssdp_updater(ssdp_tx: Sender<Renderer>, ssdp_interval_mins: f64) {
    // the hashmap used to detect new renderers
    let mut rmap: HashMap<String, Renderer> = HashMap::new();
    loop {
        let renderers = discover(&rmap, &log).unwrap_or_default();
        for r in renderers.iter() {
            if !rmap.contains_key(&r.remote_addr) {
                let _ = ssdp_tx.send(r.clone());
                app::awake();
                std::thread::yield_now();
                info!(
                    "Found new renderer {} {}  at {}",
                    r.dev_name, r.dev_model, r.remote_addr
                );
                rmap.insert(r.remote_addr.clone(), r.clone());
            }
        }
        std::thread::sleep(Duration::from_millis(
            (ssdp_interval_mins * 60.0 * 1000.0) as u64,
        ));
    }
}

/// capture_audio_output - capture the audio stream from the default audio output device
///
/// sets up an input stream for the wave_reader in the appropriate format (f32/i16/u16)
fn capture_output_audio(device: &cpal::Device) -> Option<cpal::Stream> {
    log(format!(
        "Capturing audio from: {}",
        device
            .name()
            .expect("Could not get default audio device name")
    ));
    let audio_cfg = device
        .default_output_config()
        .expect("No default output config found");
    log(format!("Default audio {:?}", audio_cfg));
    let mut i16_samples: Vec<i16> = Vec::with_capacity(16384);
    match audio_cfg.sample_format() {
        cpal::SampleFormat::F32 => match device.build_input_stream(
            &audio_cfg.config(),
            move |data, _: &_| wave_reader::<f32>(data, &mut i16_samples),
            capture_err_fn,
        ) {
            Ok(stream) => Some(stream),
            Err(e) => {
                log(format!("Error capturing f32 audio stream: {}", e));
                None
            }
        },
        cpal::SampleFormat::I16 => {
            match device.build_input_stream(
                &audio_cfg.config(),
                move |data, _: &_| wave_reader::<i16>(data, &mut i16_samples),
                capture_err_fn,
            ) {
                Ok(stream) => Some(stream),
                Err(e) => {
                    log(format!("Error capturing i16 audio stream: {}", e));
                    None
                }
            }
        }
        cpal::SampleFormat::U16 => {
            match device.build_input_stream(
                &audio_cfg.config(),
                move |data, _: &_| wave_reader::<u16>(data, &mut i16_samples),
                capture_err_fn,
            ) {
                Ok(stream) => Some(stream),
                Err(e) => {
                    log(format!("Error capturing u16 audio stream: {}", e));
                    None
                }
            }
        }
    }
}

/// capture_err_fn - called whan it's impossible to build an audio input stream
fn capture_err_fn(err: cpal::StreamError) {
    log(format!("Error {} building audio input stream", err));
}

/// wave_reader - the captured audio input stream reader
///
/// writes the captured samples to all registered clients in the
/// CLIENTS ChannnelStream hashmap
fn wave_reader<T>(samples: &[T], i16_samples: &mut Vec<i16>)
where
    T: cpal::Sample,
{
    static INITIALIZER: Once = Once::new();
    INITIALIZER.call_once(|| {
        log("The wave_reader is now receiving samples".to_string());
    });
    i16_samples.clear();
    i16_samples.extend(samples.iter().map(|x| x.to_i16()));
    let clients = CLIENTS.lock();
    for (_, v) in clients.iter() {
        v.write(i16_samples);
    }
}
