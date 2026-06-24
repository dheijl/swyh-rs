//! `swyh-rs` — GUI entry point.
//!
//! A Rust clone of SWYH (<https://www.streamwhatyouhear.com>): captures the default
//! audio output device and streams it in LPCM, WAV, RF64, or FLAC format to DLNA/OpenHome
//! renderers discovered via SSDP.  Tested on Windows 10/11 and Ubuntu/Debian with Volumio, MoOde,
//! Harman-Kardon AVR, Sonos etc... renderers.

#![cfg(feature = "gui")]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // to suppress console with debug output for release builds
use mimalloc::MiMalloc;
use swyh_rs::{
    audio::{
        audiodevices::{
            Device, capture_output_audio, get_default_audio_output_device, get_output_audio_devices,
        },
        rwstream::AudioSamples,
    },
    enums::{messages::MessageType, streaming::StreamingState},
    fl,
    globals::statics::{
        APP_DATE, APP_VERSION, SERVER_PORT, THREAD_STACK, get_clients, get_config_mut,
        get_msgchannel, get_renderers, get_renderers_mut,
    },
    renderers::rendercontrol::{Renderer, StreamInfo, WavData},
    server::streaming_server::{StreamerFeedBack, run_server},
    ui::{fatal_error::fatal_error, mainform::MainForm},
    utils::{
        bincommon::run_silence_injector,
        configuration::Configuration,
        extra_threads::{run_rms_monitor, run_ssdp_updater},
        i18n,
        local_ip_address::{get_interfaces, get_local_addr},
        priority::raise_priority,
        ui_logger::*,
    },
};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use cpal::{SampleFormat, SupportedStreamConfig, traits::StreamTrait};
use crossbeam_channel::{Receiver, Sender, unbounded};
use fltk::{app, misc::Progress, prelude::ButtonExt};
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
    let ad = get_default_audio_output_device();
    // initialize config
    let config = {
        let mut conf = get_config_mut();
        if conf.sound_source.is_none()
            && let Some(ref dev) = ad
        {
            conf.sound_source = Some(dev.name().into());
            let _ = conf.update_config();
        }
        conf.clone()
    };
    // initialize i18n before any user-facing string is produced
    i18n::init(&config.language.clone().unwrap_or("en-US".to_string()));
    // check for the default audio device
    let default_device = ad.unwrap_or_else(|| fatal_error(fl!("err-no-audio-device")));
    // if set: an app restart is required to apply the changes
    let config_changed: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    setup_logging(&config);

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
    ui_log(LogCategory::Warning, &fl!("debug-build-warning"));

    if let Some(config_id) = &config.config_id
        && !config_id.is_empty()
    {
        ui_log(
            LogCategory::Info,
            &fl!("status-loaded-config", "id" = config_id),
        );
    }
    ui_log(LogCategory::Info, &format!("{config:?}"));
    info!("Config: {config:?}");

    let (mut audio_output_device, source_names) = select_audio_source(&config, default_device);
    let networks = get_interfaces();
    let local_addr = resolve_local_addr(&config, &networks);
    let (audio_cfg, wd) = build_wav_data(&audio_output_device, &config);

    // we now have enough information to create the GUI with meaningful data
    let (ssdp_kick_tx, ssdp_kick_rx) = crossbeam_channel::unbounded::<()>();
    let mut mf = MainForm::create(
        &config,
        &config_changed,
        &source_names,
        &networks,
        local_addr,
        &wd,
        ssdp_kick_tx,
    );

    // raise process priority a bit to prevent audio stuttering under cpu load
    raise_priority();

    // the rms monitor channel
    let rms_channel = unbounded();

    // capture system audio
    debug!("Try capturing system audio");
    let mut stream: Option<cpal::Stream> = None;
    let rms_chan1 = rms_channel.clone();
    match capture_output_audio(&audio_output_device, &audio_cfg, rms_chan1.0) {
        Some(s) => {
            stream = Some(s);
        }
        _ => {
            ui_log(LogCategory::Error, &fl!("err-capture-audio"));
        }
    }
    if let Some(ref s) = stream
        && s.play().is_err()
    {
        ui_log(LogCategory::Error, &fl!("err-play-stream"));
    }

    // If silence injector is on, create a silence injector stream and keep it alive
    let _silence_stream = {
        if let Some(true) = config.inject_silence {
            if let Some(stream) = run_silence_injector(&audio_output_device) {
                ui_log(LogCategory::Info, &fl!("status-injecting-silence"));
                Some(stream)
            } else {
                ui_log(LogCategory::Error, &fl!("err-inject-silence"));
                None
            }
        } else {
            None
        }
    };

    // get the message channel
    let msg_tx = get_msgchannel().0.clone();
    let msg_rx = get_msgchannel().1.clone();

    // start the SSDP discovery update thread
    if config.ssdp_interval_mins > 0.0 {
        ui_log(LogCategory::Info, &fl!("status-starting-ssdp"));
        spawn_ssdp_updater(msg_tx.clone(), config.ssdp_interval_mins, ssdp_kick_rx);
    } else {
        ui_log(LogCategory::Info, &fl!("status-ssdp-interval-zero"));
    }

    // start the RMS monitor thread
    spawn_rms_monitor(
        wd,
        rms_channel.clone().1,
        mf.rms_mon_l.clone(),
        mf.rms_mon_r.clone(),
    );

    // start the streaming webserver
    spawn_webserver(
        local_addr,
        config.server_port.unwrap_or(SERVER_PORT),
        wd,
        msg_tx.clone(),
    );

    // give the webserver a chance to start
    thread::yield_now();

    // run the GUI event loop; app::awake() is used by threads to trigger UI updates
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
                // pushing a renderer button; turn the button off or auto-resume as needed
                MessageType::PlayerMessage(streamer_feedback) => {
                    handle_player_message(
                        streamer_feedback,
                        mf.auto_resume.is_set(),
                        wd,
                        &local_addr,
                    );
                }
                // add a new button for each renderer discovered by SSDP
                MessageType::SsdpMessage(mut newr) => {
                    let vol = newr.get_volume();
                    debug!("Renderer {} Volume: {vol}", newr.dev_name);
                    mf.add_renderer_button(&mut newr);
                }
                MessageType::LogMessage(msg) => {
                    mf.add_log_msg(&msg);
                }
                MessageType::CaptureAborted => {
                    let mut capture_retry_count = 0i32;
                    stream = None;
                    while capture_retry_count < 5 {
                        thread::sleep(Duration::from_millis(250));
                        capture_retry_count += 1;
                        debug!("Retrying capturing audio #{capture_retry_count}");
                        let audio_devices = get_output_audio_devices();
                        let config_name: &str = config.sound_source.as_ref().unwrap();
                        // ignore sound index as it may have changed, so duplicate names won't probably work
                        let mut found_audio_device = false;
                        for adev in audio_devices.into_iter() {
                            if adev.name() == config_name {
                                info!("Audio capture: reselecting audio source: {}", adev.name());
                                audio_output_device = adev;
                                found_audio_device = true;
                                break;
                            }
                        }
                        if found_audio_device {
                            let rms_chan3 = rms_channel.clone();
                            if let Some(s) =
                                capture_output_audio(&audio_output_device, &audio_cfg, rms_chan3.0)
                            {
                                stream = Some(s);
                                info!("Audio capture resumed.");
                                break;
                            }
                        }
                    }
                    if let Some(ref s) = stream
                        && s.play().is_err()
                    {
                        ui_log(LogCategory::Error, &fl!("err-play-stream"));
                    }
                }
            }
        }
    } // while app::wait()

    shutdown_and_exit();
}

