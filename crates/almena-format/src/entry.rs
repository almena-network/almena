//! The log entry: the least that places an operation in time and says whether it can be
//! interpreted.
//!
//! ```text
//! seq       position in this node's log
//! hash      hash of the operation
//! objeto    the object whose chain advances
//! prev      hash of that object's previous operation, or null if it is the first
//! tipo      kind of operation
//! version   version of that kind
//! sujeto    the object it is about, when that is not its author   (optional)
//! ```
//!
//! **This is the universal part, and therefore the expensive one** — on the order of a hundred
//! bytes, held by everyone, while the operation itself is spread across the network.
//! Every field earns its place:
//!
//! - `prev` is here as well as in the operation so that **a chain's integrity can be checked from
//!   the log alone**, without holding the history.
//! - `tipo` and `version` are here because a node is obliged to replicate what it does not
//!   understand and **to know when it does not understand it**.
//! - `sujeto` is here so that a claim about someone else — a certification — can be indexed
//!   without walking everything.

use crate::cbor::Value;
use crate::identifier::{Did, Name};
use std::collections::BTreeMap;

/// Where each part of an entry sits in the map.
mod key {
    /// Position in this node's log.
    pub const SEQUENCE: u64 = 1;
    /// The hash of the operation.
    pub const HASH: u64 = 2;
    /// The object whose chain advances.
    pub const OBJECT: u64 = 3;
    /// The hash of that object's previous operation, or null.
    pub const PREVIOUS: u64 = 4;
    /// Which kind of operation.
    pub const KIND: u64 = 5;
    /// Which version of that kind.
    pub const VERSION: u64 = 6;
    /// What it is about, when that is not its author.
    pub const SUBJECT: u64 = 7;
}

/// One line of a node's log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Position in **this node's** log. Each node has its own tree and its own `seq`,
    /// which is why nothing about validity may be decided against it.
    pub sequence: u64,
    /// The hash of the operation this places.
    pub hash: Name,
    /// The object whose chain advances.
    pub object: Did,
    /// The hash of that object's previous operation. [`None`] on the first, written as null.
    pub previous: Option<Name>,
    /// Which kind of operation.
    pub kind: u64,
    /// Which version of that kind.
    pub version: u64,
    /// What it is about, when that is not its author — a certification, a vote, a contradiction.
    ///
    /// **A daily summary carries none**: it speaks about many nodes at once, and one
    /// entry per observed node per day is the N² that an aggregate was chosen to avoid.
    pub subject: Option<Did>,
}

impl Entry {
    /// The canonical bytes of this entry.
    ///
    /// An absent optional field is **left out**, never written as null — the one exception being
    /// `prev`, which is always present because *first in the chain* is a fact worth stating rather
    /// than inferring from a gap. Two ways to write absence would be two encodings of one entry.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let previous = self
            .previous
            .as_ref()
            .map_or(Value::Null, |name| Value::Text(name.as_str().to_owned()));

        let mut fields = BTreeMap::from([
            (key::SEQUENCE, Value::Uint(self.sequence)),
            (key::HASH, Value::Text(self.hash.as_str().to_owned())),
            (key::OBJECT, Value::Text(self.object.to_string())),
            (key::PREVIOUS, previous),
            (key::KIND, Value::Uint(self.kind)),
            (key::VERSION, Value::Uint(self.version)),
        ]);
        if let Some(subject) = &self.subject {
            fields.insert(key::SUBJECT, Value::Text(subject.to_string()));
        }
        Value::Map(fields).to_bytes()
    }

    /// The entry an operation gets when a node writes it down at `sequence`.
    ///
    /// The hash is of the **whole** operation, signatures included: this is the copy everyone
    /// keeps, and what it pins down is the act exactly as it travelled. Only the *name* of an
    /// object leaves the signatures out, and for a different reason.
    #[must_use]
    pub fn of(
        operation: &crate::operation::Operation,
        sequence: u64,
        subject: Option<Did>,
    ) -> Self {
        Self {
            sequence,
            hash: Name::of(&operation.to_bytes()),
            object: operation.object.clone(),
            previous: operation.previous.clone(),
            kind: operation.kind,
            version: operation.version,
            subject,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Entry, key};
    use crate::cbor::{Value, read};
    use crate::identifier::{Did, Name, Network};
    use crate::operation::{Signed, create};
    use almena_time::Epoch;
    use std::collections::BTreeMap;

    fn operation() -> crate::operation::Operation {
        let mut operation = create(
            Network::Development,
            1,
            1,
            Epoch::GENESIS,
            BTreeMap::from([(1, Value::Uint(1))]),
        );
        operation.signatures.push(Signed {
            by: operation.object.clone(),
            key: vec![2; 33],
            signature: [5; 64],
        });
        operation
    }

    fn fields(bytes: &[u8]) -> BTreeMap<u64, Value> {
        match read(bytes) {
            Ok(Value::Map(fields)) => fields,
            other => panic!("an entry is a map, got {other:?}"),
        }
    }

    #[test]
    fn an_entry_is_canonical_and_reads_back() {
        let entry = Entry::of(&operation(), 0, None);
        let bytes = entry.to_bytes();
        assert_eq!(almena_cbor::canonical(&bytes), Ok(()));
        assert!(read(&bytes).is_ok());
    }

    #[test]
    fn an_absent_subject_is_left_out_rather_than_written_as_null() {
        let without = Entry::of(&operation(), 0, None);
        assert!(!fields(&without.to_bytes()).contains_key(&key::SUBJECT));

        let subject = Did::new(Network::Development, Name::of(b"someone else"));
        let with = Entry::of(&operation(), 0, Some(subject));
        assert!(fields(&with.to_bytes()).contains_key(&key::SUBJECT));
    }

    #[test]
    fn prev_is_written_as_null_rather_than_left_out() {
        // Unlike `sujeto`: *first in the chain* is a fact the entry states, so that a chain can be
        // checked from the log without holding the history.
        let entry = Entry::of(&operation(), 0, None);
        assert_eq!(
            fields(&entry.to_bytes()).get(&key::PREVIOUS),
            Some(&Value::Null)
        );
    }

    #[test]
    fn the_hash_covers_the_signatures() {
        // The entry pins the act as it travelled. An operation whose signature changed is a
        // different entry — which is what makes the log a record of what was received.
        let signed = operation();
        let mut resigned = signed.clone();
        resigned.signatures[0].signature = [6; 64];

        assert_ne!(
            Entry::of(&signed, 0, None).hash,
            Entry::of(&resigned, 0, None).hash
        );
        // And yet both are the same object, because the name never depended on the signature.
        assert_eq!(signed.name(), resigned.name());
    }

    #[test]
    fn two_nodes_can_place_the_same_operation_at_different_positions() {
        // Each node has its own log and its own `seq`, so nothing about validity may be decided
        // against it — the reason a future `emitida` is rejected against the epoch instead.
        let operation = operation();
        let here = Entry::of(&operation, 7, None);
        let there = Entry::of(&operation, 4_812, None);
        assert_eq!(here.hash, there.hash);
        assert_ne!(here.to_bytes(), there.to_bytes());
    }
}
