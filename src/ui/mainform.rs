//! Main application window built with FLTK.
//!
//! [`MainForm`] assembles the control panel: audio source selector, network picker,
//! renderer buttons (one per discovered DLNA/OpenHome device), RMS level meters,
//! configuration inputs, and the scrollable log text display.

#![cfg(feature = "gui")]
use crate::{
    enums::streaming::{
        StreamSize,
        StreamingFormat::{self},
    },
    fl,
    globals::{
        RendererKind,
        statics::{
            APP_DATE, APP_URL, APP_VERSION, COPYRIGHT, NSTYLES, NTHEMES, RUN_RMS_MONITOR,
            SAMPLE_RATES, STYLES, THEMES, get_config, get_renderers, get_renderers_mut,
            update_config_with,
        },
    },
    rendercontrol::{Renderer, StreamInfo, WavData},
    slimproto::types::SlimRenderer,
    utils::{configuration::Configuration, i18n, traits::FwSlashPipeEscape, ui_logger::*},
};
use fltk::{
    app,
    button::{Button, CheckButton, LightButton},
    enums::{self, Align, Color, Event, FrameType},
    frame::Frame,
    group::{Flex, FlexType, Group, Pack, PackType, Scroll, ScrollType, Tabs},
    image::SvgImage,
    input::IntInput,
    menu::Choice,
    misc::Progress,
    prelude::*,
    text::{StyleTableEntry, TextBuffer, TextDisplay},
    valuator::{Counter, HorNiceSlider},
    window::DoubleWindow,
};
//use fltk_flow::Flow;
use log::{LevelFilter, debug, info};

use fltk_theme::{ColorMap, ColorTheme, SchemeType, WidgetScheme, color_themes};

use std::{cell::Cell, fmt::Write as _, net::IpAddr, rc::Rc, str::FromStr, sync::atomic::Ordering};

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
    ThemeDesc {
        colormap: &color_themes::fleet::NORD,
        name: THEMES[5],
    },
    ThemeDesc {
        colormap: &color_themes::fleet::DRACULA,
        name: THEMES[6],
    },
    ThemeDesc {
        colormap: &color_themes::fleet::GRUVBOX_DARK,
        name: THEMES[7],
    },
    ThemeDesc {
        colormap: &color_themes::fleet::SOLARIZED_LIGHT,
        name: THEMES[8],
    },
    ThemeDesc {
        colormap: &color_themes::fleet::MONOKAI,
        name: THEMES[9],
    },
    ThemeDesc {
        colormap: &color_themes::fleet::SOLARIZED_DARK,
        name: THEMES[10],
    },
    ThemeDesc {
        colormap: &color_themes::fleet::OCEANIC_NEXT,
        name: THEMES[11],
    },
    ThemeDesc {
        colormap: &color_themes::fleet::MINIMALIST,
        name: THEMES[12],
    },
];

/// the main (and only) form
pub struct MainForm {
    pub wind: DoubleWindow,
    pub choose_audio_source_but: Choice,
    pub fmt_choice: Choice,
    pub ss_choice: Choice,
    pub sr_choice: Choice,
    pub b24_bit: CheckButton,
    pub use_dither: CheckButton,
    pub ssdp_interval: Counter,
    pub log_level_choice: Choice,
    pub auto_resume: CheckButton,
    pub auto_reconnect: CheckButton,
    pub enable_slimproto: CheckButton,
    pub show_rms: CheckButton,
    pub rms_mon_l: Progress,
    pub rms_mon_r: Progress,
    pub tb: TextDisplay,
    tb_style_buf: TextBuffer,
    flx_feedback: Flex,
    status_buf: TextBuffer,
    restartbutton: Flex,
    bwidth: i32,
    bheight: i32,
    renderer_pack: Pack,
    /// vertical-only scroll area around `renderer_pack`, sized by [`Self::fit_to_window`]
    renderer_scroll: Scroll,
    wd: WavData,
    default_sample_rate: u32,
    local_addr: IpAddr,
    /// index of the next renderer button in the merged UPnP+SlimProto
    /// `RENDERERS` list, shared by both [`Self::add_renderer_button`] and
    /// [`Self::add_slim_renderer_button`] since both push onto that one list
    player_index: usize,
    config_changed: Rc<Cell<bool>>,
    /// running line count of `tb`'s buffer, kept in sync by `add_log_msg`
    log_lines: i32,
}

/// shared width/height for the `label: widget` rows built by [`add_labeled_row`]
const GW: i32 = 640;
const ROW_H: i32 = 25;
// config tab geometry, shared by every per-tab builder below
const TAB_H: i32 = 250;
const TAB_BAR_H: i32 = 30;
const INNER_H: i32 = TAB_H - TAB_BAR_H;
const ROW_SPACING: i32 = 5;
const MARGIN: i32 = 5;
/// floor for the feedback log box height once [`MainForm::fit_to_window`]
/// starts shrinking it to keep it inside the window: 5 lines of 12pt text
/// (~14px each), plus room for its horizontal scrollbar and frame
const MIN_FEEDBACK_H: i32 = 5 * 14 + 16 + 6;
/// height the feedback log box is constructed with
const PREFERRED_FEEDBACK_H: i32 = 296;
/// spacing between the rows of the main vertical pack
const VPACK_SPACING: i32 = 15;
/// spacing between the renderer button rows
const RENDERER_SPACING: i32 = 15;
/// the renderer scroll area never shrinks below 2 button rows (if there are that many),
/// even if that pushes the feedback box partly out of a very small window
const MIN_RENDERER_AREA_H: i32 = 2 * ROW_H + RENDERER_SPACING;
const LABEL_W: i32 = 130;
const ABOUT_BTN_W: i32 = 120;
const LABEL_W_NET: i32 = 179;

/// Adds a `label: widget` row (fixed-width label, flexed widget) to `col`.
/// Factors out the row shape repeated across most config tab entries.
fn add_labeled_row(col: &mut Flex, label: &str, label_w: i32, widget: &impl WidgetExt) {
    let lbl = Frame::default().with_label(label);
    let mut row = Flex::new(0, 0, GW, ROW_H, "");
    row.set_spacing(5);
    row.set_type(FlexType::Row);
    row.end();
    row.add(&lbl);
    row.fixed(&lbl, label_w);
    row.add(widget);
    col.add(&row);
    col.fixed(&row, ROW_H);
}

/// Shared inputs every tab builder needs: the config being edited, the shared
/// "needs restart" flag, the Status tab's live text buffer (cloned handles to
/// the same underlying buffer), and the default sample rate to show when none
/// is explicitly configured.
struct TabCtx<'a> {
    config: &'a Configuration,
    config_changed: Rc<Cell<bool>>,
    status_buf: TextBuffer,
    default_sample_rate: u32,
}

/// Audio tab: source/format/bit-depth/sample-rate/buffering controls.
struct AudioTab {
    group: Group,
    choose_audio_source_but: Choice,
    fmt_choice: Choice,
    ss_choice: Choice,
    sr_choice: Choice,
    b24_bit: CheckButton,
    use_dither: CheckButton,
}

/// Network tab: network selector, HTTP port, SSDP interval + manual discovery.
struct NetworkTab {
    group: Group,
    ssdp_interval: Counter,
}

/// App tab: language/theme/style, log level, auto-resume/reconnect, RMS monitor toggle.
struct AppTab {
    group: Group,
    log_level_choice: Choice,
    auto_resume: CheckButton,
    auto_reconnect: CheckButton,
    enable_slimproto: CheckButton,
    show_rms: CheckButton,
    rms_mon_l: Progress,
    rms_mon_r: Progress,
}

/// Status tab: read-only live summary of the current config.
struct StatusTab {
    group: Group,
}

impl AudioTab {
    /// Audio source selector: lists capture devices, persists the chosen index/name.
    fn build_audio_source_choice(ctx: &TabCtx, audio_sources: &[String]) -> Choice {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        ui_log(LogCategory::Info, &fl!("status-setup-audio"));
        let mut choose_audio_source_but = Choice::new(0, 0, 0, ROW_H, "");
        for name in audio_sources {
            choose_audio_source_but.add_choice(&name.as_str().fw_slash_pipe_escape());
        }
        choose_audio_source_but.set_value(config.sound_source_index.unwrap_or(0));
        choose_audio_source_but.set_callback({
            let config_changed = ctx.config_changed.clone();
            let mut sb = ctx.status_buf.clone();
            move |b| {
                let Some(name) = b.choice().map(|n| n.as_str().fw_slash_pipe_unescape()) else {
                    return;
                };
                let index = b.value();
                if get_config().sound_source_index == Some(index) {
                    return;
                }
                ui_log(
                    LogCategory::Info,
                    &fl!("warn-audio-changed", "name" = &name),
                );
                update_config_with(|c| {
                    c.sound_source = Some(name);
                    c.sound_source_index = Some(index);
                });
                sb.set_text(&StatusTab::format_status(default_sample_rate));
                config_changed.set(true);
            }
        });
        choose_audio_source_but
    }

    /// Streaming format (FLAC/WAV/LPCM/MP3) choice.
    fn build_format_choice(ctx: &TabCtx) -> Choice {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let formats = StreamingFormat::ALL.map(StreamingFormat::as_str);
        let initial_fmt = config
            .streaming_format
            .unwrap_or(StreamingFormat::Lpcm)
            .as_str();
        let initial_fmt_idx = formats.iter().position(|f| *f == initial_fmt).unwrap_or(0) as i32;
        let mut fmt_choice = Choice::new(0, 0, 0, ROW_H, "");
        for fmt in formats {
            fmt_choice.add_choice(fmt);
        }
        fmt_choice.set_value(initial_fmt_idx);
        fmt_choice.set_callback({
            let mut sb = ctx.status_buf.clone();
            move |b| {
                let Some(format) = b.choice() else {
                    return;
                };
                let newformat = StreamingFormat::from_str(&format).unwrap();
                if get_config().streaming_format == Some(newformat) {
                    return;
                }
                ui_log(
                    LogCategory::Info,
                    &fl!("info-format-changed", "format" = &format),
                );
                update_config_with(|c| c.streaming_format = Some(newformat));
                sb.set_text(&StatusTab::format_status(default_sample_rate));
            }
        });
        fmt_choice
    }

