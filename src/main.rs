#![windows_subsystem = "windows"] // to suppress console with debug output for release builds
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
use crate::utils::audiodevices::*;
use crate::utils::configuration::Configuration;
use crate::utils::escape::{FwSlashEscape, FwSlashUnescape};
use crate::utils::local_ip_address::get_local_addr;
use crate::utils::priority::raise_priority;
use crate::utils::rwstream::ChannelStream;
use ascii::*;
use cpal::traits::{DeviceTrait, StreamTrait};
use crossbeam_channel::{unbounded, Receiver, Sender};
use fltk::{
    app, button::*, dialog, frame::*, group::*, menu::*, text::*, valuator::Counter, window::*,
};
use lazy_static::*;
use log::*;
use simplelog::{CombinedLogger, Config, TermLogger, WriteLogger};
use std::collections::HashMap;
use std::fs::File;
use std::net::IpAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tiny_http::*;

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
    let audio_cfg = &audio_output_device
        .default_output_config()
        .expect("No default output config found");

    let _app = app::App::default().with_scheme(app::Scheme::Gleam);
    let ww = 660;
    let wh = 660;
    let mut wind = Window::default().with_size(ww, wh).with_label(&format!(
        "swyh-rs UPNP/DLNA Media Renderers V{}",
        APP_VERSION
    ));
    wind.handle(move |_ev| {
        //eprintln!("{:?}", app::event());
        let ev = app::event();
        match ev {
            Event::Close => {
                _app.quit();
                std::process::exit(0);
            }
            _ => false,
        }
    });

    wind.make_resizable(false);
    wind.end();
    wind.show();

    let gw = 600;
    let fw = 600;
    let xpos = 30;
    let mut ypos = 5;

    // title frame
    let mut p1 = Pack::new(xpos, ypos, gw, 25, "");
    p1.set_type(PackType::Vertical);
    p1.end();
    let mut opt_frame = Frame::new(0, 0, 0, 25, "").with_align(Align::Center);
    opt_frame.set_frame(FrameType::BorderBox);
    opt_frame.set_label("Options");
    p1.add(&opt_frame);
    ypos += 35;
    wind.add(&p1);

    // setup feedback textbox at the bottom
    let mut p2 = Pack::new(2, wh - 160, ww - 4, 156, "");
    p2.set_type(PackType::Vertical);
    p2.end();
    let buf = TextBuffer::default();
    let mut tb = TextDisplay::new(0, 0, 0, 150, "").with_align(Align::Left);
    tb.set_buffer(Some(buf));
    p2.add(&tb);
    p2.resizable(&mut tb.clone());
    wind.add(&p2);
    wind.redraw();
    update_ui();
    // setup the feedback textbox logger thread
    let _ = std::thread::Builder::new()
        .name("textdisplay_updater".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(move || tb_logger(tb))
        .unwrap();

    // read config
    let mut config = Configuration::read_config();
    if config.sound_source == "None" {
        config.sound_source = audio_output_device.name().unwrap();
        let _ = config.update_config();
    }
    log(format!("{:?}", config));
    if cfg!(debug_assertions) {
        config.log_level = LevelFilter::Debug;
    }
    static mut CONFIG_CHANGED: bool = false;

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
    let mut p3 = Pack::new(xpos, ypos, gw, 25, "");
    p3.set_type(PackType::Horizontal);
    p3.end();
    let mut auto_resume = CheckButton::new(0, 0, 150, 25, "Autoresume play");
    if config.auto_resume {
        auto_resume.set(true);
    }
    p3.add(&auto_resume);
    let sp1 = Frame::new(0, 0, 10, 25, "");
    p3.add(&sp1);
    let auto_resume_c = auto_resume.clone();
    auto_resume.handle(move |ev| match ev {
        Event::Released => {
            let mut config = Configuration::read_config();
            if auto_resume_c.is_set() {
                config.auto_resume = true;
            } else {
                config.auto_resume = false;
            }
            let _ = config.update_config();
            true
        }
        _ => true,
    });
    // show ssdp interval counter
    let mut ssdp_interval = Counter::new(0, 0, 150, 25, "SSDP Interval (in minutes)");
    ssdp_interval.set_value(config.ssdp_interval_mins);
    let mut ssdp_interval_c = ssdp_interval.clone();
    ssdp_interval.handle(move |ev| match ev {
        Event::Leave => {
            let mut config = Configuration::read_config();
            if ssdp_interval_c.value() < 0.5 {
                ssdp_interval_c.set_value(0.5);
            }
            if (config.ssdp_interval_mins - ssdp_interval_c.value()).abs() > 0.09 {
                config.ssdp_interval_mins = ssdp_interval_c.value();
                log(format!(
                    "*W*W*> ssdp interval changed to {} minutes, restart required!!",
                    config.ssdp_interval_mins
                ));
                let _ = config.update_config();
                unsafe {
                    CONFIG_CHANGED = true;
                }
            }
            true
        }
        _ => false,
    });

    p3.add(&ssdp_interval);
    let sp2 = Frame::new(0, 0, 10, 25, "");
    p3.add(&sp2);

    // show log level choice
    let ll = format!("Log Level: {}", config.log_level.to_string());
    let mut log_level_choice = MenuButton::new(0, 0, 150, 25, &ll);
    let log_levels = vec!["Info", "Debug"];
    for ll in log_levels.iter() {
        log_level_choice.add_choice(ll);
    }
    let rlock = Mutex::new(0);
    let mut log_lc_c = log_level_choice.clone();
    log_level_choice.handle(move |ev| {
        let mut recursion = rlock.lock().unwrap();
        if *recursion > 0 {
            return false;
        }
        *recursion += 1;
        match ev {
            Event::Push => {
                let mut config = Configuration::read_config();
                let i = log_lc_c.value();
                if i < 0 {
                    return false;
                }
                let level = log_levels[i as usize];
                log(format!(
                    "*W*W*> Log level changed to {}, restart required!!",
                    level
                )); // std::env::current_exe()
                config.log_level = level.parse().unwrap_or(LevelFilter::Info);
                let _ = config.update_config();
                *recursion -= 1;
                unsafe {
                    CONFIG_CHANGED = true;
                }
                let ll = format!("Log Level: {}", config.log_level.to_string());
                log_lc_c.set_label(&ll);
                true
            }
            _ => {
                *recursion -= 1;
                false
            }
        }
    });

    p3.add(&log_level_choice);
    wind.add(&p3);
    wind.redraw();
    update_ui();
    ypos += 55;

    // set the output device
    let audio_devices = get_output_audio_devices().unwrap();
    for adev in audio_devices {
        let devname = adev.name().unwrap();
        if devname == config.sound_source {
            audio_output_device = adev;
            debug!("Selected audio source: {}", devname);
        }
    }

    // setup audio source choice
    let mut p4 = Pack::new(xpos, ypos, gw, 25, "");
    p4.set_type(PackType::Vertical);
    p4.end();
    let cur_audio_src = format!("Source: {}", config.sound_source);
    log("Setup audio sources".to_string());
    let mut choose_audio_source_but = MenuButton::new(0, 0, 0, 25, &cur_audio_src);
    let devices = get_output_audio_devices().unwrap();
    for dev in devices.iter() {
        choose_audio_source_but.add_choice(&dev.name().unwrap().fw_slash_escape());
    }
    let mut butas_cc = choose_audio_source_but.clone();
    // apparently this event can recurse on very fast machines
    // probably because it takes some time doing the file I/O, hence recursion lock
    let lock = Mutex::new(0);
    choose_audio_source_but.handle(move |ev| {
        let mut recursion = lock.lock().unwrap();
        if *recursion > 0 {
            return false;
        }
        *recursion += 1;
        match ev {
            Event::Push => {
                let mut config = Configuration::read_config();
                let i = butas_cc.value();
                if i < 0 {
                    return false;
                }
                let name = devices[i as usize].name().unwrap().fw_slash_unescape();
                log(format!(
                    "*W*W*> Audio source changed to {}, restart required!!",
                    name
                )); // std::env::current_exe()
                config.sound_source = name;
                let _ = config.update_config();
                butas_cc.set_label(&format!("New Source: {}", config.sound_source));
                *recursion -= 1;
                unsafe {
                    CONFIG_CHANGED = true;
                }
                true
            }
            _ => {
                *recursion -= 1;
                false
            }
        }
    });
    p4.add(&choose_audio_source_but);
    wind.add(&p4);
    wind.redraw();
    update_ui();
    ypos += 35;

    // we need to pass some audio config data to the play function
    let wd = WavData {
        sample_format: audio_cfg.sample_format(),
        sample_rate: audio_cfg.sample_rate(),
        channels: audio_cfg.channels(),
    };

    // raise process priority a bit to prevent audio stuttering under cpu load
    raise_priority();

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
    // start webserver on the local address, with a feedback channel for connection accept/drop
    let local_addr = get_local_addr().expect("Could not obtain local address.");
    let (feedback_tx, feedback_rx): (Sender<StreamerFeedBack>, Receiver<StreamerFeedBack>) =
        unbounded();
    let _ = std::thread::Builder::new()
        .name("swyh_rs_webserver".into())
        .stack_size(4 * 102 * 1024)
        .spawn(move || run_server(&local_addr, wd, feedback_tx.clone()))
        .unwrap();
    std::thread::yield_now();

    // show renderer buttons title
    let mut p5 = Pack::new(xpos, ypos, gw, 25, "");
    p5.set_type(PackType::Vertical);
    p5.end();
    let mut frame = Frame::new(0, 0, fw, 25, "").with_align(Align::Center);
    frame.set_frame(FrameType::BorderBox);
    frame.set_label(&format!("UPNP rendering devices on network {}", local_addr));
    p5.add(&frame);
    wind.add(&p5);
    ypos += 35;
    wind.redraw();
    update_ui();

    // create a hashmap for a button for each discovered renderer
    let mut buttons: HashMap<String, LightButton> = HashMap::new();
    // start SSDP discovery update thread with a channel for renderer updates
    let (ssdp_tx, ssdp_rx): (Sender<Renderer>, Receiver<Renderer>) = unbounded();
    // the renderers discovered so far
    let mut renderers: Vec<Renderer> = Vec::new();
    log("Running SSDP discovery".to_string());
    let _ = std::thread::Builder::new()
        .name("ssdp_updater".into())
        .stack_size(4 * 102 * 1024)
        .spawn(move || run_ssdp_updater(ssdp_tx, config.ssdp_interval_mins))
        .unwrap();

    // button dimensions and starting position
    let bwidth = frame.width();
    let bheight = frame.height();

    // run GUI, _app.wait() and _app.run() somehow block the logger channel
    // from receiving messages
    let auto_resume_c = &auto_resume;
    loop {
        app::wait_for(0.0)?;
        if wind.width() < (ww / 2) {
            wind.resize(wind.x(), wind.y(), ww, wh);
            wind.redraw();
            app::wait_for(0.0)?;
        }
        if wind.height() < (wh / 2) {
            wind.resize(wind.x(), wind.y(), ww, wh);
            wind.redraw();
            app::wait_for(0.0)?;
        }
        unsafe {
            if CONFIG_CHANGED {
                //restart_but.show();
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
                    CONFIG_CHANGED = false;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        if app::should_program_quit() {
            break;
        }
        // check if the webserver has closed a connection not caused by pushing the renderer button
        // in that case we turn the button off as a visual feedback
        if let Ok(streamer_feedback) = feedback_rx.try_recv() {
            if let Some(button) = buttons.get_mut(&streamer_feedback.remote_ip) {
                match streamer_feedback.streaming_state {
                    StreamingState::Started => {
                        if !button.is_set() {
                            button.set(true);
                        }
                    }
                    StreamingState::Ended => {
                        if auto_resume_c.is_set() && button.is_set() {
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
        // check the ssdp discovery thread channel for a newly discovered renderer
        // if yes: add a new button below the last one
        if let Ok(newr) = ssdp_rx.try_recv() {
            let mut but = LightButton::default() // create the button
                .with_size(bwidth, bheight)
                .with_pos(0, 0)
                .with_align(Align::Center)
                .with_label(&format!("{} {}", newr.dev_model, newr.dev_name));
            renderers.push(newr.clone());
            // prepare for event handler closure
            let newr_c = newr.clone();
            let but_c = but.clone();
            let bi = buttons.len();
            but.handle(move |ev| {
                let but_cc = but_c.clone();
                match ev {
                    Event::Push => {
                        debug!(
                            "Pushed renderer #{} {} {}, state = {}",
                            bi,
                            newr_c.dev_model,
                            newr_c.dev_name,
                            if but_cc.is_set() { "ON" } else { "OFF" }
                        );
                        if but_cc.is_set() {
                            let _ = newr_c.play(&local_addr, SERVER_PORT, &wd, &log);
                        } else {
                            let _ = newr_c.stop_play(&log);
                        }
                        true
                    }
                    _ => true,
                }
            });
            // the pack for the new button
            let mut pbutton = Pack::new(xpos, ypos, bwidth, bheight, "");
            pbutton.set_type(PackType::Vertical);
            pbutton.end();
            pbutton.add(&but); // add the button to the window
            wind.add(&pbutton);
            wind.redraw();
            buttons.insert(newr.remote_addr.clone(), but.clone()); // and keep a reference to it for bookkeeping
            ypos += bheight + 10; // and the button y offset
        }
    }
    Ok(())
}

///
/// update_ui - let fltk update the UI that was changed by other threads
///
fn update_ui() {
    // FLTK Windows seems to handle all messages, but Linux apparently needs a loop ?
    const COUNT: i32 = if cfg!(windows) { 2 } else { 100 };
    for _ in 1..COUNT {
        let _ = app::wait_for(0.0).unwrap_or_default();
        std::thread::yield_now();
    }
}

/// tb_logger - the TextBox logger thread
/// this function reads log messages from the LOGCHANNEL receiver
/// and adds them to an fltk TextBox
fn tb_logger(mut tb: TextDisplay) {
    let logreader: Receiver<String>;
    {
        let ch = &LOGCHANNEL.lock().unwrap();
        logreader = ch.1.clone();
    }
    loop {
        let msg = logreader
            .recv()
            .unwrap_or_else(|_| "*E*E*> TB LOGGER channel receive error".to_string());
        tb.buffer().unwrap().append(&msg);
        tb.buffer().unwrap().append("\n");
        let buflen = tb.buffer().unwrap().length();
        tb.set_insert_position(buflen);
        let buflines = tb.count_lines(0, buflen, true);
        tb.scroll(buflines, 0);
        std::thread::yield_now();
    }
}

/// log - send a logmessage to the textbox on the LOGCHANNEL
fn log(s: String) {
    let cat: &str = &s[..2];
    match cat {
        "*W" => warn!("tb_log: {}", s),
        "*E" => error!("tb_log: {}", s),
        _ => info!("tb_log: {}", s),
    };
    let logger: Sender<String>;
    {
        let ch = &LOGCHANNEL.lock().unwrap();
        logger = ch.0.clone();
    }
    logger.send(s).unwrap();
}

/// dummy_log is used during AV transport autoresume
fn dummy_log(s: String) {
    debug!("Autoresume: {}", s);
}

/// run_server - run a webserver to serve requests from OpenHome/AV media renderers
///
/// all music is sent in audio/l16 PCM format (i16) with the sample rate of the source
/// the samples are read from a crossbeam channel fed by the wave_reader
/// a ChannelStream is created for this purpose, and inserted in the array of active
/// "clients" for the wave_reader
fn run_server(local_addr: &IpAddr, wd: WavData, feedback_tx: Sender<StreamerFeedBack>) {
    let addr = format!("{}:{}", local_addr, SERVER_PORT);
    let logmsg = format!(
        "The streaming server is listening on http://{}/stream/swyh.wav",
        addr
    );
    log(logmsg);
    let server = Arc::new(Server::http(addr).unwrap());
    let mut handles = Vec::new();
    for _ in 0..8 {
        let server = server.clone();
        let feedback_tx_c = feedback_tx.clone();
        handles.push(std::thread::spawn(move || {
            for rq in server.incoming_requests() {
                if rq.url() != "/stream/swyh.wav" {
                    log(format!(
                        "Unrecognized request '{}' from {}'",
                        rq.url(),
                        rq.remote_addr()
                    ));
                }
                // get remote ip
                let remote_addr = format!("{}", rq.remote_addr());
                let mut remote_ip = remote_addr.clone();
                if let Some(i) = remote_ip.find(':') {
                    remote_ip.truncate(i);
                }
                // prpare headers
                let ct_text = format!("audio/L16;rate={};channels=2", wd.sample_rate.0.to_string());
                let ct_hdr = tiny_http::Header {
                    field: "Content-Type".parse().unwrap(),
                    value: AsciiString::from_ascii(ct_text).unwrap(),
                };
                let tm_hdr = tiny_http::Header {
                    field: "TransferMode.DLNA.ORG".parse().unwrap(),
                    value: AsciiString::from_ascii("Streaming").unwrap(),
                };
                let srvr_hdr = tiny_http::Header {
                    field: "Server".parse().unwrap(),
                    value: AsciiString::from_ascii("UPnP/1.0 DLNADOC/1.50 LAB/1.0").unwrap(),
                };
                let nm_hdr = tiny_http::Header {
                    field: "icy-name".parse().unwrap(),
                    value: AsciiString::from_ascii("swyh-rs").unwrap(),
                };
                let cc_hdr = tiny_http::Header {
                    field: "Connection".parse().unwrap(),
                    value: AsciiString::from_ascii("close").unwrap(),
                };
                // handle response, streaming if GET, headers only otherwise
                if matches!(rq.method(), Method::Get) {
                    log(format!(
                        "Received request {} from {}",
                        rq.url(),
                        rq.remote_addr()
                    ));
                    let (tx, rx): (Sender<Vec<i16>>, Receiver<Vec<i16>>) = unbounded();
                    {
                        let channel_stream = ChannelStream::new(tx.clone(), rx.clone());
                        let mut clients = CLIENTS.lock().unwrap();
                        clients.insert(remote_ip.clone(), channel_stream);
                        debug!("Now have {} streaming clients", clients.len());
                    }
                    feedback_tx_c
                        .send(StreamerFeedBack {
                            remote_ip: remote_ip.clone(),
                            streaming_state: StreamingState::Started,
                        })
                        .unwrap();
                    std::thread::yield_now();
                    let channel_stream = ChannelStream::new(tx.clone(), rx.clone());
                    let response = Response::empty(200)
                        .with_data(channel_stream, Some(0x7FFFFFFF))
                        .with_header(cc_hdr)
                        .with_header(ct_hdr)
                        .with_header(tm_hdr)
                        .with_header(srvr_hdr)
                        .with_header(nm_hdr);
                    match rq.respond(response) {
                        Ok(_) => {}
                        Err(e) => {
                            log(format!(
                                "=>Http connection with {} terminated [{}]",
                                remote_addr, e
                            ));
                        }
                    }
                    {
                        let mut clients = CLIENTS.lock().unwrap();
                        clients.remove(&remote_ip.clone());
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
                } else if matches!(rq.method(), Method::Head) {
                    debug!("HEAD rq from {}", remote_addr);
                    let response = Response::empty(200)
                        .with_header(cc_hdr)
                        .with_header(ct_hdr)
                        .with_header(tm_hdr)
                        .with_header(srvr_hdr)
                        .with_header(nm_hdr);
                    match rq.respond(response) {
                        Ok(_) => {}
                        Err(e) => {
                            log(format!(
                                "=>Http HEAD connection with {} terminated [{}]",
                                remote_addr, e
                            ));
                        }
                    }
                } else if matches!(rq.method(), Method::Post) {
                    debug!("POST rq from {}", remote_addr);
                    let response = Response::empty(200)
                        .with_header(cc_hdr)
                        .with_header(srvr_hdr)
                        .with_header(nm_hdr);
                    match rq.respond(response) {
                        Ok(_) => {}
                        Err(e) => {
                            log(format!(
                                "=>Http POST connection with {} terminated [{}]",
                                remote_addr, e
                            ));
                        }
                    }
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
}

/// get_renderers - get a list of all renderers using SSDP discovery in a seperate thread
fn get_renderers(rmap: HashMap<String, Renderer>) -> Vec<Renderer> {
    let renderers: Vec<Renderer>;
    let discover_handle: JoinHandle<Vec<Renderer>> = std::thread::Builder::new()
        .name("ssdp_discover".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(|| discover(rmap, &log).unwrap_or_default())
        .unwrap();
    // wait for discovery to complete (max 3.1 secs)
    let start = Instant::now();
    loop {
        let duration = start.elapsed();
        // keep capturing responses for more then 3 seconds (M_SEARCH MX time)
        if duration > Duration::from_millis(3_200) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // collect the discovery result
    renderers = discover_handle.join().unwrap_or_default();
    renderers
}

/// run_ssdp_updater - thread that periodically run ssdp discovery
/// and detect new renderers
/// send any new renderers to te main thread on the ssdp channel
fn run_ssdp_updater(ssdp_tx: Sender<Renderer>, ssdp_interval_mins: f64) {
    // the hashmap used to detect new renderers
    let mut rmap: HashMap<String, Renderer> = HashMap::new();
    loop {
        let renderers = get_renderers(rmap.clone());
        for r in renderers.iter() {
            if !rmap.contains_key(&r.remote_addr) {
                let _ = ssdp_tx.send(r.clone());
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
    match audio_cfg.sample_format() {
        cpal::SampleFormat::F32 => match device.build_input_stream(
            &audio_cfg.config(),
            move |data, _: &_| wave_reader::<f32>(data),
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
                move |data, _: &_| wave_reader::<i16>(data),
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
                move |data, _: &_| wave_reader::<u16>(data),
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
fn wave_reader<T>(samples: &[T])
where
    T: cpal::Sample,
{
    static mut ONETIME_SW: bool = false;
    unsafe {
        if !ONETIME_SW {
            log("The wave_reader is receiving samples".to_string());
            ONETIME_SW = true;
        }
    }

    let i16_samples: Vec<i16> = samples.iter().map(|x| x.to_i16()).collect();
    let clients = CLIENTS.lock().unwrap();
    for (_, v) in clients.iter() {
        v.write(&i16_samples);
    }
}
