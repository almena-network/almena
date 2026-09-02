//! What this partner has issued, and what each holder decided.
//!
//! **The index↔holder correspondence an issuer keeps anyway.** The status list says nothing about
//! whose each bit is, so revoking one credential and telling its holder both need this: which
//! credential sits at which index of which list, and which relationship it was offered on. Kept
//! by the credential's own identifier, which is what a holder's acknowledgement names.

use std::collections::BTreeMap;

/// One credential issued.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Record {
    /// The credential as it was sent, which is what a renewal or a reissue starts from.
    pub written: String,
    /// The far end of the relationship it was offered on.
    pub relation: String,
    /// The status list its bit is in, where it has one.
    pub list: Option<String>,
    /// Which bit, where it has one.
    pub index: Option<u64>,
    /// What the holder said, once they said anything: taken or refused.
    pub decided: Option<bool>,
    /// The epoch it was revoked in, once it was.
    pub revoked_at: Option<u64>,
}

/// Every credential issued, by identifier.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Issued {
    by_identifier: BTreeMap<String, Record>,
}

impl Issued {
    /// Write one down.
    pub fn keep(&mut self, identifier: &str, record: Record) {
        self.by_identifier.insert(identifier.to_owned(), record);
    }

    /// One, by its identifier.
    #[must_use]
    pub fn get(&self, identifier: &str) -> Option<&Record> {
        self.by_identifier.get(identifier)
    }

    /// One, to change.
    pub fn get_mut(&mut self, identifier: &str) -> Option<&mut Record> {
        self.by_identifier.get_mut(identifier)
    }

    /// Whether that index of that list is already somebody's.
    ///
    /// **The one place that can say**, because the list itself does not: an index drawn at random
    /// collides rarely and this is what notices when it does.
    #[must_use]
    pub fn taken(&self, list: &str, index: u64) -> bool {
        self.by_identifier
            .values()
            .any(|record| record.list.as_deref() == Some(list) && record.index == Some(index))
    }

    /// Every record, by identifier, in a fixed order.
    #[must_use]
    pub fn all(&self) -> Vec<(&String, &Record)> {
        self.by_identifier.iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Issued, Record};

    #[test]
    fn a_taken_index_is_noticed_and_a_free_one_is_not() {
        let mut issued = Issued::default();
        issued.keep(
            "one",
            Record {
                written: String::new(),
                relation: "did:peer:2.x".to_owned(),
                list: Some("did:almena:dev:zList".to_owned()),
                index: Some(42),
                decided: None,
                revoked_at: None,
            },
        );
        assert!(issued.taken("did:almena:dev:zList", 42));
        assert!(!issued.taken("did:almena:dev:zList", 43));
        assert!(!issued.taken("did:almena:dev:zOther", 42));
        assert_eq!(issued.get("one").map(|held| held.index), Some(Some(42)));
    }
}
