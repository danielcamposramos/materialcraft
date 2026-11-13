//! UI text lookup. Keys are the English strings, so English needs no table and a missing
//! translation falls back to English. Every user-visible literal goes through [`tr`] (or [`trf`]
//! for text with `{name}` placeholders); each language is a static `(English, translation)`
//! table, indexed into a hash map the first time the language is used. `Prefs::language` picks it.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, OnceLock};

mod ar;

pub struct Language {
    /// BCP 47 code ("en", "ar").
    pub code: &'static str,
    /// The language's own name, for the picker.
    pub name: &'static str,
    /// Its English name (the picker's fallback when no font can draw `name`).
    pub english: &'static str,
    /// `(English, translation)` pairs, in any order.
    pub table: &'static [(&'static str, &'static str)],
}

const COUNT: usize = 11;

pub static LANGUAGES: [Language; COUNT] = [
    Language { code: "en", name: "English", english: "English", table: &[] },
    Language { code: "ar", name: "العربية", english: "Arabic", table: ar::TABLE },
    Language { code: "pl", name: "Polski", english: "Polish", table: &[] },
    Language { code: "pt-BR", name: "Português (Brasil)", english: "Portuguese (Brazil)", table: &[] },
    Language { code: "fr", name: "Français", english: "French", table: &[] },
    Language { code: "de", name: "Deutsch", english: "German", table: &[] },
    Language { code: "es", name: "Español", english: "Spanish", table: &[] },
    Language { code: "ja", name: "日本語", english: "Japanese", table: &[] },
    Language { code: "ko", name: "한국어", english: "Korean", table: &[] },
    Language { code: "zh-Hans", name: "简体中文", english: "Chinese (Simplified)", table: &[] },
    Language { code: "zh-Hant", name: "繁體中文", english: "Chinese (Traditional)", table: &[] },
];

include!(concat!(env!("OUT_DIR"), "/craft_i18n.rs"));
static SHARED: LazyLock<craft_i18n::I18n> = LazyLock::new(|| {
    let codes: Vec<_> = LANGUAGES.iter().map(|l| l.code).collect();
    let (lookup, problems) = craft_i18n::I18n::new(CRAFT_I18N, &codes);
    for problem in problems {
        log::warn!("translation catalog: {problem}");
    }
    lookup
});

pub fn available(code: &str) -> bool {
    matches!(code, "en" | "ar") || SHARED.from_code(code).is_some()
}

/// Explicit language lookup for label and native-title audits, without changing UI state.
pub fn text_for<'a>(code: &str, text: &'a str) -> &'a str {
    let Some((index, language)) = LANGUAGES.iter().enumerate().find(|(_, l)| l.code.eq_ignore_ascii_case(code)) else { return text };
    if !language.table.is_empty() {
        let local = MAPS[index].get_or_init(|| language.table.iter().copied().collect());
        if let Some(value) = local.get(text) {
            return value;
        }
    }
    SHARED.from_code(language.code).map_or(text, |lang| SHARED.t(lang, text))
}

static MAPS: [OnceLock<HashMap<&'static str, &'static str>>; COUNT] = [const { OnceLock::new() }; COUNT];
static CURRENT: AtomicUsize = AtomicUsize::new(0);

/// Switch the UI language (unknown codes fall back to English).
pub fn set_language(code: &str) {
    let code = language_for_locale(code);
    CURRENT.store(LANGUAGES.iter().position(|l| l.code == code).unwrap_or(0), Ordering::Relaxed);
}

/// The current UI language.
pub fn current() -> &'static Language {
    LANGUAGES.get(CURRENT.load(Ordering::Relaxed)).unwrap_or(&LANGUAGES[0])
}

/// Returns true if `code` is a right-to-left language (e.g. Arabic `"ar"`).
pub fn is_rtl_code(code: &str) -> bool {
    craft_ui::direction::resolve(code, craft_ui::direction::Setting::Language) == craft_ui::direction::Direction::Rtl
}

/// Returns true if the current UI language is right-to-left.
pub fn is_rtl() -> bool {
    is_rtl_code(current().code)
}

/// `text` in the current language (unchanged when it has no translation).
pub fn tr(text: &str) -> &str {
    text_for(current().code, text)
}

/// Marks a static string for translation where it is defined (gettext's `N_`); it is translated
/// with [`tr`] where it is shown. The key test collects both.
pub const fn n_(text: &'static str) -> &'static str {
    text
}

/// `template` translated, with each `{name}` replaced by its value in `args`.
pub fn trf(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = tr(template);
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}').and_then(|close| args.iter().find(|(k, _)| *k == &after[..close]).map(|(_, v)| (close, v))) {
            Some((close, v)) => {
                use std::fmt::Write as _;
                let _ = write!(out, "{v}");
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The supported language for a locale such as "ar-EG", "ar-SA" ("en" when none matches).
pub fn language_for_locale(locale: &str) -> &'static str {
    let base = locale.split(['-', '_', '.', '@']).next().unwrap_or_default();
    if base.eq_ignore_ascii_case("ar") {
        return "ar";
    }
    SHARED.match_tag(locale).and_then(|language| LANGUAGES.iter().find(|l| l.code.eq_ignore_ascii_case(SHARED.code(language))).map(|l| l.code)).unwrap_or("en")
}

/// The operating system's (or browser's) UI language, for the first run.
pub fn os_language() -> &'static str {
    #[cfg(not(target_arch = "wasm32"))]
    let locale = sys_locale::get_locale();
    #[cfg(target_arch = "wasm32")]
    let locale = web_sys::window().and_then(|w| w.navigator().language());
    locale.map_or("en", |l| language_for_locale(&l))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_detection() {
        assert_eq!(language_for_locale("ar"), "ar");
        assert_eq!(language_for_locale("ar-EG"), "ar");
        assert_eq!(language_for_locale("ar_SA.UTF-8"), "ar");
        assert_eq!(language_for_locale("en-US"), "en");
        assert_eq!(language_for_locale("fr-FR"), if available("fr") { "fr" } else { "en" });
    }

    #[test]
    fn arabic_table_has_no_duplicate_keys() {
        let mut seen = std::collections::HashSet::new();
        for (k, _) in ar::TABLE {
            assert!(seen.insert(*k), "duplicate key in ar::TABLE: {k:?}");
        }
    }

    #[test]
    fn placeholders_match_between_key_and_arabic_translation() {
        for (k, v) in ar::TABLE {
            let mut kp: Vec<&str> = k.split('{').skip(1).filter_map(|s| s.split_once('}').map(|(p, _)| p)).collect();
            let mut vp: Vec<&str> = v.split('{').skip(1).filter_map(|s| s.split_once('}').map(|(p, _)| p)).collect();
            kp.sort();
            vp.sort();
            assert_eq!(kp, vp, "mismatched placeholders in {k:?} vs {v:?}");
        }
    }
}
