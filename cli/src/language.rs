//! Which language this program speaks, and how it is decided.
//!
//! Three answers are asked for in order, and the first that exists wins: **what a person chose**,
//! then **what the environment asks for**, then English. What a person chooses overrules what
//! the system says, and is remembered — and the terminal variant and the window are the same
//! node with two faces. A face that could only be told what language to speak, never asked,
//! would not be the same node.
//!
//! On a command line the environment is `LC_ALL`, `LC_MESSAGES` and `LANG`, in the order POSIX
//! gives them, and the choice arrives as `--language`. Where the choice is kept is
//! [`crate::preferences`].
//!
//! # There is no list of languages here
//!
//! Adding one must not mean touching code, so this type holds a **tag** rather than a variant
//! per language, and the tags come from the catalog directory by way of `build.rs`.
//! Adding French is adding `fr.json`.

use crate::catalog::CATALOGS;

/// A language this program has a catalog for.
///
/// It cannot be constructed from a tag nobody has a catalog for: [`Language::from_tag`] answers
/// with the source language instead, which is what makes *unrecognised* and *missing* the same
/// harmless outcome rather than two error paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language(&'static str);

impl Language {
    /// The tag of the source language: what keys are written in, and what everything falls back
    /// to. `build.rs` refuses to build without its catalog.
    pub const SOURCE: &'static str = "en";

    /// The source language.
    #[must_use]
    pub fn source() -> Self {
        Self(Self::SOURCE)
    }

    /// Every language there is a catalog for, in the order the directory gave them.
    pub fn available() -> impl Iterator<Item = Self> {
        CATALOGS.iter().map(|(tag, _)| Self(tag))
    }

    /// The tag, as it is written in the catalog directory and stored in the preferences.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        self.0
    }

    /// The language a POSIX locale or a BCP 47 tag names, else the source language.
    ///
    /// Only the part before the first `_`, `-` or `.` is read: `es_ES.UTF-8`, `es-419` and `es`
    /// are the same answer, and a region this project does not distinguish must never become a
    /// language it does not have.
    #[must_use]
    pub fn from_tag(tag: &str) -> Self {
        let primary = tag
            .split(['_', '-', '.'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();

        Self::available()
            .find(|language| language.tag() == primary)
            .unwrap_or_else(Self::source)
    }

    /// What the environment asks for.
    #[must_use]
    pub fn from_environment() -> Self {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|name| std::env::var(name).ok())
            .map_or_else(Self::source, |value| Self::from_tag(&value))
    }

    /// What this run speaks: the choice if there is one, else the environment.
    ///
    /// `chosen` is what `--language` asked for or what was stored, in that order, and it is a
    /// free string on purpose — it may name a language this build no longer ships, and the
    /// answer to that is the source language rather than a refusal to start.
    #[must_use]
    pub fn settled(chosen: Option<&str>) -> Self {
        chosen.map_or_else(Self::from_environment, Self::from_tag)
    }
}

#[cfg(test)]
mod tests {
    use super::Language;

    #[test]
    fn spanish_is_recognised_however_it_is_written() {
        for tag in ["es", "es_ES.UTF-8", "es-419", "ES", "es_MX"] {
            assert_eq!(Language::from_tag(tag).tag(), "es", "{tag}");
        }
    }

    #[test]
    fn everything_else_falls_back_to_the_source() {
        // Including the ones that merely start with the letters: a language this project does
        // not have must never be served the wrong catalog because its tag looked similar.
        for tag in ["fr", "C", "POSIX", "", "est", "eskimo"] {
            assert_eq!(Language::from_tag(tag), Language::source(), "{tag}");
        }
    }

    #[test]
    fn the_languages_are_the_catalogs_that_exist() {
        // No list is written here, so this is what the directory says today rather than what
        // this file decided. Adding a catalog changes it without changing any code.
        let tags: Vec<&str> = Language::available().map(Language::tag).collect();
        assert!(tags.contains(&"en"), "{tags:?}");
        assert!(tags.contains(&"es"), "{tags:?}");
        assert!(tags.len() >= 2, "{tags:?}");
    }

    #[test]
    fn a_choice_beats_the_environment() {
        // A person's choice overrules the system, and that is the whole of it when they
        // disagree.
        assert_eq!(Language::settled(Some("es")).tag(), "es");
        assert_eq!(Language::settled(Some("en")).tag(), "en");
    }

    #[test]
    fn a_choice_this_build_does_not_ship_is_not_a_refusal_to_start() {
        // Somebody who chose a language on a newer build, then went back to an older one.
        assert_eq!(Language::settled(Some("fr")), Language::source());
    }
}
