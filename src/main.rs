//#![windows_subsystem = "windows"]  // to suppress console with debug output for release builds
///
/// swyh-rs
///
/// Basic SWYH (https://www.streamwhatyouhear.com/, source repo https://github.com/StreamWhatYouHear/SWYH) clone entirely written in rust.
///
/// Has only been tested with Volumio (https://volumio.org/) streamers, but will probably support any streamer that supports the OpenHome
/// protocol (not the original DLNA).
///
/// I wrote this because I a) wanted to learn Rust and b) SWYH did not work on Linux and did not work well with Volumio (push streaming does not work).
///
/// For the moment all music is streamed in wav-format (audio/l16) with the sample rate of the music source (the default audio device, I use HiFi Cable Input).
///
/// I had to fork cpal (https://github.com/RustAudio/cpal), so if you want to build swyh-rs yourself you have to clone dheijl/cpal
/// and change the cargo.toml file accordingly.
///
/// I use fltk-rs (https://github.com/MoAlyousef/fltk-rs) for the GUI, as it's easy to use and works well.
///
/// Tested on Windows 10 and on Ubuntu 20.04 with Raspberry Pi based Volumio devices. Don't have access to a Mac, so I don't know if this would work.
///
/// Todo:
///
/// - make everything more robust (error handling)
/// - clean-up and comments
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

use std::path::Path;
use std::fs::File;
use simplelog::Config;
use simplelog::WriteLogger;
use simplelog::TermLogger;
use simplelog::CombinedLogger;
use crate::openhome::avmedia::{discover, Renderer, WavData};
use crate::utils::audiodevices::*;
use crate::utils::configuration::Configuration;
use crate::utils::escape::{FwSlashEscape, FwSlashUnescape};
use crate::utils::local_ip_address::get_local_addr;
use crate::utils::priority::raise_priority;
use crate::utils::rwstream::ChannelStream;
use ascii::AsciiString;
use cpal::traits::{DeviceTrait, StreamTrait};
use crossbeam_channel::{unbounded, Receiver as OtherReceiver, Sender as OtherSender};
use fltk::{app, button::*, frame::*, menu::*, text::*, window::*};
use lazy_static::*;
use log::*;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tiny_http::*;

/// app version
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[macro_export]
macro_rules! DEBUG {
    ($x:stmt) => {
        if cfg!(debug_assertions) {
            $x
        }
    };
}

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
    static ref CLIENTS: Mutex<HashMap<String, ChannelStream>> = Mutex::new(HashMap::new());
    static ref LOGCHANNEL: Mutex<(OtherSender<String>, OtherReceiver<String>)> =
        Mutex::new(unbounded());
}

