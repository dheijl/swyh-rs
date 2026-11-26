# swyh-rs — Architecture

"Stream What You Hear": captures audio from the default output device (CPAL/WASAPI loopback on Windows, ALSA/PipeWire on Linux) and streams it over HTTP to DLNA/UPnP renderers discovered via SSDP. GUI binary (FLTK, `gui` feature) and headless CLI binary (`cli` feature).

- `src/audio/` — CPAL capture, sample conversion, FLAC/WAV/MP3 encoding
- `src/server/` — HTTP streaming server
- `src/rendercontrol/` — SSDP discovery, UPnP AV/OpenHome control
- `src/ui/mainform.rs` — FLTK tabbed GUI (Audio, Network, App, Status)
- `src/utils/configuration.rs` — INI config via `serde`
- `src/bin/swyh-rs-cli.rs` — CLI, args via `lexopt`
- `src/utils/i18n.rs` — `fluent-templates`; `locales/` holds `.ftl` files
