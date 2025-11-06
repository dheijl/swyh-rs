#![cfg(feature = "gui")]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // to suppress console with debug output for release builds
///
/// swyh-rs
///
/// Basic SWYH (<https://www.streamwhatyouhear.com>, source repo <https://github.com/StreamWhatYouHear/SWYH)> clone entirely written in rust.
///
/// I wrote this because I a) wanted to learn Rust and b) SWYH did not work on Linux and did not work well with Volumio (push streaming does not work).
///
/// For the moment all music is streamed in wav-format (audio/l16) with the sample rate of the music source (the default audio device, I use `HiFi` Cable Input).
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
use swyh_rs::{
    enums::{messages::MessageType, streaming::StreamingState},
    globals::statics::{
        APP_DATE, APP_VERSION, ONE_MINUTE, SERVER_PORT, THREAD_STACK, get_clients, get_config_mut,
        get_msgchannel, get_renderers, get_renderers_mut,
    },
    openhome::rendercontrol::{Renderer, StreamInfo, WavData, discover},
    server::streaming_server::run_server,
    ui::mainform::MainForm,
    utils::{
        audiodevices::{
            capture_output_audio, get_default_audio_output_device, get_output_audio_devices,
        },
        bincommon::run_silence_injector,
        local_ip_address::{get_interfaces, get_local_addr},
        priority::raise_priority,
        ui_logger::*,
    },
};

use cpal::traits::StreamTrait;
use crossbeam_channel::{Receiver, Sender, unbounded};
use fltk::{app, misc::Progress, prelude::ButtonExt};
use hashbrown::HashMap;
use log::{LevelFilter, debug, info};
#[cfg(any(debug_assertions, target_os = "linux"))]
use simplelog::{ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, WriteLogger};
#[cfg(not(any(debug_assertions, target_os = "linux")))]
use simplelog::{CombinedLogger, ConfigBuilder, WriteLogger};
use std::{
    cell::Cell,
    fs::File,
    net::IpAddr,
    path::Path,
    rc::Rc,
    thread::{self},
    time::Duration,
};

pub const APP_NAME: &str = "SWYH-RS";

