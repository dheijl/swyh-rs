use ini::Ini;
use log::*;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Configuration {
    pub auto_resume: bool,
    pub sound_source: String,
    pub log_level: LevelFilter,
    pub ssdp_interval_mins: f64,
    pub auto_reconnect: bool,
    pub disable_chunked: bool,
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

    fn parse_bool(s: &str) -> bool {
        match s {
            "true" | "True" | "TRUE" | "1" | "T" | "t" => true,
            _ => false,
        }
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
                .set("LastRenderer", "None")
                .set("ConfigDir", &Self::get_config_dir().display().to_string());
            conf.write_to_file(&configfile).unwrap();
        }

        let conf = match Ini::load_from_file(&configfile) {
            Ok(conf) => conf,
            Err(_) => Ini::new(),
        };
        let mut config = Configuration::new();
        config.auto_resume = Configuration::parse_bool(conf.get_from_or(
            Some("Configuration"),
            "AutoResume",
            "false",
        ));
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
        config.auto_reconnect = Configuration::parse_bool(conf.get_from_or(
            Some("Configuration"),
            "AutoReconnect",
            "false",
        ));
        config.disable_chunked = Configuration::parse_bool(conf.get_from_or(
            Some("Configuration"),
            "DisableChunked",
            "false",
        ));
        config.last_renderer = conf
            .get_from_or(Some("Configuration"), "LastRenderer", "None")
            .to_string()
            .parse()
            .unwrap();
        let config_dir = conf
            .get_from_or(
                Some("Configuration"),
                "ConfigDir",
                &Self::get_config_dir().display().to_string(),
            )
            .to_string();
        config.config_dir = PathBuf::from(config_dir);

        config
    }

    pub fn update_config(&self) -> io::Result<()> {
        let configfile = Self::get_config_path();
        let mut conf = match Ini::load_from_file(&configfile) {
            Ok(conf) => conf,
            Err(_) => Ini::new(),
        };
        conf.with_section(Some("Configuration"))
            .set(
                "AutoResume",
                if self.auto_resume { "true" } else { "false" },
            )
            .set("SoundCard", &self.sound_source)
            .set("LogLevel", self.log_level.to_string())
            .set("SSDPIntervalMins", self.ssdp_interval_mins.to_string())
            .set(
                "AutoReconnect",
                if self.auto_reconnect { "true" } else { "false" },
            )
            .set(
                "DisableChunked",
                if self.disable_chunked {
                    "true"
                } else {
                    "false"
                },
            )
            .set("LastRenderer", self.last_renderer.to_string())
            .set("ConfigDir", &self.config_dir.display().to_string());
        conf.write_to_file(&configfile)
    }

    fn get_config_dir() -> PathBuf {
        let hd = dirs::home_dir().unwrap_or_default();
        let config_dir = Path::new(&hd).join("swyh-rs");
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
