//! CLI argument parsing for the `swyh-rs-cli` binary.
//!
//! Defines [`Args`] and its [`Args::parse`] method which reads flags such as
//! `--format`, `--bits`, `--player`, `--serve_only`, etc.

#![cfg(feature = "cli")]
use std::net::IpAddr;

use lexopt::{
    Arg::{Long, Short},
    Parser, ValueExt,
};
use log::LevelFilter;

use crate::{enums::streaming::*, utils::traits::SanitizeArg};

#[derive(Clone, Debug, Default)]
pub struct Args {
    pub dry_run: Option<bool>,
    pub config_id: Option<String>,
    pub server_port: Option<u16>,
    pub auto_resume: Option<bool>,
    pub sound_source_index: Option<i32>,
    pub sound_source_name: Option<String>,
    pub log_level: Option<LevelFilter>,
    pub ssdp_interval_mins: Option<f64>,
    pub use_wave_format: Option<bool>,
    pub bits_per_sample: Option<u16>,
    pub streaming_format: Option<StreamingFormat>,
    pub stream_size: Option<StreamSize>,
    pub player_ip: Option<String>,
    pub ip_address: Option<String>,
    pub active_players: Option<Vec<String>>,
    pub inject_silence: Option<bool>,
    pub serve_only: Option<bool>,
    pub volume: Option<u8>,
    pub upfront_buffer: Option<u32>,
    pub language: Option<String>,
}

impl Args {
    // print usage & bail out
    pub fn usage(&self) {
        // note: -C is handled in Configuration.read_config(), not here
        println!(
            r#"
Recognized options:
    -h (--help) : print usage
    -n (--no_run) : dry-run, don't start streaming
    -C (--configfile) string : alternative full pathname of configfile
    -c (--config_id) string : config_id [_cli]
    -p (--server_port) u16 : server_port [5901]
    -r (--auto_resume) bool : auto_resume [false]
    -s (--sound_source) u16|string  : sound_source index or name [os default]
    -l (--log_level) string : log_level (info/debug) [info]
    -i (--ssdp_interval) i32 : ssdp_interval_mins [10]
    -b (--bits) u16 : bits_per_sample (16/24) [16]
    -f (--format) string : streaming_format (lpcm/flac/wav/rf64) [LPCM]
       optionally followed by a plus sign and a streamsize [LPCM+U64maxNotChunked] 
    -o (--player_ip) string : (comma-separated) player ip address(es) [last used player]
    -e (--ip_address) string : ip address of the network interface [last used]
    -S (--inject_silence) bool : inject silence into stream (bool) [false]
    -x (--serve_only) bool: only run the music server, no ssdp discovery [false]
    -v (--volume) u8 : desired player volume between 0 and 100 [unchanged]
    -u (--upfront_buffer) u32 : initial buffering in milliseconds [0]
    -L (--language) string : UI language code (e.g. en-US, nl-BE) [en-US]
"#
        );
        println!("{self:?}");
    }

    // parse commandline arguments

