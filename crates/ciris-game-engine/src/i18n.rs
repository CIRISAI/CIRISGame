//! In-engine localization over Project Fluent (`.ftl`), DESIGN_BRIEF §5 / §7.7.
//!
//! Every announced and on-screen surface — the intro, the setup wizard, the
//! accessibility panel — pulls its text from a [`Localization`] resource keyed by
//! Fluent message id. The 29 CIRIS languages each have a file in
//! `assets/strings/<lang>.ftl`; all 29 are **embedded at compile time** with
//! `include_str!` so the lookup is synchronous and works bit-identically on
//! native and on the two wasm artifacts (no async asset load, no `std::fs`,
//! webgl2-safe).
//!
//! Missing keys (or keys a translation hasn't filled in yet) fall back to
//! English, then to a visible `⟨key⟩` sentinel — the UI never renders blank.
//! Right-to-left scripts (Arabic, Persian, Urdu) are flagged by [`is_rtl`]; the
//! UI mirrors row direction and text justification for them (see `ui_theme.rs`).
//!
//! The concurrent [`FluentBundle`] specialisation is used so the resource is
//! `Send + Sync` as Bevy requires.

use bevy::prelude::*;
use fluent::concurrent::FluentBundle;
use fluent::{FluentArgs, FluentResource};
use unic_langid::LanguageIdentifier;

