use ini::Ini;
use log::{debug, LevelFilter};
use std::path::{Path, PathBuf};
use std::{f64, file, format_args, fs, io, line, module_path};

// the configuration struct, read from and saved in config.ini
#[derive(Clone, Debug)]
pub struct Configuration {
    pub auto_resume: bool,
    pub sound_source: String,
    pub log_level: LevelFilter,
    pub ssdp_interval_mins: f64,
    pub auto_reconnect: bool,
    pub disable_chunked: bool,
    pub use_wave_format: bool,
    pub monitor_rms: bool,
    pub last_renderer: String,
    config_dir: PathBuf,
}

impl Configuration {
    pub fn new() -> Configuration {
        Configuration {
            auto_resume: false,
            sound_source: "None".to_string(),
            log_level: LevelFilter::Info,
            ssdp_interval_mins: 1.0,
            auto_reconnect: false,
            disable_chunked: false,
            use_wave_format: false,
            monitor_rms: false,
            last_renderer: "None".to_string(),
            config_dir: Self::get_config_dir(),
        }
    }

    #[allow(dead_code)]
    pub fn config_dir(&self) -> PathBuf {
        self.config_dir.clone()
    }

    #[allow(dead_code)]
    pub fn log_dir(&self) -> PathBuf {
        self.config_dir.clone()
    }

    pub fn read_config() -> Configuration {
        let configfile = Self::get_config_path();
        if !Path::new(&configfile).exists() {
            debug!("Creating a new default config {}", configfile.display());
            let mut conf = Ini::new();
            conf.with_section(Some("Configuration"))
                .set("AutoResume", "false")
                .set("SoundCard", "None")
                .set("LogLevel", LevelFilter::Info.to_string())
                .set("SSDPIntervalMins", "1")
                .set("AutoReconnect", "false")
                .set("DisableChunked", "false")
                .set("UseWaveFormat", "false")
                .set("MonitorRms", "false")
                .set("LastRenderer", "None")
                .set("ConfigDir", &Self::get_config_dir().display().to_string());
            conf.write_to_file(&configfile).unwrap();
        }

        let conf = match Ini::load_from_file(&configfile) {
            Ok(conf) => conf,
            Err(_) => Ini::new(),
        };
        let mut config = Configuration::new();
        config.auto_resume = conf
            .get_from_or(Some("Configuration"), "AutoResume", "false")
            .parse()
            .unwrap_or_default();
        config.sound_source = conf
            .get_from_or(Some("Configuration"), "SoundCard", "None")
            .to_string();
        config.log_level = conf
            .get_from_or(Some("Configuration"), "LogLevel", "Info")
            .to_string()
            .parse()
            .unwrap();
        config.ssdp_interval_mins = conf
            .get_from_or(Some("Configuration"), "SSDPIntervalMins", "1")
            .parse::<f64>()
            .unwrap();
        if config.ssdp_interval_mins < 0.5 {
            config.ssdp_interval_mins = 0.5;
        }
        config.auto_reconnect = conf
            .get_from_or(Some("Configuration"), "AutoReconnect", "false")
            .parse()
            .unwrap_or_default();
        config.disable_chunked = conf
            .get_from_or(Some("Configuration"), "DisableChunked", "false")
            .parse()
            .unwrap_or_default();
        config.use_wave_format = conf
            .get_from_or(Some("Configuration"), "UseWaveFormat", "false")
            .parse()
            .unwrap_or_default();
        config.monitor_rms = conf
            .get_from_or(Some("Configuration"), "MonitorRms", "false")
            .parse()
            .unwrap_or_default();
        config.last_renderer = conf
            .get_from_or(Some("Configuration"), "LastRenderer", "None")
            .to_string();
        config.config_dir = Self::get_config_dir();

        config
    }

    pub fn update_config(&self) -> io::Result<()> {
        let configfile = Self::get_config_path();
        let mut conf = match Ini::load_from_file(&configfile) {
            Ok(conf) => conf,
            Err(_) => Ini::new(),
        };
        conf.with_section(Some("Configuration"))
            .set("AutoResume", self.auto_resume.to_string())
            .set("SoundCard", &self.sound_source)
            .set("LogLevel", self.log_level.to_string())
            .set("SSDPIntervalMins", self.ssdp_interval_mins.to_string())
            .set("AutoReconnect", self.auto_reconnect.to_string())
            .set("DisableChunked", self.disable_chunked.to_string())
            .set("UseWaveFormat", self.use_wave_format.to_string())
            .set("MonitorRms", self.monitor_rms.to_string())
            .set("LastRenderer", self.last_renderer.to_string())
            .set("ConfigDir", &self.config_dir.display().to_string());
        conf.write_to_file(&configfile)
    }

    fn get_config_dir() -> PathBuf {
        let hd = dirs::home_dir().unwrap_or_default();
        let old_config_dir = Path::new(&hd).join("swyh-rs");
        let config_dir = Path::new(&hd).join(".swyh-rs");
        if Path::new(&old_config_dir).exists() && !Path::new(&config_dir).exists() {
            // migrate old config dir to the new "hidden" config_dir
            fs::create_dir_all(&config_dir).unwrap();
            let old_config_file = Path::new(&old_config_dir).join("config.ini");
            if Path::new(&old_config_file).exists() {
                let config_file = Path::new(&config_dir).join("config.ini");
                fs::copy(&old_config_file, &config_file).unwrap();
                fs::remove_dir_all(&old_config_dir).unwrap();
                // update the ConfigDir value in the config file
                let conf = Configuration::read_config();
                conf.update_config().unwrap();
            }
            return config_dir;
        }
        if !Path::new(&config_dir).exists() {
            fs::create_dir_all(&config_dir).unwrap();
        }
        config_dir
    }

    fn get_config_path() -> PathBuf {
        let config_dir = Self::get_config_dir();
        Path::new(&config_dir).join("config.ini")
    }
}