    /// Streaming content length / chunking mode choice.
    fn build_stream_size_choice(ctx: &TabCtx) -> Choice {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let streamsize = config
            .streaming_format
            .map_or(StreamSize::U64maxNotChunked, |fmt| {
                config
                    .stream_size_for(fmt)
                    .unwrap_or(StreamSize::NoneChunked)
            });
        let streamsizes = StreamSize::ALL.map(StreamSize::as_str);
        let streamsize_str = streamsize.as_str();
        let initial_ss_idx = streamsizes
            .iter()
            .position(|s| *s == streamsize_str)
            .unwrap_or(0) as i32;
        let mut ss_choice = Choice::new(0, 0, 0, ROW_H, "");
        for s in streamsizes {
            ss_choice.add_choice(s);
        }
        ss_choice.set_value(initial_ss_idx);
        ss_choice.set_callback({
            let mut sb = ctx.status_buf.clone();
            move |b| {
                let Some(newsize) = b.choice() else {
                    return;
                };
                let streamsize = StreamSize::from_str(&newsize).unwrap();
                let current = {
                    let cfg = get_config();
                    cfg.stream_size_for(cfg.streaming_format.unwrap_or(StreamingFormat::Flac))
                };
                if current == Some(streamsize) {
                    return;
                }
                let streaming_format = update_config_with(|c| {
                    let fmt = c.streaming_format.unwrap_or(StreamingFormat::Flac);
                    c.set_stream_size_for(fmt, streamsize);
                    fmt
                });
                ui_log(
                    LogCategory::Info,
                    &fl!(
                        "info-streamsize-changed",
                        "format" = streaming_format,
                        "size" = &newsize
                    ),
                );
                sb.set_text(&StatusTab::format_status(default_sample_rate));
            }
        });
        ss_choice
    }

    /// 24-bit and dither checkboxes — paired because toggling 24-bit forces dither off.
    fn build_bit_depth_controls(ctx: &TabCtx) -> (CheckButton, CheckButton) {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let b24_bit_lbl = fl!("chk-24bit");
        let mut b24_bit = CheckButton::new(0, 0, 0, 0, b24_bit_lbl.as_str());
        if config.bits_per_sample.unwrap_or(16) == 24 {
            b24_bit.set(true);
        }
        // dither checkbox — created before b24_bit's callback so we can clone it in
        let use_dither_lbl = fl!("chk-use-dither");
        let mut use_dither = CheckButton::new(0, 0, 0, 0, use_dither_lbl.as_str());
        if config.use_dither.unwrap_or(true) {
            use_dither.set(true);
        }
        if config.bits_per_sample.unwrap_or(16) == 24 {
            use_dither.set(false);
            use_dither.deactivate();
        }
        use_dither.set_callback({
            let mut sb = ctx.status_buf.clone();
            move |b| {
                let is_set = b.is_set();
                if get_config().use_dither == Some(is_set) {
                    return;
                }
                update_config_with(|c| c.use_dither = Some(is_set));
                sb.set_text(&StatusTab::format_status(default_sample_rate));
            }
        });
        b24_bit.set_callback({
            let mut sb = ctx.status_buf.clone();
            let mut use_dither_ref = use_dither.clone();
            move |b| {
                let new_bits: u16 = if b.is_set() { 24 } else { 16 };
                if get_config().bits_per_sample == Some(new_bits) {
                    return;
                }
                update_config_with(|c| {
                    if b.is_set() {
                        c.bits_per_sample = Some(24);
                        use_dither_ref.set(false);
                        use_dither_ref.deactivate();
                    } else {
                        c.bits_per_sample = Some(16);
                        use_dither_ref.activate();
                        use_dither_ref.set(c.use_dither.unwrap_or(true));
                    }
                });
                sb.set_text(&StatusTab::format_status(default_sample_rate));
            }
        });
        (b24_bit, use_dither)
    }

    /// Sample rate choice — index 0 is "System Default", indices 1+ are fixed rates.
    fn build_sample_rate_choice(ctx: &TabCtx) -> Choice {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let initial_sr_idx = match config.sample_rate {
            None => 0,
            Some(rate) => SAMPLE_RATES
                .iter()
                .position(|&r| r == rate)
                .map(|i| i + 1)
                .unwrap_or(0) as i32,
        };
        let mut sr_choice = Choice::new(0, 0, 0, ROW_H, "");
        sr_choice.add_choice(&fl!("sr-system-default", "rate" = default_sample_rate));
        for rate in SAMPLE_RATES {
            sr_choice.add_choice(&rate.to_string());
        }
        sr_choice.set_value(initial_sr_idx);
        sr_choice.set_callback({
            let config_changed = ctx.config_changed.clone();
            let mut sb = ctx.status_buf.clone();
            move |c| {
                let new_rate = if c.value() == 0 {
                    None
                } else {
                    Some(SAMPLE_RATES[c.value() as usize - 1])
                };
                if get_config().sample_rate == new_rate {
                    return;
                }
                update_config_with(|c| c.sample_rate = new_rate);
                sb.set_text(&StatusTab::format_status(default_sample_rate));
                config_changed.set(true);
            }
        });
        sr_choice
    }

    /// "Inject silence" checkbox.
    fn build_inject_silence_checkbox(ctx: &TabCtx) -> CheckButton {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let inj_silence_lbl = fl!("chk-inject-silence");
        let mut inj_silence = CheckButton::new(0, 0, 0, 0, inj_silence_lbl.as_str());
        if config.inject_silence.unwrap_or(false) {
            inj_silence.set(true);
        }
        inj_silence.set_callback({
            let config_changed = ctx.config_changed.clone();
            let mut sb = ctx.status_buf.clone();
            move |b| {
                let is_set = b.is_set();
                if get_config().inject_silence == Some(is_set) {
                    return;
                }
                update_config_with(|c| c.inject_silence = Some(is_set));
                sb.set_text(&StatusTab::format_status(default_sample_rate));
                config_changed.set(true);
            }
        });
        inj_silence
    }

    /// Initial upfront buffering delay (ms) input.
    fn build_buffer_input(ctx: &TabCtx) -> IntInput {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let mut upfront_buffer_ms = IntInput::new(0, 0, 50, 0, "");
        upfront_buffer_ms.set_maximum_size(5);
        let b_config = config.buffering_delay_msec.unwrap_or_default();
        upfront_buffer_ms.set_value(&b_config.to_string());
        upfront_buffer_ms.set_callback({
            let mut sb = ctx.status_buf.clone();
            move |i| {
                let mut b: i32 = i.value().parse().unwrap_or(0);
                if b < 0 {
                    i.set_value(&0i32.to_string());
                    return;
                }
                if b > 5_000 {
                    i.set_value(&5_000i32.to_string());
                    b = 5_000;
                }
                if b as u32 != b_config {
                    update_config_with(|c| c.buffering_delay_msec = Some(b as u32));
                    sb.set_text(&StatusTab::format_status(default_sample_rate));
                }
            }
        });
        upfront_buffer_ms
    }

    /// Latency reduction delay (ms) input — how long to wait after connecting before dropping.
    fn build_latency_delay_input(ctx: &TabCtx) -> IntInput {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let mut latency_delay_ms = IntInput::new(0, 0, 50, 0, "");
        latency_delay_ms.set_maximum_size(5);
        let lat_delay_config = config.latency_reduction_delay_msec.unwrap_or_default();
        latency_delay_ms.set_value(&lat_delay_config.to_string());
        latency_delay_ms.set_callback({
            let mut sb = ctx.status_buf.clone();
            move |i| {
                let mut d: i32 = i.value().parse().unwrap_or(0);
                if d < 0 {
                    i.set_value(&0i32.to_string());
                    return;
                }
                if d > 5_000 {
                    i.set_value(&5_000i32.to_string());
                    d = 5_000;
                }
                if d as u32 != lat_delay_config {
                    update_config_with(|c| c.latency_reduction_delay_msec = Some(d as u32));
                    sb.set_text(&StatusTab::format_status(default_sample_rate));
                }
            }
        });
        latency_delay_ms
    }

    /// Latency reduction amount (ms) input — how much audio to drop once the delay has elapsed.
    fn build_latency_amount_input(ctx: &TabCtx) -> IntInput {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let mut latency_reduction_ms = IntInput::new(0, 0, 50, 0, "");
        latency_reduction_ms.set_maximum_size(5);
        let lat_amount_config = config.latency_reduction_msec.unwrap_or_default();
        latency_reduction_ms.set_value(&lat_amount_config.to_string());
        latency_reduction_ms.set_callback({
            let mut sb = ctx.status_buf.clone();
            move |i| {
                let mut l: i32 = i.value().parse().unwrap_or(0);
                if l < 0 {
                    i.set_value(&0i32.to_string());
                    return;
                }
                if l > 5_000 {
                    i.set_value(&5_000i32.to_string());
                    l = 5_000;
                }
                if l as u32 != lat_amount_config {
                    update_config_with(|c| c.latency_reduction_msec = Some(l as u32));
                    sb.set_text(&StatusTab::format_status(default_sample_rate));
                }
            }
        });
        latency_reduction_ms
    }

