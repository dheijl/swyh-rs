//! Application configuration: serialization/deserialization to/from a TOML file,
//! default values, and runtime config updates.

use crate::{
    enums::streaming::{StreamSize, StreamingFormat},
    globals::statics::{DEFAULT_COLOR_THEME, DEFAULT_WIDGET_SCHEME, SERVER_PORT, STYLES, THEMES},
    utils::i18n::available_languages,
};
use anyhow::{Context, Result};
use lexopt::{Parser, prelude::*};
use log::LevelFilter;
use serde::{Deserialize, Serialize};
use std::{
    f64, fs,
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};
use toml::from_str;

const CONFIGFILE: &str = "config{}.toml";
const PKGNAME: &str = env!("CARGO_PKG_NAME");

/// get the default language for a new installation
/// use the base language if region language not present (nl-NL for nl-BE etc...)
fn detect_default_language() -> String {
    let supported = available_languages();
    let locale = sys_locale::get_locale();
    println!("Detected locale: {locale:?}");
    if let Some(l) = locale {
        if supported.contains(&l) {
            println!("Used locale: {l}");
            return l;
        }
        let base_locale = &l[..2];
        if let Some(locale) = supported.into_iter().find(|s| s.contains(base_locale)) {
            println!("Used fallback locale: {locale}");
            return locale;
        }
    }
    "en-US".to_string()
}

// default values for Serde
struct CfgDefaults {}

impl CfgDefaults {
    fn autoreconnect() -> bool {
        false
    }
    fn language() -> Option<String> {
        Some(detect_default_language())
    }
    fn log_level() -> LevelFilter {
        LevelFilter::Info
    }
    fn ssdp_interval_mins() -> f64 {
        10.0
    }
    fn stream_size() -> Option<StreamSize> {
        Some(StreamSize::U64maxNotChunked)
    }
    fn wav_stream_size() -> Option<StreamSize> {
        Some(StreamSize::U32maxNotChunked)
    }
    fn flac_stream_size() -> Option<StreamSize> {
        Some(StreamSize::NoneChunked)
    }
    fn bits_per_sample() -> Option<u16> {
        Some(16)
    }
    fn sample_rate() -> Option<u32> {
        None
    }
    fn use_dither() -> Option<bool> {
        Some(true)
    }
}

