//! The words this program says, read from the catalogs the whole project shares.
//!
//! `.agents/rules/user-facing-text.md` says a string a person reads comes from a catalog and
//! never from the source, and `.agents/rules/catalog-parity.md` says the catalogs hold the
//! same keys. Both are about one set of files, so this program reads **those** files rather
//! than growing a second set that would agree with the first for about a month.
//!
//! They are compiled in with `include_str!`. A node on a server has no frontend beside it to
//! read them from, and a catalog that could go missing at run time is a program that could
//! start speechless.
//!
//! A record is not user-facing text and never comes from here — `.agents/rules/logging.md`.

use std::sync::OnceLock;

use serde_json::Value;

use crate::language::Language;

/// The English catalog, which is the source language and the fallback.
const ENGLISH: &str = include_str!("../../src/i18n/locales/en.json");

/// The Spanish catalog.
const SPANISH: &str = include_str!("../../src/i18n/locales/es.json");

/// The catalogs, parsed once.
static PARSED: OnceLock<[Option<Value>; 2]> = OnceLock::new();

/// The words of one language, ready to be asked for a key.
#[derive(Debug, Clone, Copy)]
pub struct Catalog {
    /// Which of the two this is.
    language: Language,
}

impl Catalog {
    /// The catalog for `language`.
    #[must_use]
    pub fn of(language: Language) -> Self {
        Self { language }
    }

    /// The text at `key`, or the key itself when there is none.
    ///
    /// `key` is dotted, exactly as it is written in the catalogs — `network.peers.noNetwork`.
    /// A missing key falls back to English before it falls back to itself, which is what
    /// `.agents/rules/supported-languages.md` asks for. Returning the key rather than an empty
    /// string is deliberate: a screen with a key on it is a bug somebody reports, and a screen
    /// with a gap on it is a bug nobody notices.
    ///
    /// # Examples
    ///
    /// ```
    /// use almena_cli::{catalog::Catalog, language::Language};
    ///
    /// assert_eq!(Catalog::of(Language::English).text("app.name"), "Almena");
    /// ```
    #[must_use]
    pub fn text(&self, key: &str) -> String {
        Self::look_up(self.language, key)
            .or_else(|| Self::look_up(Language::English, key))
            .unwrap_or_else(|| key.to_owned())
    }

    /// The text at `key` with `{{name}}` replaced by `value`.
    ///
    /// One substitution, because one is all this program needs. A second would be an argument
    /// for a real formatter rather than for a second parameter here.
    #[must_use]
    pub fn filled(&self, key: &str, name: &str, value: &str) -> String {
        self.text(key).replace(&format!("{{{{{name}}}}}"), value)
    }

    /// The text at `key` in one catalog, if that catalog has it.
    fn look_up(language: Language, key: &str) -> Option<String> {
        let catalogs = PARSED.get_or_init(|| {
            [
                serde_json::from_str(ENGLISH).ok(),
                serde_json::from_str(SPANISH).ok(),
            ]
        });

        let index = match language {
            Language::English => 0,
            Language::Spanish => 1,
        };

        let mut node = catalogs.get(index)?.as_ref()?;
        for part in key.split('.') {
            node = node.get(part)?;
        }

        node.as_str().map(ToOwned::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::Catalog;
    use crate::language::Language;

    #[test]
    fn both_catalogs_parse_and_answer() {
        // If either file stopped being the JSON shape every catalog shares, this is where it
        // would say so rather than at the moment somebody ran the program.
        assert_eq!(Catalog::of(Language::English).text("app.name"), "Almena");
        assert_eq!(Catalog::of(Language::Spanish).text("app.name"), "Almena");
    }

    #[test]
    fn the_two_languages_differ_where_they_should() {
        let english = Catalog::of(Language::English).text("control.unmeasured");
        let spanish = Catalog::of(Language::Spanish).text("control.unmeasured");
        assert_ne!(english, spanish, "the Spanish catalog is a copy of English");
    }

    #[test]
    fn every_key_this_program_says_exists_in_both() {
        // The half of `catalog-parity.md` a type checker cannot do for a program written in
        // Rust: `tsc` holds the frontend's keys to the English catalog, and nothing would
        // otherwise hold this program's.
        for &key in crate::view::draw::KEYS {
            for language in [Language::English, Language::Spanish] {
                let text = Catalog::of(language).text(key);
                assert_ne!(text, key, "{key} is missing from {language:?}");
                assert!(!text.is_empty(), "{key} is empty in {language:?}");
            }
        }
    }

    #[test]
    fn a_missing_key_is_visible_rather_than_silent() {
        let catalog = Catalog::of(Language::English);
        assert_eq!(catalog.text("nothing.is.here"), "nothing.is.here");
    }

    #[test]
    fn a_placeholder_is_filled() {
        let filled = Catalog::of(Language::English).filled("app.version", "version", "0.1.0");
        assert!(filled.contains("0.1.0"), "{filled}");
        assert!(!filled.contains("{{"), "{filled}");
    }
}