    pub fn parse(&mut self) -> Result<(), Vec<String>> {
        let mut errors: Vec<String> = Vec::new();
        let mut argparser = Parser::from_env();
        while let Some(arg) = argparser.next().unwrap() {
            match arg {
                Short('h') | Long("help") => {
                    self.usage();
                }
                Short('n') | Long("no_run") => {
                    self.dry_run = Some(true);
                }
                Short('c') | Long("config_id") => {
                    if let Ok(id) = argparser.value() {
                        self.config_id = Some(id.string().unwrap_or_default());
                    };
                }
                Short('p') | Long("server_port") => {
                    if let Ok(port) = argparser.value() {
                        match port.parse() {
                            Ok(p) => self.server_port = Some(p),
                            Err(x) => errors.push(format!("Invalid server port: {x}.")),
                        }
                    }
                }
                Short('r') | Long("auto_resume") => {
                    if let Ok(auto_resume) = argparser.value() {
                        match auto_resume
                            .string()
                            .unwrap_or_default()
                            .sanitize_bool()
                            .parse()
                        {
                            Ok(v) => self.auto_resume = Some(v),
                            Err(x) => errors.push(format!("Invalid value for auto_resume: {x}.")),
                        }
                    } else {
                        self.auto_resume = Some(true);
                    }
                }
                Short('s') | Long("sound_source") => {
                    if let Ok(ssi) = argparser.value() {
                        // numeric = the index, otherwise the name
                        let ss_idx_or_nm = ssi.to_str();
                        if let Some(si) = ss_idx_or_nm {
                            if si.chars().all(|c| c.is_ascii_digit()) {
                                self.sound_source_index =
                                    Some(si.parse::<i32>().unwrap_or_default());
                                self.sound_source_name = None;
                            } else {
                                self.sound_source_name = Some(si.to_string());
                                self.sound_source_index = None;
                            }
                        }
                    }
                }
                Short('l') | Long("log_level") => {
                    if let Ok(level) = argparser.value() {
                        let loglevel = level.string().unwrap_or_default();
                        match loglevel.to_ascii_uppercase().as_str() {
                            "INFO" => self.log_level = Some(LevelFilter::Info),
                            "DEBUG" => self.log_level = Some(LevelFilter::Debug),
                            x => errors.push(format!("Invalid log_level (info or debug): {x}.")),
                        }
                    }
                }
                Short('i') | Long("ssdp_interval") => {
                    if let Ok(interval) = argparser.value() {
                        match interval.parse() {
                            Ok(v) => self.ssdp_interval_mins = Some(v),
                            Err(x) => errors.push(format!("Invalid SSDP interval: {x}.")),
                        }
                    }
                }
                Short('b') | Long("bits_per_sample") => {
                    if let Ok(bps) = argparser.value() {
                        match bps.parse::<u16>() {
                            Ok(n @ (16 | 24)) => self.bits_per_sample = Some(n),
                            Ok(n) => errors.push(format!("Invalid bps (16/24): {n}.")),
                            Err(x) => errors.push(format!("Invalid bps (16/24): {x}.")),
                        }
                    }
                }
                Short('f') | Long("format") => {
                    if let Ok(fmt) = argparser.value() {
                        let streaming_format = fmt.string().unwrap_or_default();
                        let (format, streamsize) = if streaming_format.contains('+') {
                            let parts: Vec<&str> = streaming_format.split('+').collect();
                            (parts[0], parts[1])
                        } else {
                            (streaming_format.as_str(), "")
                        };
                        match format.to_ascii_uppercase().as_str() {
                            "WAV" => {
                                self.streaming_format = Some(StreamingFormat::Wav);
                                self.use_wave_format = Some(true);
                            }
                            "RF64" => {
                                self.streaming_format = Some(StreamingFormat::Rf64);
                                self.use_wave_format = Some(true);
                            }
                            "LPCM" => {
                                self.streaming_format = Some(StreamingFormat::Lpcm);
                            }
                            "FLAC" => {
                                self.streaming_format = Some(StreamingFormat::Flac);
                            }
                            x => errors.push(format!("Invalid streaming_format {x}.")),
                        }
                        if !streamsize.is_empty() {
                            match streamsize.to_ascii_uppercase().as_str() {
                                "NONECHUNKED" => self.stream_size = Some(StreamSize::NoneChunked),
                                "U32MAXCHUNKED" => {
                                    self.stream_size = Some(StreamSize::U32maxChunked)
                                }
                                "U32MAXNOTCHUNKED" => {
                                    self.stream_size = Some(StreamSize::U32maxNotChunked)
                                }
                                "U64MAXCHUNKED" => {
                                    self.stream_size = Some(StreamSize::U64maxChunked)
                                }
                                "U64MAXNOTCHUNKED" => {
                                    self.stream_size = Some(StreamSize::U64maxNotChunked)
                                }
                                x => errors.push(format!(
                                    "Invalid streamsize {x}. Valid options: NONECHUNKED,U32MAXCHUNKED,U32MAXNOTCHUNKED,U64MAXCHUNKED,U64MAXNOTCHUNKED."
                                )),
                            }
                        }
                    }
                }
                Short('o') | Long("player") => {
                    if let Ok(player) = argparser.value() {
                        let output = player.string().unwrap_or_default();
                        let active_players = output
                            .split(',')
                            .map(|x| x.to_string())
                            .collect::<Vec<String>>();
                        self.player_ip = Some(active_players[0].clone());
                        self.active_players = Some(active_players);
                    }
                }
                Short('e') | Long("ip_address") => {
                    if let Ok(ip) = argparser.value() {
                        let ip = ip.string().unwrap_or_default();
                        if let Ok(_addr) = ip.parse::<IpAddr>() {
                            self.ip_address = Some(ip);
                        } else {
                            errors.push(format!("Invalid ip address: {ip}."));
                        }
                    }
                }
                Short('S') | Long("inject_silence") => {
                    if let Ok(inject) = argparser.value() {
                        match inject.string().unwrap_or_default().sanitize_bool().parse() {
                            Ok(v) => self.inject_silence = Some(v),
                            Err(x) => errors.push(format!("Invalid inject silence flag: {x}.")),
                        }
                    } else {
                        errors.push("Cannot parse Inject Silence: missing value.".to_string());
                    }
                }
                Short('x') | Long("serve_only") => {
                    self.serve_only = Some(true);
                }
                Short('v') | Long("volume") => {
                    if let Ok(vol) = argparser.value() {
                        match vol.parse::<u8>() {
                            Ok(v) if v <= 100 => self.volume = Some(v),
                            Ok(v) => errors.push(format!("Invalid volume (0-100): {v}.")),
                            Err(x) => errors.push(format!("Invalid volume: {x}.")),
                        }
                    }
                }
                Short('u') | Long("upfront_buffer") => {
                    if let Ok(buffer) = argparser.value() {
                        match buffer.parse::<u32>() {
                            Ok(b) => self.upfront_buffer = Some(b),
                            Err(x) => errors.push(format!("Invalid upfront buffer msec: {x}.")),
                        }
                    }
                }
                Short('L') | Long("language") => {
                    if let Ok(lang) = argparser.value() {
                        self.language = Some(lang.string().unwrap_or_default());
                    }
                }
                _ => (),
            }
        }
        println!("{self:?}\n");
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
