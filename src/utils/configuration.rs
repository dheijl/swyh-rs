use crate::{StreamingFormat, SERVER_PORT};
use log::{debug, LevelFilter};
use serde::{Deserialize, Serialize};
use std::{
    f64, file, format_args, fs,
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};
use toml::*;

const CONFIGFILE: &str = "config.toml";
const PKGNAME: &str = env!("CARGO_PKG_NAME");

// the configuration struct, read from and saved in config.ini
#[derive(Deserialize, Serialize, Clone, Debug)]
struct Config {
    #[serde(rename(deserialize = "Configuration", serialize = "Configuration"))]
    pub configuration: Configuration,
}
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Configuration {
    #[serde(rename(deserialize = "ServerPort", serialize = "ServerPort"))]
    pub server_port: Option<u16>,
    #[serde(rename(deserialize = "AutoResume", serialize = "AutoResume"))]
    pub auto_resume: bool,
    #[serde(rename(deserialize = "SoundCard", serialize = "SoundCard"))]
    pub sound_source: String,
    #[serde(rename(deserialize = "SoundCardIndex", serialize = "SoundCardIndex"))]
    pub sound_source_index: Option<i32>,
    #[serde(rename(deserialize = "LogLevel", serialize = "LogLevel"))]
    pub log_level: LevelFilter,
    #[serde(rename(deserialize = "SSDPIntervalMins", serialize = "SSDPIntervalMins"))]
    pub ssdp_interval_mins: f64,
    #[serde(rename(deserialize = "AutoReconnect", serialize = "AutoReconnect"))]
    pub auto_reconnect: bool,
    #[serde(rename(deserialize = "DisableChunked", serialize = "DisableChunked"))]
    pub disable_chunked: bool,
    #[serde(rename(deserialize = "UseWaveFormat", serialize = "UseWaveFormat"))]
    pub use_wave_format: bool,
    #[serde(rename(deserialize = "BitsPerSample", serialize = "BitsPerSample"))]
    pub bits_per_sample: Option<u16>,
    #[serde(rename(deserialize = "StreamingFormat", serialize = "StreamingFormat"))]
    pub streaming_format: Option<StreamingFormat>,
    #[serde(rename(deserialize = "MonitorRms", serialize = "MonitorRms"))]
    pub monitor_rms: bool,
    #[serde(rename(deserialize = "CaptureTimeout", serialize = "CaptureTimeout"))]
    pub capture_timeout: Option<u32>,
    #[serde(rename(deserialize = "InjectSilence", serialize = "InjectSilence"))]
    pub inject_silence: Option<bool>,
    #[serde(rename(deserialize = "LastRenderer", serialize = "LastRenderer"))]
    pub last_renderer: String,
    #[serde(rename(deserialize = "LastNetwork", serialize = "LastNetwork"))]
    pub last_network: String,
    #[serde(rename(deserialize = "ConfigDir", serialize = "ConfigDir"))]
    config_dir: PathBuf,
}

impl Default for Configuration {
    fn default() -> Self {
        Self::new()
    }
}

impl Configuration {
    pub fn new() -> Configuration {
        Configuration {
            server_port: Some(SERVER_PORT),
            auto_resume: false,
            sound_source: "None".to_string(),
            sound_source_index: None,
            log_level: LevelFilter::Info,
            ssdp_interval_mins: 1.0,
            auto_reconnect: false,
            disable_chunked: false,
            use_wave_format: false,
            bits_per_sample: Some(16),
            streaming_format: Some(StreamingFormat::Lpcm),
            monitor_rms: false,
            capture_timeout: Some(2000),
            inject_silence: Some(false),
            last_renderer: "None".to_string(),
            last_network: "None".to_string(),
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
        let mut force_update = false;
        let configfile = Self::get_config_path(CONFIGFILE);
        let old_configfile = Self::get_config_path("config.ini");
        if !Path::new(&configfile).exists() {
            if !Path::new(&old_configfile).exists() {
                debug!("Creating a new default config {}", configfile.display());
                let config = Configuration::new();
                let configuration = Config {
                    configuration: config,
                };
                let f = File::create(&configfile).unwrap();
                let s = toml::to_string(&configuration).unwrap();
                let mut w = BufWriter::new(f);
                debug!("New default CONFIG: {}", s);
                w.write_all(s.as_bytes()).unwrap();
                w.flush().unwrap();
            } else {
                Self::migrate_config_to_toml(&old_configfile, &configfile);
            }
        }
        let s = fs::read_to_string(&configfile).unwrap();
        let mut config: Config = from_str(&s).unwrap();
        if config.configuration.ssdp_interval_mins < 0.5 {
            config.configuration.ssdp_interval_mins = 0.5;
            force_update = true;
        }
        match config.configuration.server_port {
            Some(_u16) => {}
            _ => {
                config.configuration.server_port = Some(SERVER_PORT);
                force_update = true;
            }
        }
        match config.configuration.bits_per_sample {
            Some(16 | 24) => {}
            _ => {
                config.configuration.bits_per_sample = Some(16);
                force_update = true;
            }
        }
        if config.configuration.capture_timeout.is_none() {
            config.configuration.capture_timeout = Some(2000);
            force_update = true;
        }
        if config.configuration.inject_silence.is_none() {
            config.configuration.inject_silence = Some(false);
            force_update = true;
        }
        if force_update {
            config.configuration.update_config().unwrap();
        }
        config.configuration
    }

    pub fn update_config(&self) -> std::io::Result<()> {
        let configfile = Self::get_config_path(CONFIGFILE);
        let f = File::create(&configfile).unwrap();
        let conf = Config {
            configuration: self.clone(),
        };
        let s = toml::to_string(&conf).unwrap();
        let mut w = BufWriter::new(f);
        debug!("Updated CONFIG: {}", s);
        w.write_all(s.as_bytes()).unwrap();
        w.flush().unwrap();

        Ok(())
    }

    fn get_config_dir() -> PathBuf {
        let hd = dirs::home_dir().unwrap_or_default();
        let old_config_dir = Path::new(&hd).join(PKGNAME);
        let config_dir = Path::new(&hd).join(".".to_string() + PKGNAME);
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

    fn get_config_path(filename: &str) -> PathBuf {
        let config_dir = Self::get_config_dir();
        Path::new(&config_dir).join(filename)
    }

    fn migrate_config_to_toml(old_config: &Path, new_config: &Path) {
        debug!(
            "Migrating {} to {}",
            old_config.display(),
            new_config.display()
        );
        let oldf = File::open(&old_config).unwrap();
        let r = BufReader::new(&oldf);
        let newf = File::create(&new_config).unwrap();
        let mut w = BufWriter::new(&newf);
        for line in r.lines() {
            let mut s = line.unwrap();
            if let Some(n) = s.find('=') {
                const NEEDS_QUOTE: &str = "|SoundCard|LogLevel|LastRenderer|LastNetwork|ConfigDir|";
                let key = s.get(0..n).unwrap();
                if NEEDS_QUOTE.contains(key) {
                    s.insert(n + 1, '"');
                    s.insert(s.len(), '"');
                }
            }
            w.write_all(s.as_bytes()).unwrap();
            writeln!(w).unwrap();
        }
        w.flush().unwrap();
        drop(oldf);
        fs::remove_file(&old_config).unwrap();
    }
}
