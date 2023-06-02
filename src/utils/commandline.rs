use std::net::IpAddr;

use lexopt::{
    Arg::{Long, Short},
    Parser, ValueExt,
};
use log::LevelFilter;

use crate::enums::streaming::StreamingFormat;

#[derive(Clone, Debug)]
pub struct Args {
    pub dry_run: Option<bool>,
    pub config_id: Option<String>,
    pub server_port: Option<u16>,
    pub auto_resume: Option<bool>,
    pub sound_source_index: Option<i32>,
    pub log_level: Option<LevelFilter>,
    pub ssdp_interval_mins: Option<f64>,
    pub auto_reconnect: Option<bool>,
    pub disable_chunked: Option<bool>,
    pub use_wave_format: Option<bool>,
    pub bits_per_sample: Option<u16>,
    pub streaming_format: Option<StreamingFormat>,
    pub player_ip: Option<String>,
    pub ip_address: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self::new()
    }
}

impl Args {
    pub fn new() -> Args {
        Args {
            dry_run: None,
            config_id: None,
            server_port: None,
            auto_resume: None,
            sound_source_index: None,
            log_level: None,
            ssdp_interval_mins: None,
            auto_reconnect: None,
            disable_chunked: None,
            use_wave_format: None,
            bits_per_sample: None,
            streaming_format: None,
            player_ip: None,
            ip_address: None,
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
    -a (--auto_reconnect) bool : auto reconnect [true]
    -r (--auto_resume) bool : auto_resume [false]
    -s (--sound_source) u16 : sound_source index [os default]
    -l (--log_level) string : log_level (info/debug) [info]
    -i (--ssdp_interval) i32 : ssdp_interval_mins [10]
    -d (--disable_chunked) bool : disable_chunked encoding [true]
    -b (--bits) u16 : bits_per_sample (16/24) [16]
    -f (--format) string : streaming_format (lpcm/flac/wav) [LPCM]
    -o (--player_ip) string : the player ip address [last used player]
    -e (--ip_address) string : ip address of the network interface [last used]
"#
        );
        println!("{:?}", self);
        std::process::exit(0);
    }

    // parse commandline arguments
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
                Short('a') | Long("auto_reconnect") => {
                    if let Ok(auto_reconnect) = argparser.value() {
                        self.auto_reconnect = Some(auto_reconnect.parse().unwrap());
                    }
                }
                Short('r') | Long("auto_resume") => {
                    if let Ok(auto_resume) = argparser.value() {
                        self.auto_resume = Some(auto_resume.parse().unwrap());
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
                        match loglevel.as_str() {
                            "info" | "Info" | "INFO" => self.log_level = Some(LevelFilter::Info),
                            "debug" | "Debug" | "DEBUG" => {
                                self.log_level = Some(LevelFilter::Debug)
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
                Short('d') | Long("disable_chunked") => {
                    if let Ok(dc) = argparser.value() {
                        self.disable_chunked = Some(dc.parse().unwrap());
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
                        match streaming_format.as_str() {
                            "WAV" | "wav" | "Wav" => {
                                self.streaming_format = Some(StreamingFormat::Wav);
                                self.use_wave_format = Some(true);
                            }
                            "LPCM" | "lpcm" | "Lpcm" => {
                                self.streaming_format = Some(StreamingFormat::Lpcm)
                            }
                            "FLAC" | "flac" | "Flac" => {
                                self.streaming_format = Some(StreamingFormat::Flac)
                            }
                            _ => {
                                println!("invalid streaming_format {streaming_format}");
                                self.usage();
                            }
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
                _ => (),
            }
        }
        println!("{:?}\n", self);
        self.clone()
    }
}