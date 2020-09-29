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
use ascii::AsciiString;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{unbounded, Receiver as OtherReceiver, Sender as OtherSender};
use fltk::{app, button::*, frame::*, text::*, window::*};
use lazy_static::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
mod openhome;
mod utils;
use openhome::avmedia;
use utils::rwstream::ChannelStream;

#[derive(Debug, Clone, Copy)]
pub enum Message {
    Increment,
    Decrement,
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

/// swyh-rs
///
/// - set up the fltk GUI
/// - discover ssdp media renderers and show them in the GUI as buttons (start/stop play)
/// - setup and start audio capture
/// - start the webserver
/// - run the GUI
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // first initialize cpal audio to prevent COM reinitialize panic on Windows
    let audio_output_device = get_audio_device();
    let audio_cfg = &audio_output_device
        .default_output_config()
        .expect("No default output config found");

    let _app = app::App::default().with_scheme(app::Scheme::Gleam);
    let (sw, sh) = app::screen_size();
    let mut wind = Window::default()
        .with_size((sw / 2.5) as i32, (sh / 2.0) as i32)
        .with_label("swyh-rs UPNP/DLNA Media Renderers");
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

    // setup the he textbox logger thread
    let _ = std::thread::spawn(move || tb_logger(tb));

    for _ in 1..100 {
        app::wait_for(0.001)?
    }

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

    // get the av media renderers in this network
    let renderers = avmedia::discover(&log);

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
                                let _ = renderer.oh_play(&local_addr, &log);
                            } else {
                                let _ = renderer.oh_stop_play(&log);
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

/// tb_logger - a TextBox logger
/// this function reads log messages from the LOGCHANNEL receiver
/// and adds them in an fltk TextBox
fn tb_logger(tb: Arc<Mutex<TextDisplay>>) {
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
        // this seems to work to let the UI update the TextBox
        drop(_tb);
    }
}

/// log - send a logmessage on the LOGCHANNEL sender
fn log(s: String) {
    let logger: OtherSender<String>;
    {
        let ch = &LOGCHANNEL.lock().unwrap();
        logger = ch.0.clone();
    }
    let d = s.clone();
    logger.send(s).unwrap();
    DEBUG!(eprintln!("{}", d));
    // this seems to work to let the UI update the TextBox
    for _ in 1..200 {
        app::wait_for(0.000001).unwrap();
    }
}

/// run_server - run a webserver to serve requests from OpenHome media renderers
///
/// all music is sent in audio/l16 PCM format (i16) with the sample rate of the source
/// the samples are read from a crossbeam channel fed by the wave_reader
/// a ChannelStream is created for this purpose, and inserted in the array of active
/// "clients" for the wave_reader
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

/// capture_audio_output - capture the audio stream from the default audio output device
///
/// sets up an input stream for the wave_reader in the appropriate format (f32/i16/u16)
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
                    capture_err_fn,
                )
                .expect("Could not capture f32 stream format");
            s
        }
        cpal::SampleFormat::I16 => {
            let s = audio_output_device
                .build_input_stream(
                    &audio_cfg.config(),
                    move |data, _: &_| wave_reader::<i16>(data),
                    capture_err_fn,
                )
                .expect("Could not capture i16 stream format");
            s
        }
        cpal::SampleFormat::U16 => {
            let s = audio_output_device
                .build_input_stream(
                    &audio_cfg.config(),
                    move |data, _: &_| wave_reader::<u16>(data),
                    capture_err_fn,
                )
                .expect("Could not capture u16 stream format");
            s
        }
    };
    stream
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

/// _indent - indent the xml parser debug output
fn _indent(size: usize) -> String {
    const INDENT: &'static str = "    ";
    (0..size)
        .map(|_| INDENT)
        .fold(String::with_capacity(size * INDENT.len()), |r, s| r + s)
}

/// get_audio_device - return the default output audio device
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

/// get_local_address - get the local ip address, return an `Option<String>`. when it fails, return `None`.
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