/// update the playstate for the renderer with this ip address
fn update_playstate(remote_addr: &str, playing: bool) {
    if let Some(r) = get_renderers_mut()
        .iter_mut()
        .find(|r| r.remote_addr == remote_addr)
    {
        r.playing = playing;
    }
}

/// configure simplelog to write to file (and terminal in debug/Linux builds)
fn setup_logging(config: &Configuration) {
    let config_id = config.config_id.clone().unwrap_or_default();
    let logfilename = "log{}.txt".replace("{}", &config_id);
    let logfile = Path::new(&config.log_dir()).join(logfilename);
    let loglevel = if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        config.log_level
    };
    let mut log_config_builder = ConfigBuilder::new();
    log_config_builder.set_time_format_rfc2822();
    let _ = log_config_builder.set_time_offset_to_local(); // silently fall back to UTC on error
    let log_config = log_config_builder.build();
    // disable TermLogger on subsystem Windows because it panics with Rust edition 2021
    #[cfg(any(debug_assertions, target_os = "linux"))]
    let _ = CombinedLogger::init(vec![
        TermLogger::new(
            loglevel,
            log_config.clone(),
            simplelog::TerminalMode::Stderr,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            loglevel,
            log_config.clone(),
            File::create(&logfile).unwrap_or_else(|e| {
                eprintln!("Failed to create log file {}: {e}", logfile.display());
                std::process::exit(1);
            }),
        ),
    ]);
    #[cfg(not(any(debug_assertions, target_os = "linux")))]
    let _ = CombinedLogger::init(vec![WriteLogger::new(
        loglevel,
        log_config.clone(),
        File::create(&logfile).unwrap_or_else(|e| {
            eprintln!("Failed to create log file {}: {e}", logfile.display());
            std::process::exit(1);
        }),
    )]);
}

