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

use crate::{enums::streaming::*, globals::statics::SAMPLE_RATES, utils::traits::SanitizeArg};

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
    pub sample_rate: Option<u32>,
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
    -R (--sample_rate) u32 : sample rate (44100/48000/88200/96000/176400/192000/352800/384000) [configured/44100]
"#
        );
        println!("{self:?}");
    }

    // parse commandline arguments

    pub fn parse(&mut self) -> Result<(), Vec<String>> {
        self.parse_from(Parser::from_env())
    }

    pub(crate) fn parse_from(&mut self, mut argparser: Parser) -> Result<(), Vec<String>> {
        let mut errors: Vec<String> = Vec::new();
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
                        let lang = lang.string().unwrap_or_default();
                        let available = crate::utils::i18n::available_languages();
                        if available.contains(&lang) {
                            self.language = Some(lang);
                        } else {
                            errors.push(format!(
                                "Unknown language '{lang}', available: {available:?}."
                            ));
                        }
                    }
                }
                Short('R') | Long("sample_rate") => {
                    if let Ok(rate) = argparser.value() {
                        match rate.parse::<u32>() {
                            Ok(n) if SAMPLE_RATES.contains(&n) => self.sample_rate = Some(n),
                            Ok(n) => {
                                let valid = SAMPLE_RATES
                                    .iter()
                                    .map(|r| r.to_string())
                                    .collect::<Vec<_>>()
                                    .join("/");
                                errors.push(format!("Invalid sample rate ({valid}): {n}."));
                            }
                            Err(x) => errors.push(format!("Invalid sample rate: {x}.")),
                        }
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

#[cfg(test)]
#[cfg(feature = "cli")]
mod tests {
    use super::*;
    use lexopt::Parser;

    fn parse(args: &[&str]) -> Result<Args, Vec<String>> {
        let mut a = Args::default();
        a.parse_from(Parser::from_iter(args))?;
        Ok(a)
    }

    // --- flags without values ---

    #[test]
    fn dry_run_short() {
        let a = parse(&["prog", "-n"]).unwrap();
        assert_eq!(a.dry_run, Some(true));
    }

    #[test]
    fn dry_run_long() {
        let a = parse(&["prog", "--no_run"]).unwrap();
        assert_eq!(a.dry_run, Some(true));
    }

    #[test]
    fn serve_only() {
        let a = parse(&["prog", "-x"]).unwrap();
        assert_eq!(a.serve_only, Some(true));
        let a = parse(&["prog", "--serve_only"]).unwrap();
        assert_eq!(a.serve_only, Some(true));
    }

    // --- config_id ---

    #[test]
    fn config_id() {
        let a = parse(&["prog", "-c", "myconfig"]).unwrap();
        assert_eq!(a.config_id.as_deref(), Some("myconfig"));
        let a = parse(&["prog", "--config_id", "myconfig"]).unwrap();
        assert_eq!(a.config_id.as_deref(), Some("myconfig"));
    }

    // --- server_port ---

    #[test]
    fn server_port_valid() {
        let a = parse(&["prog", "-p", "5901"]).unwrap();
        assert_eq!(a.server_port, Some(5901));
    }

    #[test]
    fn server_port_invalid() {
        let errs = parse(&["prog", "-p", "notaport"]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Invalid server port")));
    }

    // --- auto_resume ---

    #[test]
    fn auto_resume_with_value_true() {
        for v in [
            "true", "True", "TRUE", "T", "t", "yes", "Yes", "YES", "Y", "y", "1",
        ] {
            let a = parse(&["prog", "-r", v]).unwrap();
            assert_eq!(a.auto_resume, Some(true), "failed for {v}");
        }
    }

    #[test]
    fn auto_resume_with_value_false() {
        for v in [
            "false", "False", "FALSE", "F", "f", "no", "No", "NO", "N", "n", "0",
        ] {
            let a = parse(&["prog", "-r", v]).unwrap();
            assert_eq!(a.auto_resume, Some(false), "failed for {v}");
        }
    }

    #[test]
    fn auto_resume_no_value_defaults_true() {
        // lexopt's value() consumes the next token even if it looks like a flag,
        // so the only way to hit the else-branch (default true) is when -r is the last arg
        let a = parse(&["prog", "-r"]).unwrap();
        assert_eq!(a.auto_resume, Some(true));
    }

    // --- sound_source ---

    #[test]
    fn sound_source_numeric() {
        let a = parse(&["prog", "-s", "2"]).unwrap();
        assert_eq!(a.sound_source_index, Some(2));
        assert!(a.sound_source_name.is_none());
    }

    #[test]
    fn sound_source_by_name() {
        let a = parse(&["prog", "-s", "Speakers"]).unwrap();
        assert_eq!(a.sound_source_name.as_deref(), Some("Speakers"));
        assert!(a.sound_source_index.is_none());
    }

    // --- log_level ---

    #[test]
    fn log_level_info() {
        use log::LevelFilter;
        let a = parse(&["prog", "-l", "info"]).unwrap();
        assert_eq!(a.log_level, Some(LevelFilter::Info));
        let a = parse(&["prog", "-l", "INFO"]).unwrap();
        assert_eq!(a.log_level, Some(LevelFilter::Info));
    }

    #[test]
    fn log_level_debug() {
        use log::LevelFilter;
        let a = parse(&["prog", "-l", "debug"]).unwrap();
        assert_eq!(a.log_level, Some(LevelFilter::Debug));
    }

    #[test]
    fn log_level_invalid() {
        let errs = parse(&["prog", "-l", "trace"]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Invalid log_level")));
    }

    // --- ssdp_interval ---

    #[test]
    fn ssdp_interval_valid() {
        let a = parse(&["prog", "-i", "10"]).unwrap();
        assert_eq!(a.ssdp_interval_mins, Some(10.0));
    }

    #[test]
    fn ssdp_interval_invalid() {
        let errs = parse(&["prog", "-i", "nope"]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Invalid SSDP interval")));
    }

    // --- bits_per_sample ---

    #[test]
    fn bits_16_and_24_valid() {
        assert_eq!(
            parse(&["prog", "-b", "16"]).unwrap().bits_per_sample,
            Some(16)
        );
        assert_eq!(
            parse(&["prog", "-b", "24"]).unwrap().bits_per_sample,
            Some(24)
        );
    }

    #[test]
    fn bits_other_values_invalid() {
        for b in ["8", "32", "0"] {
            let errs = parse(&["prog", "-b", b]).unwrap_err();
            assert!(
                errs.iter().any(|e| e.contains("Invalid bps")),
                "expected error for {b}"
            );
        }
    }

    #[test]
    fn bits_non_numeric_invalid() {
        let errs = parse(&["prog", "-b", "abc"]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Invalid bps")));
    }

    // --- streaming format ---

    #[test]
    fn format_lpcm() {
        let a = parse(&["prog", "-f", "lpcm"]).unwrap();
        assert_eq!(a.streaming_format, Some(StreamingFormat::Lpcm));
        assert_eq!(a.use_wave_format, None);
    }

    #[test]
    fn format_wav_sets_wave_flag() {
        let a = parse(&["prog", "-f", "WAV"]).unwrap();
        assert_eq!(a.streaming_format, Some(StreamingFormat::Wav));
        assert_eq!(a.use_wave_format, Some(true));
    }

    #[test]
    fn format_rf64_sets_wave_flag() {
        let a = parse(&["prog", "-f", "rf64"]).unwrap();
        assert_eq!(a.streaming_format, Some(StreamingFormat::Rf64));
        assert_eq!(a.use_wave_format, Some(true));
    }

    #[test]
    fn format_flac() {
        let a = parse(&["prog", "-f", "FLAC"]).unwrap();
        assert_eq!(a.streaming_format, Some(StreamingFormat::Flac));
    }

    #[test]
    fn format_with_streamsize() {
        let a = parse(&["prog", "-f", "LPCM+U64maxNotChunked"]).unwrap();
        assert_eq!(a.streaming_format, Some(StreamingFormat::Lpcm));
        assert_eq!(a.stream_size, Some(StreamSize::U64maxNotChunked));
    }

    #[test]
    fn format_all_streamsizes() {
        let cases = [
            ("LPCM+NoneChunked", StreamSize::NoneChunked),
            ("LPCM+U32maxChunked", StreamSize::U32maxChunked),
            ("LPCM+U32maxNotChunked", StreamSize::U32maxNotChunked),
            ("LPCM+U64maxChunked", StreamSize::U64maxChunked),
            ("LPCM+U64maxNotChunked", StreamSize::U64maxNotChunked),
        ];
        for (arg, expected) in cases {
            let a = parse(&["prog", "-f", arg]).unwrap();
            assert_eq!(a.stream_size, Some(expected), "failed for {arg}");
        }
    }

    #[test]
    fn format_invalid() {
        let errs = parse(&["prog", "-f", "mp3"]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Invalid streaming_format")));
    }

    #[test]
    fn format_invalid_streamsize() {
        let errs = parse(&["prog", "-f", "LPCM+BADSIZE"]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Invalid streamsize")));
    }

    // --- player / active_players ---

    #[test]
    fn player_single() {
        let a = parse(&["prog", "-o", "192.168.1.10"]).unwrap();
        assert_eq!(a.player_ip.as_deref(), Some("192.168.1.10"));
        assert_eq!(
            a.active_players.as_deref(),
            Some(&["192.168.1.10".to_string()][..])
        );
    }

    #[test]
    fn player_multiple_comma_separated() {
        let a = parse(&["prog", "-o", "192.168.1.10,192.168.1.20"]).unwrap();
        assert_eq!(a.player_ip.as_deref(), Some("192.168.1.10"));
        let players = a.active_players.unwrap();
        assert_eq!(players.len(), 2);
        assert_eq!(players[1], "192.168.1.20");
    }

    // --- ip_address ---

    #[test]
    fn ip_address_valid_v4() {
        let a = parse(&["prog", "-e", "192.168.0.1"]).unwrap();
        assert_eq!(a.ip_address.as_deref(), Some("192.168.0.1"));
    }

    #[test]
    fn ip_address_valid_v6() {
        let a = parse(&["prog", "-e", "::1"]).unwrap();
        assert_eq!(a.ip_address.as_deref(), Some("::1"));
    }

    #[test]
    fn ip_address_invalid() {
        let errs = parse(&["prog", "-e", "not.an.ip"]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Invalid ip address")));
    }

    // --- inject_silence ---

    #[test]
    fn inject_silence_true() {
        let a = parse(&["prog", "-S", "true"]).unwrap();
        assert_eq!(a.inject_silence, Some(true));
    }

    #[test]
    fn inject_silence_false() {
        let a = parse(&["prog", "-S", "false"]).unwrap();
        assert_eq!(a.inject_silence, Some(false));
    }

    // --- volume ---

    #[test]
    fn volume_valid_boundaries() {
        assert_eq!(parse(&["prog", "-v", "0"]).unwrap().volume, Some(0));
        assert_eq!(parse(&["prog", "-v", "100"]).unwrap().volume, Some(100));
        assert_eq!(parse(&["prog", "-v", "50"]).unwrap().volume, Some(50));
    }

    #[test]
    fn volume_over_100_invalid() {
        let errs = parse(&["prog", "-v", "101"]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Invalid volume")));
    }

    #[test]
    fn volume_non_numeric_invalid() {
        let errs = parse(&["prog", "-v", "loud"]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Invalid volume")));
    }

    // --- upfront_buffer ---

    #[test]
    fn upfront_buffer_valid() {
        let a = parse(&["prog", "-u", "500"]).unwrap();
        assert_eq!(a.upfront_buffer, Some(500));
    }

    #[test]
    fn upfront_buffer_invalid() {
        let errs = parse(&["prog", "-u", "abc"]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Invalid upfront buffer")));
    }

    // --- sample_rate ---

    #[test]
    fn sample_rate_valid() {
        for &rate in SAMPLE_RATES.iter() {
            let s = rate.to_string();
            let a = parse(&["prog", "-R", &s]).unwrap();
            assert_eq!(a.sample_rate, Some(rate), "failed for rate {rate}");
        }
    }

    #[test]
    fn sample_rate_invalid_value() {
        let errs = parse(&["prog", "-R", "22050"]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Invalid sample rate")));
    }

    #[test]
    fn sample_rate_non_numeric() {
        let errs = parse(&["prog", "-R", "fast"]).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("Invalid sample rate")));
    }

    // --- error accumulation ---

    #[test]
    fn multiple_errors_all_reported() {
        let errs = parse(&["prog", "-b", "32", "-v", "200", "-R", "22050"]).unwrap_err();
        assert_eq!(errs.len(), 3, "expected 3 errors, got: {errs:?}");
    }

    // --- combined valid args ---

    #[test]
    fn combined_args() {
        let a = parse(&[
            "prog", "-n", "-b", "24", "-f", "FLAC", "-v", "75", "-p", "5900", "-x",
        ])
        .unwrap();
        assert_eq!(a.dry_run, Some(true));
        assert_eq!(a.bits_per_sample, Some(24));
        assert_eq!(a.streaming_format, Some(StreamingFormat::Flac));
        assert_eq!(a.volume, Some(75));
        assert_eq!(a.server_port, Some(5900));
        assert_eq!(a.serve_only, Some(true));
    }

    // --- long flag aliases ---

    #[test]
    fn long_flags_work() {
        let a = parse(&[
            "prog",
            "--bits_per_sample",
            "16",
            "--volume",
            "10",
            "--server_port",
            "8080",
        ])
        .unwrap();
        assert_eq!(a.bits_per_sample, Some(16));
        assert_eq!(a.volume, Some(10));
        assert_eq!(a.server_port, Some(8080));
    }
}
