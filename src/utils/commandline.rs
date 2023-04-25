use lexopt::{
    Arg::{Long, Short},
    Parser, ValueExt,
};
use log::LevelFilter;

use crate::enums::streaming::StreamingFormat;

#[derive(Clone, Debug)]
pub struct Args {
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
    pub player: Option<String>,
    pub config_id: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self::new()
    }
}

impl Args {
    pub fn new() -> Args {
        Args {
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
            player: None,
            config_id: None,
        }
    }

    // parse commandline arguments
    pub fn parse(&mut self) -> Args {
        let mut argparser = Parser::from_env();
        while let Some(arg) = argparser.next().unwrap() {
            match arg {
                Short('h') | Long("help") => {
                    println!(
                        r#"
Recognized options:
    -h (--help) : print usage 
    -p (--server_port) u16 : server_port [5901]
    -a (--auto_reconnect) bool : auto reconnect [true]
    -r (--auto_resume) bool : auto_resume [false]
    -s (--sound_source) u16 : sound_source index [os default]
    -l (--log_level) string : log_level [info]
    -i (--ssdp_interval) i32 : ssdp_interval_mins [10]
    -d (--disable_chunked) bool : disable_chunked encoding [true]
    -u (--use_wav) : use_wav_format [false]
    -b (--bits) u16 : bits_per_sample [16]
    -f (--format) string : streaming_format [LPCM]
    -o (--player) string : the player [last used renderer]
    -c (--config_id) string : config_id [_cli]
"#
                    );
                    println!("{:?}", self);
                    std::process::exit(0);
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
                            _ => println!("log_level not info or debug"),
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
                Short('u') | Long("use_wav") => {
                    if let Ok(use_wav) = argparser.value() {
                        self.use_wave_format = Some(use_wav.parse().unwrap());
                    }
                }
                Short('b') | Long("bits_per_sample") => {
                    if let Ok(bps) = argparser.value() {
                        let n: u16 = bps.parse().unwrap();
                        if let 16 | 24 = n {
                            self.bits_per_sample = Some(n);
                        } else {
                            println!("bits_per_sample not 16 or 24");
                        }
                    }
                }
                Short('o') | Long("player") => {
                    if let Ok(player) = argparser.value() {
                        self.player = Some(player.string().unwrap_or_default());
                    }
                }
                _ => (),
            }
        }
        println!("{:?}\n", self);
        self.clone()
    }
}
