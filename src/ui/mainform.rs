#![cfg(feature = "gui")]
use crate::{
    enums::streaming::{
        StreamSize,
        StreamingFormat::{self, Flac},
    },
    globals::statics::{
        RUN_RMS_MONITOR, THEMES, get_config, get_config_mut, get_renderers, get_renderers_mut,
    },
    openhome::rendercontrol::{Renderer, StreamInfo, WavData},
    utils::{configuration::Configuration, traits::FwSlashPipeEscape, ui_logger::ui_log},
};
use fltk::{
    app,
    button::{Button, CheckButton, LightButton},
    enums::{Align, Color, Event, FrameType},
    frame::Frame,
    group::{Flex, FlexType, Pack, PackType},
    image::SvgImage,
    input::IntInput,
    menu::MenuButton,
    misc::Progress,
    prelude::*,
    text::{TextBuffer, TextDisplay},
    valuator::{Counter, HorNiceSlider},
    window::DoubleWindow,
};
//use fltk_flow::Flow;
use log::{LevelFilter, debug, info};

use fltk_theme::{ColorMap, ColorTheme, color_themes};

use std::{
    cell::Cell,
    net::IpAddr,
    rc::Rc,
    str::FromStr,
    sync::atomic::{AtomicBool, Ordering},
};

/// fltk themes
struct ThemeDesc {
    colormap: &'static [ColorMap],
    name: &'static str,
}
// keep in sync with global::statics::THEMES array
const THEMES_ARRAY: &[ThemeDesc] = &[
    ThemeDesc {
        colormap: color_themes::SHAKE_THEME,
        name: THEMES[0],
    },
    ThemeDesc {
        colormap: color_themes::GRAY_THEME,
        name: THEMES[1],
    },
    ThemeDesc {
        colormap: color_themes::TAN_THEME,
        name: THEMES[2],
    },
    ThemeDesc {
        colormap: color_themes::DARK_THEME,
        name: THEMES[3],
    },
    ThemeDesc {
        colormap: color_themes::BLACK_THEME,
        name: THEMES[4],
    },
];

/// the main (and only) form
pub struct MainForm {
    pub wind: DoubleWindow,
    pub auto_resume: CheckButton,
    pub auto_reconnect: CheckButton,
    pub ssdp_interval: Counter,
    pub log_level_choice: MenuButton,
    pub fmt_choice: MenuButton,
    pub b24_bit: CheckButton,
    pub show_rms: CheckButton,
    pub rms_mon_l: Progress,
    pub rms_mon_r: Progress,
    pub choose_audio_source_but: MenuButton,
    pub tb: TextDisplay,
    vpack: Pack,
    restartbutton: Flex,
    bwidth: i32,
    bheight: i32,
    btn_index: i32,
    wd: WavData,
    local_addr: IpAddr,
    player_index: usize,
}