/// swyh-rs
///
/// - set up the fltk GUI
/// - discover ssdp media renderers and show them in the GUI as buttons (start/stop play)
/// - setup and start audio capture
/// - start the webserver
/// - run the GUI
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // first initialize cpal audio to prevent COM reinitialize panic on Windows
    let mut audio_output_device =
        get_default_audio_output_device().expect("No default audio device");
    let audio_cfg = &audio_output_device
        .default_output_config()
        .expect("No default output config found");

    // read config
    let mut config = Configuration::read_config();
    if config.sound_source == "None" {
        config.sound_source = audio_output_device.name().unwrap();
        let _ = config.update_config();
    }
    DEBUG!(eprintln!("{:?}", config));

    // configure simplelogger
    let loglevel = config.log_level;
    let logfile = Path::new(&config.config_dir()).join("log.txt");
    let _ = CombinedLogger::init(
        vec![
            TermLogger::new(loglevel, Config::default(), simplelog::TerminalMode::Stderr),
            WriteLogger::new(loglevel, Config::default(), File::create(logfile).unwrap())
        ]
    );
    info!("swyh-rs Logging started.");

    // set the output device
    let audio_devices = get_output_audio_devices().unwrap();
    for adev in audio_devices {
        let devname = adev.name().unwrap();
        if devname == config.sound_source {
            audio_output_device = adev;
            DEBUG!(eprintln!("Selected audio source: {}", devname));
        }
    }

    let _app = app::App::default().with_scheme(app::Scheme::Gleam);
    let (sw, sh) = app::screen_size();
    let mut wind = Window::default()
        .with_size((sw / 2.5) as i32, (sh / 2.0) as i32)
        .with_label(&format!(
            "swyh-rs UPNP/DLNA Media Renderers V{}",
            APP_VERSION
        ));
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
    let mut tb =
        TextDisplay::new(2, wind.height() - 154, wind.width() - 4, 150, "").with_align(Align::Left);
    tb.set_buffer(Some(buf));
    // setup the he textbox logger thread
    let _ = std::thread::Builder::new()
        .name("textdisplay_updater".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(move || tb_logger(tb))
        .unwrap();
    update_ui();

    let local_addr = get_local_addr().expect("Could not obtain local address.");
    frame.set_label(&format!(
        "Scanning {} for UPNP rendering devices",
        local_addr
    ));
    wind.make_resizable(true);
    wind.end();
    wind.show();
    update_ui();

    // get the av media renderers in this network in  the discover thread
    let renderers: Vec<Renderer>;
    let discover_handle: JoinHandle<Vec<Renderer>> = std::thread::Builder::new()
        .name("ssdp_discover".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(|| discover(&log).unwrap_or_default())
        .unwrap();
    // wait for discovery to complete (max 3.1 secs)
    let start = Instant::now();
    loop {
        let duration = start.elapsed();
        // keep capturing responses for more then 3 seconds (M_SEARCH MX time)
        if duration > Duration::from_millis(3_200) {
            break;
        }
        update_ui();
        std::thread::sleep(std::time::Duration::new(0, 100_000_000));
    }
    // collect the discovery result
    renderers = discover_handle.join().unwrap_or_default();
    DEBUG!(eprintln!("Got {} renderers", renderers.len()));

    // now create a button for each discovered renderer
    let mut buttons: HashMap<String, LightButton> = HashMap::new();
    // button dimensions and starting position
    let bwidth = frame.width() / 2; // button width
    let bheight = frame.height(); // button height
    let bx = ((wind.width() - 30) / 2) - (bwidth / 2); // button x offset
    let mut by = frame.y() + frame.height() + 10; // button y offset
                                                  // create the buttons
                                                  // we need to pass some audio config data to the play function
    let wd = WavData {
        sample_format: audio_cfg.sample_format(),
        sample_rate: audio_cfg.sample_rate(),
        channels: audio_cfg.channels(),
    };
    // loop over the renderers with the associated button index
    for (bi, renderer) in renderers.iter().enumerate() {
        let mut but = LightButton::default() // create the button
            .with_size(bwidth, bheight)
            .with_pos(bx, by)
            .with_align(Align::Center)
            .with_label(&format!("{} {}", renderer.dev_model, renderer.dev_name));
        // prepare for closure
        let renderer_c = renderer.clone();
        let but_c = but.clone();
        but.handle(Box::new(move |ev| {
            let but_cc = but_c.clone();
            match ev {
                Event::Push => {
                    DEBUG!(eprintln!(
                        "Pushed renderer #{} {} {}, state = {}",
                        bi,
                        renderer_c.dev_model,
                        renderer_c.dev_name,
                        if but_cc.is_set() { "ON" } else { "OFF" }
                    ));
                    if but_cc.is_set() {
                        let _ = renderer_c.play(&local_addr, SERVER_PORT, &wd, &log);
                    } else {
                        let _ = renderer_c.stop_play(&log);
                    }
                    true
                }
                _ => true,
            }
        }));
        wind.add(&but); // add the button to the window
        buttons.insert(renderer.remote_addr.clone(), but.clone()); // and keep a reference to it for bookkeeping
        by += bheight + 10; // and the button y offset
    }
    frame.set_label("Rendering Devices");
    wind.redraw();
    update_ui();

    // raise process priority a bit to prevent audio stuttering under cpu load
    raise_priority();

    // capture system audio
    DEBUG!(eprintln!("Try capturing system audio"));
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
    // start webserver
    let (feedback_tx, feedback_rx): (
        OtherSender<StreamerFeedBack>,
        OtherReceiver<StreamerFeedBack>,
    ) = unbounded();
    let _ = std::thread::Builder::new()
        .name("swyh_rs_webserver".into())
        .stack_size(4 * 102 * 1024)
        .spawn(move || run_server(&local_addr, wd, feedback_tx.clone()))
        .unwrap();
    std::thread::yield_now();

    // show auto_resume option checkbox
    by += 10;
    let mut auto_resume = CheckButton::new(20, by, 150, 25, "Autoresume play");
    if config.auto_resume {
        auto_resume.set(true);
    }
    let auto_resume_c = auto_resume.clone();
    auto_resume.handle(Box::new(move |ev| match ev {
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
    }));
    wind.add(&auto_resume);
    wind.redraw();
    update_ui();

    by += 35;
    let mut choose_audio_source_but =
        MenuButton::new(20, by, (wind.width() / 3) * 2, 25, "Change Audio Source");
    let devices = get_output_audio_devices().unwrap();
    for dev in devices.iter() {
        choose_audio_source_but.add_choice(&dev.name().unwrap().fw_slash_escape());
    }
    let butas_cc = choose_audio_source_but.clone();
    // apparently this event can recurse on very fast machines
    // probably because it takes some time doing the file I/O, hence recursion lock
    let lock = Mutex::new(0);
    choose_audio_source_but.handle(Box::new(move |ev| {
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
                *recursion -= 1;
                true
            }
            _ => {
                *recursion -= 1;
                false
            }
        }
    }));
    wind.add(&choose_audio_source_but);
    wind.redraw();
    update_ui();

    // run GUI, _app.wait() and _app.run() somehow block the logger channel
    // from receiving messages
    let auto_resume_c = &auto_resume;
    loop {
        app::wait_for(0.0)?;
        if wind.width() < (sw / 3.0) as i32 || wind.height() < (sh / 3.0) as i32 {
            wind.resize(wind.x(), wind.y(), (sw / 3.0) as i32, (sh / 3.0) as i32);
            wind.redraw();
        }
        std::thread::sleep(std::time::Duration::new(0, 10_000_000));
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
    }
    Ok(())
}