    fn new(ctx: &TabCtx, audio_sources: &[String]) -> AudioTab {
        let mut group = Group::new(0, TAB_BAR_H, GW, INNER_H, "");
        group.set_label(&fl!("tab-audio"));
        group.end();
        let mut audio_col = Flex::new(0, TAB_BAR_H, GW, INNER_H, "");
        audio_col.set_type(FlexType::Column);
        audio_col.set_spacing(ROW_SPACING);
        audio_col.set_margin(MARGIN);
        audio_col.end();

        // setup audio source choice
        let choose_audio_source_but = Self::build_audio_source_choice(ctx, audio_sources);
        add_labeled_row(
            &mut audio_col,
            &fl!("audio-source-label", "name" = ""),
            LABEL_W,
            &choose_audio_source_but,
        );

        // streaming format
        let fmt_choice = Self::build_format_choice(ctx);
        add_labeled_row(
            &mut audio_col,
            &fl!("fmt-label", "format" = ""),
            LABEL_W,
            &fmt_choice,
        );

        // streaming content length / chunking
        let ss_choice = Self::build_stream_size_choice(ctx);
        add_labeled_row(
            &mut audio_col,
            &fl!("strmsize-label", "size" = ""),
            LABEL_W,
            &ss_choice,
        );

        // 24-bit and dither checkboxes
        let (b24_bit, use_dither) = Self::build_bit_depth_controls(ctx);
        // sample rate choice — index 0 is "System Default", indices 1+ are fixed rates
        let sr_choice = Self::build_sample_rate_choice(ctx);
        let lbl_sr = Frame::default().with_label(&fl!("sample-rate-label"));
        let mut flx_b24 = Flex::new(0, 0, GW, ROW_H, "");
        flx_b24.set_spacing(5);
        flx_b24.set_type(FlexType::Row);
        flx_b24.end();
        flx_b24.add(&b24_bit);
        flx_b24.fixed(&b24_bit, 80);
        flx_b24.add(&lbl_sr);
        flx_b24.fixed(&lbl_sr, LABEL_W);
        flx_b24.add(&sr_choice);
        audio_col.add(&flx_b24);
        audio_col.fixed(&flx_b24, ROW_H);

        // inject silence checkbox
        let inj_silence = Self::build_inject_silence_checkbox(ctx);
        let mut flx_inj = Flex::new(0, 0, GW, ROW_H, "");
        flx_inj.set_type(FlexType::Row);
        flx_inj.end();
        flx_inj.add(&inj_silence);
        flx_inj.add(&use_dither);
        audio_col.add(&flx_inj);
        audio_col.fixed(&flx_inj, ROW_H);

        // initial buffer
        let label_ms = Frame::default()
            .with_label(&fl!("buffer-label"))
            .with_align(Align::Left | Align::Inside);
        let upfront_buffer_ms = Self::build_buffer_input(ctx);
        let mut flx_buf = Flex::new(0, 0, GW, ROW_H, "");
        flx_buf.set_spacing(10);
        flx_buf.set_type(FlexType::Row);
        flx_buf.end();
        flx_buf.add(&label_ms);
        flx_buf.add(&upfront_buffer_ms);
        flx_buf.fixed(&upfront_buffer_ms, 50);
        audio_col.add(&flx_buf);
        audio_col.fixed(&flx_buf, ROW_H);

        // latency reduction delay (how long to wait after connecting before dropping)
        let label_lat_delay = Frame::default()
            .with_label(&fl!("latency-reduction-delay-label"))
            .with_align(Align::Left | Align::Inside);
        let latency_delay_ms = Self::build_latency_delay_input(ctx);
        // latency reduction amount (how much audio to drop once the delay above has elapsed)
        let label_lat_amount = Frame::default()
            .with_label(&fl!("latency-reduction-label"))
            .with_align(Align::Left | Align::Inside);
        let latency_reduction_ms = Self::build_latency_amount_input(ctx);
        let mut flx_lat = Flex::new(0, 0, GW, ROW_H, "");
        flx_lat.set_spacing(10);
        flx_lat.set_type(FlexType::Row);
        flx_lat.end();
        flx_lat.add(&label_lat_delay);
        flx_lat.add(&latency_delay_ms);
        flx_lat.fixed(&latency_delay_ms, 50);
        flx_lat.add(&label_lat_amount);
        flx_lat.add(&latency_reduction_ms);
        flx_lat.fixed(&latency_reduction_ms, 50);
        audio_col.add(&flx_lat);
        audio_col.fixed(&flx_lat, ROW_H);

        group.add(&audio_col);

        AudioTab {
            group,
            choose_audio_source_but,
            fmt_choice,
            ss_choice,
            sr_choice,
            b24_bit,
            use_dither,
        }
    }
}

impl NetworkTab {
    /// Active network interface choice.
    fn build_network_choice(ctx: &TabCtx, networks: &[String]) -> Choice {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let initial_nw_idx = config
            .last_network
            .as_ref()
            .and_then(|nw| networks.iter().position(|n| n == nw))
            .unwrap_or(0) as i32;
        let mut choose_network_but = Choice::new(0, 0, 0, ROW_H, "");
        for name in networks {
            choose_network_but.add_choice(name);
        }
        choose_network_but.set_value(initial_nw_idx);
        choose_network_but.set_callback({
            let config_changed = ctx.config_changed.clone();
            let mut sb = ctx.status_buf.clone();
            move |b| {
                let Some(name) = b.choice() else {
                    return;
                };
                if get_config().last_network.as_deref() == Some(name.as_str()) {
                    return;
                }
                ui_log(
                    LogCategory::Info,
                    &fl!("warn-network-changed", "name" = &name),
                );
                update_config_with(|c| c.last_network = Some(name));
                sb.set_text(&StatusTab::format_status(default_sample_rate));
                config_changed.set(true);
            }
        });
        choose_network_but
    }

    /// HTTP server listen port input.
    fn build_listen_port_input(ctx: &TabCtx) -> IntInput {
        let default_sample_rate = ctx.default_sample_rate;

        let mut listen_port = IntInput::new(0, 0, 0, 0, "");
        listen_port.set_value(&get_config().server_port.unwrap_or_default().to_string());
        listen_port.set_maximum_size(5);
        listen_port.set_callback({
            let config_changed = ctx.config_changed.clone();
            let mut sb = ctx.status_buf.clone();
            move |lp| {
                let new_value: u32 = lp.value().parse().unwrap_or(0);
                if new_value > 65535 {
                    lp.set_value(&get_config().server_port.unwrap_or_default().to_string());
                    return;
                }
                if new_value as u16 != get_config().server_port.unwrap_or_default() {
                    update_config_with(|c| c.server_port = Some(new_value as u16));
                    sb.set_text(&StatusTab::format_status(default_sample_rate));
                    config_changed.set(true);
                }
            }
        });
        listen_port
    }

    /// SSDP discovery interval counter (minutes; zero disables periodic discovery).
    fn build_ssdp_interval_counter(ctx: &TabCtx) -> Counter {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let mut ssdp_interval = Counter::new(0, 0, 0, 0, "");
        ssdp_interval.set_value(config.ssdp_interval_mins);
        ssdp_interval.set_tooltip(&fl!("ssdp-interval-tooltip"));
        ssdp_interval.handle({
            let config_changed = ctx.config_changed.clone();
            let mut sb = ctx.status_buf.clone();
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
                        let ssdp_interval_mins = b.value();
                        // the read guard in the `if` condition is dropped before the body runs
                        if (get_config().ssdp_interval_mins - ssdp_interval_mins).abs() > 0.09 {
                            update_config_with(|c| c.ssdp_interval_mins = ssdp_interval_mins);
                            ui_log(
                                LogCategory::Warning,
                                &fl!("warn-ssdp-changed", "interval" = ssdp_interval_mins),
                            );
                            sb.set_text(&StatusTab::format_status(default_sample_rate));
                            config_changed.set(true);
                        }
                        true
                    }
                    _ => false,
                }
            }
        });
        ssdp_interval
    }

    /// "Run SSDP discovery now" button — disabled when SSDP interval is 0 (thread not running).
    fn build_ssdp_now_button(ctx: &TabCtx, ssdp_kick_tx: crossbeam_channel::Sender<()>) -> Button {
        let config = ctx.config;

        let mut ssdp_now_btn = Button::new(0, 0, 0, ROW_H, "");
        ssdp_now_btn.set_label(&fl!("btn-ssdp-discover"));
        if config.ssdp_interval_mins <= 0.0 {
            ssdp_now_btn.deactivate();
        }
        ssdp_now_btn.set_callback(move |_| {
            let _ = ssdp_kick_tx.send(());
        });
        ssdp_now_btn
    }

    fn new(
        ctx: &TabCtx,
        networks: &[String],
        ssdp_kick_tx: crossbeam_channel::Sender<()>,
    ) -> NetworkTab {
        let mut group = Group::new(0, TAB_BAR_H, GW, INNER_H, "");
        group.set_label(&fl!("tab-network"));
        group.end();
        let mut network_col = Flex::new(0, TAB_BAR_H, GW, INNER_H, "");
        network_col.set_type(FlexType::Column);
        network_col.set_spacing(ROW_SPACING);
        network_col.set_margin(MARGIN);
        network_col.end();

        // network selection
        let choose_network_but = Self::build_network_choice(ctx, networks);
        add_labeled_row(
            &mut network_col,
            &fl!("active-network", "addr" = ""),
            LABEL_W_NET,
            &choose_network_but,
        );

        // HTTP server listen port
        let listen_port = Self::build_listen_port_input(ctx);
        add_labeled_row(
            &mut network_col,
            &fl!("http-port-label"),
            LABEL_W_NET,
            &listen_port,
        );

        // SSDP interval
        let ssdp_interval = Self::build_ssdp_interval_counter(ctx);
        add_labeled_row(
            &mut network_col,
            &fl!("ssdp-interval-label"),
            LABEL_W_NET,
            &ssdp_interval,
        );

        // "Run SSDP discovery now" button
        let ssdp_now_btn = Self::build_ssdp_now_button(ctx, ssdp_kick_tx);
        network_col.add(&ssdp_now_btn);
        network_col.fixed(&ssdp_now_btn, ROW_H);

        group.add(&network_col);

        NetworkTab {
            group,
            ssdp_interval,
        }
    }
}