/// The 29 CIRIS languages, in selector order: BCP-47 code + the language's own
/// endonym (kept in code, not in `.ftl`, because a language's *name for itself*
/// is identity, not translatable copy). `en` is index 0 and the fallback.
pub const LANGS: [(&str, &str); 29] = [
    ("en", "English"),
    ("am", "አማርኛ"),
    ("ar", "العربية"),
    ("bn", "বাংলা"),
    ("de", "Deutsch"),
    ("es", "Español"),
    ("fa", "فارسی"),
    ("fr", "Français"),
    ("ha", "Hausa"),
    ("hi", "हिन्दी"),
    ("id", "Bahasa Indonesia"),
    ("it", "Italiano"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("mr", "मराठी"),
    ("my", "မြန်မာ"),
    ("pa", "ਪੰਜਾਬੀ"),
    ("pt", "Português"),
    ("ru", "Русский"),
    ("sw", "Kiswahili"),
    ("ta", "தமிழ்"),
    ("te", "తెలుగు"),
    ("th", "ไทย"),
    ("tr", "Türkçe"),
    ("uk", "Українська"),
    ("ur", "اردو"),
    ("vi", "Tiếng Việt"),
    ("yo", "Yorùbá"),
    ("zh", "中文"),
];

/// Index of the English bundle — the universal fallback.
const FALLBACK: usize = 0;

/// The right-to-left scripts among the 29. Layout (row direction, text justify)
/// mirrors for these; Parley already handles per-string bidi shaping.
const RTL_LANGS: [&str; 3] = ["ar", "fa", "ur"];

/// True if `code` is written right-to-left.
pub fn is_rtl(code: &str) -> bool {
    RTL_LANGS.contains(&code)
}

/// Pair each language with its embedded `.ftl` source. `include_str!` resolves
/// relative to this file (`crates/ciris-game-engine/src/i18n.rs`), so the path
/// climbs to the repo root and into `assets/strings/`.
macro_rules! ftl_source {
    ($code:literal) => {
        (
            $code,
            include_str!(concat!("../../../assets/strings/", $code, ".ftl")),
        )
    };
}

/// All 29 `(code, ftl_source)` pairs, aligned with [`LANGS`] order.
const SOURCES: [(&str, &str); 29] = [
    ftl_source!("en"),
    ftl_source!("am"),
    ftl_source!("ar"),
    ftl_source!("bn"),
    ftl_source!("de"),
    ftl_source!("es"),
    ftl_source!("fa"),
    ftl_source!("fr"),
    ftl_source!("ha"),
    ftl_source!("hi"),
    ftl_source!("id"),
    ftl_source!("it"),
    ftl_source!("ja"),
    ftl_source!("ko"),
    ftl_source!("mr"),
    ftl_source!("my"),
    ftl_source!("pa"),
    ftl_source!("pt"),
    ftl_source!("ru"),
    ftl_source!("sw"),
    ftl_source!("ta"),
    ftl_source!("te"),
    ftl_source!("th"),
    ftl_source!("tr"),
    ftl_source!("uk"),
    ftl_source!("ur"),
    ftl_source!("vi"),
    ftl_source!("yo"),
    ftl_source!("zh"),
];

/// All language bundles plus the active selection. Built once at startup.
#[derive(Resource)]
pub struct Localization {
    /// One bundle per language, aligned with [`LANGS`] / [`SOURCES`] order.
    bundles: Vec<FluentBundle<FluentResource>>,
    /// Index of the active language into [`LANGS`].
    current: usize,
}

impl Default for Localization {
    fn default() -> Self {
        Self::new()
    }
}

impl Localization {
    /// Parse and bundle every embedded `.ftl`. A malformed entry keeps whatever
    /// messages parsed (Fluent's partial-recovery), so one bad key never blanks a
    /// whole language; anything still missing resolves through the §English
    /// fallback at lookup time.
    pub fn new() -> Self {
        let bundles = SOURCES
            .iter()
            .map(|(code, src)| {
                let langid: LanguageIdentifier = code.parse().unwrap_or_default();
                let mut bundle = FluentBundle::new_concurrent(vec![langid]);
                // No Unicode FSI/PDI isolation marks: the strings are short, and
                // the bare marks can render as tofu under Parley. Per-string bidi
                // shaping still works; whole-row mirroring is handled in layout.
                bundle.set_use_isolating(false);
                // try_new returns the (possibly partial) resource on parse error.
                let resource = match FluentResource::try_new(src.to_string()) {
                    Ok(r) => r,
                    Err((r, _errors)) => r,
                };
                let _ = bundle.add_resource(resource);
                bundle
            })
            .collect();
        Localization {
            bundles,
            current: FALLBACK,
        }
    }

    /// Set the active language by [`LANGS`] index (ignored if out of range).
    pub fn set_lang_index(&mut self, index: usize) {
        if index < self.bundles.len() {
            self.current = index;
        }
    }

    /// The active language's [`LANGS`] index.
    pub fn current_index(&self) -> usize {
        self.current
    }

    /// The active language's BCP-47 code.
    pub fn current_code(&self) -> &'static str {
        LANGS[self.current].0
    }

    /// Whether the active language lays out right-to-left.
    pub fn current_rtl(&self) -> bool {
        is_rtl(self.current_code())
    }

    /// Look up `key` in the active language with no arguments.
    pub fn t(&self, key: &str) -> String {
        self.format_in(self.current, key, None)
            .or_else(|| self.format_in(FALLBACK, key, None))
            .unwrap_or_else(|| format!("⟨{key}⟩"))
    }

    /// Look up `key` with `{ $name }` substitutions, e.g.
    /// `ta("wizard-step-of", &[("step", "1".into()), ("total", "3".into())])`.
    pub fn ta(&self, key: &str, args: &[(&str, String)]) -> String {
        let mut fluent_args = FluentArgs::new();
        for (name, value) in args {
            fluent_args.set(*name, value.clone());
        }
        self.format_in(self.current, key, Some(&fluent_args))
            .or_else(|| self.format_in(FALLBACK, key, Some(&fluent_args)))
            .unwrap_or_else(|| format!("⟨{key}⟩"))
    }

    /// Resolve `key` in bundle `index`, returning `None` if the bundle lacks the
    /// message (so the caller can fall through to English).
    fn format_in(&self, index: usize, key: &str, args: Option<&FluentArgs>) -> Option<String> {
        let bundle = self.bundles.get(index)?;
        let message = bundle.get_message(key)?;
        let pattern = message.value()?;
        let mut errors = Vec::new();
        let formatted = bundle.format_pattern(pattern, args, &mut errors);
        Some(formatted.into_owned())
    }
}