// the configuration struct, read from and saved in config.ini
#[derive(Deserialize, Serialize, Clone, Debug)]
struct Config {
    #[serde(alias = "Configuration")]
    pub configuration: Configuration,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Configuration {
    #[serde(alias = "ServerPort", default)]
    pub server_port: Option<u16>,
    #[serde(alias = "AutoResume", default)]
    pub auto_resume: bool,
    #[serde(alias = "SoundCard", default)]
    pub sound_source: Option<String>,
    #[serde(alias = "SoundCardIndex", default)]
    pub sound_source_index: Option<i32>,
    #[serde(alias = "LogLevel", default = "CfgDefaults::log_level")]
    pub log_level: LevelFilter,
    #[serde(
        alias = "SSDPIntervalMins",
        default = "CfgDefaults::ssdp_interval_mins"
    )]
    pub ssdp_interval_mins: f64,
    #[serde(alias = "AutoReconnect", default = "CfgDefaults::autoreconnect")]
    pub auto_reconnect: bool,
    // removed in 1.8.5 (obsolete)
    #[serde(alias = "DisableChunked", skip, default)]
    _disable_chunked: bool,
    // added in 1.9.9
    #[serde(alias = "LPCMStreamSize", default = "CfgDefaults::stream_size")]
    pub lpcm_stream_size: Option<StreamSize>,
    #[serde(alias = "WAVStreamSize", default = "CfgDefaults::wav_stream_size")]
    pub wav_stream_size: Option<StreamSize>,
    #[serde(alias = "RF64StreamSize", default = "CfgDefaults::stream_size")]
    pub rf64_stream_size: Option<StreamSize>,
    #[serde(alias = "FLACStreamSize", default = "CfgDefaults::flac_stream_size")]
    pub flac_stream_size: Option<StreamSize>,
    // removed in 1.10.8 (obsolete)
    #[serde(alias = "UseWaveFormat", skip, default)]
    _use_wave_format: bool,
    #[serde(alias = "BitsPerSample", default = "CfgDefaults::bits_per_sample")]
    pub bits_per_sample: Option<u16>,
    #[serde(alias = "StreamingFormat", default)]
    pub streaming_format: Option<StreamingFormat>,
    #[serde(alias = "MonitorRms", default)]
    pub monitor_rms: bool,
    #[serde(alias = "CaptureTimeout", default)]
    pub capture_timeout: Option<u32>,
    #[serde(alias = "InjectSilence", default)]
    pub inject_silence: Option<bool>,
    #[serde(alias = "BufferingDelayMSec", default)]
    pub buffering_delay_msec: Option<u32>,
    #[serde(alias = "LastRenderer", default)]
    pub last_renderer: Option<String>,
    #[serde(alias = "ActiveRenderers", default)]
    pub active_renderers: Vec<String>,
    #[serde(alias = "HiddenRenderers", default)]
    pub hidden_renderers: Vec<String>,
    #[serde(alias = "LastNetwork", default)]
    pub last_network: Option<String>,
    #[serde(alias = "ConfigDir", default)]
    config_dir: PathBuf,
    #[serde(skip, default)]
    config_path: PathBuf,
    #[serde(alias = "ConfigId", default)]
    pub config_id: Option<String>,
    #[serde(alias = "ReadOnly", default)]
    pub read_only: bool,
    #[serde(alias = "ColorTheme", default)]
    pub color_theme: Option<u8>,
    #[serde(alias = "WidgetScheme", default)]
    pub widget_scheme: Option<u8>,
    #[serde(alias = "Language", default = "CfgDefaults::language")]
    pub language: Option<String>,
    #[serde(alias = "SampleRate", default = "CfgDefaults::sample_rate")]
    pub sample_rate: Option<u32>,
    #[serde(alias = "UseDither", default = "CfgDefaults::use_dither")]
    pub use_dither: Option<bool>,
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
            lpcm_stream_size: Some(StreamSize::U64maxNotChunked),
            wav_stream_size: Some(StreamSize::U32maxNotChunked),
            rf64_stream_size: Some(StreamSize::U64maxNotChunked),
            flac_stream_size: Some(StreamSize::NoneChunked),
            _use_wave_format: false,
            bits_per_sample: Some(16),
            streaming_format: Some(StreamingFormat::Lpcm),
            monitor_rms: false,
            capture_timeout: Some(2000),
            inject_silence: Some(false),
            buffering_delay_msec: Some(0),
            last_renderer: None,
            active_renderers: Vec::new(),
            hidden_renderers: Vec::new(),
            last_network: None,
            config_dir: Self::get_config_dir().unwrap_or_default(),
            config_path: PathBuf::new(),
            config_id: Some(Self::get_config_id().unwrap_or_default()),
            read_only: false,
            color_theme: Some(DEFAULT_COLOR_THEME),
            widget_scheme: Some(DEFAULT_WIDGET_SCHEME),
            language: Some(detect_default_language()),
            sample_rate: None,
            use_dither: Some(true),
        }
    }

    /// Returns the log directory — currently the same as `config_dir`.
    #[allow(dead_code)]
    #[must_use]
    pub fn log_dir(&self) -> PathBuf {
        self.config_dir.clone()
    }

    /// Returns the configured `StreamSize` for the given streaming format.
    #[must_use]
    pub fn stream_size_for(&self, format: StreamingFormat) -> Option<StreamSize> {
        match format {
            StreamingFormat::Lpcm => self.lpcm_stream_size,
            StreamingFormat::Wav => self.wav_stream_size,
            StreamingFormat::Rf64 => self.rf64_stream_size,
            StreamingFormat::Flac => self.flac_stream_size,
        }
    }

    /// Sets the `StreamSize` for the given streaming format.
    pub fn set_stream_size_for(&mut self, format: StreamingFormat, size: StreamSize) {
        match format {
            StreamingFormat::Lpcm => self.lpcm_stream_size = Some(size),
            StreamingFormat::Wav => self.wav_stream_size = Some(size),
            StreamingFormat::Rf64 => self.rf64_stream_size = Some(size),
            StreamingFormat::Flac => self.flac_stream_size = Some(size),
        }
    }

    /// Resolves the config path (honouring CLI overrides) and loads it, creating a default file if absent.
    pub fn read_config() -> Result<Configuration> {
        let configfile = Self::choose_config_path()?;
        println!("Loading config from {}", configfile.display());
        Self::read_config_from(&configfile)
    }

    /// Deserialises config from `configfile`, migrating/clamping stale field values and falling back to defaults on parse failure.
    pub fn read_config_from(configfile: &Path) -> Result<Configuration> {
        let s = fs::read_to_string(configfile).unwrap_or_else(|error| {
            eprintln!("Unable to read config file: {error}");
            String::new()
        });
        let mut config: Config = from_str(&s).unwrap_or_else(|error| {
            eprintln!("Unable to deserialize config: {error}");
            Config {
                configuration: Configuration::new(),
            }
        });
        let mut force_update = false;
        if config.configuration.ssdp_interval_mins > 0.0
            && config.configuration.ssdp_interval_mins < 0.5
        {
            config.configuration.ssdp_interval_mins = 0.5;
            force_update = true;
        }
        // replace missing values from old configs with reasonable defaults
        if config.configuration.server_port.is_none() {
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
        if config.configuration.buffering_delay_msec.is_none() {
            config.configuration.buffering_delay_msec = Some(0);
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
        if let Some(theme) = config.configuration.color_theme
            && theme >= THEMES.len() as u8
        {
            config.configuration.color_theme = None;
            force_update = true;
        }
        if let Some(style) = config.configuration.widget_scheme
            && style >= STYLES.len() as u8
        {
            config.configuration.widget_scheme = None;
            force_update = true;
        }
        config.configuration.config_path = configfile.to_path_buf();
        if force_update && !config.configuration.read_only {
            config
                .configuration
                .update_config_to(configfile)
                .context("failed to update config")?;
        }
        Ok(config.configuration)
    }

    /// Persists the current config to the resolved config path; no-op when `read_only`.
    pub fn update_config(&self) -> Result<()> {
        if !self.config_path.as_os_str().is_empty() {
            self.update_config_to(&self.config_path)
        } else {
            let configfile = Self::choose_config_path()?;
            self.update_config_to(&configfile)
        }
    }

    /// Serialises `self` to `configfile`; silently succeeds (no write) when `read_only` is set.
    pub fn update_config_to(&self, configfile: &Path) -> Result<()> {
        if self.read_only {
            return Ok(());
        }
        let f = File::create(configfile)
            .with_context(|| format!("failed to create config file: {}", configfile.display()))?;
        let conf = Config {
            configuration: self.clone(),
        };
        let s = toml::to_string(&conf).context("failed to serialize config")?;
        let mut w = BufWriter::new(f);
        w.write_all(s.as_bytes())
            .with_context(|| format!("failed to write config file: {}", configfile.display()))?;
        w.flush()
            .with_context(|| format!("failed to flush config file: {}", configfile.display()))?;
        Ok(())
    }

    /// Returns the effective config path: `-C`/`--configfile` arg overrides the standard location; creates the file with defaults if it doesn't exist yet.
    fn choose_config_path() -> Result<PathBuf> {
        if let Some(path) = Self::get_arg_config_path()? {
            Ok(path)
        } else {
            let configfile = Self::get_config_path(CONFIGFILE)?;
            if !Path::new(&configfile).exists() {
                println!("Creating a new default config {}", configfile.display());
                let configuration = Config {
                    configuration: Configuration::new(),
                };
                let f = File::create(&configfile).with_context(|| {
                    format!("failed to create config file: {}", configfile.display())
                })?;
                let s = toml::to_string(&configuration)
                    .context("failed to serialize default config")?;
                let mut w = BufWriter::new(f);
                println!("New default CONFIG: {s}");
                w.write_all(s.as_bytes()).with_context(|| {
                    format!("failed to write config file: {}", configfile.display())
                })?;
                w.flush().with_context(|| {
                    format!("failed to flush config file: {}", configfile.display())
                })?;
            }
            Ok(configfile)
        }
    }

    /// Returns `~/.swyh-rs[…]`, creating it if absent.
    fn get_config_dir() -> Result<PathBuf> {
        let hd = dirs::home_dir().unwrap_or_default();
        let config_dir = Path::new(&hd).join(".".to_string() + PKGNAME);
        if !Path::new(&config_dir).exists() {
            fs::create_dir_all(&config_dir).with_context(|| {
                format!(
                    "failed to create config directory: {}",
                    config_dir.display()
                )
            })?;
        }
        Ok(config_dir)
    }

    fn get_config_path(filename: &str) -> Result<PathBuf> {
        let id = Self::get_config_id()?;
        let configfilename = filename.replace("{}", &id);
        let config_dir = Self::get_config_dir()?;
        Ok(Path::new(&config_dir).join(configfilename))
    }

    /// Reads the `-c`/`--configuration` CLI flag as the config-profile suffix (e.g. `"foo"` → `config_foo.toml`); CLI builds always use `"_cli"` when absent.
    fn get_config_id() -> Result<String> {
        let mut config_id = String::new();
        let mut argparser = Parser::from_env();
        while let Some(arg) = argparser
            .next()
            .context("failed to parse command line arguments")?
        {
            if let Short('c') | Long("configuration") = arg
                && let Ok(id) = argparser.value()
            {
                config_id = id.string().unwrap_or_default();
                break;
            }
        }
        #[cfg(feature = "cli")]
        if config_id.is_empty() {
            config_id = "_cli".to_string();
        }
        Ok(config_id)
    }

    /// Reads the `-C`/`--configfile` CLI flag as a full path override (bypasses the standard config directory entirely).
    fn get_arg_config_path() -> Result<Option<PathBuf>> {
        let mut argparser = Parser::from_env();
        let mut path = None;
        while let Some(arg) = argparser
            .next()
            .context("failed to parse command line arguments")?
        {
            if let Short('C') | Long("configfile") = arg
                && let Ok(opt) = argparser.value()
            {
                path = opt.string().ok().map(|s| PathBuf::from(&s));
                break;
            }
        }
        println!("ARG override configfile (-C): {path:?}");
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn temp_toml(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{content}").unwrap();
        f
    }

    #[test]
    fn empty_file_yields_defaults() {
        let f = NamedTempFile::new().unwrap();
        let cfg = Configuration::read_config_from(f.path()).unwrap();
        assert_eq!(cfg.server_port, Some(SERVER_PORT));
        assert_eq!(cfg.bits_per_sample, Some(16));
        assert_eq!(cfg.capture_timeout, Some(2000));
        assert_eq!(cfg.inject_silence, Some(false));
        assert_eq!(cfg.buffering_delay_msec, Some(0));
        assert_eq!(cfg.sound_source_index, Some(0));
    }

    #[test]
    fn round_trip_preserves_values() {
        let f = NamedTempFile::new().unwrap();
        let mut cfg = Configuration::new();
        cfg.server_port = Some(5030);
        cfg.auto_resume = true;
        cfg.ssdp_interval_mins = 5.0;
        cfg.streaming_format = Some(StreamingFormat::Flac);
        cfg.update_config_to(f.path()).unwrap();

        let loaded = Configuration::read_config_from(f.path()).unwrap();
        assert_eq!(loaded.server_port, Some(5030));
        assert!(loaded.auto_resume);
        assert_eq!(loaded.ssdp_interval_mins, 5.0);
        assert_eq!(loaded.streaming_format, Some(StreamingFormat::Flac));
    }

    #[test]
    fn ssdp_interval_clamped_below_minimum() {
        let f = temp_toml("[configuration]\nssdp_interval_mins = 0.1\n");
        let cfg = Configuration::read_config_from(f.path()).unwrap();
        assert_eq!(cfg.ssdp_interval_mins, 0.5);
    }

    #[test]
    fn ssdp_interval_zero_not_clamped() {
        // zero means "disabled", not "too small" — clamp only applies when > 0
        let f = temp_toml("[configuration]\nssdp_interval_mins = 0.0\n");
        let cfg = Configuration::read_config_from(f.path()).unwrap();
        assert_eq!(cfg.ssdp_interval_mins, 0.0);
    }

    #[test]
    fn invalid_bits_per_sample_reset_to_16() {
        let f = temp_toml("[configuration]\nbits_per_sample = 32\n");
        let cfg = Configuration::read_config_from(f.path()).unwrap();
        assert_eq!(cfg.bits_per_sample, Some(16));
    }

    #[test]
    fn valid_bits_per_sample_preserved() {
        for bps in [16u16, 24] {
            let f = temp_toml(&format!("[configuration]\nbits_per_sample = {bps}\n"));
            let cfg = Configuration::read_config_from(f.path()).unwrap();
            assert_eq!(cfg.bits_per_sample, Some(bps), "bps={bps}");
        }
    }

    #[test]
    fn out_of_range_color_theme_reset_to_none() {
        let oob = THEMES.len() as u8; // first value >= len
        let f = temp_toml(&format!("[configuration]\ncolor_theme = {oob}\n"));
        let cfg = Configuration::read_config_from(f.path()).unwrap();
        assert_eq!(cfg.color_theme, None);
    }

    #[test]
    fn valid_color_theme_preserved() {
        let f = temp_toml("[configuration]\ncolor_theme = 0\n");
        let cfg = Configuration::read_config_from(f.path()).unwrap();
        assert_eq!(cfg.color_theme, Some(0));
    }

    #[test]
    fn out_of_range_widget_scheme_reset_to_none() {
        let oob = STYLES.len() as u8; // first value >= len
        let f = temp_toml(&format!("[configuration]\nwidget_scheme = {oob}\n"));
        let cfg = Configuration::read_config_from(f.path()).unwrap();
        assert_eq!(cfg.widget_scheme, None);
    }

    #[test]
    fn valid_widget_scheme_preserved() {
        let f = temp_toml("[configuration]\nwidget_scheme = 0\n");
        let cfg = Configuration::read_config_from(f.path()).unwrap();
        assert_eq!(cfg.widget_scheme, Some(0));
    }

    #[test]
    fn legacy_pascal_case_aliases_deserialize() {
        let f = temp_toml(
            "[Configuration]\nServerPort = 5030\nAutoResume = true\nLogLevel = \"Debug\"\n",
        );
        let cfg = Configuration::read_config_from(f.path()).unwrap();
        assert_eq!(cfg.server_port, Some(5030));
        assert!(cfg.auto_resume);
        assert_eq!(cfg.log_level, LevelFilter::Debug);
    }

    #[test]
    fn missing_optional_fields_get_defaults() {
        let f = temp_toml("[configuration]\n");
        let cfg = Configuration::read_config_from(f.path()).unwrap();
        assert_eq!(cfg.server_port, Some(SERVER_PORT));
        assert_eq!(cfg.capture_timeout, Some(2000));
        assert_eq!(cfg.inject_silence, Some(false));
        assert_eq!(cfg.buffering_delay_msec, Some(0));
        assert_eq!(cfg.sound_source_index, Some(0));
    }
}
