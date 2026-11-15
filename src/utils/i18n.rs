//! Compile-time locale loading and runtime string lookup.
//!
//! Use the [`fl!`] macro for all user-facing strings:
//! - `fl!("key")` for a plain message
//! - `fl!("key", "arg" = value)` for a message with substitutions
//!
//! The `.ftl` files in `locales/` are embedded with `include_str!` and parsed
//! lazily on the first lookup. Only the active language and the en-US fallback
//! are turned into bundles.

use fluent_bundle::{FluentArgs, FluentResource, concurrent::FluentBundle};
use std::sync::OnceLock;
use unic_langid::LanguageIdentifier;

const FALLBACK: &str = "en-US";

/// All embedded locales, sorted by tag.
/// Adding a language = add a `locales/xx-XX/main.ftl` file and a line here.
const LOCALES: &[(&str, &str)] = &[
    ("de-DE", include_str!("../../locales/de-DE/main.ftl")),
    ("en-US", include_str!("../../locales/en-US/main.ftl")),
    ("es-ES", include_str!("../../locales/es-ES/main.ftl")),
    ("fr-FR", include_str!("../../locales/fr-FR/main.ftl")),
    ("it-IT", include_str!("../../locales/it-IT/main.ftl")),
    ("ja-JP", include_str!("../../locales/ja-JP/main.ftl")),
    ("nl-NL", include_str!("../../locales/nl-NL/main.ftl")),
    ("zh-CN", include_str!("../../locales/zh-CN/main.ftl")),
];

type Bundle = FluentBundle<FluentResource>;

struct Bundles {
    /// `None` when the active language is the fallback.
    current: Option<Bundle>,
    fallback: Bundle,
}

static CURRENT_TAG: OnceLock<&'static str> = OnceLock::new();
static BUNDLES: OnceLock<Bundles> = OnceLock::new();

fn build_bundle(tag: &str, source: &str) -> Bundle {
    let lang: LanguageIdentifier = tag.parse().expect("locale tags in LOCALES are valid");
    let mut bundle = FluentBundle::new_concurrent(vec![lang]);
    // Disable the unicode isolating characters FSI/PDI as FLTK shows them on Windows
    bundle.set_use_isolating(false);
    // On a syntax error Fluent still returns the entries it could parse;
    // the unit tests guarantee the embedded files are clean.
    let res = FluentResource::try_new(source.to_owned()).unwrap_or_else(|(res, _errs)| res);
    let _ = bundle.add_resource(res);
    bundle
}

fn source_for(tag: &str) -> Option<&'static str> {
    LOCALES.iter().find(|(t, _)| *t == tag).map(|(_, s)| *s)
}

fn bundles() -> &'static Bundles {
    BUNDLES.get_or_init(|| {
        let tag = CURRENT_TAG.get().copied().unwrap_or(FALLBACK);
        Bundles {
            current: (tag != FALLBACK)
                .then(|| source_for(tag).map(|src| build_bundle(tag, src)))
                .flatten(),
            fallback: build_bundle(
                FALLBACK,
                source_for(FALLBACK).expect("fallback locale is embedded"),
            ),
        }
    })
}

/// Return all locale tags embedded at compile time (e.g. `["en-US", "nl-NL"]`).
pub fn available_languages() -> Vec<String> {
    // LOCALES is kept sorted (see tests)
    LOCALES.iter().map(|(t, _)| (*t).to_string()).collect()
}

/// Call once at startup (before any [`fl!`] invocations) to set the active locale.
/// Unknown tags fall back to en-US.
pub fn init(lang_tag: &str) {
    let tag = LOCALES
        .iter()
        .map(|(t, _)| *t)
        .find(|t| t.eq_ignore_ascii_case(lang_tag))
        .unwrap_or(FALLBACK);
    let _ = CURRENT_TAG.set(tag);
}

fn format(bundle: &Bundle, key: &str, args: Option<&FluentArgs>) -> Option<String> {
    let pattern = bundle.get_message(key)?.value()?;
    let mut errors = Vec::new();
    Some(
        bundle
            .format_pattern(pattern, args, &mut errors)
            .into_owned(),
    )
}

/// Active language first, then en-US, then the key itself.
fn lookup(key: &str, args: Option<&FluentArgs>) -> String {
    let b = bundles();
    b.current
        .as_ref()
        .and_then(|cur| format(cur, key, args))
        .or_else(|| format(&b.fallback, key, args))
        .unwrap_or_else(|| key.to_string())
}

/// Look up a message with no arguments.
pub fn t(key: &str) -> String {
    lookup(key, None)
}

/// Look up a message and substitute named arguments.
pub fn t_args(key: &str, args: &[(&'static str, String)]) -> String {
    let mut fargs = FluentArgs::with_capacity(args.len());
    for (k, v) in args {
        fargs.set(*k, v.as_str());
    }
    lookup(key, Some(&fargs))
}

/// Convenience macro for localised strings.
///
/// ```ignore
/// fl!("config-options");
/// fl!("window-title", "version" = app_version);
/// fl!("warn-network-changed", "name" = name, ...);
/// ```
#[macro_export]
macro_rules! fl {
    ($key:literal) => {
        $crate::utils::i18n::t($key)
    };
    ($key:literal, $($k:literal = $v:expr),+ $(,)?) => {
        $crate::utils::i18n::t_args($key, &[$( ($k, ($v).to_string()) ),+])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_locales_parse_cleanly() {
        for (tag, src) in LOCALES {
            assert!(tag.parse::<LanguageIdentifier>().is_ok(), "bad tag {tag}");
            if let Err((_, errs)) = FluentResource::try_new((*src).to_owned()) {
                panic!("{tag}: {errs:?}");
            }
        }
    }

    #[test]
    fn locales_sorted() {
        assert!(LOCALES.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn every_locale_has_every_fallback_key() {
        // message ids = unindented lines of the form `some-key = ...`
        let base: Vec<&str> = source_for(FALLBACK)
            .unwrap()
            .lines()
            .filter(|l| l.starts_with(|c: char| c.is_ascii_alphabetic()))
            .filter_map(|l| l.split_once('=').map(|(k, _)| k.trim()))
            .collect();
        assert!(!base.is_empty());
        for (tag, src) in LOCALES {
            let b = build_bundle(tag, src);
            let missing: Vec<_> = base.iter().filter(|k| !b.has_message(k)).collect();
            assert!(missing.is_empty(), "{tag} missing {missing:?}");
        }
    }

    #[test]
    fn format_known_and_unknown_keys() {
        let b = build_bundle(FALLBACK, source_for(FALLBACK).unwrap());
        assert!(format(&b, "cli-received-ctrlc", None).is_some());
        assert!(format(&b, "no-such-key", None).is_none());
    }

    #[test]
    fn no_bidi_isolation_chars_in_args() {
        // set_use_isolating(false) must keep FSI/PDI out of substituted output
        let b = build_bundle(FALLBACK, "greet = Hello { $name }!");
        let mut args = FluentArgs::new();
        args.set("name", "swyh");
        let s = format(&b, "greet", Some(&args)).unwrap();
        assert_eq!(s, "Hello swyh!");
    }
}