impl AppTab {
    /// UI language choice.
    fn build_language_choice(ctx: &TabCtx) -> Choice {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let available_langs = i18n::available_languages();
        let cur_lang = config
            .language
            .clone()
            .unwrap_or_else(|| "en-US".to_string());
        let initial_lang_idx = available_langs
            .iter()
            .position(|l| l == &cur_lang)
            .unwrap_or(0) as i32;
        let mut lang_button = Choice::new(0, 0, 0, ROW_H, "");
        for lang in &available_langs {
            lang_button.add_choice(lang);
        }
        lang_button.set_value(initial_lang_idx);
        lang_button.set_callback({
            let config_changed = ctx.config_changed.clone();
            let mut sb = ctx.status_buf.clone();
            move |b| {
                let Some(lang) = b.choice() else {
                    return;
                };
                if get_config().language.as_deref() == Some(lang.as_str()) {
                    return;
                }
                ui_log(
                    LogCategory::Warning,
                    &fl!("warn-language-changed", "lang" = &lang),
                );
                update_config_with(|c| c.language = Some(lang));
                sb.set_text(&StatusTab::format_status(default_sample_rate));
                config_changed.set(true);
            }
        });
        lang_button
    }

    /// Color theme choice; applies the theme immediately, including at startup.
    fn build_theme_choice(ctx: &TabCtx) -> Choice {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        if let Some(theme) = config.color_theme {
            MainForm::apply_theme(theme.into());
        }
        let initial_theme_idx = config.color_theme.unwrap_or(0) as i32;
        let mut theme_button = Choice::new(0, 0, 0, ROW_H, "");
        for theme in THEMES.iter() {
            theme_button.add_choice(theme);
        }
        theme_button.set_value(initial_theme_idx);
        theme_button.set_callback({
            let mut sb = ctx.status_buf.clone();
            move |b| {
                if b.value() < 0 {
                    return;
                }
                let new_theme = b.value() as u8;
                if get_config().color_theme == Some(new_theme) {
                    return;
                }
                let name = MainForm::apply_theme(b.value() as usize);
                debug!("New theme = {name}");
                update_config_with(|c| c.color_theme = Some(new_theme));
                sb.set_text(&StatusTab::format_status(default_sample_rate));
            }
        });
        theme_button
    }

    /// Widget style choice.
    ///
    /// Note: unlike the color theme, a widget style change cannot be cleanly
    /// undone at runtime (fltk-theme has no "reset to default box drawing"),
    /// so it's only applied at startup and otherwise requires a restart.
    fn build_widget_style_choice(ctx: &TabCtx) -> Choice {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        if let Some(style) = config.widget_scheme {
            MainForm::apply_scheme(style.into());
        }
        let initial_style_idx = config.widget_scheme.unwrap_or(NSTYLES as u8) as i32;
        let mut style_button = Choice::new(0, 0, 0, ROW_H, "");
        for style in STYLES.iter() {
            style_button.add_choice(style);
        }
        style_button.set_value(initial_style_idx);
        style_button.set_callback({
            let config_changed = ctx.config_changed.clone();
            let mut sb = ctx.status_buf.clone();
            move |b| {
                if b.value() < 0 {
                    return;
                }
                let new_style = b.value() as u8;
                if get_config().widget_scheme == Some(new_style) {
                    return;
                }
                let name = STYLES[b.value() as usize];
                update_config_with(|c| c.widget_scheme = Some(new_style));
                ui_log(
                    LogCategory::Warning,
                    &fl!("warn-widget-style-changed", "style" = name),
                );
                sb.set_text(&StatusTab::format_status(default_sample_rate));
                config_changed.set(true);
            }
        });
        style_button
    }

    /// Log level choice (Info/Debug).
    fn build_log_level_choice(ctx: &TabCtx) -> Choice {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let log_levels = ["Info", "Debug"];
        let initial_ll_idx = if config.log_level == LevelFilter::Debug {
            1i32
        } else {
            0i32
        };
        let mut log_level_choice = Choice::new(0, 0, 0, ROW_H, "");
        for ll in &log_levels {
            log_level_choice.add_choice(ll);
        }
        log_level_choice.set_value(initial_ll_idx);
        log_level_choice.set_callback({
            let config_changed = ctx.config_changed.clone();
            let mut sb = ctx.status_buf.clone();
            move |b| {
                if b.value() < 0 {
                    return;
                }
                let level = log_levels[b.value() as usize];
                let loglevel = level.parse().unwrap_or(LevelFilter::Info);
                if get_config().log_level == loglevel {
                    return;
                }
                ui_log(
                    LogCategory::Warning,
                    &fl!("warn-log-changed", "level" = level),
                );
                update_config_with(|c| c.log_level = loglevel);
                sb.set_text(&StatusTab::format_status(default_sample_rate));
                config_changed.set(true);
            }
        });
        log_level_choice
    }

    /// "Auto resume" checkbox.
    fn build_auto_resume_checkbox(ctx: &TabCtx) -> CheckButton {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let auto_resume_lbl = fl!("chk-autoresume");
        let mut auto_resume = CheckButton::new(0, 0, 0, 0, auto_resume_lbl.as_str());
        if config.auto_resume {
            auto_resume.set(true);
        }
        auto_resume.set_callback({
            let mut sb = ctx.status_buf.clone();
            move |b| {
                let is_set = b.is_set();
                if get_config().auto_resume == is_set {
                    return;
                }
                update_config_with(|c| c.auto_resume = is_set);
                sb.set_text(&StatusTab::format_status(default_sample_rate));
            }
        });
        auto_resume
    }

    /// "Auto reconnect" checkbox.
    fn build_auto_reconnect_checkbox(ctx: &TabCtx) -> CheckButton {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let auto_reconnect_lbl = fl!("chk-autoreconnect");
        let mut auto_reconnect = CheckButton::new(0, 0, 0, 0, auto_reconnect_lbl.as_str());
        if config.auto_reconnect {
            auto_reconnect.set(true);
        }
        auto_reconnect.set_callback({
            let mut sb = ctx.status_buf.clone();
            move |b| {
                let is_set = b.is_set();
                if get_config().auto_reconnect == is_set {
                    return;
                }
                update_config_with(|c| c.auto_reconnect = is_set);
                sb.set_text(&StatusTab::format_status(default_sample_rate));
            }
        });
        auto_reconnect
    }

    /// SlimProto (squeezelite) discovery enable checkbox — startup-time setting,
    /// like ssdp_interval_mins, so toggling it needs a restart to take
    /// effect (see spawn_slim_discovery's gate in swyh-rs.rs).
    fn build_enable_slimproto_checkbox(ctx: &TabCtx) -> CheckButton {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let enable_slimproto_lbl = fl!("chk-enable-slimproto");
        let mut enable_slimproto = CheckButton::new(0, 0, 0, 0, enable_slimproto_lbl.as_str());
        if config.enable_slimproto {
            enable_slimproto.set(true);
        }
        enable_slimproto.set_callback({
            let config_changed = ctx.config_changed.clone();
            let mut sb = ctx.status_buf.clone();
            move |b| {
                let is_set = b.is_set();
                if get_config().enable_slimproto == is_set {
                    return;
                }
                ui_log(LogCategory::Warning, &fl!("warn-slimproto-changed"));
                update_config_with(|c| c.enable_slimproto = is_set);
                sb.set_text(&StatusTab::format_status(default_sample_rate));
                config_changed.set(true);
            }
        });
        enable_slimproto
    }

    /// RMS monitor enable checkbox plus its two level meters — paired because
    /// the checkbox's callback resets both meters.
    fn build_rms_monitor_controls(ctx: &TabCtx) -> (CheckButton, Progress, Progress) {
        let config = ctx.config;
        let default_sample_rate = ctx.default_sample_rate;

        let show_rms_lbl = fl!("chk-rms-monitor");
        let mut show_rms = CheckButton::new(0, 0, 0, 0, show_rms_lbl.as_str());
        if config.monitor_rms {
            show_rms.set(true);
        }
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
        show_rms.set_callback({
            let mut rms_mon_l = rms_mon_l.clone();
            let mut rms_mon_r = rms_mon_r.clone();
            let mut sb = ctx.status_buf.clone();
            move |b| {
                rms_mon_l.set_value(0.0);
                rms_mon_r.set_value(0.0);
                let run_rms = b.is_set();
                RUN_RMS_MONITOR.store(run_rms, Ordering::Release);
                if get_config().monitor_rms == run_rms {
                    return;
                }
                update_config_with(|c| c.monitor_rms = run_rms);
                sb.set_text(&StatusTab::format_status(default_sample_rate));
            }
        });
        (show_rms, rms_mon_l, rms_mon_r)
    }

    /// "About..." button that pops up version, copyright and license info
    fn build_about_button() -> Button {
        let about_lbl = fl!("btn-about");
        let mut about_btn = Button::new(0, 0, 0, 0, about_lbl.as_str());
        about_btn.set_callback(|_| {
            let text = format!(
                "swyh-rs {APP_VERSION} (build: {})\n{}\n\n{COPYRIGHT} - MIT License\n{APP_URL}\n\n{}\n{}",
                APP_DATE.unwrap_or("beta"),
                fl!("about-tagline"),
                fl!("about-fltk"),
                fl!("about-third-party"),
            );
            fltk::dialog::message_title(&fl!("about-title"));
            fltk::dialog::message_default(&text);
        });
        about_btn
    }

