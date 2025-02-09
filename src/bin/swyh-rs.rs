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
    enums::{
        messages::MessageType,
        streaming::{StreamingFormat::Flac, StreamingState},
    },
    globals::statics::{
        get_clients, get_config, get_config_mut, get_msgchannel, APP_VERSION, SERVER_PORT,
    },
    openhome::rendercontrol::{discover, Renderer, StreamInfo, WavData},
    server::streaming_server::run_server,
    ui::mainform::MainForm,
    utils::{
        audiodevices::{
            capture_output_audio, get_default_audio_output_device, get_output_audio_devices,
        },
        bincommon::run_silence_injector,
        local_ip_address::{get_interfaces, get_local_addr},
        priority::raise_priority,
        ui_logger::ui_log,
    },
};

use cpal::{traits::StreamTrait, Sample};
use crossbeam_channel::{unbounded, Receiver, Sender};
use fltk::{
    app, dialog,
    misc::Progress,
    prelude::{ButtonExt, WidgetExt},
};
use hashbrown::HashMap;
use log::{debug, info, LevelFilter};
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, WriteLogger};
use std::{cell::Cell, fs::File, net::IpAddr, path::Path, rc::Rc, thread, time::Duration};

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
    // disable TermLogger on susbsystem Windows because it panics now with Rust edition 2021
    if cfg!(debug_assertions) || cfg!(target_os = "linux") {
        let _ = CombinedLogger::init(vec![
            TermLogger::new(
                loglevel,
                Config::default(),
                simplelog::TerminalMode::Stderr,
                ColorChoice::Auto,
            ),
            WriteLogger::new(loglevel, Config::default(), File::create(logfile).unwrap()),
        ]);
    } else {
        let _ = CombinedLogger::init(vec![WriteLogger::new(
            loglevel,
            Config::default(),
            File::create(logfile).unwrap(),
        )]);
    }
    info!(
        "{} V {} - Running on {}, {}, {} - Logging started.",
        APP_NAME,
        APP_VERSION,
        std::env::consts::ARCH,
        std::env::consts::FAMILY,
        std::env::consts::OS
    );
    if cfg!(debug_assertions) {
        ui_log("*W*W*>Running DEBUG build => log level set to DEBUG!");
    }

    if let Some(config_id) = &config.config_id {
        if !config_id.is_empty() {
            ui_log(&format!("Loaded configuration -c {config_id}"));
        }
    }
    ui_log(&format!("{config:?}"));

    info!("Config: {:?}", config);

    // get the output device from the config and get all available audio source names
    let audio_devices = get_output_audio_devices();
    let mut source_names: Vec<String> = Vec::new();
    let config_name = config.sound_source.as_ref().unwrap();
    for (index, adev) in audio_devices.into_iter().enumerate() {
        let adevname = adev.name().to_string();
        if let Some(config_id) = config.sound_source_index {
            // index is needed for duplicate audio device names in Windows
            if config_id == index as i32 && adevname == *config_name {
                audio_output_device = adev;
                info!("Selected audio source: {}[#{}]", adevname, index);
            }
        } else if adevname == *config_name {
            audio_output_device = adev;
            info!("Selected audio source: {}", adevname);
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
    let mut mf = MainForm::create(
        &config,
        &config_changed,
        &source_names,
        &networks,
        local_addr,
        &wd,
        APP_VERSION,
    );

    // raise process priority a bit to prevent audio stuttering under cpu load
    raise_priority();

    // the rms monitor channel
    let rms_channel: (Sender<Vec<f32>>, Receiver<Vec<f32>>) = unbounded();

    // capture system audio
    debug!("Try capturing system audio");
    let stream: cpal::Stream;
    match capture_output_audio(&audio_output_device, rms_channel.0) {
        Some(s) => {
            stream = s;
            stream.play().unwrap();
        }
        None => {
            ui_log("*E*E*> Could not capture audio ...Please check configuration.");
        }
    }

    // If silence injector is on, create a silence injector stream.
    let _silence_stream = if let Some(true) = get_config().inject_silence {
        ui_log("Injecting silence into the output stream");
        Some(run_silence_injector(&audio_output_device))
    } else {
        None
    };

    // get the message channel
    let msg_tx = get_msgchannel().0.clone();
    let msg_rx = get_msgchannel().1.clone();

    // now start the SSDP discovery update thread with a Crossbeam channel for renderer updates
    // the discovered renderers will be kept in this list
    let mut renderers: Vec<Renderer> = Vec::new();
    if config.ssdp_interval_mins > 0.0 {
        ui_log("Starting SSDP discovery");
        let ssdp_int = config.ssdp_interval_mins;
        let ssdp_tx = msg_tx.clone();
        let _ = thread::Builder::new()
            .name("ssdp_updater".into())
            .stack_size(4 * 1024 * 1024)
            .spawn(move || run_ssdp_updater(&ssdp_tx, ssdp_int))
            .unwrap();
    } else {
        ui_log("SSDP interval 0 => Skipping SSDP discovery");
    }
    // also start the "monitor_rms" thread
    let rms_receiver = rms_channel.1;
    let mon_l = mf.rms_mon_l.clone();
    let mon_r = mf.rms_mon_r.clone();
    let _ = thread::Builder::new()
        .name("rms_monitor".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(move || {
            run_rms_monitor(wd, &rms_receiver, mon_l, mon_r);
        })
        .unwrap();

    // finally start a webserver on the local address,
    let server_port = config.server_port.unwrap_or(SERVER_PORT);
    let feedback_tx = msg_tx.clone();
    let _ = thread::Builder::new()
        .name("swyh_rs_webserver".into())
        .stack_size(4 * 1024 * 1024)
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
        if config_changed.get() && app_restart(&mf) != 0 {
            config_changed.set(false);
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
                    let same_ip: Vec<&Renderer> = renderers
                        .iter()
                        .filter(|r| r.remote_addr == streamer_feedback.remote_ip)
                        .collect();
                    if same_ip.len() == 1 {
                        // got the only renderer with this IP address
                        let renderer = same_ip[0];
                        // get the button associated with this renderer
                        if let Some(button) = mf.buttons.get_mut(&renderer.location) {
                            match streamer_feedback.streaming_state {
                                StreamingState::Started => {
                                    if !button.is_set() {
                                        button.set(true);
                                    }
                                }
                                StreamingState::Ended => {
                                    // first check if the renderer has actually not started streaming again
                                    // as this can happen with Bubble/Nest Audio Openhome
                                    let still_streaming = get_clients().values().any(|chanstrm| {
                                        chanstrm.remote_ip == streamer_feedback.remote_ip
                                    });
                                    if !still_streaming {
                                        if mf.auto_resume.is_set() && button.is_set() {
                                            if let Some(r) = renderers.iter().find(|r| {
                                                r.remote_addr == streamer_feedback.remote_ip
                                            }) {
                                                let config = get_config().clone();
                                                let streaminfo = StreamInfo {
                                                    sample_rate: wd.sample_rate.0,
                                                    bits_per_sample: config
                                                        .bits_per_sample
                                                        .unwrap_or(16),
                                                    streaming_format: config
                                                        .streaming_format
                                                        .unwrap_or(Flac),
                                                };
                                                let _ = r.play(
                                                    &local_addr,
                                                    server_port,
                                                    &ui_log,
                                                    streaminfo,
                                                );
                                            }
                                        } else if button.is_set() {
                                            button.set(false);
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        // we have multiple renderers at this IP address, no correlation to a button
                        // so there's nothing we can do here...
                        // except perhaps inquire each player with same_ip for the current transport state ?
                    }
                }
                // check the ssdp discovery thread channel for newly discovered renderers
                // add a new button below the last one for each discovered renderer
                MessageType::SsdpMessage(mut newr) => {
                    let vol = newr.get_volume(&ui_log);
                    debug!("Renderer {} Volume: {vol}", newr.dev_name);
                    mf.add_renderer_button(&newr);
                    renderers.push(newr.clone());
                }
                // check the logchannel for new log messages to show in the logger textbox
                MessageType::LogMessage(msg) => {
                    mf.add_log_msg(&msg);
                }
            }
        }
    } // while app::wait()

    // if anyone is still streaming: stop them first
    let mut active_players: Vec<String> = Vec::new();
    for button in &mf.buttons {
        if button.1.is_set() {
            if let Some(r) = renderers.iter().find(|r| r.location == *button.0) {
                active_players.push(r.remote_addr.clone());
                r.stop_play(&ui_log);
            }
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
        if get_clients().len() == 0 {
            info!("No active HTTP streaming connections - exiting.");
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    if get_clients().len() > 0 {
        info!("Time-out waiting for HTTP streaming shutdown - exiting.");
    }
}

fn app_restart(mf: &MainForm) -> i32 {
    let c = dialog::choice2(
        mf.wind.width() / 2 - 100,
        mf.wind.height() / 2 - 50,
        "Configuration value changed!",
        "Restart",
        "Cancel",
        "",
    );
    if c == Some(0) {
        // restart
        std::process::Command::new(std::env::current_exe().unwrap().into_os_string())
            .spawn()
            .expect("Unable to spawn myself!");
        std::process::exit(0);
    } else {
        // cancel
        1
    }
}

/// run the `ssdp_updater` - thread that periodically run ssdp discovery
/// and detect new renderers
/// send any new renderers to te main thread on the Crossbeam ssdp channel
fn run_ssdp_updater(ssdp_tx: &Sender<MessageType>, ssdp_interval_mins: f64) {
    // the hashmap used to detect new renderers
    let mut rmap: HashMap<String, Renderer> = HashMap::new();
    loop {
        let renderers = discover(&rmap, &ui_log).unwrap_or_default();
        for r in &renderers {
            rmap.entry(r.location.clone()).or_insert_with(|| {
                info!(
                    "Found new renderer {} {}  at {}",
                    r.dev_name, r.dev_model, r.remote_addr
                );
                ssdp_tx.send(MessageType::SsdpMessage(r.clone())).unwrap();
                app::awake();
                thread::yield_now();
                r.clone()
            });
        }
        thread::sleep(Duration::from_millis(
            (ssdp_interval_mins * 60.0 * 1000.0) as u64,
        ));
    }
}

fn run_rms_monitor(
    wd: WavData,
    rms_receiver: &Receiver<Vec<f32>>,
    mut rms_frame_l: Progress,
    mut rms_frame_r: Progress,
) {
    // compute # of samples needed to get a 10 Hz refresh rate
    let samples_per_update = ((wd.sample_rate.0 * u32::from(wd.channels)) / 10) as usize;
    let mut total_samples = 0usize;
    let mut sum_l = 0f64;
    let mut sum_r = 0f64;
    while let Ok(samples) = rms_receiver.recv() {
        total_samples += samples.len();
        // sum left channel samples
        sum_l = samples.iter().step_by(2).fold(sum_l, |acc, x| {
            let v = f64::from(i16::from_sample(*x));
            acc + (v * v)
        });
        // sum right channel samples
        sum_r = samples.iter().skip(1).step_by(2).fold(sum_r, |acc, x| {
            let v = f64::from(i16::from_sample(*x));
            acc + (v * v)
        });
        // compute and show current RMS values if enough samples collected
        if total_samples >= samples_per_update {
            let samples_per_channel = (total_samples / wd.channels as usize) as f64;
            let rms_l = (sum_l / samples_per_channel).sqrt();
            let rms_r = (sum_r / samples_per_channel).sqrt();
            total_samples = 0;
            sum_l = 0.0;
            sum_r = 0.0;
            rms_frame_l.set_value(rms_l);
            rms_frame_r.set_value(rms_r);
            app::awake();
        }
    }
}
