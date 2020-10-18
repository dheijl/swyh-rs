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
    config_dir: PathBuf,
}

impl Configuration {
    pub fn new() -> Configuration {
        Configuration {
            auto_resume: false,
            sound_source: "None".to_string(),
            config_dir: Self::get_config_dir(),
            log_level: LevelFilter::Info,
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
                .set("ConfigDir", &Self::get_config_dir().display().to_string());
            conf.write_to_file(&configfile).unwrap();
        }

        let conf = match Ini::load_from_file(&configfile) {
            Ok(conf) => conf,
            Err(_) => Ini::new(),
        };
        let mut config = Configuration::new();
        match conf.get_from_or(Some("Configuration"), "AutoResume", "false") {
            "true" | "True" | "TRUE" | "1" | "T" | "t" => config.auto_resume = true,
            _ => config.auto_resume = false,
        }
        config.sound_source = conf
            .get_from_or(Some("Configuration"), "SoundCard", "None")
            .to_string();
        config.log_level = conf
            .get_from_or(Some("Configuration"), "LogLevel", "Info")
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