    fn new(ctx: &TabCtx) -> AppTab {
        let mut group = Group::new(0, TAB_BAR_H, GW, INNER_H, "");
        group.set_label(&fl!("tab-app"));
        group.end();
        let mut app_col = Flex::new(0, TAB_BAR_H, GW, INNER_H, "");
        app_col.set_type(FlexType::Column);
        app_col.set_spacing(ROW_SPACING);
        app_col.set_margin(MARGIN);
        app_col.end();

        // Language choice
        let lang_button = Self::build_language_choice(ctx);
        add_labeled_row(
            &mut app_col,
            &fl!("language-label", "lang" = ""),
            LABEL_W,
            &lang_button,
        );

        // Theme choice
        let theme_button = Self::build_theme_choice(ctx);
        add_labeled_row(
            &mut app_col,
            &fl!("color-theme-label", "name" = ""),
            LABEL_W,
            &theme_button,
        );

        // Widget style choice
        let style_button = Self::build_widget_style_choice(ctx);
        add_labeled_row(
            &mut app_col,
            &fl!("widget-style-label", "style" = ""),
            LABEL_W,
            &style_button,
        );

        // log level choice
        let log_level_choice = Self::build_log_level_choice(ctx);
        add_labeled_row(
            &mut app_col,
            &fl!("log-level-label", "level" = ""),
            LABEL_W,
            &log_level_choice,
        );

        // auto_resume + auto_reconnect row
        let auto_resume = Self::build_auto_resume_checkbox(ctx);
        let auto_reconnect = Self::build_auto_reconnect_checkbox(ctx);
        let mut flx_autoresume = Flex::new(0, 0, GW, ROW_H, "");
        flx_autoresume.set_type(FlexType::Row);
        flx_autoresume.end();
        flx_autoresume.add(&auto_resume);
        flx_autoresume.add(&auto_reconnect);
        app_col.add(&flx_autoresume);
        app_col.fixed(&flx_autoresume, ROW_H);

        // SlimProto (squeezelite) discovery on/off
        let enable_slimproto = Self::build_enable_slimproto_checkbox(ctx);
        let mut flx_enable_slimproto = Flex::new(0, 0, GW, ROW_H, "");
        flx_enable_slimproto.set_type(FlexType::Row);
        flx_enable_slimproto.end();
        flx_enable_slimproto.add(&enable_slimproto);
        app_col.add(&flx_enable_slimproto);
        app_col.fixed(&flx_enable_slimproto, ROW_H);

        // RMS animation enable + meters
        let (show_rms, rms_mon_l, rms_mon_r) = Self::build_rms_monitor_controls(ctx);
        let mut flx_rms = Flex::new(0, 0, GW, ROW_H, "");
        flx_rms.set_type(FlexType::Row);
        flx_rms.end();
        flx_rms.add(&show_rms);
        let about_btn = Self::build_about_button();
        flx_rms.add(&about_btn);
        flx_rms.fixed(&about_btn, ABOUT_BTN_W);
        app_col.add(&flx_rms);
        app_col.fixed(&flx_rms, ROW_H);

        group.add(&app_col);

        AppTab {
            group,
            log_level_choice,
            auto_resume,
            auto_reconnect,
            enable_slimproto,
            show_rms,
            rms_mon_l,
            rms_mon_r,
        }
    }
}

impl StatusTab {
    fn new(ctx: &TabCtx) -> StatusTab {
        let mut status_buf = ctx.status_buf.clone();
        let default_sample_rate = ctx.default_sample_rate;

        let mut group = Group::new(0, TAB_BAR_H, GW, INNER_H, "");
        group.set_label(&fl!("tab-status"));
        group.end();
        let mut status_display = TextDisplay::new(0, TAB_BAR_H, GW, INNER_H, "");
        status_display.set_buffer(status_buf.clone());
        status_display.set_text_font(enums::Font::Courier);
        status_display.set_text_size(12);
        status_display.set_scrollbar_size(8);
        let status_style_buf = TextBuffer::default();
        let normal_color = status_display.text_color();
        let styles = vec![
            StyleTableEntry {
                color: normal_color,
                font: enums::Font::Courier,
                size: 12,
            },
            StyleTableEntry {
                color: Color::Green,
                font: enums::Font::Courier,
                size: 12,
            },
            StyleTableEntry {
                color: Color::Red,
                font: enums::Font::Courier,
                size: 12,
            },
        ];
        status_display.set_highlight_data(status_style_buf.clone(), styles);
        {
            let mut ssb = status_style_buf.clone();
            let sb_ref = status_buf.clone();
            status_buf.add_modify_callback(move |_, _, _, _, _| {
                ssb.set_text(&MainForm::build_status_style(&sb_ref.text()));
            });
        }
        status_buf.set_text(&StatusTab::format_status(default_sample_rate));
        group.add(&status_display);

        StatusTab { group }
    }

    /// build the status tab text from the current config
    fn format_status(default_sample_rate: u32) -> String {
        let config = get_config();
        let bool_icon = |b: bool| if b { "✓" } else { "✗" };

        let audio = config.sound_source.as_deref().unwrap_or("-");
        let format = config
            .streaming_format
            .map_or("lpcm".to_string(), |f| f.to_string());
        let bits = config.bits_per_sample.unwrap_or(16);
        let sample_rate = config.sample_rate.unwrap_or(default_sample_rate);
        let streamsize = config
            .streaming_format
            .map_or(StreamSize::U64maxNotChunked, |fmt| {
                config
                    .stream_size_for(fmt)
                    .unwrap_or(StreamSize::NoneChunked)
            });
        let buf_ms = config.buffering_delay_msec.unwrap_or(0);
        let lat_delay_ms = config.latency_reduction_delay_msec.unwrap_or(0);
        let lat_reduction_ms = config.latency_reduction_msec.unwrap_or(0);
        let network = config.last_network.as_deref().unwrap_or("-");
        let port = config.server_port.unwrap_or_default();
        let ssdp = config.ssdp_interval_mins;
        let lang = config.language.as_deref().unwrap_or("en-US");
        let theme = config
            .color_theme
            .and_then(|i| THEMES.get(i as usize).copied())
            .unwrap_or("-");
        let style = config
            .widget_scheme
            .and_then(|i| STYLES.get(i as usize).copied())
            .unwrap_or("-");
        let log_level = config.log_level;

        let mut s = String::with_capacity(512);
        s.push_str(&fl!("audio-source-label", "name" = audio));
        s.push('\n');
        let _ = write!(
            s,
            "{}  {} bit  {} Hz",
            fl!("fmt-label", "format" = &format),
            bits,
            sample_rate,
        );
        if bits == 16 {
            let _ = write!(
                s,
                "  TPDF dither {}",
                bool_icon(config.use_dither.unwrap_or(true))
            );
        }
        s.push('\n');
        s.push_str(&fl!("strmsize-label", "size" = streamsize));
        let _ = writeln!(s, "   buffer: {buf_ms} ms");
        let _ = write!(
            s,
            "{}  {}",
            fl!("chk-inject-silence"),
            bool_icon(config.inject_silence.unwrap_or(false))
        );
        let _ = writeln!(
            s,
            "   {}",
            fl!(
                "latency-reduction-status-label",
                "amount" = lat_reduction_ms,
                "delay" = lat_delay_ms
            )
        );
        s.push('\n');
        s.push_str(&fl!("active-network", "addr" = network));
        s.push('\n');
        let _ = writeln!(
            s,
            "{}  {}   SSDP: {} min",
            fl!("http-port-label"),
            port,
            ssdp
        );
        s.push('\n');
        s.push_str(&fl!("language-label", "lang" = lang));
        s.push('\n');
        let _ = writeln!(
            s,
            "{}   {}",
            fl!("color-theme-label", "name" = theme),
            fl!("widget-style-label", "style" = style)
        );
        s.push_str(&fl!("log-level-label", "level" = log_level));
        s.push('\n');
        s.push('\n');
        let _ = writeln!(
            s,
            "{}  {}   {}  {}   {}  {}   {}  {}",
            fl!("chk-autoresume"),
            bool_icon(config.auto_resume),
            fl!("chk-autoreconnect"),
            bool_icon(config.auto_reconnect),
            fl!("chk-rms-monitor"),
            bool_icon(config.monitor_rms),
            fl!("chk-enable-slimproto"),
            bool_icon(config.enable_slimproto),
        );
        s
    }
}

/// compute optimum height for the feedback log box on windows resize
fn feedback_target_height(window_h: i32, feedback_y: i32) -> i32 {
    (window_h - feedback_y).max(MIN_FEEDBACK_H)
}

/// total height of rows stacked with `spacing` between them, as `Fl_Pack` lays them out
fn stacked_height(heights: impl IntoIterator<Item = i32>, spacing: i32) -> i32 {
    let (sum, n) = heights
        .into_iter()
        .fold((0, 0), |(sum, n), h| (sum + h, n + 1));
    if n == 0 { 0 } else { sum + spacing * (n - 1) }
}

/// height for the renderer scroll area: fit all the buttons if they fit, otherwise
/// whatever is left above a minimum-height feedback box (but at least 2 rows)
fn renderer_area_height(window_h: i32, area_y: i32, content_h: i32) -> i32 {
    let available = window_h - area_y - VPACK_SPACING - MIN_FEEDBACK_H;
    content_h.min(available.max(MIN_RENDERER_AREA_H))
}

/// `Fl_Scroll` never resizes its children, so make the renderer pack follow the scroll
/// area's width, minus the vertical scrollbar when the buttons don't all fit
fn fit_pack_width(scroll: &Scroll, pack: &mut Pack) {
    let pack_w = if pack.h() > scroll.h() {
        scroll.w() - app::scrollbar_size()
    } else {
        scroll.w()
    };
    if pack.w() != pack_w {
        pack.resize(pack.x(), pack.y(), pack_w, pack.h());
    }
}

