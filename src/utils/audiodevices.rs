use crate::{
    globals::statics::{RUN_RMS_MONITOR, get_clients, get_config},
    utils::ui_logger::ui_log,
};
use cpal::{
    DefaultStreamConfigError, Sample, SupportedStreamConfig,
    traits::{DeviceTrait, HostTrait},
};
use crossbeam_channel::Sender;
use dasp_sample::ToSample;
use log::debug;
use std::sync::{Once, atomic::Ordering};

/// A [`cpal::Device`] with either a default input or default output config.
///
/// The internal device may be retrieved via [`AsRef::as_ref`].
pub struct Device {
    /// Indicates if `cpal::Device` is primarily output or input.
    kind: DeviceKind,
    /// Device name as reported by the backend.
    name: String,
    /// Default stream config based on [`DeviceKind`].
    stream_config: SupportedStreamConfig,
}

/// Type indicating whether a device is being treated as input or output.
pub enum DeviceKind {
    /// An input device such as S/PDIF in or a microphone.
    Input(cpal::Device),
    /// An output device such as speakers.
    Output(cpal::Device),
}

impl AsRef<cpal::Device> for DeviceKind {
    #[inline]
    fn as_ref(&self) -> &cpal::Device {
        match self {
            Self::Input(device) | Self::Output(device) => device,
        }
    }
}

impl DeviceKind {
    /// Returns the default [`cpal::SupportedStreamConfig`] regardless of device type.
    #[inline]
    pub fn default_config_any(
        &self,
    ) -> Result<cpal::SupportedStreamConfig, cpal::DefaultStreamConfigError> {
        match self {
            DeviceKind::Input(device) => device.default_input_config(),
            DeviceKind::Output(device) => device.default_output_config(),
        }
    }
}

impl Device {
    /// Construct a [`Device`] from a [`cpal::Device`].
    ///
    /// Devices may support both input and output.
    /// This defaults to output if both are present on one device.
    pub fn from_device(device: cpal::Device) -> Result<Self, DefaultStreamConfigError> {
        let name = device.name().unwrap_or_else(|e| {
            debug!("Unable to retrieve device name due to:\n\t{e}");
            "Unknown/unnamed".into()
        });

        // Only use the default config for output or input
        // Prefer output if a device supports both
        let (kind, stream_config) = if let Ok(conf) = device.default_output_config() {
            debug!("    Default output stream config:\n      {conf:?}");
            (DeviceKind::Output(device), conf)
        } else {
            // If there's no output AND no input then return the error.
            let conf = device.default_input_config()?;
            debug!("    Default input stream config:\n      {conf:?}");
            (DeviceKind::Input(device), conf)
        };
        Ok(Self {
            kind,
            name,
            stream_config,
        })
    }

    /// Device name as reported by the operating system, or a reasonable default if the
    /// name can't be retrieved.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Default stream config
    #[must_use]
    pub fn default_config(&self) -> &SupportedStreamConfig {
        &self.stream_config
    }
}

impl AsRef<cpal::Device> for Device {
    /// Reference to the internal [`cpal::Device`].
    fn as_ref(&self) -> &cpal::Device {
        self.kind.as_ref()
    }
}

impl TryFrom<DeviceKind> for Device {
    type Error = DefaultStreamConfigError;

    #[inline]
    fn try_from(kind: DeviceKind) -> Result<Self, Self::Error> {
        let name = kind
            .as_ref()
            .name()
            .unwrap_or_else(|_| "Unknown/unnamed".into());
        let stream_config = kind.default_config_any()?;
        Ok(Self {
            kind,
            name,
            stream_config,
        })
    }
}

/// Log all supported stream configs for both input and output devices.
fn log_stream_configs(
    // Iterator returned by [cpal::Device::supported_input_configs] or [cpal::Device::supported_output_configs].
    configs: Result<
        impl Iterator<Item = cpal::SupportedStreamConfigRange>,
        cpal::SupportedStreamConfigsError,
    >,
    // "output" or "input"
    cfg_type: &str,
    // Device index in relation to the iterator returned by [cpal::Host::devices]
    device_index: usize,
) {
    match configs {
        Ok(configs) => {
            let mut configs = configs.peekable();
            if configs.peek().is_some() {
                debug!("    All supported {cfg_type} stream configs:");
                for (config_index, config) in configs.enumerate() {
                    debug!(
                        "      {}.{}. {:?}",
                        device_index + 1,
                        config_index + 1,
                        config
                    );
                }
            }
        }
        Err(e) => {
            debug!("Error retrieving {cfg_type} stream configs: {e:?}");
        }
    }
}

