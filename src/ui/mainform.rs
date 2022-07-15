use crate::openhome::rendercontrol::StreamInfo;
use crate::ui_log;
use crate::utils::escape::FwSlashPipeEscape;
use crate::Configuration;
use crate::Renderer;
use crate::StreamingFormat;
use crate::WavData;
use crate::CONFIG;
use fltk::{
    app,
    button::{CheckButton, LightButton},
    enums::{Align, Color, Event, FrameType},
    frame::Frame,
    group::{Pack, PackType},
    image::SvgImage,
    input::IntInput,
    menu::MenuButton,
    misc::Progress,
    prelude::*,
    text::{TextBuffer, TextDisplay},
    valuator::Counter,
    window::DoubleWindow,
};
//use fltk_flow::Flow;
use log::{debug, LevelFilter};
use parking_lot::Mutex;
use std::cell::Cell;
use std::collections::HashMap;
use std::net::IpAddr;
use std::rc::Rc;

pub struct MainForm {
    pub wind: DoubleWindow,
    pub auto_resume: CheckButton,
    pub auto_reconnect: CheckButton,
    pub ssdp_interval: Counter,
    pub log_level_choice: MenuButton,
    pub fmt_choice: MenuButton,
    pub b24_bit: CheckButton,
    pub disable_chunked: CheckButton,
    pub show_rms: CheckButton,
    pub rms_mon_l: Progress,
    pub rms_mon_r: Progress,
    pub choose_audio_source_but: MenuButton,
    pub tb: TextDisplay,
    pub buttons: HashMap<String, LightButton>,
    vpack: Pack,
    bwidth: i32,
    bheight: i32,
    btn_index: i32,
    wd: WavData,
    local_addr: IpAddr,
}

