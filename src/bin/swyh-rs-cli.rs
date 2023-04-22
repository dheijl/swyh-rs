use std::{collections::HashMap, fs::File, net::IpAddr, path::Path, thread, time::Duration};

use cpal::{
    traits::{DeviceTrait, StreamTrait},
    Sample,
};
use crossbeam_channel::{unbounded, Receiver, Sender};
use log::{debug, info, LevelFilter};
use simplelog::{ColorChoice, CombinedLogger, Config, TermLogger, WriteLogger};
use swyh_rs::{
    enums::streaming::StreamingState,
    globals::statics::{APP_NAME, APP_VERSION, CLIENTS, CONFIG, LOGCHANNEL},
    openhome::rendercontrol::{discover, Renderer, StreamInfo, WavData},
    server::streaming_server::{run_server, StreamerFeedBack},
    utils::{
        audiodevices::{
            capture_output_audio, get_default_audio_output_device, get_output_audio_devices,
        },
        local_ip_address::{get_interfaces, get_local_addr},
        priority::raise_priority,
        ui_logger::{disable_ui_log, ui_log},
    },
};

fn main() {
    // tell everyone we're running without UI
    disable_ui_log();
    // first initialize cpal audio to prevent COM reinitialize panic on Windows
    let mut audio_output_device =
        get_default_audio_output_device().expect("No default audio device");

    // initialize config
    let mut config = {
        let mut conf = CONFIG.write();
        if conf.sound_source == "None" {
            conf.sound_source = audio_output_device.name().unwrap();
            let _ = conf.update_config();
        }
        conf.clone()
    };
    if let Some(config_id) = &config.config_id {
        if !config_id.is_empty() {
            ui_log(format!("Loaded configuration -c {config_id}"));
        }
    }
    config.monitor_rms = false;
    ui_log(format!("{config:?}"));
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
        "{} V {} - Logging started.",
        APP_NAME.to_string(),
        APP_VERSION.to_string()
    );
    if cfg!(debug_assertions) {
        ui_log("*W*W*>Running DEBUG build => log level set to DEBUG!".to_string());
    }
    info!("Config: {:?}", config);

    // get the output device from the config and get all available audio source names
    let audio_devices = get_output_audio_devices().unwrap();
    let mut source_names: Vec<String> = Vec::new();
    for (index, adev) in audio_devices.into_iter().enumerate() {
        let devname = adev.name().unwrap();
        ui_log(format!(
            "Found Audio Source: index = {index}, name = {devname}"
        ));
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
    let audio_cfg = &audio_output_device
        .default_output_config()
        .expect("No default output config found");
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
    match capture_output_audio(&audio_output_device, rms_channel.0) {
        Some(s) => {
            stream = s;
            stream.play().unwrap();
        }
        None => {
            ui_log("*E*E*> Could not capture audio ...Please check configuration.".to_string());
        }
    }

    // If silence injector is on, start the "silence_injector" thread
    if let Some(true) = CONFIG.read().inject_silence {
        let _ = thread::Builder::new()
            .name("silence_injector".into())
            .stack_size(4 * 1024 * 1024)
            .spawn(move || run_silence_injector(&audio_output_device))
            .unwrap();
    }

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
    // give the webserver a chance to start and wait for ssdp to complete
    thread::sleep(Duration::from_secs(5));

    // get the results of the ssdp discovery
    let mut n = 0;
    while let Ok(newr) = ssdp_rx.try_recv() {
        renderers.push(newr.clone());
        ui_log(format!(
            "Renderer #{n}: {} at {}",
            newr.dev_name, newr.remote_addr
        ));
        n += 1;
    }

    // get the logreader channel
    let logreader = &LOGCHANNEL.read().1;

    loop {
        while let Ok(streamer_feedback) = feedback_rx.try_recv() {
            match streamer_feedback.streaming_state {
                StreamingState::Started => {}
                StreamingState::Ended => {
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
                        }
                    }
                }
            }
        }
        // check the logchannel for new log messages to show in the logger textbox
        while let Ok(msg) = logreader.try_recv() {
            ui_log(msg);
        }
        thread::sleep(Duration::from_millis(10));
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

///
/// inject silence into the audio stream to solve problems with Sonos when pusing audio
/// contributed by @genekellyjr, see issue #71
///
fn run_silence_injector(audio_output_device: &cpal::Device) {
    // straight up copied from cpal docs cause I don't know syntax or anything
    let mut supported_configs_range = audio_output_device
        .supported_output_configs()
        .expect("error while querying configs");
    let supported_config = supported_configs_range
        .next()
        .expect("no supported config?!")
        .with_max_sample_rate();
    let err_fn = |err| eprintln!("an error occurred on the output audio stream: {err}");
    let config = supported_config.into();

    // CPAL 0.15 switched to dasp_sample:
    // see https://github.com/RustAudio/cpal/commit/85d773d59f1725b25002c6f04aa2eb9b43a75b76#diff-babb62f9985b4798a655658e440a565984ce15b25e63a82fc4b3cc0b54fd2a02
    fn write_silence<T: Sample>(data: &mut [T], _: &cpal::OutputCallbackInfo) {
        for sample in data.iter_mut() {
            *sample = Sample::EQUILIBRIUM;
        }
    }
    let stream = audio_output_device
        .build_output_stream(&config, write_silence::<f32>, err_fn, None)
        .unwrap();
    stream.play().unwrap();

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
