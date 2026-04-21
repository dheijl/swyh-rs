//! Compile-time locale loading and runtime string lookup.
//!
//! Use the [`fl!`] macro for all user-facing strings:
//! - `fl!("key")` for a plain message
//! - `fl!("key", "arg" = value)` for a message with substitutions

use fluent_templates::fluent_bundle::FluentValue;
use fluent_templates::{LanguageIdentifier, Loader, static_loader};
use std::{borrow::Cow, collections::HashMap, sync::OnceLock};

static_loader! {
    // Note: disable the unicode isolating characters FSI/PDI as FLTK shows them on Windows
    static LOCALES = {
        locales: "./locales",
        fallback_language: "en-US",
        customise: |bundle| bundle.set_use_isolating(false),
    };
}

static CURRENT_LANG: OnceLock<LanguageIdentifier> = OnceLock::new();

/// Return all locale tags embedded at compile time (e.g. `["en-US", "nl-NL"]`).
pub fn available_languages() -> Vec<String> {
    let mut langs: Vec<String> = LOCALES.locales().map(|l| l.to_string()).collect();
    langs.sort();
    langs
}

/// Call once at startup (before any [`fl!`] invocations) to set the active locale.
pub fn init(lang_tag: &str) {
    let lang: LanguageIdentifier = lang_tag
        .parse()
        .unwrap_or_else(|_| "en-US".parse().unwrap());
    let _ = CURRENT_LANG.set(lang);
}

fn lang() -> &'static LanguageIdentifier {
    CURRENT_LANG.get_or_init(|| "en-US".parse().unwrap())
}

/// Look up a message with no arguments.
pub fn t(key: &str) -> String {
    LOCALES.lookup(lang(), key)
}

/// Look up a message and substitute named arguments.
///
/// `lookup_complete` in fluent-templates >= 0.12 requires a `HashMap<Cow<str>, FluentValue>`.
/// We build it with borrowed keys (`'static`) and borrowed value slices to avoid
/// unnecessary `String` clones — only the `HashMap` itself is heap-allocated.
pub fn t_args(key: &str, args: &[(&'static str, String)]) -> String {
    let map: HashMap<Cow<str>, FluentValue<'_>> = args
        .iter()
        .map(|(k, v)| {
            (
                Cow::Borrowed(*k),
                FluentValue::String(Cow::Borrowed(v.as_str())),
            )
        })
        .collect();
    LOCALES.lookup_complete(lang(), key, Some(&map))
}

/// Convenience macro for localised strings.
///
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
