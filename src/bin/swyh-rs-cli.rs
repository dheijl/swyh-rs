use cpal::traits::DeviceTrait;
use log::LevelFilter;
use swyh_rs::{
    globals::statics::CONFIG,
    utils::{
        audiodevices::get_default_audio_output_device,
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
    ui_log(format!("{config:?}"));
    if cfg!(debug_assertions) {
        config.log_level = LevelFilter::Debug;
    }
}