impl MainForm {
    pub fn create(
        config: &Configuration,
        config_changed: Rc<Cell<bool>>,
        audio_sources: &[String],
        networks: &[String],
        local_addr: IpAddr,
        wd: &WavData,
        app_version: String,
    ) -> MainForm {
        let title_color: Color = Color::from_u32(0xe6fff0);
        let app = app::App::default().with_scheme(app::Scheme::Gtk);
        app::background(247, 247, 247);
        const WW: i32 = 660;
        const WH: i32 = 660;
        let mut wind = DoubleWindow::default()
            .with_size(WW, WH)
            .with_label(&format!(
                "swyh-rs UPNP/DLNA Media Renderers V{}",
                app_version
            ));

        wind.make_resizable(true);
        wind.size_range(WW, WH * 2 / 3, 0, 0);

        // set window icon
        //        if cfg!(never) {
        let icon_bytes = include_str!("../../assets/swyh-rs logo note-only 16x16.svg");
        if let Ok(icon) = SvgImage::from_data(icon_bytes) {
            wind.set_icon(Some(icon));
        }
        //        }

        wind.end();
        wind.show();

        wind.handle(move |_, _ev| {
            // Event::Hide fires before Event::Close, hiding the Window and preventing the Close handler being called
            // eprintln!("_ev = {:?}, app_event = {:?}", _ev, app::event());
            let ev = app::event();
            match ev {
                Event::Close => {
                    app.quit();
                    std::process::exit(0);
                }
                _ => false,
            }
        });

        const GW: i32 = 600;
        const FW: i32 = 600;
        const XPOS: i32 = 30;
        const YPOS: i32 = 5;

        let mut vpack = Pack::new(XPOS, YPOS, GW, WH - 10, "");
        vpack.make_resizable(false);
        vpack.set_spacing(15);
        vpack.end();
        wind.add(&vpack);

        // title frame
        let mut p1 = Pack::new(0, 0, GW, 25, "");
        p1.end();
        let mut opt_frame = Frame::new(0, 0, 0, 25, "").with_align(Align::Center);
        opt_frame.set_frame(FrameType::BorderBox);
        opt_frame.set_label("Configuration Options");
        opt_frame.set_color(title_color);
        p1.add(&opt_frame);
        vpack.add(&p1);

        // show config option widgets

        // network selection
        let mut pnw = Pack::new(0, 0, GW, 25, "");
        pnw.end();
        let cur_nw = {
            if config.last_network != "None" {
                format!("Active network: {}", config.last_network.clone())
            } else {
                format!("Active network: {local_addr}")
            }
        };
        let mut choose_network_but = MenuButton::new(0, 0, 0, 25, None).with_label(&cur_nw);
        for name in networks.iter() {
            choose_network_but.add_choice(name);
        }
        let rlock = Mutex::new(0);
        let config_ch_flag = config_changed.clone();
        let networks_c = networks.to_vec();
        choose_network_but.set_callback(move |b| {
            let mut recursion = rlock.lock();
            if *recursion > 0 {
                return;
            }
            *recursion += 1;
            let mut conf = CONFIG.write();
            let mut i = b.value();
            if i < 0 {
                return;
            }
            if i as usize >= networks_c.len() {
                i = (networks_c.len() - 1) as i32;
            }
            let name = networks_c[i as usize].clone();
            ui_log(format!(
                "*W*W*> Network changed to {}, restart required!!",
                name
            ));
            conf.last_network = name;
            let _ = conf.update_config();
            b.set_label(&format!("New Network: {}", conf.last_network));
            config_ch_flag.set(true);
            app::awake();
            *recursion -= 1;
        });
        pnw.add(&choose_network_but);
        vpack.add(&pnw);

        // setup audio source choice
        let mut pas = Pack::new(0, 0, GW, 25, "");
        pas.end();
        let cur_audio_src = format!("Audio Source: {}", config.sound_source);
        ui_log("Setup audio sources".to_string());
        let mut choose_audio_source_but =
            MenuButton::new(0, 0, 0, 25, None).with_label(&cur_audio_src);
        for name in audio_sources.iter() {
            choose_audio_source_but.add_choice(&name.fw_slash_pipe_escape());
        }
        let rlock = Mutex::new(0);
        let config_ch_flag = config_changed.clone();
        let audio_sources_c = audio_sources.to_vec();
        choose_audio_source_but.set_callback(move |b| {
            let mut recursion = rlock.lock();
            if *recursion > 0 {
                return;
            }
            *recursion += 1;
            let mut conf = CONFIG.write();
            let mut i = b.value();
            if i < 0 {
                return;
            }
            if i as usize >= audio_sources_c.len() {
                i = (audio_sources_c.len() - 1) as i32;
            }
            let name = audio_sources_c[i as usize].clone();
            ui_log(format!(
                "*W*W*> Audio source changed to {name}, restart required!!"
            ));
            conf.sound_source = name;
            let _ = conf.update_config();
            b.set_label(&format!("New Audio Source: {}", conf.sound_source));
            config_ch_flag.set(true);
            app::awake();
            *recursion -= 1;
        });
        pas.add(&choose_audio_source_but);
        vpack.add(&pas);

        // all other options
        let mut pconfig1 = Pack::new(0, 0, GW, 20, "");
        pconfig1.set_spacing(10);
        pconfig1.set_type(PackType::Horizontal);
        pconfig1.end();

        // auto_resume button for AVTransport autoresume play
        let mut auto_resume = CheckButton::new(0, 0, 0, 0, "Autoresume play");
        if config.auto_resume {
            auto_resume.set(true);
        }
        auto_resume.set_callback(move |b| {
            let mut conf = CONFIG.write();
            if b.is_set() {
                conf.auto_resume = true;
            } else {
                conf.auto_resume = false;
            }
            let _ = conf.update_config();
        });
        pconfig1.add(&auto_resume);

        // AutoReconnect to last renderer on startup button
        let mut auto_reconnect = CheckButton::new(0, 0, 0, 0, "Autoreconnect");
        if config.auto_reconnect {
            auto_reconnect.set(true);
        }
        auto_reconnect.set_callback(move |b| {
            let mut conf = CONFIG.write();
            if b.is_set() {
                conf.auto_reconnect = true;
            } else {
                conf.auto_reconnect = false;
            }
            let _ = conf.update_config();
        });
        pconfig1.add(&auto_reconnect);

        // SSDP interval counter
        let mut ssdp_interval = Counter::new(0, 0, 0, 0, "SSDP Interval (in minutes)");
        ssdp_interval.set_value(config.ssdp_interval_mins);
        let config_ch_flag = config_changed.clone();
        ssdp_interval.handle(move |b, ev| {
            if b.value() < 0.5 {
                b.set_value(0.5);
            }
            match ev {
                Event::Leave | Event::Enter | Event::Unfocus => {
                    let mut conf = CONFIG.write();
                    if (conf.ssdp_interval_mins - b.value()).abs() > 0.09 {
                        conf.ssdp_interval_mins = b.value();
                        ui_log(format!(
                            "*W*W*> ssdp interval changed to {} minutes, restart required!!",
                            conf.ssdp_interval_mins
                        ));
                        let _ = conf.update_config();
                        config_ch_flag.set(true);
                        app::awake();
                    }
                    true
                }
                _ => false,
            }
        });
        pconfig1.add(&ssdp_interval);

        // show log level choice
        let ll = format!("Log Level: {}", config.log_level);
        let mut log_level_choice = MenuButton::default().with_label(&ll);
        let log_levels = vec!["Info", "Debug"];
        for ll in log_levels.iter() {
            log_level_choice.add_choice(ll);
        }
        // apparently this event can recurse on very fast machines
        // probably because it takes some time doing the file I/O, hence recursion lock
        let rlock = Mutex::new(0);
        let config_ch_flag = config_changed.clone();
        log_level_choice.set_callback(move |b| {
            let mut recursion = rlock.lock();
            if *recursion > 0 {
                return;
            }
            *recursion += 1;
            let mut conf = CONFIG.write();
            let i = b.value();
            if i < 0 {
                return;
            }
            let level = log_levels[i as usize];
            ui_log(format!(
                "*W*W*> Log level changed to {}, restart required!!",
                level
            ));
            conf.log_level = level.parse().unwrap_or(LevelFilter::Info);
            let _ = conf.update_config();
            config_ch_flag.set(true);
            let ll = format!("Log Level: {}", conf.log_level);
            b.set_label(&ll);
            app::awake();
            *recursion -= 1;
        });
        pconfig1.add(&log_level_choice);
        pconfig1.auto_layout();
        pconfig1.make_resizable(false);
        vpack.add(&pconfig1);
        // spacer
        let mut pspacer = Pack::new(0, 0, GW, 10, "");
        pspacer.make_resizable(false);
        vpack.add(&pspacer);

        let mut pconfig2 = Pack::new(0, 0, GW, 20, "");
        pconfig2.set_spacing(10);
        pconfig2.set_type(PackType::Horizontal);
        pconfig2.end();

        // streaming format
        let fmt = if let Some(format) = config.streaming_format {
            format!("FMT: {}", format)
        } else {
            "FMT: LPCM".to_string()
        };
        let mut fmt_choice = MenuButton::default().with_label(&fmt);
        let formats = vec![
            StreamingFormat::Lpcm.to_string(),
            StreamingFormat::Wav.to_string(),
            StreamingFormat::Flac.to_string(),
        ];
        for fmt in formats.iter() {
            fmt_choice.add_choice(fmt.as_str());
        }
        // apparently this event can recurse on very fast machines
        // probably because it takes some time doing the file I/O, hence recursion lock
        let rlock = Mutex::new(0);
        let config_ch_flag = config_changed.clone();
        fmt_choice.set_callback(move |b| {
            let mut recursion = rlock.lock();
            if *recursion > 0 {
                return;
            }
            *recursion += 1;
            let mut conf = CONFIG.write();
            let i = b.value();
            if i < 0 {
                return;
            }
            let format = formats[i as usize].clone();
            ui_log(format!(
                "*W*W*> Streaming Format changed to {}, restart required!!",
                format
            ));
            let newformat = match format.as_str() {
                "LPCM" => StreamingFormat::Lpcm,
                "WAV" => StreamingFormat::Wav,
                "FLAC" => StreamingFormat::Flac,
                _ => StreamingFormat::Lpcm,
            };
            conf.use_wave_format = newformat == StreamingFormat::Wav;
            conf.streaming_format = Some(newformat);
            let _ = conf.update_config();
            config_ch_flag.set(true);
            let fmt = format!("FMT: {}", format);
            b.set_label(&fmt);
            app::awake();
            *recursion -= 1;
        });
        pconfig2.add(&fmt_choice);

        // checkbutton to select 24 bit samples instead of the 16 bit default
        let mut b24_bit = CheckButton::new(0, 0, 0, 0, "24 bit");
        if config.bits_per_sample.unwrap() == 24 {
            b24_bit.set(true);
        }
        let config_ch_flag = config_changed.clone();
        b24_bit.set_callback(move |b| {
            let mut conf = CONFIG.write();
            if b.is_set() {
                conf.bits_per_sample = Some(24);
            } else {
                conf.bits_per_sample = Some(16);
            }
            let _ = conf.update_config();
            config_ch_flag.set(true);
        });
        pconfig2.add(&b24_bit);
        // HTTP server listen port
        let mut listen_port = IntInput::new(0, 0, 0, 0, "HTTP Port:");
        listen_port.set_value(&CONFIG.read().server_port.unwrap_or_default().to_string());
        listen_port.set_maximum_size(5);
        let config_ch_flag = config_changed;
        listen_port.set_callback(move |lp| {
            let new_value: u32 = lp.value().parse().unwrap();
            if new_value > 65535 {
                lp.set_value(&CONFIG.read().server_port.unwrap_or_default().to_string());
                return;
            }
            if new_value as u16 != CONFIG.read().server_port.unwrap_or_default() {
                let mut conf = CONFIG.write();
                conf.server_port = Some(new_value as u16);
                let _ = conf.update_config();
                config_ch_flag.set(true);
            }
        });

        pconfig2.add(&listen_port);

        // disable chunked transfer (for AVTransport renderers that can't handle chunkeed transfer)
        let mut disable_chunked = CheckButton::new(0, 0, 0, 0, "No Chunked Tr. Enc.");
        if config.disable_chunked {
            disable_chunked.set(true);
        }
        disable_chunked.set_callback(move |b| {
            let mut conf = CONFIG.write();
            if b.is_set() {
                conf.disable_chunked = true;
            } else {
                conf.disable_chunked = false;
            }
            let _ = conf.update_config();
        });
        pconfig2.add(&disable_chunked);

        pconfig2.auto_layout();
        pconfig2.make_resizable(false);
        vpack.add(&pconfig2);

        // RMS animation
        let mut pconfig3 = Pack::new(0, 0, GW, 20, "");
        pconfig3.set_spacing(10);
        pconfig3.set_type(PackType::Horizontal);
        pconfig3.end();
        // RMS animation enable checkbox
        let mut show_rms = CheckButton::new(0, 0, 0, 0, "Enable RMS Monitor");
        if config.monitor_rms {
            show_rms.set(true);
        }
        // rms monitor meters widgets
        let mut rms_mon_l = Progress::new(0, 0, 0, 0, "");
        let mut rms_mon_r = Progress::new(0, 0, 0, 0, "");
        rms_mon_l.set_minimum(0.0);
        rms_mon_l.set_maximum(16384.0);
        rms_mon_l.set_value(0.0);
        rms_mon_l.set_color(Color::White);
        rms_mon_l.set_selection_color(Color::Green);
        rms_mon_r.set_minimum(0.0);
        rms_mon_r.set_maximum(16384.0);
        rms_mon_r.set_value(0.0);
        rms_mon_r.set_color(Color::White);
        rms_mon_r.set_selection_color(Color::Green);
        // rms checkbox callback
        let mut mon_l = rms_mon_l.clone();
        let mut mon_r = rms_mon_r.clone();
        show_rms.set_callback(move |b| {
            let mut conf = CONFIG.write();
            if b.is_set() {
                conf.monitor_rms = true;
            } else {
                conf.monitor_rms = false;
            }
            let _ = conf.update_config();
            mon_l.set_value(0.0);
            mon_r.set_value(0.0);
        });
        pconfig3.add(&show_rms);
        // vertical pack for the RMS meters
        let mut pconfig3_v = Pack::new(0, 0, GW, 25, "");
        pconfig3_v.set_spacing(4);
        pconfig3_v.set_type(PackType::Vertical);
        pconfig3_v.end();
        pconfig3_v.add(&rms_mon_l);
        pconfig3_v.add(&rms_mon_r);
        pconfig3_v.auto_layout();
        pconfig3_v.make_resizable(false);
        pconfig3.add(&pconfig3_v);

        pconfig3.auto_layout();
        pconfig3.make_resizable(false);
        vpack.add(&pconfig3);

        // show renderer buttons title with our local ip address
        let mut pbuttons = Pack::new(0, 0, GW, 25, "");
        pbuttons.end();
        let mut frame = Frame::new(0, 0, FW, 25, "").with_align(Align::Center);
        frame.set_frame(FrameType::BorderBox);
        frame.set_label(&format!("UPNP rendering devices on network {}", local_addr));
        frame.set_color(title_color);
        pbuttons.add(&frame);
        vpack.add(&pbuttons);

        // setup feedback textbox at the bottom
        let mut pfeedback = Pack::new(0, 0, GW, 156, "");
        pfeedback.end();
        let buf = TextBuffer::default();
        let mut tb = TextDisplay::new(0, 0, 0, 150, "").with_align(Align::Left);
        tb.set_buffer(Some(buf));
        pfeedback.add(&tb);
        pfeedback.resizable(&tb);
        vpack.add(&pfeedback);
        vpack.resizable(&pfeedback);

        // create a hashmap for a button for each discovered renderer
        let buttons: HashMap<String, LightButton> = HashMap::new();

        MainForm {
            wind,
            vpack,
            auto_resume,
            auto_reconnect,
            ssdp_interval,
            log_level_choice,
            fmt_choice,
            b24_bit,
            disable_chunked,
            show_rms,
            rms_mon_l,
            rms_mon_r,
            choose_audio_source_but,
            tb,
            buttons,
            btn_index: 8,
            bwidth: frame.width(),
            bheight: frame.height(),
            wd: *wd,
            local_addr,
        }
    }

