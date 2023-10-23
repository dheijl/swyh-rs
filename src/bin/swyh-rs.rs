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
use swyh_rs::{
    enums::streaming::StreamingState,
    globals::statics::{APP_NAME, APP_VERSION, CLIENTS, CONFIG, LOGCHANNEL},
    openhome::rendercontrol::{discover, Renderer, StreamInfo, WavData},
    server::streaming_server::{run_server, StreamerFeedBack},
    ui::mainform::MainForm,
    utils::{
        audiodevices::{
            capture_output_audio, get_default_audio_output_device, get_output_audio_devices,
        },
        bincommon::run_silence_injector,
        local_ip_address::*,
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
use log::{debug, info, LevelFilter};
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, WriteLogger};
use std::{
    cell::Cell, collections::HashMap, fs::File, net::IpAddr, path::Path, rc::Rc, thread,
    time::Duration,
};

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
        let mut conf = CONFIG.write();
        if conf.sound_source == "None" {
            conf.sound_source = audio_output_device.name().into();
            let _ = conf.update_config();
        }
        conf.clone()
    };
    if let Some(config_id) = &config.config_id {
        if !config_id.is_empty() {
            ui_log(format!("Loaded configuration -c {config_id}"));
        }
    }
    ui_log(format!("{config:?}"));
    if cfg!(debug_assertions) {
        config.log_level = LevelFilter::Debug;
    }

    let config_changed: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    // configure simplelogger
    let loglevel = config.log_level;
    let config_id = config.config_id.clone().unwrap();
    let logfilename = "log{}.txt".replace("{}", &config_id);
    let logfile = Path::new(&config.log_dir()).join(logfilename);
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
        ui_log("*W*W*>Running DEBUG build => log level set to DEBUG!".to_string());
    }
    info!("Config: {:?}", config);

    // get the output device from the config and get all available audio source names
    let audio_devices = get_output_audio_devices();
    let mut source_names: Vec<String> = Vec::new();
    for (index, adev) in audio_devices.into_iter().enumerate() {
        let devname = adev.name().to_owned();
        if config.sound_source_index.is_none() {
            if devname == config.sound_source {
                audio_output_device = adev;
                info!("Selected audio source: {}", devname);
            }
        } else if devname == config.sound_source
            && config.sound_source_index.unwrap() == index as i32
        {
            audio_output_device = adev;
            info!("Selected audio source: {}[#{}]", devname, index);
        }
        source_names.push(devname);
    }

    // get the default network that connects to the internet
    let local_addr: IpAddr = {
        if config.last_network == "None" {
            let addr = get_local_addr().expect("Could not obtain local address.");
            let mut conf = CONFIG.write();
            conf.last_network = addr.to_string();
            let _ = conf.update_config();
            addr
        } else {
            config.last_network.parse().unwrap()
        }
    };

    // get the list of available networks
    let networks = get_interfaces();

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
        config_changed.clone(),
        &source_names,
        &networks,
        local_addr,
        &wd,
        APP_VERSION.to_string(),
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
            ui_log("*E*E*> Could not capture audio ...Please check configuration.".to_string());
        }
    }

    // If silence injector is on, create a silence injector stream.
    let _silence_stream = if let Some(true) = CONFIG.read().inject_silence {
        ui_log("Injecting silence into the output stream".to_owned());
        Some(run_silence_injector(&audio_output_device))
    } else {
        None
    };

    // now start the SSDP discovery update thread with a Crossbeam channel for renderer updates
    // the discovered renderers will be kept in this list
    ui_log("Discover networks".to_string());
    let mut renderers: Vec<Renderer> = Vec::new();
    let (ssdp_tx, ssdp_rx): (Sender<Renderer>, Receiver<Renderer>) = unbounded();
    ui_log("Starting SSDP discovery".to_string());
    let ssdp_int = config.ssdp_interval_mins;
    let _ = thread::Builder::new()
        .name("ssdp_updater".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(move || run_ssdp_updater(ssdp_tx, ssdp_int))
        .unwrap();

    // also start the "monitor_rms" thread
    let rms_receiver = rms_channel.1;
    let mon_l = mf.rms_mon_l.clone();
    let mon_r = mf.rms_mon_r.clone();
    let _ = thread::Builder::new()
        .name("rms_monitor".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(move || run_rms_monitor(&wd.clone(), rms_receiver, mon_l, mon_r))
        .unwrap();

    // finally start a webserver on the local address,
    // with a Crossbeam feedback channel for connection accept/drop
    let (feedback_tx, feedback_rx): (Sender<StreamerFeedBack>, Receiver<StreamerFeedBack>) =
        unbounded();
    let server_port = config.server_port;
    let _ = thread::Builder::new()
        .name("swyh_rs_webserver".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(move || {
            run_server(
                &local_addr,
                server_port.unwrap_or_default(),
                wd,
                feedback_tx,
            )
        })
        .unwrap();
    // give the webserver a chance to start
    thread::yield_now();

    // get the logreader channel
    let logreader = &LOGCHANNEL.read().1;

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
        // check if the streaming webserver has closed a connection not caused by
        // pushing a renderer button
        // in that case we turn the button off as a visual feedback for the user
        // but if auto_resume is set, we restart playing instead
        while let Ok(streamer_feedback) = feedback_rx.try_recv() {
            if let Some(button) = mf.buttons.get_mut(&streamer_feedback.remote_ip) {
                match streamer_feedback.streaming_state {
                    StreamingState::Started => {
                        if !button.is_set() {
                            button.set(true);
                        }
                    }
                    StreamingState::Ended => {
                        // first check if the renderer has actually not started streaming again
                        // as this can happen with Bubble/Nest Audio Openhome
                        let still_streaming = CLIENTS
                            .read()
                            .values()
                            .any(|chanstrm| chanstrm.remote_ip == streamer_feedback.remote_ip);
                        if !still_streaming {
                            if mf.auto_resume.is_set() && button.is_set() {
                                if let Some(r) = renderers
                                    .iter()
                                    .find(|r| r.remote_addr == streamer_feedback.remote_ip)
                                {
                                    let config = CONFIG.read().clone();
                                    let streaminfo = StreamInfo {
                                        sample_rate: wd.sample_rate.0,
                                        bits_per_sample: config.bits_per_sample.unwrap(),
                                        streaming_format: config.streaming_format.unwrap(),
                                    };
                                    let _ = r.play(
                                        &local_addr,
                                        server_port.unwrap_or_default(),
                                        &dummy_log,
                                        &streaminfo,
                                    );
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
            mf.add_renderer_button(&newr);
            renderers.push(newr.clone());
        }
        // check the logchannel for new log messages to show in the logger textbox
        while let Ok(msg) = logreader.try_recv() {
            mf.add_log_msg(msg);
        }
    } // while app::wait()
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

/// a dummy_log is used during AV transport autoresume
fn dummy_log(s: String) {
    debug!("Autoresume: {}", s);
}

/// run the ssdp_updater - thread that periodically run ssdp discovery
/// and detect new renderers
/// send any new renderers to te main thread on the Crossbeam ssdp channel
fn run_ssdp_updater(ssdp_tx: Sender<Renderer>, ssdp_interval_mins: f64) {
    // the hashmap used to detect new renderers
    let mut rmap: HashMap<String, Renderer> = HashMap::new();
    loop {
        let renderers = discover(&rmap, &ui_log).unwrap_or_default();
        for r in renderers.iter() {
            if !rmap.contains_key(&r.remote_addr) {
                let _ = ssdp_tx.send(r.clone());
                app::awake();
                thread::yield_now();
                info!(
                    "Found new renderer {} {}  at {}",
                    r.dev_name, r.dev_model, r.remote_addr
                );
                rmap.insert(r.remote_addr.clone(), r.clone());
            }
        }
        thread::sleep(Duration::from_millis(
            (ssdp_interval_mins * 60.0 * 1000.0) as u64,
        ));
    }
}

fn run_rms_monitor(
    wd: &WavData,
    rms_receiver: Receiver<Vec<f32>>,
    mut rms_frame_l: Progress,
    mut rms_frame_r: Progress,
) {
    // compute # of samples needed to get a 10 Hz refresh rate
    let samples_per_update = ((wd.sample_rate.0 * wd.channels as u32) / 10) as i64;
    let mut total_samples = 0i64;
    let mut sum_l = 0f64;
    let mut sum_r = 0f64;
    while let Ok(samples) = rms_receiver.recv() {
        total_samples += samples.len() as i64;
        sum_l += samples
            .iter()
            .step_by(2)
            .map(|s| {
                let v = i16::from_sample(*s) as f64;
                v * v
            })
            .sum::<f64>();
        // / nsamples) as f64).sqrt();
        sum_r += samples
            .iter()
            .skip(1)
            .step_by(2)
            .map(|s| {
                let v = i16::from_sample(*s) as f64;
                v * v
            })
            .sum::<f64>();
        // / nsamples) as f64).sqrt();
        if total_samples >= samples_per_update {
            let rms_l = (sum_l / total_samples as f64).sqrt();
            let rms_r = (sum_r / total_samples as f64).sqrt();
            total_samples = 0;
            sum_l = 0.0;
            sum_r = 0.0;
            rms_frame_l.set_value(rms_l);
            rms_frame_r.set_value(rms_r);
            app::awake();
        }
    }
}
