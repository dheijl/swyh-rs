#![cfg(feature = "cli")]
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

use cpal::traits::StreamTrait;
use crossbeam_channel::{Receiver, Sender, unbounded};
use hashbrown::HashMap;
use log::{LevelFilter, debug, error, info};
use simplelog::{ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, WriteLogger};
use swyh_rs::{
    enums::{
        messages::MessageType,
        streaming::{
            StreamingFormat::{Flac, Lpcm, Rf64, Wav},
            StreamingState,
        },
    },
    globals::statics::{
        APP_DATE, APP_VERSION, ONE_MINUTE, THREAD_STACK, get_clients, get_config_mut,
        get_msgchannel, get_renderers, get_renderers_mut,
    },
    openhome::rendercontrol::{Renderer, StreamInfo, WavData, discover},
    server::streaming_server::run_server,
    utils::{
        audiodevices::{
            capture_output_audio, get_default_audio_output_device, get_output_audio_devices,
        },
        bincommon::run_silence_injector,
        commandline::Args,
        configuration::Configuration,
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
    let mut args = Args::new().parse();
    // first initialize cpal audio to prevent COM reinitialize panic on Windows
    // but it's possible that there is no default audio device
    let mut audio_output_device_opt = get_default_audio_output_device();

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
    if let Some(config_id) = &config.config_id
        && !config_id.is_empty()
    {
        println!("Loaded configuration -c {config_id}");
    }
    config.monitor_rms = false;
    // set args loglevel
    if let Some(level) = args.log_level {
        config.log_level = level;
    }
    if cfg!(debug_assertions) {
        config.log_level = LevelFilter::Debug;
    }
    // configure simplelogger
    let loglevel = config.log_level;
    let config_id = config.config_id.clone().unwrap();
    let logfilename = "log{}.txt".replace("{}", &config_id);
    let logfile = Path::new(&config.log_dir()).join(logfilename);
    let log_config = ConfigBuilder::new()
        .set_time_format_rfc2822()
        .set_time_offset_to_local()
        .unwrap()
        .build();

    let _ = CombinedLogger::init(vec![
        TermLogger::new(
            loglevel,
            log_config.clone(),
            simplelog::TerminalMode::Stderr,
            ColorChoice::Auto,
        ),
        WriteLogger::new(loglevel, log_config.clone(), File::create(logfile).unwrap()),
    ]);

    info!(
        "{} V {}(build: {}) - Running on {}, {}, {} - Logging started.",
        APP_NAME,
        APP_VERSION,
        APP_DATE.unwrap_or("beta"),
        std::env::consts::ARCH,
        std::env::consts::FAMILY,
        std::env::consts::OS
    );
    if cfg!(debug_assertions) {
        ui_log(
            LogCategory::Warning,
            "Running DEBUG build => log level set to DEBUG!",
        );
    }
    info!("Commandline args: {args:?}");
    info!("Current config: {config:?}");

    if args.inject_silence.is_some() {
        config.inject_silence = args.inject_silence;
    }
    // set soundsource index or name from args or config
    let audio_devices = get_output_audio_devices();
    // get the index from args or config
    let mut ss_index = if let Some(index) = args.sound_source_index {
        args.sound_source_name = None;
        index
    } else if let Some(index) = config.sound_source_index {
        index
    } else {
        -1i32
    };
    // config index can be overridden by name from args
    let ss_name = {
        if let Some(name) = args.sound_source_name {
            ss_index = -1i32;
            name
        } else if let Some(name) = config.sound_source.clone() {
            name
        } else {
            String::new()
        }
    };
    // use index from config if present and no name arg present
    if ss_index >= 0 {
        // args - sound source index
        config.sound_source_index = Some(ss_index);
        for (index, adev) in audio_devices.into_iter().enumerate() {
            let devname = adev.name().to_owned();
            ui_log(
                LogCategory::Info,
                &format!("Found Audio Source: index = {index}, name = {devname}"),
            );
            if index == ss_index as usize {
                audio_output_device_opt = Some(adev);
                config.sound_source = Some(devname.clone());
                ui_log(
                    LogCategory::Info,
                    &format!("Selected audio source: {devname}[#{index}]"),
                );
            } else {
                let config_sound_source = config.sound_source.clone().unwrap_or_default();
                if devname == config_sound_source {
                    audio_output_device_opt = Some(adev);
                    ui_log(
                        LogCategory::Info,
                        &format!("Selected audio source: {devname}"),
                    );
                }
            }
        }
    } else if !ss_name.is_empty() {
        // args = sound source name, check for duplicate name position
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
                    &format!("Found Audio Source: index = {index}, name = {devname}"),
                );
                if devname.to_uppercase().contains(&ss_name.to_uppercase()) {
                    audio_output_device_opt = Some(adev);
                    config.sound_source = Some(devname.clone());
                    config.sound_source_index = Some(index as i32);
                    ui_log(
                        LogCategory::Info,
                        &format!("Selected audio source: {devname}[#{index}]"),
                    );
                } else if devname == *config.sound_source.as_ref().unwrap() {
                    audio_output_device_opt = Some(adev);
                    ui_log(
                        LogCategory::Info,
                        &format!("Selected audio source: {devname}"),
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
                        &format!("Selected audio source: {devname}:{pos}"),
                    );
                }
            }
        }
    }

    let mut audio_output_device = audio_output_device_opt.expect("No default audio device");

    // get the list of available networks
    let networks = get_interfaces();
    for ip in &networks {
        ui_log(LogCategory::Info, &format!("Found network: {ip}"));
    }
    // args: ip_address
    if let Some(ip) = args.ip_address
        && networks.contains(&ip)
    {
        config.last_network = Some(ip.parse().unwrap());
    }
    // get the local network network address
    let local_addr: IpAddr = {
        fn get_default_address(config: &mut Configuration) -> IpAddr {
            let addr = get_local_addr().expect("Could not obtain local address.");
            config.last_network = Some(addr.to_string());
            info!("Using network {addr}");
            addr
        }
        if let Some(ref network) = config.last_network {
            if networks.contains(network) {
                info!("Using network {network}");
                network.parse().unwrap()
            } else {
                get_default_address(&mut config)
            }
        } else {
            get_default_address(&mut config)
        }
    };
    // we need to pass some audio config data to the streaming server
    let audio_cfg = audio_output_device.default_config().clone();
    let wd = WavData {
        sample_format: audio_cfg.sample_format(),
        sample_rate: audio_cfg.sample_rate(),
        channels: audio_cfg.channels(),
    };

    // raise process priority a bit to prevent audio stuttering under cpu load
    raise_priority();

    // the rms monitor channel
    let rms_channel: (Sender<Vec<f32>>, Receiver<Vec<f32>>) = unbounded();

    // capture system audio
    debug!("Try capturing system audio");
    let rms_chan1 = rms_channel.clone();
    let mut stream: cpal::Stream = match capture_output_audio(&audio_output_device, rms_chan1.0) {
        Some(s) => s,
        _ => {
            ui_log(
                LogCategory::Error,
                "> Could not capture audio ...Please check configuration.",
            );
            return Err(-2);
        }
    };
    stream.play().unwrap();

    // If silence injector is on, create a silence injector stream.
    let _silence_stream = {
        if let Some(true) = config.inject_silence {
            if let Some(stream) = run_silence_injector(&audio_output_device) {
                ui_log(
                    LogCategory::Info,
                    "Injecting silence into the output stream",
                );
                Some(stream)
            } else {
                ui_log(LogCategory::Error, "E Unable to inject silence !!");
                None
            }
        } else {
            None
        }
    };

    // set args ssdp_interval
    if let Some(mut minutes) = args.ssdp_interval_mins {
        minutes = minutes.clamp(0.5, minutes);
        config.ssdp_interval_mins = minutes;
    }

    // update config with new args
    let _ = config.update_config();
    // update in_memory shared config for other threads
    {
        let mut conf = get_config_mut();
        *conf = config.clone();
    }

    // get the message channel
    let msg_tx = get_msgchannel().0.clone();
    let msg_rx = get_msgchannel().1.clone();

    let mut serve_only = args.serve_only.unwrap_or(false);
    // if only serving: no ssdp discovery
    if !serve_only || args.dry_run.is_some() {
        // now start the SSDP discovery update thread with a Crossbeam channel for renderer updates
        // the discovered renderers will be kept in this list
        ui_log(LogCategory::Info, "Starting SSDP discovery");
        let ssdp_int = config.ssdp_interval_mins;
        let ssdp_tx = msg_tx.clone();
        let _ = thread::Builder::new()
            .name("ssdp_updater".into())
            .stack_size(THREAD_STACK)
            .spawn(move || run_ssdp_updater(&ssdp_tx, ssdp_int))
            .unwrap();
    }
    // set args autoresume
    config.auto_resume = args.auto_resume.unwrap_or(config.auto_resume);
    // set args server port
    if args.server_port.is_some() {
        config.server_port = args.server_port;
    }
    // set args bits per sample
    if args.bits_per_sample.is_some() {
        config.bits_per_sample = args.bits_per_sample;
    }
    // set args streaming format and streamsize
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
    // upfront buffering
    if args.upfront_buffer.is_some() {
        config.buffering_delay_msec = args.upfront_buffer;
    }

    // start the webserver
    let server_port = config.server_port;
    let feedback_tx = msg_tx.clone();
    let _ = thread::Builder::new()
        .name("swyh_rs_webserver".into())
        .stack_size(THREAD_STACK)
        .spawn(move || {
            run_server(
                &local_addr,
                server_port.unwrap_or_default(),
                wd,
                &feedback_tx,
            );
        })
        .unwrap();
    // give the web server thread a chance to start
    thread::yield_now();

    // we may have to translate player names to IP addresses
    if !serve_only && (args.player_ip.is_some() || config.last_renderer.is_some()) {
        // give the webserver a chance to start and wait for ssdp to complete
        thread::sleep(Duration::from_secs(5));
        // get the results of the ssdp discovery
        let mut n = 0;
        while let Ok(msg) = msg_rx.try_recv() {
            match msg {
                MessageType::SsdpMessage(newr) => {
                    get_renderers_mut().push(*newr.clone());
                    ui_log(
                        LogCategory::Info,
                        &format!(
                            "Available renderer #{n}: {} at {}",
                            newr.dev_name, newr.remote_addr
                        ),
                    );
                    n += 1;
                }
                MessageType::PlayerMessage(_) => (),
                MessageType::LogMessage(_) => (),
                MessageType::CaptureAborted() => (),
            }
        }
        // now check for player names(s) instead of ip addresses
        if let Some(ref pl_ip) = args.player_ip
            && let Some(r) = get_renderers().iter().find(|r| r.dev_name.contains(pl_ip))
        {
            ui_log(
                LogCategory::Info,
                &format!("Default renderer ip: {pl_ip} => {}", r.remote_addr),
            );
            args.player_ip = Some(r.remote_addr.clone());
        }
        if args.active_players.is_some() {
            let mut ip_players: Vec<String> = Vec::new();
            args.active_players.as_ref().unwrap().iter().for_each(|ap| {
                if let Some(r) = get_renderers().iter().find(|r| r.dev_name.contains(ap)) {
                    ip_players.push(r.remote_addr.clone());
                    ui_log(
                        LogCategory::Info,
                        &format!("Active renderer: {ap} => {} ", r.remote_addr),
                    );
                }
            });
            if !ip_players.is_empty() {
                args.active_players = Some(ip_players);
            }
        }
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

    // in serve-only mode (-x): disable auto_reconnect else it's always on
    if serve_only {
        config.auto_reconnect = false;
    } else {
        // else autoreconnect is always on
        config.auto_reconnect = true;
    }
    // update config with new args
    let _ = config.update_config();
    // update in_memory shared config for other threads
    {
        let mut conf = get_config_mut();
        *conf = config.clone();
    }

    let mut player: Option<Renderer> = None;
    // select the player unless only serving
    if !serve_only {
        let last_renderer = config.last_renderer.as_ref().unwrap();
        if get_renderers().is_empty() {
            error!("No renderers found!!!");
            return Err(-1);
        }
        // default = first player
        player = Some(get_renderers()[0].clone());
        // but use the configured renderer if present
        if let Some(pl) = get_renderers()
            .iter()
            .find(|&renderer| renderer.remote_addr == *last_renderer)
        {
            player = Some(pl.clone());
        }
        let def_player = player.as_ref().unwrap();
        // if specified player ip not found: use default player
        if *last_renderer != def_player.remote_addr {
            config.last_renderer = Some(def_player.remote_addr.clone());
        }
        ui_log(
            LogCategory::Info,
            &format!("Default player ip = {}", def_player.remote_addr),
        );
    }

    // update config with new args
    let _ = config.update_config();
    // update in_memory shared config for other threads
    {
        let mut conf = get_config_mut();
        *conf = config.clone();
    }

    info!("New config: {config:?}");

    // exit here if dry-run
    if args.dry_run.is_some() {
        ui_log(LogCategory::Info, "dry-run - exiting...");
        return Ok(());
    }

    // prepare for playing
    let wd = WavData {
        sample_format: audio_cfg.sample_format(),
        sample_rate: audio_cfg.sample_rate(),
        channels: audio_cfg.channels(),
    };
    let streaminfo = StreamInfo {
        sample_rate: wd.sample_rate.0,
        bits_per_sample: config.bits_per_sample.unwrap_or(16),
        streaming_format: config.streaming_format.unwrap_or(Lpcm),
    };

    // start playing unless only serving
    let mut playing = Vec::new();
    if serve_only {
        let port = config.server_port.unwrap_or(5901);
        ui_log(
            LogCategory::Info,
            &format!("Serving started on port {port}..."),
        );
    } else {
        for ip in config.active_renderers {
            if let Some(pl) = get_renderers()
                .iter()
                .find(|&renderer| renderer.remote_addr == ip)
            {
                let mut player = pl.clone();
                if let Some(vol) = args.volume
                    && player.get_volume(&ui_log) > -1
                {
                    player.set_volume(&ui_log, vol.into());
                }
                let _ = player.play(
                    &local_addr,
                    config.server_port.unwrap_or(5901),
                    &ui_log,
                    streaminfo,
                );
                let pl_name = &player.dev_url;
                ui_log(LogCategory::Info, &format!("Playing to {pl_name}"));
                playing.push(player);
            }
        }
    }

    let autoresume = config.auto_resume;
    let streaminfo = {
        StreamInfo {
            sample_rate: wd.sample_rate.0,
            bits_per_sample: config.bits_per_sample.unwrap_or(16),
            streaming_format: config.streaming_format.unwrap_or(Flac),
        }
    };

    loop {
        while let Ok(msg) = msg_rx.try_recv() {
            match msg {
                MessageType::SsdpMessage(newr) => {
                    if !serve_only {
                        ui_log(
                            LogCategory::Info,
                            &format!("New renderer {} at {}", newr.dev_name, newr.remote_addr),
                        );
                        get_renderers_mut().push(*newr);
                    }
                }
                MessageType::PlayerMessage(streamer_feedback) => {
                    match streamer_feedback.streaming_state {
                        StreamingState::Started => {}
                        StreamingState::Ended => {
                            if !serve_only {
                                // first check if the renderer has actually not started streaming again
                                // as this can happen with Bubble/Nest Audio Openhome
                                let still_streaming = get_clients().values().any(|chanstrm| {
                                    chanstrm.remote_ip == streamer_feedback.remote_ip
                                });
                                if !still_streaming
                                    && autoresume
                                    && let Some(r) = get_renderers_mut()
                                        .iter_mut()
                                        .find(|r| r.remote_addr == streamer_feedback.remote_ip)
                                {
                                    let _ = r.play(
                                        &local_addr,
                                        server_port.unwrap_or_default(),
                                        &ui_log,
                                        streaminfo,
                                    );
                                }
                            }
                        }
                    }
                }
                MessageType::LogMessage(msg) => ui_log(LogCategory::Info, &msg),
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
                            let rms_chan2 = rms_channel.clone();
                            if let Some(s) = capture_output_audio(&audio_output_device, rms_chan2.0)
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
        // check the logchannel for new log messages to show in the logger textbox
        thread::sleep(Duration::from_millis(100));
        // handle CTL-C interrupt: shutdown the player(s)
        if shutting_down.load(Ordering::Relaxed) {
            println!("Received ^C -> exiting.");
            if !serve_only && player.is_some() && !get_clients().is_empty() {
                for mut pl in playing {
                    if get_clients()
                        .values()
                        .any(|cs| cs.remote_ip == pl.remote_addr)
                    {
                        println!("^C: Stopping streaming to {}", pl.dev_name);
                        pl.stop_play(&ui_log);
                    }
                }
                // also wait some time for the player(s) to drop the HTTP streaming connection
                for _ in 0..100 {
                    if get_clients().is_empty() {
                        println!("^C: No HTTP streaming connections active");
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                if !get_clients().is_empty() {
                    println!("^C: Time-out waiting for HTTP streaming shutdown - exiting.");
                }
            }
            log::logger().flush();
            std::process::exit(0);
        }
    }
}

/// run the `ssdp_updater` - thread that periodically run ssdp discovery
/// and detect new renderers
/// send any new renderers to te main thread on the Crossbeam ssdp channel
fn run_ssdp_updater(ssdp_tx: &Sender<MessageType>, ssdp_interval_mins: f64) {
    // the hashmap used to detect new renderers
    let mut rmap: HashMap<String, Renderer> = HashMap::new();
    let agent = ureq::agent();
    loop {
        let renderers = discover(&agent, &rmap, &ui_log).unwrap_or_default();
        for r in &renderers {
            rmap.entry(r.remote_addr.clone()).or_insert_with(|| {
                info!(
                    "Found new renderer {} {}  at {}",
                    r.dev_name, r.dev_model, r.remote_addr
                );
                ssdp_tx
                    .send(MessageType::SsdpMessage(Box::new(r.clone())))
                    .unwrap();
                r.clone()
            });
        }
        thread::sleep(Duration::from_millis(
            (ssdp_interval_mins * ONE_MINUTE) as u64,
        ));
    }
}
