//! Which language this program speaks, decided by the environment and by nothing else.
//!
//! `.agents/rules/supported-languages.md` says the device is asked first and a person
//! overrules it. On a command line the device is the environment — `LC_ALL`, `LC_MESSAGES`,
//! `LANG`, in the order POSIX gives them — and there is no way to overrule it yet, because
//! that would mean storing a choice and this program stores nothing. `TODO.md` carries that.

/// A language the interface is available in.
///
/// English is the source language and the fallback, so anything the environment asks for that
/// is not Spanish is English rather than an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// The source language, and what an unrecognised environment gets.
    English,
    /// The other complete catalog.
    Spanish,
}

impl Language {
    /// What the environment asks for.
    #[must_use]
    pub fn from_environment() -> Self {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|name| std::env::var(name).ok())
            .map_or(Self::English, |value| Self::from_tag(&value))
    }

    /// The language a POSIX locale or a BCP 47 tag names.
    ///
    /// Only the part before the first `_`, `-` or `.` is read: `es_ES.UTF-8`, `es-419` and
    /// `es` are the same answer, and a region this project does not distinguish must never
    /// become a language it does not have.
    #[must_use]
    pub fn from_tag(tag: &str) -> Self {
        let primary = tag
            .split(['_', '-', '.'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();

        if primary == "es" {
            Self::Spanish
        } else {
            Self::English
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Language;

    #[test]
    fn spanish_is_recognised_however_it_is_written() {
        for tag in ["es", "es_ES.UTF-8", "es-419", "ES", "es_MX"] {
            assert_eq!(Language::from_tag(tag), Language::Spanish, "{tag}");
        }
    }

    #[test]
    fn everything_else_falls_back_to_english() {
        // Including the ones that merely start with the letters: a language this project does
        // not have must never be served the wrong catalog because its tag looked similar.
        for tag in ["en_GB.UTF-8", "fr", "C", "POSIX", "", "est", "eskimo"] {
            assert_eq!(Language::from_tag(tag), Language::English, "{tag}");
        }
    }
}
