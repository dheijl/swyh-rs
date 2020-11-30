use cpal::traits::{DeviceTrait, HostTrait};
use log::*;

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
    match default_host.default_output_device() {
        Some(device) => Some(device),
        None => None,
    }
}
