use crate::{ui_log, CLIENTS, CONFIG};
use cpal::traits::{DeviceTrait, HostTrait};
use crossbeam_channel::Sender;
use log::debug;
use parking_lot::Once;

pub fn get_output_audio_devices() -> Option<Vec<cpal::Device>> {
    let mut result: Vec<cpal::Device> = Vec::new();
    debug!("Supported hosts:\n  {:?}", cpal::ALL_HOSTS);
    let available_hosts = cpal::available_hosts();
    debug!("Available hosts:\n  {:?}", available_hosts);

    for host_id in available_hosts {
        debug!("{}", host_id.name());
        let host = cpal::host_from_id(host_id).unwrap();

        let default_out = host.default_output_device().map(|e| e.name().unwrap());
        debug!("  Default Output Device:\n    {:?}", default_out);

        let devices = host.devices().unwrap();
        debug!("  Devices: ");
        for (device_index, device) in devices.enumerate() {
            debug!("  {}. \"{}\"", device_index + 1, device.name().unwrap());

            // Output configs
            let mut output_configs = match device.supported_output_configs() {
                Ok(f) => f.peekable(),
                Err(e) => {
                    debug!("Error: {:?}", e);
                    continue;
                }
            };
            if output_configs.peek().is_some() {
                debug!("    All supported output stream configs:");
                for (config_index, config) in output_configs.enumerate() {
                    debug!(
                        "      {}.{}. {:?}",
                        device_index + 1,
                        config_index + 1,
                        config
                    );
                }
            }
            // use only device with default config
            if let Ok(conf) = device.default_output_config() {
                debug!("    Default output stream config:\n      {:?}", conf);
                result.push(device);
            }
        }
    }

    Some(result)
}

pub fn get_default_audio_output_device() -> Option<cpal::Device> {
    // audio hosts
    let _available_hosts = cpal::available_hosts();
    let default_host = cpal::default_host();
    default_host.default_output_device()
}

/// capture_audio_output - capture the audio stream from the default audio output device
///
/// sets up an input stream for the wave_reader in the appropriate format (f32/i16/u16)
pub fn capture_output_audio(
    device: &cpal::Device,
    rms_sender: Sender<Vec<f32>>,
) -> Option<cpal::Stream> {
    ui_log(format!(
        "Capturing audio from: {}",
        device
            .name()
            .expect("Could not get default audio device name")
    ));
    let audio_cfg = device
        .default_output_config()
        .expect("No default output config found");
    ui_log(format!("Default audio {audio_cfg:?}"));
    let mut f32_samples: Vec<f32> = Vec::with_capacity(16384);
    match audio_cfg.sample_format() {
        cpal::SampleFormat::F32 => match device.build_input_stream(
            &audio_cfg.config(),
            move |data, _: &_| wave_reader::<f32>(data, &mut f32_samples, rms_sender.clone()),
            capture_err_fn,
        ) {
            Ok(stream) => Some(stream),
            Err(e) => {
                ui_log(format!("Error capturing f32 audio stream: {e}"));
                None
            }
        },
        cpal::SampleFormat::I16 => {
            match device.build_input_stream(
                &audio_cfg.config(),
                move |data, _: &_| wave_reader::<i16>(data, &mut f32_samples, rms_sender.clone()),
                capture_err_fn,
            ) {
                Ok(stream) => Some(stream),
                Err(e) => {
                    ui_log(format!("Error capturing i16 audio stream: {e}"));
                    None
                }
            }
        }
        cpal::SampleFormat::U16 => {
            match device.build_input_stream(
                &audio_cfg.config(),
                move |data, _: &_| wave_reader::<u16>(data, &mut f32_samples, rms_sender.clone()),
                capture_err_fn,
            ) {
                Ok(stream) => Some(stream),
                Err(e) => {
                    ui_log(format!("Error capturing u16 audio stream: {e}"));
                    None
                }
            }
        }
    }
}

/// capture_err_fn - called whan it's impossible to build an audio input stream
fn capture_err_fn(err: cpal::StreamError) {
    ui_log(format!("Error {err} building audio input stream"));
}

/// wave_reader - the captured audio input stream reader
///
/// writes the captured samples to all registered clients in the
/// CLIENTS ChannnelStream hashmap
/// also feeds the RMS monitor channel if the RMS option is set
fn wave_reader<T>(samples: &[T], f32_samples: &mut Vec<f32>, rms_sender: Sender<Vec<f32>>)
where
    T: cpal::Sample,
{
    static INITIALIZER: Once = Once::new();
    INITIALIZER.call_once(|| {
        ui_log("The wave_reader is now receiving samples".to_string());
    });
    f32_samples.clear();
    f32_samples.extend(samples.iter().map(|x| x.to_f32()));
    for (_, v) in CLIENTS.read().iter() {
        v.write(f32_samples);
    }
    if CONFIG.read().monitor_rms {
        rms_sender.send(f32_samples.to_vec()).unwrap();
    }
}