/// Size the renderer scroll area and the feedback box below it to the window height.
/// Relies on `vpack` having positioned `scroll` (i.e. on a previous draw).
/// Returns true if anything was resized.
fn fit_bottom_to_window(
    window_h: i32,
    scroll: &mut Scroll,
    pack: &mut Pack,
    feedback: &mut Flex,
) -> bool {
    let mut changed = false;
    let feedback_y = if scroll.visible() {
        let content_h = stacked_height(
            (0..pack.children())
                .filter_map(|i| pack.child(i))
                .filter(|c| c.visible())
                .map(|c| c.h()),
            RENDERER_SPACING,
        );
        if pack.h() != content_h {
            pack.resize(pack.x(), pack.y(), pack.w(), content_h);
            changed = true;
        }
        let scroll_h = renderer_area_height(window_h, scroll.y(), content_h);
        if scroll.h() != scroll_h {
            // the scroll's resize callback refits the pack width
            scroll.resize(scroll.x(), scroll.y(), scroll.w(), scroll_h);
            changed = true;
        } else {
            // narrow the buttons only when the vertical scrollbar shows, so it never covers them
            fit_pack_width(scroll, pack);
        }
        // don't leave the view scrolled past the end after the area grew
        let max_ypos = (content_h - scroll_h).max(0);
        if scroll.yposition() > max_ypos {
            scroll.scroll_to(0, max_ypos);
            changed = true;
        }
        scroll.y() + scroll_h + VPACK_SPACING
    } else {
        feedback.y()
    };
    let target = feedback_target_height(window_h, feedback_y);
    if feedback.y() != feedback_y || feedback.h() != target {
        feedback.resize(feedback.x(), feedback_y, feedback.w(), target);
        changed = true;
    }
    changed
}

impl MainForm {
    /// Builds the entire main window.
    pub fn create(
        config: &Configuration,
        config_changed: &Rc<Cell<bool>>,
        audio_sources: &[String],
        networks: &[String],
        local_addr: IpAddr,
        wd: &WavData,
        ssdp_kick_tx: crossbeam_channel::Sender<()>,
    ) -> MainForm {
        let default_sample_rate = wd.default_sample_rate;
        const FW: i32 = 640;
        const XPOS: i32 = 30;
        const YPOS: i32 = 5;
        const WW: i32 = 700;
        const WH: i32 = 740;

        let app_version = format!("{APP_VERSION}(build: {})", APP_DATE.unwrap_or("beta"));

        let title_color: Color = Color::from_u32(0x00e6_fff0);
        let app = app::App::default().with_scheme(app::Scheme::Gtk);
        app::background(247, 247, 247);
        // never open taller than the screen's work area (minus room for the title bar),
        // the renderer scroll area takes up the slack
        const TITLEBAR_ALLOWANCE: i32 = 40;
        let (_, _, _, work_h) = app::screen_work_area(0);
        let wh = if work_h > TITLEBAR_ALLOWANCE {
            WH.min(work_h - TITLEBAR_ALLOWANCE)
        } else {
            WH
        };
        let mut wind = DoubleWindow::default()
            .with_size(WW, wh)
            .with_label(&fl!("window-title", "version" = app_version));

        wind.make_resizable(true);
        wind.size_range(WW, (WH * 2 / 3).min(wh), 0, 0);

        // set window icon
        let icon_bytes = include_str!("../../assets/swyh-rs logo note-only 16x16.svg");
        if let Ok(icon) = SvgImage::from_data(icon_bytes) {
            wind.set_icon(Some(icon));
        }

        wind.end();

        let mut vpack: Pack = Pack::new(XPOS, YPOS, GW, wh - 10, "");
        vpack.make_resizable(true);
        vpack.set_type(PackType::Vertical);
        vpack.set_spacing(VPACK_SPACING);
        vpack.end();
        wind.add(&vpack);

        // title frame
        let mut flx_title = Flex::new(0, 0, GW, 25, "");
        flx_title.end();
        let mut opt_frame = Frame::new(0, 0, 0, 25, "").with_align(Align::Center);
        opt_frame.set_frame(FrameType::BorderBox);
        opt_frame.set_label(&fl!("config-options"));
        opt_frame.set_color(title_color);
        flx_title.add(&opt_frame);
        vpack.add(&flx_title);

        // show config option widgets in tabs, one struct per visible tab
        let status_buf = TextBuffer::default();

        let mut tabs = Tabs::new(0, 0, GW, TAB_H, "");
        tabs.end();

        let ctx = TabCtx {
            config,
            config_changed: config_changed.clone(),
            status_buf: status_buf.clone(),
            default_sample_rate,
        };

        let audio_tab = AudioTab::new(&ctx, audio_sources);
        tabs.add(&audio_tab.group);

        let network_tab = NetworkTab::new(&ctx, networks, ssdp_kick_tx);
        tabs.add(&network_tab.group);

        let app_tab = AppTab::new(&ctx);
        tabs.add(&app_tab.group);

        let status_tab = StatusTab::new(&ctx);
        tabs.add(&status_tab.group);

        tabs.end();
        vpack.add(&tabs);
        let _ = tabs.set_value(&status_tab.group);

        // RMS level meters always visible below the tabs
        let mut flx_rms_v = Flex::new(0, 0, GW, 20, "");
        flx_rms_v.set_spacing(4);
        flx_rms_v.set_type(FlexType::Column);
        flx_rms_v.end();
        flx_rms_v.add(&app_tab.rms_mon_l);
        flx_rms_v.add(&app_tab.rms_mon_r);
        vpack.add(&flx_rms_v);

        // hidden restart button
        let mut flx_restart = Flex::new(0, 0, GW, 25, "");
        flx_restart.end();
        let mut restartbutton = Button::default().with_label(&fl!("btn-apply-config"));
        restartbutton.set_label_color(Color::Red);
        restartbutton.set_callback(|_| {
            std::process::Command::new(std::env::current_exe().unwrap().into_os_string())
                .spawn()
                .expect("Unable to spawn myself!");
            std::process::exit(0)
        });
        flx_restart.add(&restartbutton);
        flx_restart.hide();
        vpack.add(&flx_restart);

        // show renderer buttons title with our local ip address
        let mut flx_buttons_title = Flex::new(0, 0, GW, 25, "");
        flx_buttons_title.end();
        let mut frame = Frame::new(0, 0, FW, 25, "").with_align(Align::Center);
        frame.set_frame(FrameType::BorderBox);
        frame.set_label(&fl!("upnp-devices", "addr" = local_addr));
        frame.set_color(title_color);
        // right-click on the header un-hides all previously hidden renderers
        frame.handle({
            let config_changed = config_changed.clone();
            move |_, ev| {
                // event_button() is a plain i32 (3 = right); avoid event_mouse_button(),
                // which transmutes the raw code and panics on non-button events (Move, Enter, ...)
                if ev == Event::Push && app::event_button() == 3 {
                    if !get_config().hidden_renderers.is_empty() {
                        update_config_with(|c| c.hidden_renderers.clear());
                        config_changed.set(true);
                    }
                    true
                } else {
                    false
                }
            }
        });
        flx_buttons_title.add(&frame);
        vpack.add(&flx_buttons_title);

        // ssdp discovered renderer buttons go here, stacked in their own nested pack
        // so new buttons can just be inserted/removed from it directly.
        // The pack lives in a vertical scroll area whose height is capped by
        // fit_bottom_to_window(), so the buttons can't push the form past the window.
        // Hidden until the first renderer button arrives.
        let mut renderer_scroll = Scroll::new(0, 0, GW, 0, "");
        renderer_scroll.set_type(ScrollType::Vertical);
        renderer_scroll.end();
        let mut renderer_pack = Pack::new(0, 0, GW, 0, "");
        renderer_pack.set_type(PackType::Vertical);
        renderer_pack.set_spacing(RENDERER_SPACING);
        renderer_pack.end();
        renderer_scroll.add(&renderer_pack);
        // vpack widens the scroll area with the window, pass that on to the buttons
        renderer_scroll.resize_callback({
            let mut renderer_pack = renderer_pack.clone();
            move |s, _, _, _, _| fit_pack_width(s, &mut renderer_pack)
        });
        renderer_scroll.hide();
        vpack.add(&renderer_scroll);

        // setup feedback textbox at the bottom
        let mut flx_feedback = Flex::new(0, 0, GW, PREFERRED_FEEDBACK_H, "");
        flx_feedback.end();
        let buf = TextBuffer::default();
        let mut tb = TextDisplay::new(0, 0, 0, PREFERRED_FEEDBACK_H, "").with_align(Align::Left);
        tb.set_text_size(12);
        tb.set_selection_color(enums::Color::DarkYellow);
        tb.set_buffer(Some(buf));
        let tb_normal_color = tb.text_color();
        let tb_normal_font = tb.text_font();
        let tb_style_buf = TextBuffer::default();
        tb.set_highlight_data(
            tb_style_buf.clone(),
            vec![
                StyleTableEntry {
                    color: tb_normal_color,
                    font: tb_normal_font,
                    size: 12,
                },
                StyleTableEntry {
                    color: Color::from_u32(0x00ff_8c00), // DarkOrange, for *W warnings
                    font: tb_normal_font,
                    size: 12,
                },
                StyleTableEntry {
                    color: Color::Red, // for *E errors
                    font: tb_normal_font,
                    size: 12,
                },
            ],
        );
        flx_feedback.add(&tb);
        flx_feedback.resizable(&tb);
        vpack.add(&flx_feedback);
        //vpack.resizable(&flx_feedback);

        // Close and Resize both live on the same handle() callback, since a widget
        // only keeps the last handler registered - Event::Hide fires before
        // Event::Close, hiding the Window and preventing the Close handler being called.
        wind.handle({
            let mut flx_feedback = flx_feedback.clone();
            let mut renderer_scroll = renderer_scroll.clone();
            let mut renderer_pack = renderer_pack.clone();
            let mut wind = wind.clone();
            move |w, ev| match ev {
                Event::Close => {
                    app.quit();
                    true
                }
                Event::Resize => {
                    // refit the renderer scroll area and the feedback box to the new
                    // window height, and only redraw if something actually changed
                    if fit_bottom_to_window(
                        w.h(),
                        &mut renderer_scroll,
                        &mut renderer_pack,
                        &mut flx_feedback,
                    ) {
                        // force a redraw of the textdisplay tb
                        wind.flush();
                    }
                    false
                }
                _ => false,
            }
        });
        // calculate widget sizes etc...
        wind.show();
        wind.flush();

        let mut mf = MainForm {
            player_index: 0,
            wind,
            choose_audio_source_but: audio_tab.choose_audio_source_but,
            fmt_choice: audio_tab.fmt_choice,
            ss_choice: audio_tab.ss_choice,
            sr_choice: audio_tab.sr_choice,
            b24_bit: audio_tab.b24_bit,
            use_dither: audio_tab.use_dither,
            ssdp_interval: network_tab.ssdp_interval,
            log_level_choice: app_tab.log_level_choice,
            auto_resume: app_tab.auto_resume,
            auto_reconnect: app_tab.auto_reconnect,
            enable_slimproto: app_tab.enable_slimproto,
            show_rms: app_tab.show_rms,
            rms_mon_l: app_tab.rms_mon_l,
            rms_mon_r: app_tab.rms_mon_r,
            tb,
            tb_style_buf,
            flx_feedback,
            status_buf,
            restartbutton: flx_restart,
            renderer_pack,
            renderer_scroll,
            bwidth: frame.width(),
            bheight: frame.height(),
            wd: *wd,
            default_sample_rate,
            local_addr,
            config_changed: config_changed.clone(),
            log_lines: 0,
        };
        // fix the hor scrollbar problem on first show
        mf.fit_to_window();
        mf
    }

