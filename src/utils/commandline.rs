use lexopt::{
    Arg::{Long, Short},
    Parser, ValueExt,
};
use log::LevelFilter;

use crate::enums::streaming::StreamingFormat;

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
            config_id: None,
        }
    }

    // commandline:
    // -h (--help) : print usage
    // -p (--server_port) u16 : server_port
    // -r (--auto_resume) : auto_resume
    // -s (--sound_source) string : sound_source
    // -l (--log_level) string : log_level
    // -i (--ssdp_interval) i32 : ssdp_interval_mins
    // -d (--disable_chunked) : disable_chunked
    // -u (--use_wav) : use_wav_format
    // -b (--bits) u16 : bits_per_sample
    // -f (--format) string : streaming_format
    // -c (--configuration) string : config_id

    pub fn parse_args(&mut self) {
        let mut argparser = Parser::from_env();
        while let Some(arg) = argparser.next().unwrap() {
            match arg {
                Short('h') | Long("help") => {
                    println!(
                        r#"
Recognized options:
    -h (--help) : print usage 
    -p (--server_port) u16 : server_port
    -r (--auto_resume) : auto_resume
    -s (--sound_source) string : sound_source
    -l (--log_level) string : log_level
    -i (--ssdp_interval) i32 : ssdp_interval_mins
    -d (--disable_chunked) : disable_chunked
    -u (--use_wav) : use_wav_format
    -b (--bits) u16 : bits_per_sample
    -f (--format) string : streaming_format
    -c (--configuration) string : config_id
"#
                    );
                }
                Short('c') | Long("configuration") => {
                    if let Ok(id) = argparser.value() {
                        self.config_id = Some(id.string().unwrap_or_default());
                    };
                }
                Short('p') | Long("port") => {
                    if let Ok(port) = argparser.value() {
                        self.server_port = Some(port.parse().unwrap());
                    }
                }
                _ => (),
            }
        }
    }
}
