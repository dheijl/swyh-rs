use crate::ui_log;
use crate::utils::escape::FwSlashPipeEscape;
use crate::Configuration;
use crate::Renderer;
use crate::WavData;
use crate::CONFIG;
use crate::SERVER_PORT;
use fltk::{
    app,
    button::{
        Align, ButtonExt, CheckButton, Color, DisplayExt, Event, FrameType, GroupExt, LightButton,
        MenuExt, ValuatorExt, WidgetBase, WidgetExt, WindowExt,
    },
    frame::Frame,
    group::{Pack, PackType},
    image::SvgImage,
    menu::MenuButton,
    misc::Progress,
    text::{TextBuffer, TextDisplay},
    valuator::Counter,
    window::DoubleWindow,
};
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
    pub disable_chunked: CheckButton,
    pub use_wma: CheckButton,
    pub show_rms: CheckButton,
    pub rms_mon_l: Progress,
    pub rms_mon_r: Progress,
    pub choose_audio_source_but: MenuButton,
    pub tb: TextDisplay,
    pub buttons: HashMap<String, LightButton>,
    vpack: Pack,
    bwidth: i32,
    bheight: i32,
    btn_index: u32,
    wd: WavData,
    local_addr: IpAddr,
}

impl MainForm {
    pub fn create(
        config: &Configuration,
        config_changed: Rc<Cell<bool>>,
        audio_sources: &[String],
        local_addr: IpAddr,
        wd: &WavData,
        app_version: String,
    ) -> MainForm {
        let app = app::App::default().with_scheme(app::Scheme::Gtk);
        app::background(247, 247, 247);
        let ww = 660;
        let wh = 660;
        let mut wind = DoubleWindow::default()
            .with_size(ww, wh)
            .with_label(&format!(
                "swyh-rs UPNP/DLNA Media Renderers V{}",
                app_version
            ));

        wind.make_resizable(true);
        wind.size_range(ww, wh * 2 / 3, 0, 0);
        let icon_bytes = include_str!("../../assets/swyh-rs-logo-note-only.svg");
        if let Ok(icon) = SvgImage::from_data(icon_bytes) {
            wind.set_icon(Some(icon));
        }
        wind.end();
        wind.show();

        wind.handle(move |_ev| {
            //eprintln!("{:?}", app::event());
            let ev = app::event();
            match ev {
                Event::Close => {
                    app.quit();
                    std::process::exit(0);
                }
                _ => false,
            }
        });

        let gw = 600;
        let fw = 600;
        let xpos = 30;
        let ypos = 5;

        let mut vpack = Pack::new(xpos, ypos, gw, wh - 10, "");
        vpack.make_resizable(false);
        vpack.set_spacing(15);
        vpack.end();
        wind.add(&vpack);

        // title frame
        let mut p1 = Pack::new(0, 0, gw, 25, "");
        p1.end();
        let mut opt_frame = Frame::new(0, 0, 0, 25, "").with_align(Align::Center);
        opt_frame.set_frame(FrameType::BorderBox);
        opt_frame.set_label("Options");
        opt_frame.set_color(Color::Light2);
        p1.add(&opt_frame);
        vpack.add(&p1);

        // show config option widgets
        let mut p2 = Pack::new(0, 0, gw, 25, "");
        p2.set_spacing(10);
        p2.set_type(PackType::Horizontal);
        p2.end();

        // auto_resume button for AVTransport autoresume play
        let mut auto_resume = CheckButton::new(0, 0, 0, 0, "Autoresume play");
        if config.auto_resume {
            auto_resume.set(true);
        }
        auto_resume.set_callback2(move |b| {
            let mut conf = CONFIG.write();
            if b.is_set() {
                conf.auto_resume = true;
            } else {
                conf.auto_resume = false;
            }
            let _ = conf.update_config();
        });
        p2.add(&auto_resume);

        // AutoReconnect to last renderer on startup button
        let mut auto_reconnect = CheckButton::new(0, 0, 0, 0, "Autoreconnect");
        if config.auto_reconnect {
            auto_reconnect.set(true);
        }
        auto_reconnect.set_callback2(move |b| {
            let mut conf = CONFIG.write();
            if b.is_set() {
                conf.auto_reconnect = true;
            } else {
                conf.auto_reconnect = false;
            }
            let _ = conf.update_config();
        });
        p2.add(&auto_reconnect);

        // SSDP interval counter
        let mut ssdp_interval = Counter::new(0, 0, 0, 0, "SSDP Interval (in minutes)");
        ssdp_interval.set_value(config.ssdp_interval_mins);
        let config_ch_flag = config_changed.clone();
        ssdp_interval.handle2(move |b, ev| {
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
        p2.add(&ssdp_interval);

        // show log level choice
        let ll = format!("Log Level: {}", config.log_level.to_string());
        let mut log_level_choice = MenuButton::new(0, 0, 0, 0, &ll);
        let log_levels = vec!["Info", "Debug"];
        for ll in log_levels.iter() {
            log_level_choice.add_choice(ll);
        }
        // apparently this event can recurse on very fast machines
        // probably because it takes some time doing the file I/O, hence recursion lock
        let rlock = Mutex::new(0);
        let config_ch_flag = config_changed.clone();
        log_level_choice.set_callback2(move |b| {
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
            let ll = format!("Log Level: {}", conf.log_level.to_string());
            b.set_label(&ll);
            app::awake();
            *recursion -= 1;
        });
        p2.add(&log_level_choice);
        p2.auto_layout();
        p2.make_resizable(false);
        vpack.add(&p2);

        let mut p2b = Pack::new(0, 0, gw, 25, "");
        p2b.set_spacing(10);
        p2b.set_type(PackType::Horizontal);
        p2b.end();

        // disable chunked transfer (for AVTransport renderers that can't handle chunkeed transfer)
        let mut disable_chunked = CheckButton::new(0, 0, 0, 0, "Disable Chunked TransferEncoding");
        if config.disable_chunked {
            disable_chunked.set(true);
        }
        disable_chunked.set_callback2(move |b| {
            let mut conf = CONFIG.write();
            if b.is_set() {
                conf.disable_chunked = true;
            } else {
                conf.disable_chunked = false;
            }
            let _ = conf.update_config();
        });
        p2b.add(&disable_chunked);
        let mut use_wma = CheckButton::new(0, 0, 0, 0, "Use WMA/WAV format");
        if config.use_wave_format {
            use_wma.set(true);
        }
        use_wma.set_callback2(move |b| {
            let mut conf = CONFIG.write();
            if b.is_set() {
                conf.use_wave_format = true;
            } else {
                conf.use_wave_format = false;
            }
            let _ = conf.update_config();
        });
        p2b.add(&use_wma);
        p2b.auto_layout();
        p2b.make_resizable(false);
        vpack.add(&p2b);

        // RMS animation
        let mut p2c = Pack::new(0, 0, gw, 25, "");
        p2c.set_spacing(10);
        p2c.set_type(PackType::Horizontal);
        p2c.end();
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
        show_rms.set_callback2(move |b| {
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
        p2c.add(&show_rms);
        // vertical pack for the RMS meters
        let mut p2c_v = Pack::new(0, 0, gw, 25, "");
        p2c_v.set_spacing(4);
        p2c_v.set_type(PackType::Vertical);
        p2c_v.end();
        p2c_v.add(&rms_mon_l);
        p2c_v.add(&rms_mon_r);
        p2c_v.auto_layout();
        p2c_v.make_resizable(false);
        p2c.add(&p2c_v);

        p2c.auto_layout();
        p2c.make_resizable(false);
        vpack.add(&p2c);

        // setup audio source choice
        let mut p3 = Pack::new(0, 0, gw, 25, "");
        p3.end();
        let cur_audio_src = format!("Source: {}", config.sound_source);
        ui_log("Setup audio sources".to_string());
        let mut choose_audio_source_but = MenuButton::new(0, 0, 0, 25, &cur_audio_src);
        for name in audio_sources.iter() {
            choose_audio_source_but.add_choice(&name.fw_slash_pipe_escape());
        }
        let rlock = Mutex::new(0);
        let config_ch_flag = config_changed;
        let audio_sources_c = audio_sources.to_vec();
        choose_audio_source_but.set_callback2(move |b| {
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
                "*W*W*> Audio source changed to {}, restart required!!",
                name
            ));
            conf.sound_source = name;
            let _ = conf.update_config();
            b.set_label(&format!("New Source: {}", conf.sound_source));
            config_ch_flag.set(true);
            app::awake();
            *recursion -= 1;
        });
        p3.add(&choose_audio_source_but);
        vpack.add(&p3);

        // show renderer buttons title with our local ip address
        let mut p4 = Pack::new(0, 0, gw, 25, "");
        p4.end();
        let mut frame = Frame::new(0, 0, fw, 25, "").with_align(Align::Center);
        frame.set_frame(FrameType::BorderBox);
        frame.set_label(&format!("UPNP rendering devices on network {}", local_addr));
        frame.set_color(Color::Light2);
        p4.add(&frame);
        vpack.add(&p4);

        // setup feedback textbox at the bottom
        let mut p5 = Pack::new(0, 0, gw, 156, "");
        p5.end();
        let buf = TextBuffer::default();
        let mut tb = TextDisplay::new(0, 0, 0, 150, "").with_align(Align::Left);
        tb.set_buffer(Some(buf));
        p5.add(&tb);
        p5.resizable(&tb);
        vpack.add(&p5);
        vpack.resizable(&p5);

        // create a hashmap for a button for each discovered renderer
        let buttons: HashMap<String, LightButton> = HashMap::new();

        MainForm {
            wind,
            vpack,
            auto_resume,
            auto_reconnect,
            ssdp_interval,
            log_level_choice,
            disable_chunked,
            use_wma,
            show_rms,
            rms_mon_l,
            rms_mon_r,
            choose_audio_source_but,
            tb,
            buttons,
            btn_index: 6,
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
        but.set_callback2(move |b| {
            debug!(
                "Pushed renderer #{} {} {}, state = {}",
                bi,
                newr_c.dev_model,
                newr_c.dev_name,
                if b.is_set() { "ON" } else { "OFF" }
            );
            if b.is_set() {
                let use_wav_format = {
                    let mut conf = CONFIG.write();
                    conf.last_renderer = b.label();
                    let _ = conf.update_config();
                    conf.use_wave_format
                };
                let _ = newr_c.play(&local_addr, SERVER_PORT, &wd, &ui_log, use_wav_format);
            } else {
                let _ = newr_c.stop_play(&ui_log);
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