    /// show a log message in the text box
    pub fn add_log_msg(&mut self, msg: &LogData) {
        let Some(mut textbuffer) = self.tb.buffer() else {
            return;
        };
        let line = format!("{}{}", msg.category, msg.message);
        textbuffer.append(&line);
        textbuffer.append("\n");
        // extend the style buffer in lockstep with the content buffer: one style
        // byte per text byte, matching the style table built in `new()`
        let style_byte = match msg.category {
            LogCategory::Error => b'C',
            LogCategory::Warning => b'B',
            LogCategory::Info => b'A',
        };
        let style_run = String::from_utf8(vec![style_byte; line.len() + 1])
            .expect("style_byte is ASCII, so the run is valid UTF-8");
        self.tb_style_buf.append(&style_run);
        let buflen = textbuffer.length();
        self.tb.set_insert_position(buflen);
        // track the line count
        self.log_lines += line
            .as_bytes()
            .iter()
            .fold(0, |acc, &b| acc + (b == b'\n') as i32)
            + 1;
        self.tb.scroll(self.log_lines, 0);
    }

    /// show the restart button after a config change that needs a restart
    /// to take effect
    pub fn show_restart_button(&mut self) {
        self.restartbutton.show();
        self.refresh_status();
        self.fit_to_window();
        app::redraw();
    }

    /// `vpack` is an `Fl_Pack`, which auto-grows to fit the total height of its
    /// (visible) children on every draw, regardless of the window's own height
    /// (see `Fl_Pack::draw()` in cfltk). So showing the restart button or
    /// inserting a renderer button would push `flx_feedback` (the trailing
    /// feedback log) below the visible window. Cap the renderer scroll area and
    /// the feedback box so everything stays inside the window.
    fn fit_to_window(&mut self) {
        // the scroll area only takes part in the layout once there are buttons in it
        if self.renderer_pack.children() > 0 && !self.renderer_scroll.visible() {
            self.renderer_scroll.show();
        }
        // let vpack position its (possibly just shown) children first
        app::redraw();
        app::flush();
        fit_bottom_to_window(
            self.wind.h(),
            &mut self.renderer_scroll,
            &mut self.renderer_pack,
            &mut self.flx_feedback,
        );
        app::redraw();
    }

    /// refresh the status tab content from the current config
    pub fn refresh_status(&mut self) {
        self.status_buf
            .set_text(&StatusTab::format_status(self.default_sample_rate));
    }

    /// apply a red/green color style to all cross/check marks
    /// called once when the entire formatted text is set in the text buffer
    fn build_status_style(text: &str) -> String {
        let mut style = vec![b'A'; text.len()];
        for (i, m) in text.match_indices('✓') {
            style[i..i + m.len()].fill(b'B');
        }
        for (i, m) in text.match_indices('✗') {
            style[i..i + m.len()].fill(b'C');
        }
        String::from_utf8(style).unwrap()
    }

