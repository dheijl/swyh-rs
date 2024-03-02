#![cfg(feature = "cli")]
use std::net::IpAddr;

use lexopt::{
    Arg::{Long, Short},
    Parser, ValueExt,
};
use log::LevelFilter;

use crate::{enums::streaming::*, utils::traits::SanitizeArg};

#[derive(Clone, Debug)]
pub struct Args {
    pub dry_run: Option<bool>,
    pub config_id: Option<String>,
    pub server_port: Option<u16>,
    pub auto_resume: Option<bool>,
    pub sound_source_index: Option<i32>,
    pub log_level: Option<LevelFilter>,
    pub ssdp_interval_mins: Option<f64>,
    pub use_wave_format: Option<bool>,
    pub bits_per_sample: Option<u16>,
    pub streaming_format: Option<StreamingFormat>,
    pub stream_size: Option<StreamSize>,
    pub player_ip: Option<String>,
    pub ip_address: Option<String>,
    pub inject_silence: Option<bool>,
    pub serve_only: Option<bool>,
    pub volume: Option<u8>,
}

impl Default for Args {
    fn default() -> Self {
        Self::new()
    }
}

impl Args {
    #[must_use]
    pub fn new() -> Args {
        Args {
            dry_run: None,
            config_id: None,
            server_port: None,
            auto_resume: None,
            sound_source_index: None,
            log_level: None,
            ssdp_interval_mins: None,
            use_wave_format: None,
            bits_per_sample: None,
            streaming_format: None,
            stream_size: None,
            player_ip: None,
            ip_address: None,
            inject_silence: None,
            serve_only: None,
            volume: None,
        }
    }

    // print usage & bail out
    fn usage(&self) {
        println!(
            r#"
Recognized options:
    -h (--help) : print usage
    -n (--no_run) : dry-run, don't start streaming
    -c (--config_id) string : config_id [_cli]
    -p (--server_port) u16 : server_port [5901]
    -r (--auto_resume) bool : auto_resume [false]
    -s (--sound_source) u16 : sound_source index [os default]
    -l (--log_level) string : log_level (info/debug) [info]
    -i (--ssdp_interval) i32 : ssdp_interval_mins [10]
    -b (--bits) u16 : bits_per_sample (16/24) [16]
    -f (--format) string : streaming_format (lpcm/flac/wav/rf64) [LPCM]
       optionally followed by a plus sign and a streamsize[LPCM+U64maxNotChunked] 
    -o (--player_ip) string : the player ip address [last used player]
    -e (--ip_address) string : ip address of the network interface [last used]
    -S (--inject_silence) bool : inject silence into stream (bool) [false]
    -x (--serve_only) bool: only run the music server, no ssdp discovery [false]
    -v (--volume) u8 : desired player volume between 0 and 100 [unchanged]
"#
        );
        println!("{self:?}");
        std::process::exit(0);
    }

    // parse commandline arguments
    #[must_use]
    pub fn parse(&mut self) -> Args {
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
                        self.server_port = Some(port.parse().unwrap());
                    }
                }
                Short('r') | Long("auto_resume") => {
                    if let Ok(auto_resume) = argparser.value() {
                        self.auto_resume = Some(
                            auto_resume
                                .string()
                                .unwrap()
                                .sanitize_bool()
                                .parse()
                                .unwrap(),
                        );
                    } else {
                        self.auto_resume = Some(true);
                    }
                }
                Short('s') | Long("sound_source_index") => {
                    if let Ok(ssi) = argparser.value() {
                        self.sound_source_index = Some(ssi.parse().unwrap());
                    }
                }
                Short('l') | Long("log_level") => {
                    if let Ok(level) = argparser.value() {
                        let loglevel = level.string().unwrap_or_default();
                        match loglevel.to_uppercase().as_str() {
                            "INFO" => self.log_level = Some(LevelFilter::Info),
                            "DEBUG" => {
                                self.log_level = Some(LevelFilter::Debug);
                            }
                            _ => {
                                println!("log_level not info or debug");
                                self.usage();
                            }
                        }
                    }
                }
                Short('i') | Long("ssdp_interval") => {
                    if let Ok(interval) = argparser.value() {
                        self.ssdp_interval_mins = Some(interval.parse().unwrap());
                    }
                }
                Short('b') | Long("bits_per_sample") => {
                    if let Ok(bps) = argparser.value() {
                        let n: u16 = bps.parse().unwrap();
                        if let 16 | 24 = n {
                            self.bits_per_sample = Some(n);
                        } else {
                            println!("bits_per_sample not 16 or 24");
                            self.usage();
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
                        match format.to_uppercase().as_str() {
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
                            _ => {
                                println!("invalid streaming_format {streaming_format}");
                                self.usage();
                            }
                        }
                        if !streamsize.is_empty() {
                            self.stream_size = match streamsize.to_uppercase().as_str() {
                                "NONECHUNKED" => Some(StreamSize::NoneChunked),
                                "U32MAXCHUNKED" => Some(StreamSize::U32maxChunked),
                                "U32MAXNOTCHUNKED" => Some(StreamSize::U32maxNotChunked),
                                "U64MAXCHUNKED" => Some(StreamSize::U64maxChunked),
                                "U64MAXNOTCHUNKED" => Some(StreamSize::U64maxNotChunked),
                                _ => {
                                    println!("invalid streamsize {streamsize}");
                                    println!("valid options: NONECHUNKED,U32MAXCHUNKED,U32MAXNOTCHUNKED,U64MAXCHUNKED,U64MAXNOTCHUNKED");
                                    self.usage();
                                    Some(StreamSize::U64maxNotChunked)
                                }
                            };
                        }
                    }
                }
                Short('o') | Long("player") => {
                    if let Ok(player) = argparser.value() {
                        self.player_ip = Some(player.string().unwrap_or_default());
                    }
                }
                Short('e') | Long("ip_address") => {
                    if let Ok(ip) = argparser.value() {
                        let ip = ip.string().unwrap_or_default();
                        if let Ok(_addr) = ip.parse::<IpAddr>() {
                            self.ip_address = Some(ip);
                        } else {
                            println!("invalid ip address {ip}");
                            self.usage();
                        }
                    }
                }
                Short('S') | Long("inject_silence") => {
                    if let Ok(inject) = argparser.value() {
                        self.inject_silence =
                            Some(inject.string().unwrap().sanitize_bool().parse().unwrap());
                    } else {
                        self.inject_silence = Some(true);
                    }
                }
                Short('x') | Long("serve_only") => {
                    self.serve_only = Some(true);
                }
                Short('v') | Long("volume") => {
                    if let Ok(vol) = argparser.value() {
                        let v: u8 = vol.parse().unwrap();
                        if v <= 100 {
                            self.volume = Some(v);
                        }
                    }
                }
                _ => (),
            }
        }
        println!("{self:?}\n");
        self.clone()
    }
}
