# swyh-rs — Gotchas

- **CPAL `SampleRate`** is a plain `u32` alias, not a newtype — never use `.0`.
- **`lexopt` value-swallowing**: an optional-value arg only takes the "no value" branch when it's the last token; otherwise the next token is consumed as its value, even if it looks like a flag.
- **Accept-Ranges and MPD**: only emit `Accept-Ranges` on `206` and `416` responses, never on `200` — MPD loops forever on GET otherwise.
- **fluent-templates**: always `set_use_isolating(false)` in `static_loader!`, or FSI/PDI bidi chars show as garbage in FLTK labels on Windows.
