#![cfg(feature = "cli")]
use std::{collections::HashMap, fs::File, net::IpAddr, path::Path, thread, time::Duration};

use cpal::traits::StreamTrait;
use crossbeam_channel::{unbounded, Receiver, Sender};
use log::{debug, error, info, LevelFilter};
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, WriteLogger};
use swyh_rs::{
    enums::streaming::{
        StreamingFormat::{Flac, Lpcm, Rf64, Wav},
        StreamingState,
    },
    globals::statics::{APP_VERSION, CLIENTS, CONFIG, LOGCHANNEL},
    openhome::rendercontrol::{discover, Renderer, StreamInfo, WavData},
    server::streaming_server::{run_server, StreamerFeedBack},
    utils::{
        audiodevices::{
            capture_output_audio, get_default_audio_output_device, get_output_audio_devices,
        },
        bincommon::run_silence_injector,
        commandline::Args,
        local_ip_address::{get_interfaces, get_local_addr},
        priority::raise_priority,
        ui_logger::ui_log,
    },
};

pub const APP_NAME: &str = "SWYH-RS-CLI";

fn main() -> Result<(), i32> {
    // gracefully exit on Ctrl-C
    ctrlc::set_handler(move || {
        println!("Received Ctrl+C -> exiting.");
        std::process::exit(0);
    })
    .expect("Error setting Ctrl-C handler");

    // collect command line arguments
    let args = Args::new().parse();
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
    // autoreconnect is ignored but effectively always on
    config.auto_reconnect = true;
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
        config.last_network = Some(ip.parse().unwrap());
    }
    // get the local network network address
    let local_addr: IpAddr = {
        if let Some(ref network) = config.last_network {
            info!("Using network {}", network);
            network.parse().unwrap()
        } else {
            let addr = get_local_addr().expect("Could not obtain local address.");
            config.last_network = Some(addr.to_string());
            info!("Using network {}", addr);
            addr
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
    if let Some(minutes) = args.ssdp_interval_mins {
        config.ssdp_interval_mins = minutes;
    }

    // update config with new args
    let _ = config.update_config();
    // update in_memory shared config for other threads
    {
        let mut conf = CONFIG.write();
        *conf = config.clone();
    }

    let (ssdp_tx, ssdp_rx): (Sender<Renderer>, Receiver<Renderer>) = unbounded();
    let mut renderers: Vec<Renderer> = Vec::new();

    let mut serve_only = args.serve_only.unwrap_or(false);
    // if only serving: no ssdp discovery
    if !serve_only || args.dry_run.is_some() {
        // now start the SSDP discovery update thread with a Crossbeam channel for renderer updates
        // the discovered renderers will be kept in this list
        ui_log("Discover networks");
        ui_log("Starting SSDP discovery");
        let ssdp_int = config.ssdp_interval_mins;
        let _ = thread::Builder::new()
            .name("ssdp_updater".into())
            .stack_size(4 * 1024 * 1024)
            .spawn(move || run_ssdp_updater(&ssdp_tx, ssdp_int))
            .unwrap();
    }
    // set args player
    if let Some(player_ip) = args.player_ip {
        config.last_renderer = Some(player_ip);
    }
    // if no player specified: switch to serve mode
    if config.last_renderer.is_none() {
        serve_only = true;
    }
    // set args streaming format
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
        config.use_wave_format = [Some(Wav), Some(Rf64)].contains(&config.streaming_format);
    }

    if args.streaming_format.is_some() && args.stream_size.is_some() {
        match args.streaming_format.unwrap() {
            Lpcm => config.lpcm_stream_size = args.stream_size,
            Wav => config.wav_stream_size = args.stream_size,
            Flac => config.flac_stream_size = args.stream_size,
            Rf64 => config.rf64_stream_size = args.stream_size,
        }
    }

    // update config with new args
    let _ = config.update_config();
    // update in_memory shared config for other threads
    {
        let mut conf = CONFIG.write();
        *conf = config.clone();
    }

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
                &feedback_tx,
            );
        })
        .unwrap();

    if !serve_only || args.dry_run.is_some() {
        // give the webserver a chance to start and wait for ssdp to complete
        thread::sleep(Duration::from_secs(5));
        // get the results of the ssdp discovery
        let mut n = 0;
        while let Ok(newr) = ssdp_rx.try_recv() {
            renderers.push(newr.clone());
            ui_log(&format!(
                "Available renderer #{n}: {} at {}",
                newr.dev_name, newr.remote_addr
            ));
            n += 1;
        }
    }

    let mut player: Option<&Renderer> = None;
    // select the player unless only serving
    if !serve_only {
        let last_renderer = config.last_renderer.as_ref().unwrap();
        if renderers.is_empty() {
            error!("No renderers found!!!");
            return Err(-1);
        }
        // default = first player
        player = Some(&renderers[0]);
        // but use the configured renderer if present
        if let Some(pl) = renderers
            .iter()
            .find(|&renderer| renderer.remote_addr == *last_renderer)
        {
            player = Some(pl);
        }
        // if specified player ip not found: use default player
        if *last_renderer != player.unwrap().remote_addr {
            config.last_renderer = Some(player.unwrap().remote_addr.clone());
        }
        ui_log(&format!(
            "Selected player with ip = {}",
            player.unwrap().remote_addr
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

    // get the logreader channel
    let logreader = &LOGCHANNEL.read().1;

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
        let mut pl = player.unwrap().clone();
        if let Some(vol) = args.volume {
            if pl.get_volume(&ui_log) > -1 {
                pl.set_volume(&ui_log, vol.into());
            }
        }
        let _ = pl.play(
            &local_addr,
            config.server_port.unwrap_or(5901),
            &ui_log,
            streaminfo,
        );
        let pl_name = &pl.dev_url;
        ui_log(&format!("Playing to {pl_name}"));
    }

    loop {
        while let Ok(streamer_feedback) = feedback_rx.try_recv() {
            match streamer_feedback.streaming_state {
                StreamingState::Started => {}
                StreamingState::Ended => {
                    if !serve_only {
                        // first check if the renderer has actually not started streaming again
                        // as this can happen with Bubble/Nest Audio Openhome
                        let still_streaming = CLIENTS
                            .read()
                            .values()
                            .any(|chanstrm| chanstrm.remote_ip == streamer_feedback.remote_ip);
                        if !still_streaming {
                            let config = CONFIG.read().clone();
                            if config.auto_resume {
                                if let Some(r) = renderers
                                    .iter()
                                    .find(|r| r.remote_addr == streamer_feedback.remote_ip)
                                {
                                    let streaminfo = StreamInfo {
                                        sample_rate: wd.sample_rate.0,
                                        bits_per_sample: config.bits_per_sample.unwrap_or(16),
                                        streaming_format: config.streaming_format.unwrap_or(Flac),
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
        // check the logchannel for new log messages to show in the logger textbox
        while let Ok(msg) = logreader.try_recv() {
            ui_log(&msg);
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// run the `ssdp_updater` - thread that periodically run ssdp discovery
/// and detect new renderers
/// send any new renderers to te main thread on the Crossbeam ssdp channel
fn run_ssdp_updater(ssdp_tx: &Sender<Renderer>, ssdp_interval_mins: f64) {
    // the hashmap used to detect new renderers
    let mut rmap: HashMap<String, Renderer> = HashMap::new();
    loop {
        let renderers = discover(&rmap, &ui_log).unwrap_or_default();
        for r in &renderers {
            rmap.entry(r.remote_addr.clone()).or_insert_with(|| {
                let _ = ssdp_tx.send(r.clone());
                thread::yield_now();
                info!(
                    "Found new renderer {} {}  at {}",
                    r.dev_name, r.dev_model, r.remote_addr
                );
                r.clone()
            });
        }
        thread::sleep(Duration::from_millis(
            (ssdp_interval_mins * 60.0 * 1000.0) as u64,
        ));
    }
}
