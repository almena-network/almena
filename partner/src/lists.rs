//! The status lists this partner publishes, one per cohort of expiries.
//!
//! **What the record does not hold, and the issuer must.** The record holds the hash of each
//! version and the network holds the current bytes; what only the issuer has is the ability to
//! make the next version — which needs the bits as they stand and the name of the act that
//! published the last version, so that the next one follows it on the list's own chain.

use std::collections::BTreeMap;

use almena_status::list::List;

use crate::failed::Failed;

/// One list, as this partner last published it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Held {
    /// The list's identifier in the record.
    pub list: String,
    /// The act that published the version in force, which the next version follows.
    pub previous: String,
    /// The bits, as they travel.
    pub written: String,
}

impl Held {
    /// The bits, read back.
    ///
    /// # Errors
    ///
    /// `lists_unreadable` where what was kept is not a list.
    pub fn bits(&self) -> Result<List, Failed> {
        List::read(&self.written).map_err(|_| Failed::new("lists_unreadable"))
    }
}

/// Every list, by the cohort it covers, written as `2026-Q3`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Lists {
    by_cohort: BTreeMap<String, Held>,
}

impl Lists {
    /// The list for that cohort, if one has been opened.
    #[must_use]
    pub fn get(&self, cohort: &str) -> Option<&Held> {
        self.by_cohort.get(cohort)
    }

    /// Write one down, replacing what was known about that cohort.
    pub fn keep(&mut self, cohort: &str, held: Held) {
        self.by_cohort.insert(cohort.to_owned(), held);
    }

    /// The cohort whose list is that one, by identifier.
    #[must_use]
    pub fn cohort_of(&self, list: &str) -> Option<&str> {
        self.by_cohort
            .iter()
            .find(|(_, held)| held.list == list)
            .map(|(cohort, _)| cohort.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{Held, Lists};
    use almena_status::list::List;

    #[test]
    fn a_list_is_kept_by_cohort_and_found_by_identifier() {
        let mut lists = Lists::default();
        let mut bits = List::empty();
        bits.revoke(7);
        lists.keep(
            "2026-Q3",
            Held {
                list: "did:almena:dev:zList".to_owned(),
                previous: "zAct".to_owned(),
                written: bits.written(),
            },
        );
        assert_eq!(lists.cohort_of("did:almena:dev:zList"), Some("2026-Q3"));
        assert!(
            lists
                .get("2026-Q3")
                .expect("held")
                .bits()
                .expect("bits")
                .revoked(7)
        );
        assert_eq!(lists.get("2026-Q4"), None);
    }
}
