#![cfg(feature = "cli")]
use std::{
    fs::File,
    net::IpAddr,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use cpal::traits::StreamTrait;
use crossbeam_channel::{unbounded, Receiver, Sender};
use hashbrown::HashMap;
use log::{debug, error, info, LevelFilter};
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, WriteLogger};
use swyh_rs::{
    enums::{
        messages::MessageType,
        streaming::{
            StreamingFormat::{Flac, Lpcm, Rf64, Wav},
            StreamingState,
        },
    },
    globals::statics::{APP_VERSION, CLIENTS, CONFIG, MSGCHANNEL},
    openhome::rendercontrol::{discover, Renderer, StreamInfo, WavData},
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
        ui_logger::ui_log,
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
        let mut conf = CONFIG.write();
        if conf.sound_source.is_none() && conf.sound_source_index.is_none() {
            if let Some(ref audio_output_device) = audio_output_device_opt {
                conf.sound_source = Some(audio_output_device.name().into());
                let _ = conf.update_config();
            }
        }
        conf.clone()
    };
    if let Some(config_id) = &config.config_id {
        if !config_id.is_empty() {
            println!("Loaded configuration -c {config_id}");
        }
    }
    config.monitor_rms = false;
    println!("Current config: {config:?}");
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
    let _ = CombinedLogger::init(vec![
        TermLogger::new(
            loglevel,
            Config::default(),
            simplelog::TerminalMode::Stderr,
            ColorChoice::Auto,
        ),
        WriteLogger::new(loglevel, Config::default(), File::create(logfile).unwrap()),
    ]);

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
    if args.inject_silence.is_some() {
        config.inject_silence = args.inject_silence;
    }
    // set soundsource index or name
    let audio_devices = get_output_audio_devices();
    if let Some(index) = args.sound_source_index {
        // args - sound source index
        config.sound_source_index = Some(index);
        for (index, adev) in audio_devices.into_iter().enumerate() {
            let devname = adev.name().to_owned();
            ui_log(&format!(
                "Found Audio Source: index = {index}, name = {devname}"
            ));
            if index == config.sound_source_index.unwrap() as usize {
                audio_output_device_opt = Some(adev);
                config.sound_source = Some(devname.clone());
                ui_log(&format!("Selected audio source: {devname}[#{index}]"));
            } else {
                let config_sound_source = config.sound_source.clone().unwrap_or_default();
                if devname == *config_sound_source {
                    audio_output_device_opt = Some(adev);
                    ui_log(&format!("Selected audio source: {devname}"));
                }
            }
        }
    } else if let Some(ref name) = args.sound_source_name {
        // args = sound source name, check for duplicate name position
        let (dupname, duppos) = if name.contains(':') {
            let parts: Vec<&str> = name.split(':').collect();
            (parts[0], parts[1])
        } else {
            ("", "")
        };
        if duppos.is_empty() {
            for (index, adev) in audio_devices.into_iter().enumerate() {
                let devname = adev.name().to_owned();
                ui_log(&format!(
                    "Found Audio Source: index = {index}, name = {devname}"
                ));
                if devname.to_uppercase().contains(&name.to_uppercase()) {
                    audio_output_device_opt = Some(adev);
                    config.sound_source = Some(devname.clone());
                    ui_log(&format!("Selected audio source: {devname}[#{index}]"));
                } else if devname == *config.sound_source.as_ref().unwrap() {
                    audio_output_device_opt = Some(adev);
                    ui_log(&format!("Selected audio source: {devname}"));
                }
            }
        } else if let Ok(pos) = duppos.parse::<usize>() {
            let dups: Vec<_> = audio_devices
                .into_iter()
                .filter(|d| d.name().to_uppercase().contains(&dupname.to_uppercase()))
                .collect();
            for (index, dev) in dups.into_iter().enumerate() {
                if index == pos {
                    let devname = dev.name().to_string();
                    audio_output_device_opt = Some(dev);
                    config.sound_source = Some(devname.clone());
                    ui_log(&format!("Selected audio source: {devname}:{pos}"));
                }
            }
        }
    }

    let audio_output_device = audio_output_device_opt.expect("No default audio device");

    // get the list of available networks
    let networks = get_interfaces();
    for ip in &networks {
        ui_log(&format!("Found network: {ip}"));
    }
    // args: ip_address
    if let Some(ip) = args.ip_address {
        if networks.contains(&ip) {
            config.last_network = Some(ip.parse().unwrap());
        }
    }
    // get the local network network address
    let local_addr: IpAddr = {
        fn get_default_address(config: &mut Configuration) -> IpAddr {
            let addr = get_local_addr().expect("Could not obtain local address.");
            config.last_network = Some(addr.to_string());
            info!("Using network {}", addr);
            addr
        }
        if let Some(ref network) = config.last_network {
            if networks.contains(network) {
                info!("Using network {}", network);
                network.parse().unwrap()
            } else {
                get_default_address(&mut config)
            }
        } else {
            get_default_address(&mut config)
        }
    };
    // we need to pass some audio config data to the play function
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
    let stream: cpal::Stream;
    if let Some(s) = capture_output_audio(&audio_output_device, rms_channel.0) {
        stream = s;
        stream.play().unwrap();
    } else {
        ui_log("*E*E*> Could not capture audio ...Please check configuration.");
        return Err(-2);
    }
    // If silence injector is on, create a silence injector stream.
    let _silence_stream = if let Some(true) = CONFIG.read().inject_silence {
        ui_log("Injecting silence into the output stream");
        Some(run_silence_injector(&audio_output_device))
    } else {
        None
    };

    // set args ssdp_interval
    if let Some(mut minutes) = args.ssdp_interval_mins {
        if minutes < 0.5 {
            minutes = 0.5;
        }
        config.ssdp_interval_mins = minutes;
    }

    // update config with new args
    let _ = config.update_config();
    // update in_memory shared config for other threads
    {
        let mut conf = CONFIG.write();
        *conf = config.clone();
    }

    // get the message channel
    let msg_tx = MSGCHANNEL.read().0.clone();
    let msg_rx = MSGCHANNEL.read().1.clone();

    let mut renderers: Vec<Renderer> = Vec::new();
    let mut serve_only = args.serve_only.unwrap_or(false);
    // if only serving: no ssdp discovery
    if !serve_only || args.dry_run.is_some() {
        // now start the SSDP discovery update thread with a Crossbeam channel for renderer updates
        // the discovered renderers will be kept in this list
        ui_log("Discover networks");
        ui_log("Starting SSDP discovery");
        let ssdp_int = config.ssdp_interval_mins;
        let ssdp_tx = msg_tx.clone();
        let _ = thread::Builder::new()
            .name("ssdp_updater".into())
            .stack_size(4 * 1024 * 1024)
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
    // set args streaming format
    if args.streaming_format.is_some() {
        config.streaming_format = args.streaming_format;
    }
    // and stream-size
    if args.streaming_format.is_some() && args.stream_size.is_some() {
        match args.streaming_format.unwrap() {
            Lpcm => config.lpcm_stream_size = args.stream_size,
            Wav => config.wav_stream_size = args.stream_size,
            Flac => config.flac_stream_size = args.stream_size,
            Rf64 => config.rf64_stream_size = args.stream_size,
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
        .stack_size(4 * 1024 * 1024)
        .spawn(move || {
            run_server(
                &local_addr,
                server_port.unwrap_or_default(),
                wd,
                &feedback_tx,
            );
        })
        .unwrap();

    // we may have to translate player names to IP addresses
    if !serve_only && (args.player_ip.is_some() || config.last_renderer.is_some()) {
        // give the webserver a chance to start and wait for ssdp to complete
        thread::sleep(Duration::from_secs(5));
        // get the results of the ssdp discovery
        let mut n = 0;
        while let Ok(msg) = msg_rx.try_recv() {
            match msg {
                MessageType::SsdpMessage(newr) => {
                    renderers.push(newr.clone());
                    ui_log(&format!(
                        "Available renderer #{n}: {} at {}",
                        newr.dev_name, newr.remote_addr
                    ));
                    n += 1;
                }
                MessageType::PlayerMessage(_) => (),
                MessageType::LogMessage(_) => (),
            }
        }
        // now check for player names(s) instead of ip addresses
        if args.player_ip.is_some() {
            if let Some(r) = renderers
                .iter()
                .find(|r| r.dev_name.contains(args.player_ip.as_ref().unwrap()))
            {
                ui_log(&format!(
                    "Default renderer ip: {} => {}",
                    args.player_ip.as_ref().unwrap(),
                    r.remote_addr
                ));
                args.player_ip = Some(r.remote_addr.clone());
            }
        }
        if args.active_players.is_some() {
            let mut ip_players: Vec<String> = Vec::new();
            args.active_players.as_ref().unwrap().iter().for_each(|ap| {
                if let Some(r) = renderers.iter().find(|r| r.dev_name.contains(ap)) {
                    ip_players.push(r.remote_addr.clone());
                    ui_log(&format!("Active renderer: {ap} => {} ", r.remote_addr));
                }
            });
            if !ip_players.is_empty() {
                args.active_players = Some(ip_players);
            }
        }
    }

    // set args last_renderer and active players
    if let Some(player_ip) = args.player_ip {
        config.last_renderer = Some(player_ip);
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
        let mut conf = CONFIG.write();
        *conf = config.clone();
    }

    let mut player: Option<Renderer> = None;
    // select the player unless only serving
    if !serve_only {
        let last_renderer = config.last_renderer.as_ref().unwrap();
        if renderers.is_empty() {
            error!("No renderers found!!!");
            return Err(-1);
        }
        // default = first player
        player = Some(renderers[0].clone());
        // but use the configured renderer if present
        if let Some(pl) = renderers
            .iter()
            .find(|&renderer| renderer.remote_addr == *last_renderer)
        {
            player = Some(pl.clone());
        }
        // if specified player ip not found: use default player
        if *last_renderer != player.as_ref().unwrap().remote_addr {
            config.last_renderer = Some(player.as_ref().unwrap().remote_addr.clone());
        }
        ui_log(&format!(
            "Default player ip = {}",
            player.as_ref().unwrap().remote_addr
        ));
    }

    // update config with new args
    let _ = config.update_config();
    // update in_memory shared config for other threads
    {
        let mut conf = CONFIG.write();
        *conf = config.clone();
    }

    info!("New config: {config:?}");

    // exit here if dry-run
    if args.dry_run.is_some() {
        ui_log("dry-run - exiting...");
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
    if serve_only {
        let port = config.server_port.unwrap_or(5901);
        ui_log(&format!("Serving started on port {port}..."));
    } else {
        for ip in config.active_renderers {
            if let Some(pl) = renderers
                .iter()
                .find(|&renderer| renderer.remote_addr == ip)
            {
                let mut player = pl.clone();
                if let Some(vol) = args.volume {
                    if player.get_volume(&ui_log) > -1 {
                        player.set_volume(&ui_log, vol.into());
                    }
                }
                let _ = player.play(
                    &local_addr,
                    config.server_port.unwrap_or(5901),
                    &ui_log,
                    streaminfo,
                );
                let pl_name = &player.dev_url;
                ui_log(&format!("Playing to {pl_name}"));
            }
        }
    }

    loop {
        while let Ok(msg) = msg_rx.try_recv() {
            match msg {
                MessageType::SsdpMessage(newr) => {
                    if !serve_only {
                        renderers.push(newr.clone());
                        ui_log(&format!(
                            "New renderer {} at {}",
                            newr.dev_name, newr.remote_addr
                        ));
                    }
                }
                MessageType::PlayerMessage(streamer_feedback) => {
                    match streamer_feedback.streaming_state {
                        StreamingState::Started => {}
                        StreamingState::Ended => {
                            if !serve_only {
                                // first check if the renderer has actually not started streaming again
                                // as this can happen with Bubble/Nest Audio Openhome
                                let still_streaming = CLIENTS.read().values().any(|chanstrm| {
                                    chanstrm.remote_ip == streamer_feedback.remote_ip
                                });
                                if !still_streaming {
                                    let config = CONFIG.read().clone();
                                    if config.auto_resume {
                                        if let Some(r) = renderers
                                            .iter()
                                            .find(|r| r.remote_addr == streamer_feedback.remote_ip)
                                        {
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
                                                server_port.unwrap_or_default(),
                                                &ui_log,
                                                streaminfo,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                MessageType::LogMessage(msg) => ui_log(&msg),
            }
        }
        // check the logchannel for new log messages to show in the logger textbox
        thread::sleep(Duration::from_millis(100));
        // handle CTL-C interrupt: shutdown the player
        if shutting_down.load(Ordering::Relaxed) {
            println!("Received ^C -> exiting.");
            if !serve_only && player.is_some() && CLIENTS.read().len() > 0 {
                let pl = player.unwrap().clone();
                println!("^C: Stopping streaming to {}", pl.dev_name);
                pl.stop_play(&ui_log);
                // also wait some time for the player to drop the HTTP streaming connection
                println!(
                    "^C: Waiting for HTTP streaming to {} to end.",
                    pl.remote_addr
                );
                for _ in 0..100 {
                    if CLIENTS.read().len() == 0 {
                        println!(
                            "^C: HTTP streaming connection with {} closed.",
                            pl.remote_addr
                        );
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                if CLIENTS.read().len() > 0 {
                    println!("^C: Time-out waiting for HTTP streaming shutdown - exiting.");
                }
            }
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
    loop {
        let renderers = discover(&rmap, &ui_log).unwrap_or_default();
        for r in &renderers {
            rmap.entry(r.remote_addr.clone()).or_insert_with(|| {
                info!(
                    "Found new renderer {} {}  at {}",
                    r.dev_name, r.dev_model, r.remote_addr
                );
                ssdp_tx.send(MessageType::SsdpMessage(r.clone())).unwrap();
                thread::yield_now();
                r.clone()
            });
        }
        thread::sleep(Duration::from_millis(
            (ssdp_interval_mins * 60.0 * 1000.0) as u64,
        ));
    }
}