    /// show a new renderer button for a new renderer discovered by ssdp
    /// add the associated UI button and slider to the renderer
    /// add the new renderer to the global renderer list
    pub fn add_renderer_button(&mut self, new_renderer: &mut Renderer) {
        if get_config()
            .hidden_renderers
            .iter()
            .any(|hidden| hidden == &new_renderer.controller.remote_addr)
        {
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
                new_renderer.dev_model, new_renderer.controller.dev_name
            ));
        pbut.set_callback({
            let player_index = self.player_index;
            let newr_c = new_renderer.clone();
            let local_addr = self.local_addr;
            let wd = self.wd;
            move |b| {
                info!(
                    "Pushed renderer #{} {} {}, state = {}",
                    player_index,
                    newr_c.dev_model,
                    newr_c.controller.dev_name,
                    if b.is_on() { "ON" } else { "OFF" },
                );
                if b.is_on() {
                    update_config_with(|c| c.last_renderer = Some(b.label()));
                    let streaminfo = StreamInfo::new(wd.sample_rate);
                    newr_c.spawn_play(local_addr, streaminfo);
                } else {
                    newr_c.spawn_stop_play();
                }
                if let RendererKind::Upnp(r) = &mut get_renderers_mut()[player_index] {
                    r.playing.store(b.is_on(), Ordering::Relaxed);
                }
            }
        });
        // the flex for the new button
        let mut flx_button = Flex::new(0, 0, self.bwidth, self.bheight, "");
        flx_button.set_spacing(5);
        flx_button.set_type(FlexType::Row);
        flx_button.end();
        // add the renderer button to the window
        flx_button.add(&pbut);
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
            sl.set_trigger(enums::CallbackTrigger::Release);
            // slider callback
            sl.set_callback({
                let player_index = self.player_index;
                let mut this_renderer = new_renderer.clone();
                move |s| {
                    let vol: i32 = s.value() as i32; // guaranteed between 0.0 and 100.0
                    debug!(
                        "Setting new volume for {} to {vol}",
                        this_renderer.controller.dev_name
                    );
                    this_renderer.spawn_set_volume(vol);
                    if let RendererKind::Upnp(r) = &mut get_renderers_mut()[player_index] {
                        r.volume = vol;
                    }
                    if app::is_event_shift() {
                        debug!("Syncing volume for other active renderers");
                        // get a copy of the renderers to use for network IO; `index` is
                        // this entry's position in the merged RENDERERS list, so it stays
                        // valid for the get_renderers_mut() write below even though we
                        // only act on the UPnP entries here (same-protocol sync only)
                        let renderers = get_renderers().clone().into_iter().enumerate();
                        for (index, kind) in renderers {
                            let RendererKind::Upnp(mut rend) = kind else {
                                continue;
                            };
                            // if this renderer is playing but not the active slider renderer
                            if rend.playing.load(Ordering::Relaxed)
                                && (this_renderer.player_index != rend.player_index)
                            {
                                // and it supports setting the volume: sync volume
                                if let Some(mut slider) = rend.rend_ui.slider.clone() {
                                    debug!(
                                        "Setting new volume for {} to {vol}",
                                        rend.controller.dev_name
                                    );
                                    rend.spawn_set_volume(vol);
                                    // update the original renderer volume value
                                    if let RendererKind::Upnp(r) = &mut get_renderers_mut()[index] {
                                        r.volume = vol;
                                    }
                                    // and update the slider too
                                    slider.set_value(s.value());
                                }
                            }
                        }
                    }
                }
            });
            flx_button.add(&sl);
            new_renderer.rend_ui.slider = Some(sl.clone());
        } else {
            new_renderer.rend_ui.slider = None;
        }
        // right-click hides this renderer's button (and slider, if any)
        // fltk-rs runs the widget's built-in handle() first by default and ORs in
        // our result, which would fire the button's own play/stop callback on
        // FL_RELEASE before we ever get to swallow the event — flip that order.
        pbut.super_handle_first(false);
        pbut.handle({
            let config_changed = self.config_changed.clone();
            let remote_addr = new_renderer.controller.remote_addr.clone();
            let mut slider = new_renderer.rend_ui.slider.clone();
            move |b, ev| {
                // event_button() is a plain i32 (3 = right); avoid event_mouse_button(),
                // which transmutes the raw code and panics on non-button events (Move, Enter, ...)
                match ev {
                    Event::Push if app::event_button() == 3 => {
                        b.hide();
                        if let Some(sl) = slider.as_mut() {
                            sl.hide();
                        }
                        update_config_with(|c| c.hidden_renderers.push(remote_addr.to_string()));
                        config_changed.set(true);
                        true
                    }
                    // swallow the rest of the right-click gesture too, so the button's
                    // own click handling (fired on Released) doesn't also toggle play/stop
                    Event::Released | Event::Drag if app::event_button() == 3 => true,
                    _ => false,
                }
            }
        });
        new_renderer.rend_ui.button = Some(pbut.clone());
        // add the new renderer to the merged global list of renderers
        get_renderers_mut().push(RendererKind::Upnp(new_renderer.clone()));
        self.renderer_pack.insert(&flx_button, 0);
        self.fit_to_window();
        app::redraw();
        // now add the new player to the global list of renderers
        // check if autoreconnect is set for this renderer
        if self.auto_reconnect.is_set() {
            let active_players = get_config().active_renderers.clone();
            info!("AutoReconnect: Active Renderers = {active_players:?}");
            if active_players
                .iter()
                .any(|active| active == &new_renderer.controller.remote_addr)
            {
                pbut.turn_on(true);
                pbut.do_callback();
            }
        }
        // bump player_index
        self.player_index += 1;
    }

    /// SlimProto analogue of [`Self::add_renderer_button`]: builds a toggle
    /// button for a squeezelite client that just sent `HELO`. Playback control
    /// follows the app's configured streaming format, same as UPnP (see
    /// `SlimRenderer::send_strm_start`) — Lpcm/Wav/Rf64 are sent headerless
    /// to squeezelite regardless, see `slimproto::strm`'s doc comment.
    /// Volume is initialized at 10 % as the renderer does not supply Volume information.
    pub fn add_slim_renderer_button(&mut self, new_renderer: &mut SlimRenderer) {
        if get_config()
            .hidden_renderers
            .iter()
            .any(|hidden| hidden == &new_renderer.remote_addr)
        {
            return;
        }
        new_renderer.player_index = self.player_index;
        new_renderer.spawn_set_volume(new_renderer.volume);
        // squeezelite always supports AUDG (unlike UPnP's GetVolume, which
        // can fail per-device), so the slider is unconditional here.
        let pbwidth = (self.bwidth / 3) * 2;
        let slwidth = self.bwidth / 3;
        let mut pbut = LightButton::default()
            .with_size(pbwidth, self.bheight)
            .with_pos(0, 0)
            .with_align(Align::Center | Align::Clip)
            .with_label(&format!(
                "{} {}",
                new_renderer.model(),
                new_renderer.remote_addr
            ));
        pbut.set_callback({
            let player_index = self.player_index;
            let renderer = new_renderer.clone();
            let local_addr = self.local_addr;
            let wd = self.wd;
            move |b| {
                info!(
                    "Pushed SlimProto renderer #{player_index} {}, state = {}",
                    renderer.remote_addr,
                    if b.is_on() { "ON" } else { "OFF" },
                );
                if b.is_on() {
                    if let IpAddr::V4(server_ip) = local_addr {
                        let streaminfo = StreamInfo::new(wd.sample_rate);
                        renderer.spawn_strm_start(server_ip, streaminfo);
                    } else {
                        ui_log(
                            LogCategory::Error,
                            &format!(
                                "SlimProto: {} needs an IPv4 local address, got {local_addr}",
                                renderer.remote_addr
                            ),
                        );
                    }
                } else {
                    renderer.spawn_strm_stop();
                }
                if let RendererKind::Slim(r) = &mut get_renderers_mut()[player_index] {
                    r.playing = b.is_on();
                }
            }
        });
        // the flex for the new button
        let mut flx_button = Flex::new(0, 0, self.bwidth, self.bheight, "");
        flx_button.set_spacing(5);
        flx_button.set_type(FlexType::Row);
        flx_button.end();
        flx_button.add(&pbut);
        // right-click hides this renderer's button (see add_renderer_button for
        // why super_handle_first(false) is needed)
        pbut.super_handle_first(false);
        pbut.handle({
            let config_changed = self.config_changed.clone();
            let remote_addr = new_renderer.remote_addr.clone();
            move |b, ev| match ev {
                Event::Push if app::event_button() == 3 => {
                    b.hide();
                    update_config_with(|c| c.hidden_renderers.push(remote_addr.to_string()));
                    config_changed.set(true);
                    true
                }
                Event::Released | Event::Drag if app::event_button() == 3 => true,
                _ => false,
            }
        });
        new_renderer.rend_ui.button = Some(pbut.clone());
        // the volume slider, mirroring add_renderer_button's — squeezelite
        // always supports AUDG, so unlike UPnP's conditional slider this one
        // is unconditional
        let mut sl = HorNiceSlider::default()
            .with_size(slwidth, self.bheight)
            .with_pos(0, 0);
        sl.set_maximum(100.0);
        sl.set_minimum(0.0);
        sl.set_step(1.0, 1);
        sl.set_selection_color(Color::XtermGreen);
        sl.set_color(Color::XtermWhite);
        sl.set_value(new_renderer.volume.into());
        sl.set_trigger(enums::CallbackTrigger::Release);
        sl.set_callback({
            let player_index = self.player_index;
            let this_renderer = new_renderer.clone();
            move |s| {
                let vol: i32 = s.value() as i32; // guaranteed between 0.0 and 100.0
                debug!(
                    "Setting new volume for SlimProto {} to {vol}",
                    this_renderer.remote_addr
                );
                this_renderer.spawn_set_volume(vol);
                if let RendererKind::Slim(r) = &mut get_renderers_mut()[player_index] {
                    r.volume = vol;
                }
                if app::is_event_shift() {
                    debug!("Syncing volume for other active SlimProto renderers");
                    // get a copy of the renderers to use for network IO; `index` is
                    // this entry's position in the merged RENDERERS list, so it stays
                    // valid for the get_renderers_mut() write below even though we
                    // only act on the SlimProto entries here (same-protocol sync only)
                    let renderers = get_renderers().clone().into_iter().enumerate();
                    for (index, kind) in renderers {
                        let RendererKind::Slim(rend) = kind else {
                            continue;
                        };
                        // if this renderer is playing but not the active slider renderer
                        if rend.playing && (this_renderer.player_index != rend.player_index) {
                            // squeezelite always supports AUDG: sync volume
                            if let Some(mut slider) = rend.rend_ui.slider.clone() {
                                debug!(
                                    "Setting new volume for SlimProto {} to {vol}",
                                    rend.remote_addr
                                );
                                rend.spawn_set_volume(vol);
                                // update the original renderer volume value
                                if let RendererKind::Slim(r) = &mut get_renderers_mut()[index] {
                                    r.volume = vol;
                                }
                                // and update the slider too
                                slider.set_value(s.value());
                            }
                        }
                    }
                }
            }
        });
        flx_button.add(&sl);
        new_renderer.rend_ui.slider = Some(sl.clone());
        // add the new renderer to the merged global list of renderers
        get_renderers_mut().push(RendererKind::Slim(new_renderer.clone()));
        self.renderer_pack.insert(&flx_button, 0);
        self.fit_to_window();
        app::redraw();
        // check if autoreconnect is set for this renderer
        if self.auto_reconnect.is_set() {
            let active_players = get_config().active_renderers.clone();
            info!("AutoReconnect: Active SlimProto Renderers = {active_players:?}");
            if active_players
                .iter()
                .any(|active| active == &new_renderer.remote_addr)
            {
                pbut.turn_on(true);
                pbut.do_callback();
            }
        }
        // bump player_index
        self.player_index += 1;
    }

    /// change the theme
    fn apply_theme(theme_index: usize) -> &'static str {
        if let 0..NTHEMES = theme_index {
            ColorTheme::new(THEMES_ARRAY[theme_index].colormap).apply();
            THEMES_ARRAY[theme_index].name
        } else {
            fltk_theme::reset_color_map();
            THEMES[NTHEMES]
        }
    }

    /// change the widget style (box/frame drawing scheme)
    ///
    /// note: fltk-theme provides no way to reset the custom frame-type draw
    /// callbacks it installs, so unlike `apply_theme` there's no "None" branch
    /// here that undoes a previous style - that's handled by requiring a restart
    fn apply_scheme(style_index: usize) -> &'static str {
        let scheme = match style_index {
            0 => SchemeType::Fleet1,
            1 => SchemeType::Fleet2,
            _ => return STYLES[NSTYLES],
        };
        WidgetScheme::new(scheme).apply();
        STYLES[style_index]
    }
}

#[cfg(test)]
mod feedback_height_tests {
    use super::*;

    #[test]
    fn shrinks_when_content_above_overflows_the_window() {
        // vpack content above flx_feedback already reaches y=700 in a 740px window:
        // only 40px are left, below MIN_FEEDBACK_H, so clamp to the floor
        assert_eq!(feedback_target_height(740, 700), MIN_FEEDBACK_H);
    }

    #[test]
    fn shrinks_to_exactly_the_available_space() {
        // 100px available, between the floor and the constructed height
        assert_eq!(feedback_target_height(740, 640), 100);
    }

    #[test]
    fn grows_uncapped_past_the_constructed_height_when_window_is_enlarged() {
        // 800px available, well past PREFERRED_FEEDBACK_H (296): should fill it
        // all rather than getting capped back down to the construction-time size
        assert_eq!(feedback_target_height(1200, 400), 800);
    }

    #[test]
    fn stacked_height_adds_spacing_between_rows_only() {
        assert_eq!(stacked_height([], 15), 0);
        assert_eq!(stacked_height([25], 15), 25);
        assert_eq!(stacked_height([25, 25, 25], 15), 3 * 25 + 2 * 15);
    }

    #[test]
    fn renderer_area_fits_content_when_there_is_room() {
        // 740 - 400 - spacing - min feedback leaves well over 2 rows
        let content = stacked_height([25, 25], RENDERER_SPACING);
        assert_eq!(renderer_area_height(740, 400, content), content);
    }

    #[test]
    fn renderer_area_is_capped_above_a_minimum_feedback_box() {
        let content = stacked_height([25; 20], RENDERER_SPACING);
        let h = renderer_area_height(740, 400, content);
        assert_eq!(h, 740 - 400 - VPACK_SPACING - MIN_FEEDBACK_H);
        // and the feedback box then gets exactly its minimum height
        assert_eq!(
            feedback_target_height(740, 400 + h + VPACK_SPACING),
            MIN_FEEDBACK_H
        );
    }

    #[test]
    fn renderer_area_keeps_two_rows_in_a_tiny_window() {
        let content = stacked_height([25; 20], RENDERER_SPACING);
        assert_eq!(renderer_area_height(500, 400, content), MIN_RENDERER_AREA_H);
        // but never more than the content itself
        assert_eq!(renderer_area_height(500, 400, 25), 25);
    }

    #[test]
    fn renderer_area_is_empty_without_renderers() {
        assert_eq!(renderer_area_height(740, 400, 0), 0);
    }
}
