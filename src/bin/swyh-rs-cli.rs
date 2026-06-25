//! `swyh-rs-cli` — headless CLI entry point.
//!
//! Command-line equivalent of `swyh-rs`: captures audio and streams to one or more
//! DLNA/OpenHome renderers without a GUI.  Renderer selection, streaming format, bit
//! depth, and network interface are all configurable via command-line flags.

#![cfg(feature = "cli")]
use mimalloc::MiMalloc;
use std::{
    fs::File,
    net::IpAddr,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
use cpal::{SampleFormat, SupportedStreamConfig, traits::StreamTrait};
use crossbeam_channel::{Receiver, Sender, unbounded};
use hashbrown::HashMap;
use log::{LevelFilter, debug, error, info};
use simplelog::{ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, WriteLogger};
use swyh_rs::{
    audio::audiodevices::{
        Device, capture_output_audio, get_default_audio_output_device, get_output_audio_devices,
    },
    enums::{
        messages::MessageType,
        streaming::{
            StreamingFormat::{Flac, Lpcm, Rf64, Wav},
            StreamingState,
        },
    },
    fl,
    globals::statics::{
        APP_DATE, APP_VERSION, ONE_MINUTE, SERVER_PORT, THREAD_STACK, get_clients, get_config_mut,
        get_msgchannel, get_renderers, get_renderers_mut,
    },
    renderers::rendercontrol::{Renderer, StreamInfo, WavData, discover},
    server::streaming_server::run_server,
    utils::{
        bincommon::run_silence_injector,
        commandline::Args,
        configuration::Configuration,
        i18n,
        local_ip_address::{get_interfaces, get_local_addr},
        priority::raise_priority,
        ui_logger::*,
    },
};

pub const APP_NAME: &str = "SWYH-RS-CLI";

fn main() -> Result<(), i32> {
    let shutting_down = Arc::new(AtomicBool::new(false));
    // gracefully exit on Ctrl-C
    let shutdown = shutting_down.clone();
    ctrlc::set_handler(move || {
        shutdown.store(true, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    // collect command line arguments
    let mut args = Args::default();
    if let Err(errors) = args.parse() {
        for e in &errors {
            eprintln!("Argument error: {e}");
        }
        args.usage();
        return Err(1);
    }
    // first initialize cpal audio to prevent COM reinitialize panic on Windows
    // but it's possible that there is no default audio device
    let audio_output_device_opt = get_default_audio_output_device();

    // initialize config
    let mut config = {
        let mut conf = get_config_mut();
        if conf.sound_source.is_none()
            && conf.sound_source_index.is_none()
            && let Some(ref audio_output_device) = audio_output_device_opt
        {
            conf.sound_source = Some(audio_output_device.name().into());
            let _ = conf.update_config();
        }
        conf.clone()
    };
    // initialize i18n before any user-facing string is produced
    if let Some(ref lang) = args.language {
        config.language = Some(lang.clone());
    }
    i18n::init(&config.language.clone().unwrap_or("en-US".to_string()));
    if let Some(config_id) = &config.config_id
        && !config_id.is_empty()
    {
        println!("{}", fl!("status-loaded-config", "id" = config_id));
    }
    config.monitor_rms = false;

    setup_logging(&config, args.log_level);

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

    info!("Commandline args: {args:?}");
    info!("Current config: {config:?}");

    if args.inject_silence.is_some() {
        config.inject_silence = args.inject_silence;
    }
    if args.use_dither.is_some() {
        config.use_dither = args.use_dither;
    }

    let mut audio_output_device =
        select_audio_source_cli(&mut args, &mut config, audio_output_device_opt)
            .expect("No default audio device");

    // get the list of available networks and log them
    let networks = get_interfaces();
    for ip in &networks {
        ui_log(LogCategory::Info, &fl!("cli-found-network", "ip" = ip));
    }
    // apply sample rate from args, overriding config if supplied
    if let Some(rate) = args.sample_rate {
        config.sample_rate = Some(rate);
    }

    let local_addr = resolve_local_addr_cli(args.ip_address.as_deref(), &mut config, &networks);
    let (audio_cfg, wd) = build_wav_data(&audio_output_device, &config);

    // raise process priority a bit to prevent audio stuttering under cpu load
    raise_priority();

    // the rms monitor channel
    let rms_channel = unbounded();

    // capture system audio
    debug!("Try capturing system audio");
    let rms_chan1 = rms_channel.clone();
    let mut stream: cpal::Stream =
        match capture_output_audio(&audio_output_device, &audio_cfg, rms_chan1.0) {
            Some(s) => s,
            _ => {
                ui_log(LogCategory::Error, &fl!("err-capture-audio"));
                return Err(-2);
            }
        };
    stream.play().expect("Unable to play audio stream");

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

    // set args ssdp_interval, minimum is 0.5 minutes
    if let Some(mut minutes) = args.ssdp_interval_mins {
        minutes = minutes.max(0.5);
        config.ssdp_interval_mins = minutes;
    }

    // update config with new args
    sync_config(&config);

    // get the message channel
    let msg_tx = get_msgchannel().0.clone();
    let msg_rx = get_msgchannel().1.clone();

    let mut serve_only = args.serve_only.unwrap_or(false);
    // if only serving: no ssdp discovery
    if !serve_only || args.dry_run.is_some() {
        ui_log(LogCategory::Info, &fl!("status-starting-ssdp"));
        spawn_cli_ssdp_updater(msg_tx.clone(), config.ssdp_interval_mins);
    }

    apply_streaming_args(&args, &mut config);

    // start the webserver
    spawn_cli_webserver(
        local_addr,
        config.server_port.unwrap_or_default(),
        wd,
        msg_tx.clone(),
    );
    // give the web server thread a chance to start
    thread::yield_now();

    // translate player names to IP addresses using SSDP discovery results
    if !serve_only && (args.player_ip.is_some() || config.last_renderer.is_some()) {
        resolve_player_names(&msg_rx, &mut args);
    }

    // set args last_renderer and active players
    if args.player_ip.is_some() {
        config.last_renderer = args.player_ip;
    }
    if let Some(ref active_players) = args.active_players {
        config.active_renderers.clone_from(active_players);
    }

    // if no player specified: switch to serve mode
    if config.last_renderer.is_none() {
        serve_only = true;
    }

    // in serve-only mode: disable auto_reconnect; else it's always on
    config.auto_reconnect = !serve_only;

    // update config with new args
    sync_config(&config);

    let mut player: Option<Renderer> = None;
    if !serve_only {
        let Some(r) = select_primary_renderer(&mut config) else {
            return Err(-1);
        };
        player = Some(r);
    }

    // update config with new args
    sync_config(&config);
    info!("New config: {config:?}");

    // exit here if dry-run
    if args.dry_run.is_some() {
        ui_log(LogCategory::Info, &fl!("status-dry-run-exit"));
        return Ok(());
    }

    // prepare for playing
    let streaminfo = StreamInfo::new(wd.sample_rate);

    // start playing unless only serving
    let mut playing = Vec::new();
    if serve_only {
        let port = config.server_port.unwrap_or(SERVER_PORT);
        ui_log(
            LogCategory::Info,
            &fl!("status-serving-started", "port" = port),
        );
    } else {
        for ip in config.active_renderers {
            if let Some(pl) = get_renderers()
                .iter()
                .find(|&renderer| renderer.remote_addr == ip)
            {
                let mut player = pl.clone();
                if let Some(vol) = args.volume
                    && player.volume > -1
                {
                    player.set_volume(vol.into());
                }
                let _ = player.play(&local_addr, streaminfo);
                let pl_name = &player.dev_url;
                ui_log(
                    LogCategory::Info,
                    &fl!("status-playing-to", "name" = pl_name),
                );
                playing.push(player);
            }
        }
    }

    let autoresume = config.auto_resume;
    loop {
        while let Ok(msg) = msg_rx.try_recv() {
            match msg {
                MessageType::SsdpMessage(newr) => {
                    if !serve_only {
                        ui_log(
                            LogCategory::Info,
                            &fl!(
                                "status-new-renderer",
                                "name" = &newr.dev_name,
                                "addr" = &newr.remote_addr
                            ),
                        );
                        get_renderers_mut().push(*newr);
                    }
                }
                MessageType::PlayerMessage(streamer_feedback) => {
                    if let StreamingState::Ended = streamer_feedback.streaming_state
                        && !serve_only
                    {
                        let still_streaming = get_clients()
                            .values()
                            .any(|chanstrm| chanstrm.remote_ip == streamer_feedback.remote_ip);
                        if !still_streaming
                            && autoresume
                            && let Some(r) = get_renderers_mut()
                                .iter_mut()
                                .find(|r| r.remote_addr == streamer_feedback.remote_ip)
                        {
                            let _ = r.play(&local_addr, streaminfo);
                        }
                    }
                }
                MessageType::LogMessage(msg) => ui_log(LogCategory::Info, &msg),
                MessageType::CaptureAborted => {
                    let mut capture_retry_count = 0i32;
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
                            let rms_chan2 = rms_channel.clone();
                            if let Some(s) =
                                capture_output_audio(&audio_output_device, &audio_cfg, rms_chan2.0)
                            {
                                stream = s;
                                stream.play().expect("Unable to play audio stream");
                                info!("Audio capture resumed.");
                                break;
                            }
                        }
                    }
                }
            }
        }
        // handle Ctrl-C: stop all players and exit
        if shutting_down.load(Ordering::Relaxed) {
            shutdown_ctrlc(serve_only, player.as_ref(), playing);
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn sync_config(config: &Configuration) {
    let _ = config.update_config();
    // update in-memory shared config for other threads
    {
        let mut conf = get_config_mut();
        *conf = config.clone();
    }
}

/// configure simplelog with both terminal and file sinks
fn setup_logging(config: &Configuration, args_log_level: Option<LevelFilter>) {
    let loglevel = if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        args_log_level.unwrap_or(config.log_level)
    };
    let config_id = config.config_id.clone().unwrap_or_default();
    let logfilename = "log{}.txt".replace("{}", &config_id);
    let logfile = Path::new(&config.log_dir()).join(logfilename);
    let mut log_config_builder = ConfigBuilder::new();
    log_config_builder.set_time_format_rfc2822();
    let _ = log_config_builder.set_time_offset_to_local(); // silently fall back to UTC on error
    let log_config = log_config_builder.build();
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
}

/// select the audio output device from args or config;
/// updates `config.sound_source` / `config.sound_source_index` and returns the chosen device
fn select_audio_source_cli(
    args: &mut Args,
    config: &mut Configuration,
    default_device: Option<Device>,
) -> Option<Device> {
    let audio_devices = get_output_audio_devices();
    let mut audio_output_device_opt = default_device;

    // get the index from args or config
    let mut ss_index = {
        if let Some(index) = args.sound_source_index {
            args.sound_source_name = None;
            index
        } else {
            config.sound_source_index.unwrap_or(-1i32)
        }
    };
    // config index can be overridden by name from args
    let ss_name = {
        if let Some(name) = args.sound_source_name.take() {
            ss_index = -1i32;
            name
        } else {
            config.sound_source.clone().unwrap_or_default()
        }
    };

    // use index from config if present and no name arg present
    if ss_index >= 0 {
        config.sound_source_index = Some(ss_index);
        for (index, adev) in audio_devices.into_iter().enumerate() {
            let devname = adev.name().to_owned();
            ui_log(
                LogCategory::Info,
                &fl!("cli-found-audio-source", "index" = index, "name" = &devname),
            );
            if index == ss_index as usize {
                audio_output_device_opt = Some(adev);
                config.sound_source = Some(devname.clone());
                ui_log(
                    LogCategory::Info,
                    &fl!(
                        "cli-selected-audio-source-idx",
                        "name" = &devname,
                        "index" = index
                    ),
                );
            } else {
                let config_sound_source = config.sound_source.clone().unwrap_or_default();
                if devname == config_sound_source {
                    audio_output_device_opt = Some(adev);
                    ui_log(
                        LogCategory::Info,
                        &fl!("cli-selected-audio-source", "name" = &devname),
                    );
                }
            }
        }
    } else if !ss_name.is_empty() {
        // args = sound source name; check for duplicate name position syntax "name:pos"
        let (dupname, duppos) = if ss_name.contains(':') {
            let parts: Vec<&str> = ss_name.split(':').collect();
            (parts[0], parts[1])
        } else {
            ("", "")
        };
        if duppos.is_empty() {
            for (index, adev) in audio_devices.into_iter().enumerate() {
                let devname = adev.name().to_owned();
                ui_log(
                    LogCategory::Info,
                    &fl!("cli-found-audio-source", "index" = index, "name" = &devname),
                );
                if devname.to_uppercase().contains(&ss_name.to_uppercase()) {
                    audio_output_device_opt = Some(adev);
                    config.sound_source = Some(devname.clone());
                    config.sound_source_index = Some(index as i32);
                    ui_log(
                        LogCategory::Info,
                        &fl!(
                            "cli-selected-audio-source-idx",
                            "name" = &devname,
                            "index" = index
                        ),
                    );
                } else if devname == *config.sound_source.as_ref().unwrap() {
                    audio_output_device_opt = Some(adev);
                    ui_log(
                        LogCategory::Info,
                        &fl!("cli-selected-audio-source", "name" = &devname),
                    );
                }
            }
        } else if let Ok(pos) = duppos.parse::<usize>() {
            let dups: Vec<_> = audio_devices
                .into_iter()
                .enumerate()
                .filter(|(_i, d)| d.name().to_uppercase().contains(&dupname.to_uppercase()))
                .collect();
            for (index, dev) in dups.into_iter().enumerate() {
                if index == pos {
                    let devname = dev.1.name().to_string();
                    audio_output_device_opt = Some(dev.1);
                    config.sound_source = Some(devname.clone());
                    config.sound_source_index = Some(dev.0 as i32);
                    ui_log(
                        LogCategory::Info,
                        &fl!(
                            "cli-selected-audio-source-pos",
                            "name" = &devname,
                            "pos" = pos
                        ),
                    );
                }
            }
        }
    }

    audio_output_device_opt
}

/// resolve the local IP address to bind to, applying the `--ip` arg and persisting the result
fn resolve_local_addr_cli(
    ip_arg: Option<&str>,
    config: &mut Configuration,
    networks: &[String],
) -> IpAddr {
    if let Some(ip) = ip_arg
        && networks.contains(&ip.to_string())
    {
        config.last_network = Some(ip.to_string());
    }

    fn get_default(config: &mut Configuration) -> IpAddr {
        let addr = get_local_addr().unwrap_or_else(|| {
            eprintln!("Could not obtain local network address.");
            std::process::exit(1);
        });
        config.last_network = Some(addr.to_string());
        info!("Using network {addr}");
        addr
    }

    let last_network = config.last_network.clone();
    if let Some(ref network) = last_network {
        if networks.contains(network) {
            info!("Using network {network}");
            network.parse().unwrap()
        } else {
            get_default(config)
        }
    } else {
        get_default(config)
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
    (audio_cfg, wd)
}

/// spawn the CLI SSDP discovery thread
fn spawn_cli_ssdp_updater(ssdp_tx: Sender<MessageType>, ssdp_interval_mins: f64) {
    thread::Builder::new()
        .name("ssdp_updater".into())
        .stack_size(THREAD_STACK)
        .spawn(move || run_ssdp_updater(&ssdp_tx, ssdp_interval_mins))
        .unwrap();
}

/// spawn the HTTP streaming webserver thread
fn spawn_cli_webserver(
    local_addr: IpAddr,
    server_port: u16,
    wd: WavData,
    feedback_tx: Sender<MessageType>,
) {
    thread::Builder::new()
        .name("swyh_rs_webserver".into())
        .stack_size(THREAD_STACK)
        .spawn(move || run_server(&local_addr, server_port, wd, &feedback_tx))
        .unwrap();
}

/// apply streaming-related args (format, bit depth, buffer, etc.) to config
fn apply_streaming_args(args: &Args, config: &mut Configuration) {
    config.auto_resume = args.auto_resume.unwrap_or(config.auto_resume);
    if args.server_port.is_some() {
        config.server_port = args.server_port;
    }
    if args.bits_per_sample.is_some() {
        config.bits_per_sample = args.bits_per_sample;
    }
    if let Some(ref sf) = args.streaming_format {
        config.streaming_format = args.streaming_format;
        if args.stream_size.is_some() {
            match sf {
                Lpcm => config.lpcm_stream_size = args.stream_size,
                Wav => config.wav_stream_size = args.stream_size,
                Flac => config.flac_stream_size = args.stream_size,
                Rf64 => config.rf64_stream_size = args.stream_size,
            }
        }
    }
    if args.upfront_buffer.is_some() {
        config.buffering_delay_msec = args.upfront_buffer;
    }
}

/// wait for SSDP discovery to complete, then translate any player names in `args`
/// to their IP addresses
fn resolve_player_names(msg_rx: &Receiver<MessageType>, args: &mut Args) {
    // give the webserver a chance to start and wait for ssdp to complete
    thread::sleep(Duration::from_secs(5));
    let mut n = 0;
    while let Ok(msg) = msg_rx.try_recv() {
        if let MessageType::SsdpMessage(newr) = msg {
            get_renderers_mut().push(*newr.clone());
            ui_log(
                LogCategory::Info,
                &fl!(
                    "cli-available-renderer",
                    "n" = n,
                    "name" = &newr.dev_name,
                    "addr" = &newr.remote_addr
                ),
            );
            n += 1;
        }
    }
    // resolve player name to IP address if a name was given instead of an IP
    if let Some(ref pl_ip) = args.player_ip
        && let Some(r) = get_renderers().iter().find(|r| r.dev_name.contains(pl_ip))
    {
        ui_log(
            LogCategory::Info,
            &fl!(
                "cli-default-renderer-ip",
                "ip" = pl_ip,
                "addr" = &r.remote_addr
            ),
        );
        args.player_ip = Some(r.remote_addr.clone());
    }
    if let Some(active_players) = &args.active_players {
        let mut ip_players: Vec<String> = Vec::new();
        active_players.iter().for_each(|ap| {
            if let Some(r) = get_renderers().iter().find(|r| r.dev_name.contains(ap)) {
                ip_players.push(r.remote_addr.clone());
                ui_log(
                    LogCategory::Info,
                    &fl!("cli-active-renderer", "name" = ap, "addr" = &r.remote_addr),
                );
            }
        });
        if !ip_players.is_empty() {
            args.active_players = Some(ip_players);
        }
    }
}

/// select the primary renderer from the discovered list based on config
fn select_primary_renderer(config: &mut Configuration) -> Option<Renderer> {
    if get_renderers().is_empty() {
        error!("{}", fl!("cli-no-renderers"));
        return None;
    }
    let last_renderer = config.last_renderer.as_deref().unwrap_or("");
    // default = first player
    let mut player = get_renderers()[0].clone();
    // use the configured renderer if present
    if let Some(pl) = get_renderers()
        .iter()
        .find(|r| r.remote_addr == last_renderer)
    {
        player = pl.clone();
    }
    // if specified player ip not found: record which default we're using
    if last_renderer != player.remote_addr {
        config.last_renderer = Some(player.remote_addr.clone());
    }
    ui_log(
        LogCategory::Info,
        &fl!("cli-default-player-ip", "ip" = &player.remote_addr),
    );
    Some(player)
}

/// stop all playing renderers, wait for HTTP connections to drain, then exit
fn shutdown_ctrlc(serve_only: bool, player: Option<&Renderer>, playing: Vec<Renderer>) -> ! {
    println!("{}", fl!("cli-received-ctrlc"));
    if !serve_only && player.is_some() && !get_clients().is_empty() {
        for mut pl in playing {
            if get_clients()
                .values()
                .any(|cs| cs.remote_ip == pl.remote_addr)
            {
                println!("{}", fl!("cli-ctrlc-stopping", "name" = &pl.dev_name));
                pl.stop_play();
            }
        }
        for _ in 0..100 {
            if get_clients().is_empty() {
                println!("{}", fl!("cli-ctrlc-no-connections"));
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        if !get_clients().is_empty() {
            println!("{}", fl!("cli-ctrlc-timeout"));
        }
    }
    log::logger().flush();
    std::process::exit(0);
}

/// run the `ssdp_updater` — periodically discover DLNA/OpenHome renderers
/// and forward new ones to the main thread via `ssdp_tx`
fn run_ssdp_updater(ssdp_tx: &Sender<MessageType>, ssdp_interval_mins: f64) {
    let mut rmap: HashMap<String, Renderer> = HashMap::new();
    let agent = ureq::agent();
    loop {
        let renderers = discover(&agent, &rmap).unwrap_or_default();
        for r in &renderers {
            rmap.entry(r.remote_addr.clone()).or_insert_with(|| {
                info!(
                    "Found new renderer {} {}  at {}",
                    r.dev_name, r.dev_model, r.remote_addr
                );
                ssdp_tx
                    .send(MessageType::SsdpMessage(Box::new(r.clone())))
                    .expect("Message Channel disconnected.");
                r.clone()
            });
        }
        thread::sleep(Duration::from_millis(
            (ssdp_interval_mins * ONE_MINUTE) as u64,
        ));
    }
}
