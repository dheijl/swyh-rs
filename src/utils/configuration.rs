use ini::Ini;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Configuration {
    pub auto_resume: bool,
    pub sound_source: String,
}

impl Configuration {
    fn get_config(&self) -> PathBuf {
        let hd = dirs::home_dir().unwrap_or_default();
        let config_dir = Path::new(&hd).join("swyh-rs");
        if !Path::new(&config_dir).exists() {
            fs::create_dir_all(&config_dir).unwrap();
        }
        let configfile = Path::new(&config_dir).join("config.ini");

        configfile
    }

    pub fn new() -> Configuration {
        Configuration {
            auto_resume: false,
            sound_source: "None".to_string(),
        }
    }

    pub fn read_config(mut self) -> Configuration {
        let configfile = self.get_config();

        if !Path::new(&configfile).exists() {
            eprintln!("Creating empty config {}", configfile.display());
            let mut conf = Ini::new();
            conf.with_section(Some("Configuration"))
                .set("AutoResume", "false")
                .set("SoundCard", "None");
            conf.write_to_file(&configfile).unwrap();
        }

        let conf = match Ini::load_from_file(&configfile) {
            Ok(conf) => conf,
            Err(_) => Ini::new(),
        };
        match conf.get_from_or(Some("Configuration"), "AutoResume", "false") {
            "true" | "True" | "TRUE" | "1" | "T" | "t" => self.auto_resume = true,
            _ => self.auto_resume = false,
        }
        self.sound_source = conf
            .get_from_or(Some("Configuration"), "SoundCard", "None")
            .to_string();

        self
    }

    pub fn update_config(&self) -> io::Result<()> {
        let configfile = self.get_config();
        let mut conf = match Ini::load_from_file(&configfile) {
            Ok(conf) => conf,
            Err(_) => Ini::new(),
        };
        conf.with_section(Some("Configuration"))
            .set(
                "AutoResume",
                if self.auto_resume { "true" } else { "false" },
            )
            .set("SoundCard", &self.sound_source);
        conf.write_to_file(&configfile)
    }
}