#[must_use]
pub fn get_output_audio_devices() -> Vec<Device> {
    let mut result = Vec::new();
    debug!("Supported hosts:\n  {:?}", cpal::ALL_HOSTS);
    let available_hosts = cpal::available_hosts();
    debug!("Available hosts:\n  {available_hosts:?}");

    for host_id in available_hosts {
        debug!("{}", host_id.name());
        let host = cpal::host_from_id(host_id).unwrap();

        let default_out = host.default_output_device().and_then(|e| e.name().ok());
        debug!("  Default Output Device:\n    {default_out:?}");

        let default_in = host.default_input_device().and_then(|e| e.name().ok());
        debug!("  Default Input Device:\n    {default_in:?}");

        let devices = host.devices().unwrap();
        debug!("  Devices: ");
        for (device_index, device) in devices.enumerate() {
            debug!(
                "  {}. \"{}\"",
                device_index + 1,
                device.name().unwrap_or_default()
            );
            // List all of the supported stream configs per device.
            log_stream_configs(device.supported_output_configs(), "output", device_index);
            log_stream_configs(device.supported_input_configs(), "input", device_index);
            match Device::from_device(device) {
                Ok(device) => {
                    result.push(device);
                }
                _ => {
                    debug!("  Device seems to not support either input or output.");
                }
            }
        }
    }

    result
}

#[must_use]
pub fn get_default_audio_output_device() -> Option<Device> {
    // audio hosts
    let _available_hosts = cpal::available_hosts();
    let default_host = cpal::default_host();
    default_host
        .default_output_device()
        .and_then(|device| DeviceKind::Output(device).try_into().ok())
}

/// `capture_audio_output` - capture the audio stream from the default audio output device
///
/// sets up an input stream for the `wave_reader` in the appropriate format (f32/i16/u16)
pub fn capture_output_audio(
    device_wrap: &Device,
    rms_sender: Sender<Vec<f32>>,
) -> Option<cpal::Stream> {
    let device = device_wrap.as_ref();
    ui_log(&format!(
        "Capturing audio from: {}",
        device
            .name()
            .expect("Could not get default audio device name")
    ));
    let audio_cfg = device_wrap
        .kind
        .default_config_any()
        .expect("No default stream config found");
    ui_log(&format!("Default audio {audio_cfg:?}"));
    let mut f32_samples: Vec<f32> = Vec::with_capacity(16384);
    match audio_cfg.sample_format() {
        cpal::SampleFormat::F32 => match device.build_input_stream(
            &audio_cfg.config(),
            move |data, _: &_| wave_reader::<f32>(data, &mut f32_samples, &rms_sender),
            capture_err_fn,
            None,
        ) {
            Ok(stream) => {
                ui_log("Audio capture sample format = F32");
                Some(stream)
            }
            Err(e) => {
                ui_log(&format!("Error capturing f32 audio stream: {e}"));
                None
            }
        },
        cpal::SampleFormat::I16 => {
            match device.build_input_stream(
                &audio_cfg.config(),
                move |data, _: &_| wave_reader::<i16>(data, &mut f32_samples, &rms_sender),
                capture_err_fn,
                None,
            ) {
                Ok(stream) => {
                    ui_log("Audio capture sample format = I16");
                    Some(stream)
                }
                Err(e) => {
                    ui_log(&format!("Error capturing i16 audio stream: {e}"));
                    None
                }
            }
        }
        cpal::SampleFormat::U16 => {
            match device.build_input_stream(
                &audio_cfg.config(),
                move |data, _: &_| wave_reader::<u16>(data, &mut f32_samples, &rms_sender),
                capture_err_fn,
                None,
            ) {
                Ok(stream) => {
                    ui_log("Audio capture sample format = U16");
                    Some(stream)
                }
                Err(e) => {
                    ui_log(&format!("Error capturing u16 audio stream: {e}"));
                    None
                }
            }
        }
        _ => None,
    }
}

/// `capture_err_fn` - called whan it's impossible to build an audio input stream
fn capture_err_fn(err: cpal::StreamError) {
    ui_log(&format!("Error {err} building audio input stream"));
}

/// `wave_reader` - the captured audio input stream reader
///
/// writes the captured samples to all registered clients in the
/// CLIENTS `ChannnelStream` hashmap
/// also feeds the RMS monitor channel if the RMS option is set
fn wave_reader<T>(samples: &[T], f32_samples: &mut Vec<f32>, rms_sender: &Sender<Vec<f32>>)
where
    T: Sample + ToSample<f32>,
{
    static ONFIRSTCALL: Once = Once::new();
    ONFIRSTCALL.call_once(|| {
        ui_log("The wave_reader is now receiving samples");
        if get_config().monitor_rms {
            RUN_RMS_MONITOR.store(true, Ordering::Relaxed);
        }
    });
    f32_samples.clear();
    f32_samples.extend(samples.iter().map(|x: &T| T::to_sample::<f32>(*x)));
    #[cfg(feature = "trace_samples")]
    {
        let zs = if f32_samples.iter().all(|&s| s == 0.0) {
            "allzero"
        } else {
            "nonzero"
        };
        debug!("wave_reader: got {} {zs} samples", f32_samples.len());
    }
    get_clients()
        .iter()
        .for_each(|(_, client)| client.write(f32_samples));
    if RUN_RMS_MONITOR.load(Ordering::Acquire) {
        rms_sender.send(Vec::from(f32_samples.as_slice())).unwrap();
    }
}
