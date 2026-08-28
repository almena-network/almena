//! The words this program says, read from the catalogs the whole project shares.
//!
//! A string a person reads comes from a catalog and never from the source, and every catalog
//! holds the same keys as every other. Both are about one set of files, so this program reads
//! **those** files rather than growing a second set that would agree with the first for about
//! a month.
//!
//! They are compiled in with `include_str!`. A node on a server has no frontend beside it to
//! read them from, and a catalog that could go missing at run time is a program that could
//! start speechless.
//!
//! **Which files those are is not written here.** `build.rs` reads the directory and generates
//! the table below, because adding a language must not mean touching code and a `const` per
//! language is exactly the code it would mean touching.
//!
//! A record is not user-facing text and never comes from here: an operator quotes a code and
//! somebody else finds it, whatever language either of them has the interface in.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_json::Value;

use crate::language::Language;

include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));

/// The catalogs, parsed once, by the language each is written in.
static PARSED: OnceLock<BTreeMap<&'static str, Value>> = OnceLock::new();

/// The words of one language, ready to be asked for a key.
#[derive(Debug, Clone, Copy)]
pub struct Catalog {
    /// Which language this one is written in.
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
    /// A missing key falls back to English before it falls back to itself, English being the
    /// source language and what everything falls back to. Returning the key rather than an empty
    /// string is deliberate: a screen with a key on it is a bug somebody reports, and a screen
    /// with a gap on it is a bug nobody notices.
    ///
    /// # Examples
    ///
    /// ```
    /// use almena_cli::{catalog::Catalog, language::Language};
    ///
    /// assert_eq!(Catalog::of(Language::source()).text("app.name"), "Almena");
    /// ```
    #[must_use]
    pub fn text(&self, key: &str) -> String {
        Self::look_up(self.language, key)
            .or_else(|| Self::look_up(Language::source(), key))
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
            CATALOGS
                .iter()
                .filter_map(|(tag, text)| Some((*tag, serde_json::from_str(text).ok()?)))
                .collect()
        });

        let mut node = catalogs.get(language.tag())?;
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
        assert_eq!(Catalog::of(Language::source()).text("app.name"), "Almena");
        assert_eq!(
            Catalog::of(Language::from_tag("es")).text("app.name"),
            "Almena"
        );
    }

    #[test]
    fn the_two_languages_differ_where_they_should() {
        let english = Catalog::of(Language::source()).text("control.unmeasured");
        let spanish = Catalog::of(Language::from_tag("es")).text("control.unmeasured");
        assert_ne!(english, spanish, "the Spanish catalog is a copy of English");
    }

    #[test]
    fn every_key_this_program_says_exists_in_both() {
        // Holding the catalogs to one set of keys, for the program a type checker cannot
        // follow: `tsc` holds the frontend's keys to the English catalog, and nothing would
        // otherwise hold this program's.
        for &key in crate::view::draw::KEYS {
            for language in Language::available() {
                let text = Catalog::of(language).text(key);
                assert_ne!(text, key, "{key} is missing from {language:?}");
                assert!(!text.is_empty(), "{key} is empty in {language:?}");
            }
        }
    }

    #[test]
    fn a_missing_key_is_visible_rather_than_silent() {
        let catalog = Catalog::of(Language::source());
        assert_eq!(catalog.text("nothing.is.here"), "nothing.is.here");
    }

    #[test]
    fn a_placeholder_is_filled() {
        let filled = Catalog::of(Language::source()).filled("app.version", "version", "0.1.0");
        assert!(filled.contains("0.1.0"), "{filled}");
        assert!(!filled.contains("{{"), "{filled}");
    }
}