/// select the audio output device named in `config`, falling back to `default_device`;
/// also returns the names of all available output devices for the GUI selector
fn select_audio_source(config: &Configuration, default_device: Device) -> (Device, Vec<String>) {
    if config.sound_source.is_none() {
        fatal_error(fl!("err-no-sound-source"));
    }
    let audio_devices = get_output_audio_devices();
    let mut source_names: Vec<String> = Vec::with_capacity(audio_devices.len());
    let config_name = config.sound_source.as_ref().unwrap();
    let mut selected = default_device;
    for (index, adev) in audio_devices.into_iter().enumerate() {
        let adevname = adev.name().to_string();
        if let Some(config_id) = config.sound_source_index {
            // index is needed for duplicate audio device names in Windows
            if config_id == index as i32 && adevname == *config_name {
                info!("Selected audio source: {adevname}[#{index}]");
                selected = adev;
            }
        } else if adevname == *config_name {
            info!("Selected audio source: {adevname}");
            selected = adev;
        }
        source_names.push(adevname);
    }
    (selected, source_names)
}

/// resolve the local IP address to bind to, persisting it to config
fn resolve_local_addr(config: &Configuration, networks: &[String]) -> IpAddr {
    let get_default = || -> Option<IpAddr> {
        let addr = get_local_addr()?;
        let mut conf = get_config_mut();
        conf.last_network = Some(addr.to_string());
        let _ = conf.update_config();
        Some(addr)
    };
    if let Some(ref net) = config.last_network {
        let mut nw = net.parse().unwrap_or_else(|_| {
            get_default().unwrap_or_else(|| fatal_error(fl!("err-no-local-address")))
        });
        if !networks.contains(net) {
            nw = get_default().unwrap_or_else(|| fatal_error(fl!("err-no-local-address")));
        }
        nw
    } else {
        get_default().unwrap_or_else(|| fatal_error(fl!("err-no-local-address")))
    }
}

/// determine the stream config and build the `WavData` descriptor
fn build_wav_data(device: &Device, config: &Configuration) -> (SupportedStreamConfig, WavData) {
    let default_rate = device.default_config().sample_rate();
    let audio_cfg = if let Some(rate) = config.sample_rate {
        device
            .find_config(rate, SampleFormat::F32, 2)
            .unwrap_or_else(|| *device.default_config())
    } else {
        *device.default_config()
    };
    let wd = WavData {
        sample_format: audio_cfg.sample_format(),
        sample_rate: audio_cfg.sample_rate(),
        // post-downmix the stream is always 2-channel
        channels: 2,
        default_sample_rate: default_rate,
    };
    debug!("wavdata: {:?}", wd);
    (audio_cfg, wd)
}