impl MainForm {
    pub fn create(
        config: &Configuration,
        config_changed: &Rc<Cell<bool>>,
        audio_sources: &[String],
        networks: &[String],
        local_addr: IpAddr,
        wd: &WavData,
        app_version: &str,
    ) -> MainForm {
        const GW: i32 = 600;
        const FW: i32 = 600;
        const XPOS: i32 = 30;
        const YPOS: i32 = 5;
        const WW: i32 = 660;
        const WH: i32 = 660;

        let title_color: Color = Color::from_u32(0x00e6_fff0);
        let app = app::App::default().with_scheme(app::Scheme::Gtk);
        app::background(247, 247, 247);
        let mut wind = DoubleWindow::default()
            .with_size(WW, WH)
            .with_label(&format!("swyh-rs UPNP/DLNA Media Renderers V{app_version}"));

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

        wind.handle({
            let config_changed = config_changed.clone();
            move |_, _ev| {
                // Event::Hide fires before Event::Close, hiding the Window and preventing the Close handler being called
                // debug!("_ev = {:?}, app_event = {:?}", _ev, app::event());
                let ev = app::event();
                match ev {
                    Event::Close => {
                        app.quit();
                        //std::process::exit(0);
                        true
                    }
                    Event::Push => {
                        if app::event_mouse_button() == app::MouseButton::Right {
                            if let Some(lightbtn) = app::belowmouse::<LightButton>() {
                                let players = get_renderers().clone();
                                for player in players {
                                    if let Some(mut button) = player.rend_ui.button
                                        && button == lightbtn
                                    {
                                        button.hide();
                                        if let Some(mut slider) = player.rend_ui.slider {
                                            slider.hide()
                                        }
                                        let mut config = get_config_mut();
                                        config.hidden_renderers.push(player.remote_addr.clone());
                                        let _ = config.update_config();
                                        config_changed.set(true);
                                        return true;
                                    }
                                }
                            } else if let Some(frame) = app::belowmouse::<Frame>() {
                                if frame.label().contains("UPNP rendering devices on network") {
                                    let mut config = get_config_mut();
                                    config.hidden_renderers.clear();
                                    let _ = config.update_config();
                                    config_changed.set(true);
                                    return true;
                                }
                            }
                        }
                        false
                    }
                    _ => false,
                }
            }
        });

        let mut vpack: Pack = Pack::new(XPOS, YPOS, GW, WH - 10, "");
        vpack.make_resizable(true);
        vpack.set_type(PackType::Vertical);
        vpack.set_spacing(15);
        vpack.end();
        wind.add(&vpack);

        // title frame
        let mut p1 = Flex::new(0, 0, GW, 25, "");
        p1.end();
        let mut opt_frame = Frame::new(0, 0, 0, 25, "").with_align(Align::Center);
        opt_frame.set_frame(FrameType::BorderBox);
        opt_frame.set_label("Configuration Options");
        opt_frame.set_color(title_color);
        p1.add(&opt_frame);
        vpack.add(&p1);

        // show config option widgets

        // Theme
        let cur_theme = if let Some(theme) = config.color_theme {
            let name = Self::apply_theme(theme.into());
            &("Color Theme: ".to_string() + name)
        } else {
            "Choose Color Theme"
        };
        let mut ptheme = Pack::new(0, 0, GW, 25, "");
        ptheme.end();
        let mut theme_button = MenuButton::new(0, 0, 0, 25, None).with_label(cur_theme);
        theme_button.add_choice(&THEMES.join("|"));
        let rlock = AtomicBool::new(false);
        theme_button.set_callback(move |b| {
            if rlock.swap(true, Ordering::Acquire) {
                return;
            }
            if b.value() < 0 {
                return;
            }
            let name = Self::apply_theme(b.value() as usize);
            debug!("New theme = {name}");
            let cur_theme = "Color theme: ".to_string() + name;
            b.set_label(&cur_theme);
            {
                let mut conf = get_config_mut();
                conf.color_theme = Some(b.value() as u8);
                let _ = conf.update_config();
            }
            rlock.store(false, Ordering::Release);
        });
        ptheme.add(&theme_button);
        vpack.add(&ptheme);

        // network selection
        let mut pnw = Flex::new(0, 0, GW, 25, "");
        pnw.end();
        let cur_nw = {
            if config.last_network.is_none() {
                format!("Active network: {local_addr}")
            } else {
                format!("Active network: {}", config.last_network.as_ref().unwrap())
            }
        };
        let mut choose_network_but = MenuButton::new(0, 0, 0, 25, None).with_label(&cur_nw);
        for name in networks {
            choose_network_but.add_choice(name);
        }
        let rlock = AtomicBool::new(false);
        choose_network_but.set_callback({
            let networks = networks.to_vec();
            let config_changed = config_changed.clone();
            move |b| {
                if rlock.swap(true, Ordering::Acquire) {
                    return;
                }
                if b.value() < 0 {
                    return;
                }
                let name = &networks[(b.value() as usize).clamp(0, networks.len() - 1)];
                ui_log(&format!(
                    "*W*W*> Network changed to {name}, restart required!!"
                ));
                {
                    let mut conf = get_config_mut();
                    conf.last_network = Some(name.to_string());
                    let _ = conf.update_config();
                }
                b.set_label(&format!("New Network: {name}"));
                config_changed.set(true);
                rlock.store(false, Ordering::Release);
            }
        });
        pnw.add(&choose_network_but);
        vpack.add(&pnw);

        // setup audio source choice
        let mut pas = Flex::new(0, 0, GW, 25, "");
        pas.end();
        let cur_audio_src = format!("Audio Source: {}", config.sound_source.as_ref().unwrap());
        ui_log("Setup audio sources");
        let mut choose_audio_source_but =
            MenuButton::new(0, 0, 0, 25, None).with_label(&cur_audio_src);
        for name in audio_sources {
            choose_audio_source_but.add_choice(&name.fw_slash_pipe_escape());
        }
        let rlock = AtomicBool::new(false);
        choose_audio_source_but.set_callback({
            let audio_sources = audio_sources.to_vec();
            let config_changed = config_changed.clone();
            move |b| {
                if rlock.swap(true, Ordering::Acquire) {
                    return;
                }
                if b.value() < 0 {
                    return;
                }
                let name = &audio_sources[(b.value() as usize).clamp(0, audio_sources.len() - 1)];
                ui_log(&format!(
                    "*W*W*> Audio source changed to {name}, restart required!!"
                ));
                b.set_label(&format!("New Audio Source: {name}",));
                {
                    let mut conf = get_config_mut();
                    conf.sound_source = Some(name.to_string());
                    conf.sound_source_index = Some(b.value());
                    let _ = conf.update_config();
                }
                config_changed.set(true);
                rlock.store(false, Ordering::Release);
            }
        });
        pas.add(&choose_audio_source_but);
        vpack.add(&pas);

        // all other options
        let mut pconfig1 = Flex::new(0, 0, GW, 20, "");
        pconfig1.set_spacing(10);
        pconfig1.set_type(FlexType::Row);
        pconfig1.end();

        // auto_resume button for AVTransport autoresume play
        let mut auto_resume = CheckButton::new(0, 0, 0, 0, "Autoresume play");
        if config.auto_resume {
            auto_resume.set(true);
        }
        auto_resume.set_callback(move |b| {
            let mut conf = get_config_mut();
            conf.auto_resume = b.is_set();
            let _ = conf.update_config();
        });
        pconfig1.add(&auto_resume);

        // AutoReconnect to last renderer on startup button
        let mut auto_reconnect = CheckButton::new(0, 0, 0, 0, "Autoreconnect");
        if config.auto_reconnect {
            auto_reconnect.set(true);
        }
        auto_reconnect.set_callback(move |b| {
            let mut conf = get_config_mut();
            conf.auto_reconnect = b.is_set();
            let _ = conf.update_config();
        });
        pconfig1.add(&auto_reconnect);

        // SSDP interval counter
        let mut ssdp_interval = Counter::new(0, 0, 0, 0, "SSDP Interval (in minutes)");
        ssdp_interval.set_value(config.ssdp_interval_mins);
        ssdp_interval.handle({
            let config_changed = config_changed.clone();
            move |b, ev| {
                // zero = no ssdp, else minimum ssdp discovery interval is 0,5 minutes
                match ev {
                    Event::Leave | Event::Enter | Event::Unfocus => {
                        let v = match b.value() {
                            ..=0.0 => 0.0,
                            0.01..=1.0 => 1.0,
                            _ => b.value(),
                        };
                        b.set_value(v);
                        let mut ssdp_interval_mins: f64 = -1.0;
                        {
                            let mut conf = get_config_mut();
                            if (conf.ssdp_interval_mins - b.value()).abs() > 0.09 {
                                conf.ssdp_interval_mins = b.value();
                                ssdp_interval_mins = conf.ssdp_interval_mins;
                                let _ = conf.update_config();
                            }
                        }
                        if ssdp_interval_mins >= 0.0 {
                            ui_log(&format!(
                                "*W*W*> ssdp interval changed to {ssdp_interval_mins} minutes, restart required!!"
                            ));
                            config_changed.set(true);
                        }
                        true
                    }
                    _ => false,
                }
            }
        });
        pconfig1.add(&ssdp_interval);

        // show log level choice
        let ll = format!("Log Level: {}", config.log_level);
        let mut log_level_choice = MenuButton::default().with_label(&ll);
        let log_levels = ["Info", "Debug"];
        for ll in &log_levels {
            log_level_choice.add_choice(ll);
        }
        // apparently this event can recurse on very fast machines
        // probably because it takes some time doing the file I/O, hence recursion lock
        let rlock = AtomicBool::new(false);
        log_level_choice.set_callback({
            let config_changed = config_changed.clone();
            move |b| {
                if rlock.swap(true, Ordering::Acquire) {
                    return;
                }
                if b.value() < 0 {
                    return;
                }
                let level = log_levels[b.value() as usize];
                ui_log(&format!(
                    "*W*W*> Log level changed to {level}, restart required!!"
                ));
                let loglevel = level.parse().unwrap_or(LevelFilter::Info);
                {
                    let mut conf = get_config_mut();
                    conf.log_level = loglevel;
                    let _ = conf.update_config();
                }
                config_changed.set(true);
                let ll = format!("Log Level: {loglevel}");
                b.set_label(&ll);
                rlock.store(false, Ordering::Release);
            }
        });
        pconfig1.add(&log_level_choice);
        //pconfig1.auto_layout();
        pconfig1.make_resizable(true);
        vpack.add(&pconfig1);
        // spacer
        let mut pspacer = Flex::new(0, 0, GW, 10, "");
        pspacer.make_resizable(true);
        vpack.add(&pspacer);

        let mut pconfig2 = Flex::new(0, 0, GW, 20, "");
        pconfig2.set_spacing(10);
        pconfig2.set_type(FlexType::Row);
        pconfig2.end();

        // streaming format
        let fmt = if let Some(format) = config.streaming_format {
            format!("FMT: {format}")
        } else {
            "FMT: Lpcm".to_string()
        };
        let mut fmt_choice = MenuButton::default().with_label(&fmt);
        let formats = vec![
            StreamingFormat::Lpcm.to_string(),
            StreamingFormat::Wav.to_string(),
            StreamingFormat::Flac.to_string(),
            StreamingFormat::Rf64.to_string(),
        ];
        for fmt in &formats {
            fmt_choice.add_choice(fmt.as_str());
        }
        // apparently this event can recurse on very fast machines
        // probably because it takes some time doing the file I/O, hence recursion lock
        let rlock = AtomicBool::new(false);
        fmt_choice.set_callback({
            move |b| {
                if rlock.swap(true, Ordering::Acquire) {
                    return;
                }
                if b.value() < 0 {
                    return;
                }
                let format = formats[b.value() as usize].clone();
                ui_log(&format!("Current streaming Format changed to {format}"));
                let newformat = StreamingFormat::from_str(&format).unwrap();
                {
                    let mut conf = get_config_mut();
                    conf.streaming_format = Some(newformat);
                    let _ = conf.update_config();
                }
                let fmt = format!("FMT: {format}");
                b.set_label(&fmt);
                rlock.store(false, Ordering::Release);
            }
        });
        pconfig2.add(&fmt_choice);

        // checkbutton to select 24 bit samples instead of the 16 bit default
        let mut b24_bit = CheckButton::new(0, 0, 0, 0, "24 bit");
        if config.bits_per_sample.unwrap_or(16) == 24 {
            b24_bit.set(true);
        }
        b24_bit.set_callback({
            move |b| {
                let mut conf = get_config_mut();
                if b.is_set() {
                    conf.bits_per_sample = Some(24);
                } else {
                    conf.bits_per_sample = Some(16);
                }
                let _ = conf.update_config();
            }
        });
        pconfig2.add(&b24_bit);
        // HTTP server listen port
        let mut listen_port = IntInput::new(0, 0, 0, 0, "HTTP Port:");
        listen_port.set_value(&get_config().server_port.unwrap_or_default().to_string());
        listen_port.set_maximum_size(5);
        listen_port.set_callback({
            let config_changed = config_changed.clone();
            move |lp| {
                let new_value: u32 = lp.value().parse().unwrap();
                if new_value > 65535 {
                    lp.set_value(&get_config().server_port.unwrap_or_default().to_string());
                    return;
                }
                if new_value as u16 != get_config().server_port.unwrap_or_default() {
                    let mut conf = get_config_mut();
                    conf.server_port = Some(new_value as u16);
                    let _ = conf.update_config();
                    config_changed.set(true);
                }
            }
        });

        pconfig2.add(&listen_port);
        // inject continuous silence into audio stream checkbox
        // to prevent Sonos to disconnect if no audio is being captured
        let mut inj_silence = CheckButton::new(0, 0, 0, 0, "Inject silence");
        if config.inject_silence.unwrap() {
            inj_silence.set(true);
        }
        inj_silence.set_callback({
            let config_changed = config_changed.clone();
            move |b| {
                let mut conf = get_config_mut();
                conf.inject_silence = Some(b.is_set());
                let _ = conf.update_config();
                config_changed.set(true);
            }
        });
        pconfig2.add(&inj_silence);

        //pconfig2.auto_layout();
        pconfig2.make_resizable(true);
        vpack.add(&pconfig2);

        // streaming content length and chunking
        let mut pconfig3 = Flex::new(0, 0, GW, 20, "");
        pconfig3.set_spacing(10);
        pconfig3.set_type(FlexType::Row);
        pconfig3.end();

        let streamsize = if let Some(fmt) = config.streaming_format {
            match fmt {
                StreamingFormat::Lpcm => config.lpcm_stream_size.unwrap(),
                StreamingFormat::Wav => config.wav_stream_size.unwrap(),
                StreamingFormat::Rf64 => config.rf64_stream_size.unwrap(),
                StreamingFormat::Flac => config.flac_stream_size.unwrap(),
            }
        } else {
            StreamSize::U64maxNotChunked
        };
        let fmt = format!("StrmSize: {streamsize}");
        let mut ss_choice = MenuButton::default()
            .with_label(&fmt)
            .with_align(Align::Center | Align::Clip);
        let streamsizes = vec![
            StreamSize::U64maxNotChunked.to_string(),
            StreamSize::NoneChunked.to_string(),
            StreamSize::U64maxChunked.to_string(),
            StreamSize::U32maxNotChunked.to_string(),
            StreamSize::U32maxChunked.to_string(),
        ];
        for fmt in &streamsizes {
            ss_choice.add_choice(fmt.as_str());
        }
        // apparently this event can recurse on very fast machines
        // probably because it takes some time doing the file I/O, hence recursion lock
        let rlock = AtomicBool::new(false);
        ss_choice.set_callback({
            move |b| {
                if rlock.swap(true, Ordering::Acquire) {
                    return;
                }
                if b.value() < 0 {
                    return;
                }
                let newsize = streamsizes[b.value() as usize].clone();
                let streamsize = StreamSize::from_str(&newsize).unwrap();
                let streaming_format = {
                    let mut conf = get_config_mut();
                    match conf.streaming_format.unwrap() {
                        StreamingFormat::Lpcm => conf.lpcm_stream_size = Some(streamsize),
                        StreamingFormat::Wav => conf.wav_stream_size = Some(streamsize),
                        StreamingFormat::Rf64 => conf.rf64_stream_size = Some(streamsize),
                        StreamingFormat::Flac => conf.flac_stream_size = Some(streamsize),
                    }
                    let _ = conf.update_config();
                    conf.streaming_format.unwrap()
                };
                ui_log(&format!(
                    "StreamSize for {streaming_format} changed to {newsize}"
                ));
                let fmt = format!("StrmSize: {newsize}");
                b.set_label(&fmt);
                rlock.store(false, Ordering::Release);
            }
        });
        pconfig3.add(&ss_choice);

        let label_ms = Frame::default().with_label("                       Inital buffer (msec): ");
        pconfig3.add(&label_ms);
        let mut upfront_buffer_ms = IntInput::new(0, 0, 50, 0, "");
        upfront_buffer_ms.set_maximum_size(5);
        let b_config = config.buffering_delay_msec.unwrap_or_default();
        upfront_buffer_ms.set_value(&b_config.to_string());
        upfront_buffer_ms.set_callback({
            move |i| {
                let mut b: i32 = i.value().parse().unwrap();
                if b < 0 {
                    i.set_value(&0i32.to_string());
                    return;
                }
                if b > 5_000 {
                    i.set_value(&5_000i32.to_string());
                    b = 5_000;
                }
                if b as u32 != b_config {
                    let mut conf = get_config_mut();
                    conf.buffering_delay_msec = Some(b as u32);
                    let _ = conf.update_config();
                }
            }
        });
        pconfig3.add(&upfront_buffer_ms);

        //pconfig3.auto_layout();
        pconfig3.make_resizable(true);
        vpack.add(&pconfig3);

        // RMS animation
        let mut pconfig4 = Flex::new(0, 0, GW, 20, "");
        pconfig4.set_spacing(10);
        pconfig4.set_type(FlexType::Row);
        pconfig4.end();
        // RMS animation enable checkbox
        let mut show_rms = CheckButton::new(0, 0, 0, 0, "RMS Monitor");
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
        show_rms.set_callback({
            let mut rms_mon_l = rms_mon_l.clone();
            let mut rms_mon_r = rms_mon_r.clone();
            move |b| {
                rms_mon_l.set_value(0.0);
                rms_mon_r.set_value(0.0);
                let run_rms = b.is_set();
                RUN_RMS_MONITOR.store(run_rms, Ordering::Release);
                let mut conf = get_config_mut();
                conf.monitor_rms = run_rms;
                let _ = conf.update_config();
            }
        });
        pconfig4.add(&show_rms);
        // vertical pack for the RMS meters
        let mut pconfig3_v = Flex::new(0, 0, GW, 16, "");
        pconfig3_v.set_spacing(4);
        pconfig3_v.set_type(FlexType::Column);
        pconfig3_v.end();
        pconfig3_v.add(&rms_mon_l);
        pconfig3_v.add(&rms_mon_r);
        //pconfig3_v.auto_layout();
        pconfig3_v.make_resizable(true);
        pconfig4.add(&pconfig3_v);

        //pconfig4.auto_layout();
        pconfig4.make_resizable(true);
        vpack.add(&pconfig4);

        // hidden restart button
        let mut prestart = Flex::new(0, 0, GW, 25, "");
        let mut restartbutton =
            Button::default().with_label("Press to apply configuration changes");
        restartbutton.set_label_color(Color::Red);
        restartbutton.set_callback(|_| {
            std::process::Command::new(std::env::current_exe().unwrap().into_os_string())
                .spawn()
                .expect("Unable to spawn myself!");
            std::process::exit(0)
        });
        prestart.add(&restartbutton);
        prestart.hide();
        vpack.add(&prestart);

        // show renderer buttons title with our local ip address
        let mut pbuttons = Flex::new(0, 0, GW, 25, "");
        pbuttons.end();
        let mut frame = Frame::new(0, 0, FW, 25, "").with_align(Align::Center);
        frame.set_frame(FrameType::BorderBox);
        frame.set_label(&format!("UPNP rendering devices on network {local_addr}"));
        frame.set_color(title_color);
        pbuttons.add(&frame);
        vpack.add(&pbuttons);

        // ssdp discovered renderer buttons go here
        let btn_insert_index = vpack.children();

        // setup feedback textbox at the bottom
        let mut pfeedback = Flex::new(0, 0, GW, 156, "");
        pfeedback.end();
        let buf = TextBuffer::default();
        let mut tb = TextDisplay::new(0, 0, 0, 150, "").with_align(Align::Left);
        tb.set_buffer(Some(buf));
        pfeedback.add(&tb);
        pfeedback.resizable(&tb);
        vpack.add(&pfeedback);
        vpack.resizable(&pfeedback);

        MainForm {
            player_index: 0,
            wind,
            vpack,
            restartbutton: prestart,
            auto_resume,
            auto_reconnect,
            ssdp_interval,
            log_level_choice,
            fmt_choice: ss_choice,
            b24_bit,
            show_rms,
            rms_mon_l,
            rms_mon_r,
            choose_audio_source_but,
            tb,
            btn_index: btn_insert_index,
            bwidth: frame.width(),
            bheight: frame.height(),
            wd: *wd,
            local_addr,
        }
    }

