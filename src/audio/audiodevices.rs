use crate::{
    globals::statics::{CLIENTS, CONFIG},
    utils::ui_logger::ui_log,
};
use cpal::{
    traits::{DeviceTrait, HostTrait},
    DefaultStreamConfigError, Sample, SupportedStreamConfig,
};
use crossbeam_channel::Sender;
use dasp_sample::ToSample;
use log::debug;
use parking_lot::Once;
use rust_i18n::t;

pub struct Device {
    kind: DeviceKind,
    name: String,
    stream_config: SupportedStreamConfig,
}

pub enum DeviceKind {
    Input(cpal::Device),
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
    pub fn from_device(device: cpal::Device) -> Result<Self, DefaultStreamConfigError> {
        let name = device.name().unwrap_or_else(|e| {
            debug!("Unable to retrieve device name due to:\n\t{e}");
            "Unknown/unnamed".into()
        });

        let (kind, stream_config) = if let Ok(conf) = device.default_output_config() {
            debug!("    Default output stream config:\n      {:?}", conf);
            (DeviceKind::Output(device), conf)
        } else {
            let conf = device.default_input_config()?;
            debug!("    Default input stream config:\n      {:?}", conf);
            (DeviceKind::Input(device), conf)
        };

        Ok(Self {
            kind,
            name,
            stream_config,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn default_config(&self) -> &SupportedStreamConfig {
        &self.stream_config
    }
}

impl AsRef<cpal::Device> for Device {
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

fn log_stream_configs(
    configs: Result<
        impl Iterator<Item = cpal::SupportedStreamConfigRange>,
        cpal::SupportedStreamConfigsError,
    >,
    cfg_type: &str,
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
            debug!("Error retrieving {cfg_type} stream configs: {:?}", e);
        }
    };
}

#[must_use]
pub fn get_output_audio_devices() -> Vec<Device> {
    let mut result = Vec::new();
    debug!("Supported hosts:\n  {:?}", cpal::ALL_HOSTS);
    let available_hosts = cpal::available_hosts();
    debug!("Available hosts:\n  {:?}", available_hosts);

    for host_id in available_hosts {
        debug!("{}", host_id.name());
        let host = cpal::host_from_id(host_id).unwrap();

        let default_out = host.default_output_device().and_then(|e| e.name().ok());
        debug!("  Default Output Device:\n    {:?}", default_out);

        let default_in = host.default_input_device().and_then(|e| e.name().ok());
        debug!("  Default Input Device:\n    {:?}", default_in);

        let devices = host.devices().unwrap();
        debug!("  Devices: ");
        for (device_index, device) in devices.enumerate() {
            debug!(
                "  {}. \"{}\"",
                device_index + 1,
                device.name().unwrap_or_default()
            );
            log_stream_configs(device.supported_output_configs(), "output", device_index);
            log_stream_configs(device.supported_input_configs(), "input", device_index);
            if let Ok(device) = Device::from_device(device) {
                result.push(device);
            } else {
                debug!("  Device seems to not support either input or output.");
            }
        }
    }

    result
}

#[must_use]
pub fn get_default_audio_output_device() -> Option<Device> {
    let _available_hosts = cpal::available_hosts();
    let default_host = cpal::default_host();
    default_host
        .default_output_device()
        .and_then(|device| DeviceKind::Output(device).try_into().ok())
}

pub fn capture_output_audio(
    device_wrap: &Device,
    rms_sender: Sender<Vec<f32>>,
) -> Option<cpal::Stream> {
    let device = device_wrap.as_ref();
    ui_log(&format!(
        "{} {}",
        t!("capturing_audio_from"),
        device
            .name()
            .expect("Could not get default audio device name")
    ));
    let audio_cfg = device_wrap
        .kind
        .default_config_any()
        .expect("No default stream config found");
    ui_log(&format!("{} {:?}", t!("default_audio_config"), audio_cfg));
    let mut f32_samples: Vec<f32> = Vec::with_capacity(16384);
    match audio_cfg.sample_format() {
        cpal::SampleFormat::F32 => match device.build_input_stream(
            &audio_cfg.config(),
            move |data, _: &_| wave_reader::<f32>(data, &mut f32_samples, &rms_sender),
            capture_err_fn,
            None,
        ) {
            Ok(stream) => Some(stream),
            Err(e) => {
                ui_log(&format!("{} f32: {e}", t!("error_capturing_audio_stream")));
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
                Ok(stream) => Some(stream),
                Err(e) => {
                    ui_log(&format!("{} i16: {e}", t!("error_capturing_audio_stream")));
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
                Ok(stream) => Some(stream),
                Err(e) => {
                    ui_log(&format!("{} u16: {e}", t!("error_capturing_audio_stream")));
                    None
                }
            }
        }
        _ => None,
    }
}

fn capture_err_fn(err: cpal::StreamError) {
    ui_log(&format!("{} {err}", t!("error_building_audio_input_stream")));
}

fn wave_reader<T>(samples: &[T], f32_samples: &mut Vec<f32>, rms_sender: &Sender<Vec<f32>>)
where
    T: Sample + ToSample<f32>,
{
    static INITIALIZER: Once = Once::new();
    INITIALIZER.call_once(|| {
        ui_log(&*t!("wave_reader_receiving"));
    });
    f32_samples.clear();
    f32_samples.extend(samples.iter().map(|x: &T| T::to_sample::<f32>(*x)));
    CLIENTS
        .read()
        .iter()
        .for_each(|(_, client)| client.write(f32_samples));
    if CONFIG.read().monitor_rms {
        rms_sender.send(Vec::from(f32_samples.as_slice())).unwrap();
    }
}