///
/// update_ui - let fltk update the UI that was changed by other threads
///
fn update_ui() {
    for _ in 1..100 {
        let _ = app::wait_for(0.0).unwrap_or_default();
        std::thread::sleep(std::time::Duration::new(0, 1_000_000));
    }
}

/// tb_logger - the TextBox logger thread
/// this function reads log messages from the LOGCHANNEL receiver
/// and adds them to an fltk TextBox (using a mutex)
fn tb_logger(mut tb: TextDisplay) {
    let logreader: OtherReceiver<String>;
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

/// log - send a logmessage on the LOGCHANNEL sender
fn log(s: String) {
    DEBUG!(eprintln!("log: {}", s));
    let logger: OtherSender<String>;
    {
        let ch = &LOGCHANNEL.lock().unwrap();
        logger = ch.0.clone();
    }
    logger.send(s).unwrap();
}

/// dummy_log is used during AV transport autoresume
fn dummy_log(s: String) {
    DEBUG!(eprintln!("Autoresume: {}", s));
}

/// run_server - run a webserver to serve requests from OpenHome media renderers
///
/// all music is sent in audio/l16 PCM format (i16) with the sample rate of the source
/// the samples are read from a crossbeam channel fed by the wave_reader
/// a ChannelStream is created for this purpose, and inserted in the array of active
/// "clients" for the wave_reader
fn run_server(local_addr: &IpAddr, wd: WavData, feedback_tx: OtherSender<StreamerFeedBack>) {
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
                    let (tx, rx): (OtherSender<Vec<i16>>, OtherReceiver<Vec<i16>>) = unbounded();
                    {
                        let channel_stream = ChannelStream::new(tx.clone(), rx.clone());
                        let mut clients = CLIENTS.lock().unwrap();
                        clients.insert(remote_ip.clone(), channel_stream);
                        DEBUG!(eprintln!("Now have {} streaming clients", clients.len()));
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
                        DEBUG!(eprintln!(
                            "Now have {} streaming clients left",
                            clients.len()
                        ));
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
                    DEBUG!(eprintln!("HEAD rq from {}", remote_addr));
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
                    DEBUG!(eprintln!("POST rq from {}", remote_addr));
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