    pub fn add_log_msg(&mut self, msg: String) {
        self.tb.buffer().unwrap().append(&msg);
        self.tb.buffer().unwrap().append("\n");
        let buflen = self.tb.buffer().unwrap().length();
        self.tb.set_insert_position(buflen);
        let buflines = self.tb.count_lines(0, buflen, true);
        self.tb.scroll(buflines, 0);
    }

    pub fn add_renderer_button(&mut self, new_renderer: &Renderer) {
        let mut but = LightButton::default() // create the button
            .with_size(self.bwidth, self.bheight)
            .with_pos(0, 0)
            .with_align(Align::Center)
            .with_label(&format!(
                "{} {}",
                new_renderer.dev_model, new_renderer.dev_name
            ));
        // prepare for event handler closure
        let newr_c = new_renderer.clone();
        let bi = self.buttons.len();
        let local_addr = self.local_addr;
        let wd = self.wd;
        but.set_callback(move |b| {
            debug!(
                "Pushed renderer #{} {} {}, state = {}",
                bi,
                newr_c.dev_model,
                newr_c.dev_name,
                if b.is_set() { "ON" } else { "OFF" }
            );
            if b.is_set() {
                {
                    let mut conf = CONFIG.write();
                    conf.last_renderer = b.label();
                    let _ = conf.update_config();
                }
                let config = CONFIG.read().clone();
                let streaminfo = StreamInfo {
                    sample_rate: wd.sample_rate.0,
                    bits_per_sample: config.bits_per_sample.unwrap(),
                    streaming_format: config.streaming_format.unwrap(),
                };
                let _ = newr_c.play(
                    &local_addr,
                    config.server_port.unwrap_or_default(),
                    &ui_log,
                    streaminfo,
                );
            } else {
                newr_c.stop_play(&ui_log);
            }
        });
        // the pack for the new button
        let mut pbutton = Pack::new(0, 0, self.bwidth, self.bheight, "");
        pbutton.end();
        pbutton.add(&but); // add the button to the window
        self.vpack.insert(&pbutton, self.btn_index);
        self.buttons
            .insert(new_renderer.remote_addr.clone(), but.clone()); // and keep a reference to it for bookkeeping
        app::redraw();
        // check if autoreconnect is set for this renderer
        if self.auto_reconnect.is_set() && but.label() == CONFIG.read().last_renderer {
            but.turn_on(true);
            but.do_callback();
        }
    }
}