    /// show a log message in the text box
    pub fn add_log_msg(&mut self, msg: &str) {
        if let Some(mut textbuffer) = self.tb.buffer() {
            textbuffer.append(msg);
            textbuffer.append("\n");
            let buflen = textbuffer.length();
            self.tb.set_insert_position(buflen);
            let buflines = self.tb.count_lines(0, buflen, true);
            self.tb.scroll(buflines, 0);
        }
    }

    /// show the restart button after a config change that needs a restart
    /// to take effect
    pub fn show_restart_button(&mut self) {
        self.restartbutton.show();
        app::redraw();
    }

    /// show a new renderer button for a new enderer discovered by ssdp
    /// add the associated UI button and slider to the renderer
    /// add the new renderer to the global renderer list
    pub fn add_renderer_button(&mut self, new_renderer: &mut Renderer) {
        let config = get_config().clone();
        if config.hidden_renderers.contains(&new_renderer.remote_addr) {
            return;
        }
        // initialize renderers player_index
        new_renderer.player_index = self.player_index;
        // check if the renderer responded to GetVolume and make room for the slider if yes
        let (show_vol_slider, pbwidth, slwidth) = if new_renderer.volume >= 0 {
            (true, (self.bwidth / 3) * 2, self.bwidth / 3)
        } else {
            (false, self.bwidth, 0)
        };
        let mut pbut = LightButton::default() // create the button
            .with_size(pbwidth, self.bheight)
            .with_pos(0, 0)
            .with_align(Align::Center | Align::Clip)
            .with_label(&format!(
                "{} {}",
                new_renderer.dev_model, new_renderer.dev_name
            ));
        pbut.set_callback({
            let player_index = self.player_index;
            let mut newr_c = new_renderer.clone();
            let local_addr = self.local_addr;
            let wd = self.wd;
            move |b| {
                info!(
                    "Pushed renderer #{} {} {}, state = {}",
                    player_index,
                    newr_c.dev_model,
                    newr_c.dev_name,
                    if b.is_on() { "ON" } else { "OFF" },
                );
                if b.is_on() {
                    {
                        let mut conf = get_config_mut();
                        conf.last_renderer = Some(b.label());
                        let _ = conf.update_config();
                    }
                    let (streaminfo, server_port) = {
                        let config = get_config();
                        (
                            StreamInfo {
                                sample_rate: wd.sample_rate.0,
                                bits_per_sample: config.bits_per_sample.unwrap_or(16),
                                streaming_format: config.streaming_format.unwrap_or(Flac),
                            },
                            config.server_port.unwrap_or_default(),
                        )
                    };
                    let _ = newr_c.play(&local_addr, server_port, &ui_log, streaminfo);
                } else {
                    newr_c.stop_play(&ui_log);
                }
                get_renderers_mut()[player_index].playing = b.is_on();
            }
        });
        // the pack for the new button
        let mut pbutton = Flex::new(0, 0, self.bwidth, self.bheight, "");
        pbutton.set_spacing(5);
        pbutton.set_type(FlexType::Row);
        pbutton.end();
        // add the renderer button to the window
        pbutton.add(&pbut);
        // Only if GetVolume worked: add the volume slider
        if show_vol_slider {
            let mut sl = HorNiceSlider::default()
                .with_size(slwidth, self.bheight)
                .with_pos(0, 0);
            sl.set_maximum(100.0);
            sl.set_minimum(0.0);
            sl.set_step(1.0, 1);
            sl.set_selection_color(Color::XtermGreen);
            sl.set_color(Color::XtermWhite);
            sl.set_value(new_renderer.volume.into());
            sl.set_trigger(fltk::enums::CallbackTrigger::Release);
            // slider callback
            sl.set_callback({
                let player_index = self.player_index;
                let mut this_renderer = new_renderer.clone();
                move |s| {
                    let vol: i32 = s.value() as i32; // guaranteed between 0.0 and 100.0
                    debug!("Setting new volume for {} to {vol}", this_renderer.dev_name);
                    this_renderer.set_volume(&ui_log, vol);
                    get_renderers_mut()[player_index].volume = vol;
                    if app::is_event_shift() {
                        debug!("Syncing volume for other active renderers");
                        // get a copy of the renderers to use for network IO
                        let renderers = get_renderers().clone().into_iter().enumerate();
                        for (index, mut rend) in renderers {
                            // if this renderer is playing but not the active slider renderer
                            if rend.playing && (this_renderer.player_index != rend.player_index) {
                                // and it supports setting the volume: sync volume
                                if let Some(mut slider) = rend.rend_ui.slider.clone() {
                                    debug!("Setting new volume for {} to {vol}", rend.dev_name);
                                    rend.set_volume(&ui_log, vol);
                                    // update the original renderer volume value
                                    get_renderers_mut()[index].volume = vol;
                                    // and update the slider too
                                    slider.set_value(s.value());
                                }
                            }
                        }
                    }
                }
            });
            pbutton.add(&sl);
            new_renderer.rend_ui.slider = Some(sl.clone());
        } else {
            new_renderer.rend_ui.slider = None;
        }
        new_renderer.rend_ui.button = Some(pbut.clone());
        // add the new renderer to the global list of renderers
        get_renderers_mut().push(new_renderer.clone());
        self.vpack.insert(&pbutton, self.btn_index);
        app::redraw();
        // now add the new player to the global list of renderers
        // check if autoreconnect is set for this renderer
        if self.auto_reconnect.is_set() {
            let active_players = get_config().active_renderers.clone();
            info!("AutoReconnect: Active Renderers = {active_players:?}");
            if active_players.contains(&new_renderer.remote_addr) {
                pbut.turn_on(true);
                pbut.do_callback();
            }
        }
        // bump player_index
        self.player_index += 1;
    }

    /// change the theme
    fn apply_theme(theme_index: usize) -> &'static str {
        // number of available themes (excluding the last dummy one, "None")
        const NTHEMES: usize = THEMES.len() - 1;
        match theme_index {
            0..NTHEMES => {
                ColorTheme::new(THEMES_ARRAY[theme_index].colormap).apply();
                THEMES_ARRAY[theme_index].name
            }
            _ => {
                fltk_theme::reset_color_map();
                THEMES[NTHEMES]
            }
        }
    }
}