/// spawn the SSDP discovery thread
fn spawn_ssdp_updater(ssdp_tx: Sender<MessageType>, ssdp_int: f64, ssdp_kick_rx: Receiver<()>) {
    let jh = thread::Builder::new()
        .name("ssdp_updater".into())
        .stack_size(THREAD_STACK)
        .spawn(move || run_ssdp_updater(&ssdp_tx, ssdp_int, ssdp_kick_rx));
    if let Err(e) = jh {
        ui_log(
            LogCategory::Error,
            &fl!("err-ssdp-spawn", "error" = format!("{e:?}")),
        );
    }
}

/// spawn the RMS level-meter monitor thread
fn spawn_rms_monitor(
    wd: WavData,
    rms_rx: Receiver<AudioSamples>,
    mon_l: Progress,
    mon_r: Progress,
) {
    let jh = thread::Builder::new()
        .name("rms_monitor".into())
        .stack_size(THREAD_STACK)
        .spawn(move || run_rms_monitor(wd, &rms_rx, mon_l, mon_r));
    if let Err(e) = jh {
        ui_log(
            LogCategory::Error,
            &fl!("err-rms-spawn", "error" = format!("{e:?}")),
        );
    }
}

/// spawn the HTTP streaming webserver thread
fn spawn_webserver(
    local_addr: IpAddr,
    server_port: u16,
    wd: WavData,
    feedback_tx: Sender<MessageType>,
) {
    let jh = thread::Builder::new()
        .name("swyh_rs_webserver".into())
        .stack_size(THREAD_STACK)
        .spawn(move || run_server(&local_addr, server_port, wd, &feedback_tx));
    if let Err(e) = jh {
        ui_log(
            LogCategory::Error,
            &fl!("err-server-spawn", "error" = format!("{e:?}")),
        );
    }
}

/// handle a `PlayerMessage` from the streaming server:
/// update the renderer button state and auto-resume if configured
fn handle_player_message(
    streamer_feedback: StreamerFeedBack,
    auto_resume: bool,
    wd: WavData,
    local_addr: &IpAddr,
) {
    // check for multiple renderers at same ip address (Bubble UPnP)
    let mut same_ip: Vec<Renderer> = get_renderers()
        .iter()
        .filter(|r| r.remote_addr == streamer_feedback.remote_ip)
        .cloned()
        .collect();
    // the following only works for players with a unique IP address
    if same_ip.len() != 1 {
        return;
    }
    let renderer = &mut same_ip[0];
    let Some(mut button) = renderer.rend_ui.button.clone() else {
        return;
    };
    match streamer_feedback.streaming_state {
        StreamingState::Started => {
            update_playstate(&streamer_feedback.remote_ip, true);
            button.set(true);
        }
        StreamingState::Ended => {
            // check if the renderer has actually not started streaming again
            // as this can happen with Bubble/Nest Audio OpenHome
            let still_streaming = get_clients()
                .values()
                .any(|chanstrm| chanstrm.remote_ip == streamer_feedback.remote_ip);
            if still_streaming {
                button.set(true);
                update_playstate(&streamer_feedback.remote_ip, true);
            } else {
                update_playstate(&streamer_feedback.remote_ip, false);
                if auto_resume && button.is_set() {
                    let streaminfo = StreamInfo::new(wd.sample_rate);
                    let _ = renderer.play(local_addr, streaminfo);
                    update_playstate(&streamer_feedback.remote_ip, true);
                } else {
                    button.set(false);
                }
            }
        }
    }
}

/// stop all active renderers, persist the active-renderer list for auto-reconnect,
/// then wait for HTTP streaming connections to close before returning
fn shutdown_and_exit() {
    let mut active_players: Vec<String> = Vec::new();
    let renderers = get_renderers().clone();
    for mut renderer in renderers {
        if let Some(button) = renderer.rend_ui.button.as_ref()
            && button.is_set()
        {
            ui_log(
                LogCategory::Info,
                &fl!("status-shutting-down", "name" = &renderer.dev_name),
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
    // wait for HTTP streaming connections to drain
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
