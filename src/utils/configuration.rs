use crate::{enums::streaming::StreamingFormat, globals::statics::SERVER_PORT};
use lexopt::{prelude::*, Parser};
use log::LevelFilter;
use serde::{Deserialize, Serialize};
use std::{
    f64, fs,
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};
use toml::from_str;

const CONFIGFILE: &str = "config{}.toml";
const PKGNAME: &str = env!("CARGO_PKG_NAME");

// the configuration struct, read from and saved in config.ini
#[derive(Deserialize, Serialize, Clone, Debug)]
struct Config {
    #[serde(alias = "Configuration")]
    pub configuration: Configuration,
}

struct Defaults {}

impl Defaults {
    pub fn t() -> bool {
        true
    }
    pub fn log_level() -> LevelFilter {
        LevelFilter::Info
    }
    pub fn ssdp_interval_mins() -> f64 {
        10.0
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Configuration {
    #[serde(alias = "ServerPort", default)]
    pub server_port: Option<u16>,
    #[serde(alias = "AutoResume", default = "Defaults::t")]
    pub auto_resume: bool,
    #[serde(alias = "SoundCard", default)]
    pub sound_source: Option<String>,
    #[serde(alias = "SoundCardIndex", default)]
    pub sound_source_index: Option<i32>,
    #[serde(alias = "LogLevel", default = "Defaults::log_level")]
    pub log_level: LevelFilter,
    #[serde(alias = "SSDPIntervalMins", default = "Defaults::ssdp_interval_mins")]
    pub ssdp_interval_mins: f64,
    #[serde(alias = "AutoReconnect", default = "Defaults::t")]
    pub auto_reconnect: bool,
    // removed in 1.8.5
    #[serde(alias = "DisableChunked", skip, default = "Defaults::t")]
    _disable_chunked: bool,
    #[serde(alias = "UseWaveFormat", default = "Defaults::t")]
    pub use_wave_format: bool,
    #[serde(alias = "BitsPerSample", default)]
    pub bits_per_sample: Option<u16>,
    #[serde(alias = "StreamingFormat", default)]
    pub streaming_format: Option<StreamingFormat>,
    #[serde(alias = "MonitorRms", default)]
    pub monitor_rms: bool,
    #[serde(alias = "CaptureTimeout", default)]
    pub capture_timeout: Option<u32>,
    #[serde(alias = "InjectSilence", default)]
    pub inject_silence: Option<bool>,
    #[serde(alias = "LastRenderer", default)]
    pub last_renderer: Option<String>,
    #[serde(alias = "LastNetwork", default)]
    pub last_network: Option<String>,
    #[serde(alias = "ConfigDir", default)]
    config_dir: PathBuf,
    #[serde(alias = "ConfigId", default)]
    pub config_id: Option<String>,
    #[serde(alias = "ReadOnly", default)]
    pub read_only: bool,
}

impl Default for Configuration {
    fn default() -> Self {
        Self::new()
    }
}

impl Configuration {
    #[must_use]
    pub fn new() -> Configuration {
        Configuration {
            server_port: Some(SERVER_PORT),
            auto_resume: false,
            sound_source: None,
            sound_source_index: Some(0),
            log_level: LevelFilter::Info,
            ssdp_interval_mins: 10.0,
            auto_reconnect: false,
            _disable_chunked: true,
            use_wave_format: false,
            bits_per_sample: Some(16),
            streaming_format: Some(StreamingFormat::Lpcm),
            monitor_rms: false,
            capture_timeout: Some(2000),
            inject_silence: Some(false),
            last_renderer: None,
            last_network: None,
            config_dir: Self::get_config_dir(),
            config_id: Some(Self::get_config_id()),
            read_only: false,
        }
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn config_dir(&self) -> PathBuf {
        self.config_dir.clone()
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn log_dir(&self) -> PathBuf {
        self.config_dir.clone()
    }

    #[must_use]
    pub fn read_config() -> Configuration {
        let mut force_update = false;
        let configfile = Self::chose_config_path();
        println!("Loading config from {}", configfile.display());
        let s = fs::read_to_string(&configfile).unwrap_or_else(|error| {
            eprintln!("Unable to read config file: {error}");
            String::new()
        });
        let mut config: Config = from_str(&s).unwrap_or_else(|error| {
            eprintln!("Unable to deserialize config: {error}");
            let config = Configuration::new();
            Config {
                configuration: config,
            }
        });
        if config.configuration.ssdp_interval_mins < 0.5 {
            config.configuration.ssdp_interval_mins = 0.5;
            force_update = true;
        }
        // replace missing values from old configs with reasonable defaults
        if let Some(_u16) = config.configuration.server_port {
        } else {
            config.configuration.server_port = Some(SERVER_PORT);
            force_update = true;
        }
        if let Some(16 | 24) = config.configuration.bits_per_sample {
        } else {
            config.configuration.bits_per_sample = Some(16);
            force_update = true;
        }
        if config.configuration.capture_timeout.is_none() {
            config.configuration.capture_timeout = Some(2000);
            force_update = true;
        }
        if config.configuration.inject_silence.is_none() {
            config.configuration.inject_silence = Some(false);
            force_update = true;
        }
        if config.configuration.config_id.is_none() {
            config.configuration.config_id = Some(String::new());
            force_update = true;
        }
        if config.configuration.sound_source_index.is_none() {
            config.configuration.sound_source_index = Some(0);
            force_update = true;
        }
        if !config.configuration.read_only {
            let meta = fs::metadata(configfile);
            if let Ok(meta) = meta {
                config.configuration.read_only = meta.permissions().readonly();
            }
        }
        if force_update && !config.configuration.read_only {
            config.configuration.update_config().unwrap();
        }
        config.configuration
    }

    pub fn update_config(&self) -> std::io::Result<()> {
        if self.read_only {
            return Ok(());
        }
        let configfile = Self::get_config_path(CONFIGFILE);
        let f = File::create(configfile).unwrap();
        let conf = Config {
            configuration: self.clone(),
        };
        let s = toml::to_string(&conf).unwrap();
        let mut w = BufWriter::new(f);
        w.write_all(s.as_bytes()).unwrap();
        w.flush().unwrap();
        Ok(())
    }

    fn chose_config_path() -> PathBuf {
        if let Some(path) = Self::get_arg_config_path() {
            path
        } else {
            let configfile = Self::get_config_path(CONFIGFILE);
            let old_configfile = Self::get_config_path("config.ini");
            if !Path::new(&configfile).exists() {
                if Path::new(&old_configfile).exists() {
                    Self::migrate_config_to_toml(&old_configfile, &configfile);
                } else {
                    println!("Creating a new default config {}", configfile.display());
                    let config = Configuration::new();
                    let configuration = Config {
                        configuration: config,
                    };
                    let f = File::create(&configfile).unwrap();
                    let s = toml::to_string(&configuration).unwrap();
                    let mut w = BufWriter::new(f);
                    println!("New default CONFIG: {s}");
                    w.write_all(s.as_bytes()).unwrap();
                    w.flush().unwrap();
                }
            }
            configfile
        }
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
                fs::copy(old_config_file, config_file).unwrap();
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
        let id = Self::get_config_id();
        let configfilename = filename.replace("{}", &id);
        let config_dir = Self::get_config_dir();
        Path::new(&config_dir).join(configfilename)
    }

    fn get_config_id() -> String {
        let mut config_id = String::new();
        let mut argparser = Parser::from_env();
        while let Some(arg) = argparser.next().unwrap() {
            if let Short('c') | Long("configuration") = arg {
                if let Ok(id) = argparser.value() {
                    config_id = id.string().unwrap_or_default();
                };
            };
        }
        if cfg!(feature = "cli") && config_id.is_empty() {
            config_id = "_cli".to_string();
        }
        config_id
    }

    fn get_arg_config_path() -> Option<PathBuf> {
        let mut argparser = Parser::from_env();
        let mut path = None;
        while let Some(arg) = argparser.next().unwrap() {
            if let Short('C') | Long("configfile") = arg {
                if let Ok(opt) = argparser.value() {
                    path = opt.string().ok().map(|s| PathBuf::from(&s));
                };
            };
        }
        path
    }

    fn migrate_config_to_toml(old_config: &Path, new_config: &Path) {
        println!(
            "Migrating {} to {}",
            old_config.display(),
            new_config.display()
        );
        let oldf = File::open(old_config).unwrap();
        let r = BufReader::new(&oldf);
        let newf = File::create(new_config).unwrap();
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
        fs::remove_file(old_config).unwrap();
    }
}
