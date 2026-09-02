//! Why something did not happen, as an identifier and never a sentence.
//!
//! Every failure this program reports travels as a stable `snake_case` word with, where there is
//! one, a `key=value` detail after it — the same shape the node and the holder's app use. An
//! operator pastes the word into a search and finds the same word in another log; a sentence in
//! one language would be found by nobody running the program in another.

/// Something that did not happen, and the word for why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failed(String);

impl Failed {
    /// A failure by its word alone.
    #[must_use]
    pub fn new(word: &str) -> Self {
        Self(word.to_owned())
    }

    /// A failure with a detail after the word, written as `word key=value`.
    #[must_use]
    pub fn with(word: &str, key: &str, value: &str) -> Self {
        Self(format!("{word} {key}={value}"))
    }

    /// A failure from a line already written in the shape every failure has: the word, then
    /// `key=value` details. For the one caller that has two details to give.
    #[must_use]
    pub fn line(text: String) -> Self {
        Self(text)
    }

    /// The word, with whatever detail follows it.
    #[must_use]
    pub fn word(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for Failed {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Failed {}

impl From<&str> for Failed {
    fn from(word: &str) -> Self {
        Self::new(word)
    }
}

#[cfg(test)]
mod tests {
    use super::Failed;

    #[test]
    fn a_failure_is_a_word_and_a_detail_and_never_a_sentence() {
        assert_eq!(
            Failed::new("node_unreachable").to_string(),
            "node_unreachable"
        );
        assert_eq!(
            Failed::with("act_not_taken", "rule", "6").to_string(),
            "act_not_taken rule=6"
        );
    }
}