/// swyh-rs
///
/// - set up the fltk GUI
/// - setup and start audio capture
/// - start the streaming webserver
/// - start ssdp discovery of media renderers thread
/// - run the GUI, and show any renderers found in the GUI as buttons (to start/stop playing)
fn main() {
    // first initialize cpal audio to prevent COM reinitialize panic on Windows
    let mut audio_output_device =
        get_default_audio_output_device().expect("No default audio device");

    // initialize config
    let mut config = {
        let mut conf = get_config_mut();
        if conf.sound_source.is_none() {
            conf.sound_source = Some(audio_output_device.name().into());
            let _ = conf.update_config();
        }
        conf.clone()
    };

    let config_changed: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    // configure simplelogger
    let config_id = config.config_id.clone().unwrap();
    let logfilename = "log{}.txt".replace("{}", &config_id);
    let logfile = Path::new(&config.log_dir()).join(logfilename);
    if cfg!(debug_assertions) {
        config.log_level = LevelFilter::Debug;
    }
    let loglevel = config.log_level;
    let log_config = ConfigBuilder::new()
        .set_time_format_rfc2822()
        .set_time_offset_to_local()
        .unwrap()
        .build();
    // disable TermLogger on susbsystem Windows because it panics now with Rust edition 2021
    #[cfg(any(debug_assertions, target_os = "linux"))]
    let _ = CombinedLogger::init(vec![
        TermLogger::new(
            loglevel,
            log_config.clone(),
            simplelog::TerminalMode::Stderr,
            ColorChoice::Auto,
        ),
        WriteLogger::new(loglevel, log_config.clone(), File::create(logfile).unwrap()),
    ]);
    #[cfg(not(any(debug_assertions, target_os = "linux")))]
    let _ = CombinedLogger::init(vec![WriteLogger::new(
        loglevel,
        log_config.clone(),
        File::create(logfile).unwrap(),
    )]);

    info!(
        "{} V {}(build: {}) - Running on {}, {}, {} - Logging started.",
        APP_NAME,
        APP_VERSION,
        APP_DATE.unwrap_or("beta"),
        std::env::consts::ARCH,
        std::env::consts::FAMILY,
        std::env::consts::OS
    );
    #[cfg(debug_assertions)]
    ui_log(
        LogCategory::Warning,
        "Running DEBUG build => log level set to DEBUG!",
    );

    if let Some(config_id) = &config.config_id
        && !config_id.is_empty()
    {
        ui_log(
            LogCategory::Info,
            &format!("Loaded configuration -c {config_id}"),
        );
    }
    ui_log(LogCategory::Info, &format!("{config:?}"));

    info!("Config: {config:?}");

    // get the output device from the config and get all available audio source names
    let audio_devices = get_output_audio_devices();
    let mut source_names: Vec<String> = Vec::with_capacity(audio_devices.len());
    let config_name = config.sound_source.as_ref().unwrap();
    for (index, adev) in audio_devices.into_iter().enumerate() {
        let adevname = adev.name().to_string();
        if let Some(config_id) = config.sound_source_index {
            // index is needed for duplicate audio device names in Windows
            if config_id == index as i32 && adevname == *config_name {
                audio_output_device = adev;
                info!("Selected audio source: {adevname}[#{index}]");
            }
        } else if adevname == *config_name {
            audio_output_device = adev;
            info!("Selected audio source: {adevname}");
        }
        source_names.push(adevname);
    }

    // get the list of available networks
    let networks = get_interfaces();

    // get the default network that connects to the internet
    let local_addr: IpAddr = {
        fn get_default_address() -> IpAddr {
            let addr = get_local_addr().expect("Could not obtain local address.");
            let mut conf = get_config_mut();
            conf.last_network = Some(addr.to_string());
            let _ = conf.update_config();
            addr
        }
        if let Some(ref net) = config.last_network {
            let mut nw = net.parse().unwrap();
            if !networks.contains(net) {
                nw = get_default_address();
            }
            nw
        } else {
            get_default_address()
        }
    };

    // we need to pass some audio config data to the play function
    let audio_cfg = audio_output_device.default_config();
    let wd = WavData {
        sample_format: audio_cfg.sample_format(),
        sample_rate: audio_cfg.sample_rate(),
        channels: audio_cfg.channels(),
    };

    // we now have enough information to create the GUI with meaningful data
    let version_string = format!("{APP_VERSION}(build: {})", APP_DATE.unwrap_or("beta"));
    let mut mf = MainForm::create(
        &config,
        &config_changed,
        &source_names,
        &networks,
        local_addr,
        &wd,
        &version_string,
    );

    // raise process priority a bit to prevent audio stuttering under cpu load
    raise_priority();

    // the rms monitor channel
    let rms_channel: (Sender<Vec<f32>>, Receiver<Vec<f32>>) = unbounded();

    // capture system audio
    debug!("Try capturing system audio");
    let mut stream: cpal::Stream;
    let rms_chan1 = rms_channel.clone();
    match capture_output_audio(&audio_output_device, rms_chan1.0) {
        Some(s) => {
            stream = s;
            stream.play().unwrap();
        }
        _ => {
            ui_log(
                LogCategory::Error,
                "Could not capture audio ...Please check configuration.",
            );
        }
    }

    // If silence injector is on, create a silence injector stream and keep it alive
    let _silence_stream = {
        if let Some(true) = config.inject_silence {
            if let Some(stream) = run_silence_injector(&audio_output_device) {
                ui_log(
                    LogCategory::Info,
                    "Injecting silence into the output stream",
                );
                Some(stream)
            } else {
                ui_log(LogCategory::Error, "Unable to inject silence !!");
                None
            }
        } else {
            None
        }
    };

    // get the message channel
    let msg_tx = get_msgchannel().0.clone();
    let msg_rx = get_msgchannel().1.clone();

    // now start the SSDP discovery update thread with a Crossbeam channel for renderer updates
    if config.ssdp_interval_mins > 0.0 {
        ui_log(LogCategory::Info, "Starting SSDP discovery");
        let ssdp_int = config.ssdp_interval_mins;
        let ssdp_tx = msg_tx.clone();
        let _ = thread::Builder::new()
            .name("ssdp_updater".into())
            .stack_size(THREAD_STACK)
            .spawn(move || run_ssdp_updater(&ssdp_tx, ssdp_int))
            .unwrap();
    } else {
        ui_log(
            LogCategory::Info,
            "SSDP interval 0 => Skipping SSDP discovery",
        );
    }
    // also start the "monitor_rms" thread
    let rms_chan2 = rms_channel.clone();
    let rms_receiver = rms_chan2.1;
    let mon_l = mf.rms_mon_l.clone();
    let mon_r = mf.rms_mon_r.clone();
    let _ = thread::Builder::new()
        .name("rms_monitor".into())
        .stack_size(THREAD_STACK)
        .spawn(move || {
            run_rms_monitor(wd, &rms_receiver, mon_l, mon_r);
        })
        .unwrap();

    // finally start a webserver on the local address,
    let server_port = config.server_port.unwrap_or(SERVER_PORT);
    let feedback_tx = msg_tx.clone();
    let _ = thread::Builder::new()
        .name("swyh_rs_webserver".into())
        .stack_size(THREAD_STACK)
        .spawn(move || {
            run_server(&local_addr, server_port, wd, &feedback_tx);
        })
        .unwrap();
    // give the webserver a chance to start
    thread::yield_now();

    // and now we can run the GUI event loop, app::awake() is used by the various threads to
    // trigger updates when something has changed, some threads use Crossbeam channels
    // to signal what has changed
    while app::wait() {
        if app::should_program_quit() {
            break;
        }
        // test for a configuration change that needs an app restart to take effect
        if config_changed.get() {
            mf.show_restart_button();
        }
        // handle the messages from other threads
        while let Ok(msg) = msg_rx.try_recv() {
            match msg {
                // check if the streaming webserver has closed a connection not caused by
                // pushing a renderer button
                // in that case we turn the button off as a visual feedback for the user
                // but if auto_resume is set, we restart playing instead
                MessageType::PlayerMessage(streamer_feedback) => {
                    // check for multiple renderers at same ip address (Bubble UPNP)
                    let mut same_ip: Vec<Renderer> = get_renderers()
                        .clone()
                        .into_iter()
                        .filter(|r| r.remote_addr == streamer_feedback.remote_ip)
                        .collect();
                    // the following only works for players with a unique IP address
                    if same_ip.len() == 1 {
                        // we have only one renderer with this IP address
                        let renderer = &mut same_ip[0];
                        // get the button associated with this renderer
                        if let Some(mut button) = renderer.rend_ui.button.clone() {
                            match streamer_feedback.streaming_state {
                                StreamingState::Started => {
                                    update_playstate(&streamer_feedback.remote_ip, true);
                                    button.set(true);
                                }
                                StreamingState::Ended => {
                                    // first check if the renderer has actually not started streaming again
                                    // as this can happen with Bubble/Nest Audio Openhome
                                    let still_streaming = get_clients().values().any(|chanstrm| {
                                        chanstrm.remote_ip == streamer_feedback.remote_ip
                                    });
                                    if still_streaming {
                                        // still streaming, this is possible with Bubble/Nest
                                        button.set(true);
                                        update_playstate(&streamer_feedback.remote_ip, true);
                                    } else {
                                        // streaming has really ended
                                        update_playstate(&streamer_feedback.remote_ip, false);
                                        if mf.auto_resume.is_set() && button.is_set() {
                                            let streaminfo = StreamInfo::new(wd.sample_rate.0);
                                            let _ = renderer.play(&local_addr, streaminfo);
                                            update_playstate(&streamer_feedback.remote_ip, true);
                                        } else {
                                            button.set(false);
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        // we have multiple renderers at this IP address, so no correlation to a button
                        // so there's nothing we can do here...
                        // except perhaps inquire each player with same_ip for the current transport state ?
                    }
                }
                // check the ssdp discovery thread channel for newly discovered renderers
                // add a new button below the last one for each discovered renderer
                MessageType::SsdpMessage(mut newr) => {
                    let vol = newr.get_volume();
                    debug!("Renderer {} Volume: {vol}", newr.dev_name);
                    // add a button for the new player
                    mf.add_renderer_button(&mut newr);
                }
                // check the logchannel for new log messages to show in the logger textbox
                MessageType::LogMessage(msg) => {
                    mf.add_log_msg(&msg);
                }
                MessageType::CaptureAborted() => {
                    // retry count when audio capture is broken
                    let mut capture_retry_count = 0i32;
                    while capture_retry_count <= 5 {
                        thread::sleep(Duration::from_millis(250));
                        capture_retry_count += 1;
                        debug!("Retrying capturing audio #{capture_retry_count}");
                        let audio_devices = get_output_audio_devices();
                        let config_name: &String = config.sound_source.as_ref().unwrap();
                        // ignore sound index as it may have changed, so duplicate names won't probably work
                        let mut found_audio_device = false;
                        for adev in audio_devices.into_iter() {
                            let adevname = adev.name().to_string();
                            if adevname == *config_name {
                                audio_output_device = adev;
                                info!("Audio capture: reselecting audio source: {adevname}");
                                found_audio_device = true;
                                break;
                            }
                        }
                        if found_audio_device {
                            let rms_chan3 = rms_channel.clone();
                            if let Some(s) = capture_output_audio(&audio_output_device, rms_chan3.0)
                            {
                                stream = s;
                                stream.play().unwrap();
                                info!("Audio capture resumed.");
                                break;
                            }
                        }
                    }
                }
            }
        }
    } // while app::wait()

    // if anyone is still streaming: stop them first
    let mut active_players: Vec<String> = Vec::new();
    let renderers = get_renderers_mut().clone();
    for mut renderer in renderers {
        if let Some(button) = renderer.rend_ui.button.as_ref()
            && button.is_set()
        {
            ui_log(
                LogCategory::Info,
                &format!("Shutting down {}", &renderer.dev_name),
            );
            app::redraw();
            active_players.push(renderer.remote_addr.clone());
            renderer.stop_play();
            app::redraw();
        }
    }
    // remember active players in config for auto_reconnect
    {
        let mut config = get_config_mut();
        config.active_renderers = active_players;
        let _ = config.update_config();
    }
    // and now wait some time for them to stop the HTTP streaming connection too
    for _ in 0..50 {
        if get_clients().is_empty() {
            info!("No active HTTP streaming connections - exiting.");
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    if !get_clients().is_empty() {
        info!("Time-out waiting for HTTP streaming shutdown - exiting.");
    }
    log::logger().flush();
}

/// update the playstate for the renderer with this ip address
fn update_playstate(remote_addr: &str, playing: bool) {
    get_renderers_mut()
        .iter_mut()
        .find(|r| r.remote_addr == remote_addr)
        .unwrap_or_else(|| {
            panic!("Global Renderers list unconsistent with local Renderers for {remote_addr}")
        })
        .playing = playing;
}

/// run the `ssdp_updater` - thread that periodically run ssdp discovery
/// and detect new renderers
/// send any new renderers to te main thread on the Crossbeam ssdp channel
fn run_ssdp_updater(ssdp_tx: &Sender<MessageType>, ssdp_interval_mins: f64) {
    let agent = ureq::agent();
    // the hashmap used to detect new renderers
    let mut rmap: HashMap<String, Renderer> = HashMap::new();
    loop {
        let renderers = discover(&agent, &rmap).unwrap_or_default();
        for r in &renderers {
            rmap.entry(r.location.clone()).or_insert_with(|| {
                info!(
                    "Found new renderer {} {}  at {}",
                    r.dev_name, r.dev_model, r.remote_addr
                );
                ssdp_tx
                    .send(MessageType::SsdpMessage(Box::new(r.clone())))
                    .unwrap();
                app::awake();
                r.clone()
            });
        }
        thread::sleep(Duration::from_millis(
            (ssdp_interval_mins * ONE_MINUTE) as u64,
        ));
    }
}

/// compute the left and right channel RMS value for every 100 ms period
/// and show the values in the UI
fn run_rms_monitor(
    wd: WavData,
    rms_receiver: &Receiver<Vec<f32>>,
    mut rms_frame_l: Progress,
    mut rms_frame_r: Progress,
) {
    const I16_MAX: f32 = i16::MAX as f32;
    // compute # of samples needed to get a 10 Hz refresh rate, multiple of 4 samples
    let samples_per_update =
        (((wd.sample_rate.0 * u32::from(wd.channels)) / 10) as usize) & !3usize;
    let mut total_samples = 0usize;
    let mut ch_sum = (0f32, 0f32);
    while let Ok(samples) = rms_receiver.recv() {
        total_samples += samples.len();
        // sum left and right channel samples, 4 samples at a time (uses simd mulps)
        ch_sum = samples.chunks(4).fold(ch_sum, |acc, x| {
            let vl1 = x[0] * I16_MAX;
            let vr1 = x[1] * I16_MAX;
            let vl2 = x[2] * I16_MAX;
            let vr2 = x[3] * I16_MAX;
            (
                acc.0 + (vl1 * vl1) + (vl2 * vl2),
                acc.1 + (vr1 * vr1) + (vr2 * vr2),
            )
        });
        // compute and show current RMS values if enough samples collected
        if total_samples >= samples_per_update {
            let samples_per_channel = (total_samples / wd.channels as usize) as f32;
            let rms_l = f64::from((ch_sum.0 / samples_per_channel).sqrt());
            let rms_r = f64::from((ch_sum.1 / samples_per_channel).sqrt());
            total_samples = 0;
            ch_sum = (0.0, 0.0);
            rms_frame_l.set_value(rms_l);
            rms_frame_r.set_value(rms_r);
            app::awake();
        }
    }
}
