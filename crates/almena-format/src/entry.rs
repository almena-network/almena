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
//! critical  the odd field numbers the operation's payload carries, in order
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
//! - `critical` is here because `tipo` alone does not say whether an act can be applied. A field
//!   nobody knew when the kind was numbered is what an extension looks like, and a reader that
//!   cannot read it may not claim to have applied the act — but the payload is spread across the
//!   network while the log is everywhere, so without this a reader checking a summary against the
//!   log could vouch across an act it would have declared opaque had it held the act. The numbers
//!   and never the values: what is asked is *could this build read it*, which the numbers answer
//!   and the values do not, and two builds hash one entry to one thing only if nothing in it
//!   depends on what either of them understands.

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
    /// The odd field numbers the operation's payload carries.
    ///
    /// Odd itself: an entry read without it is an entry whose act nobody can say they could
    /// apply, so a reader that skipped it would be back to judging by kind alone.
    pub const CRITICAL: u64 = 9;
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
    /// The odd field numbers the operation's payload carries, strictly ascending.
    ///
    /// **Every one of them, whatever this build makes of them.** A reader holding the log and not
    /// the act asks whether *its* vocabulary for that kind covers every number here; a number
    /// outside it is a field the reader could not have applied, and the act is one it may not
    /// vouch across. Written by whoever writes the entry from the payload itself, so two builds
    /// that disagree about what a number means still write the same list.
    pub critical: Vec<u64>,
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
        // Always present, and an empty list when the payload has no critical field: an act that
        // any build can apply is a fact stated, exactly as *first in the chain* is.
        fields.insert(
            key::CRITICAL,
            Value::Array(self.critical.iter().copied().map(Value::Uint).collect()),
        );
        Value::Map(fields).to_bytes()
    }

    /// The entry an operation gets when a node writes it down at `sequence`.
    ///
    /// The hash is what the act is **called**, which leaves out how it was signed. It has to: an
    /// ECDSA signature has two valid forms for one message, so a hash taken over the signatures
    /// would give one act two names — and anybody who merely saw it go past could reprint it in the
    /// other form and have it read as a second act. Two nodes holding the same act in different
    /// forms would also write different entries for it, and neither could check the other's proof
    /// that it is in their tree.
    ///
    /// What the log stores and hands back is still every byte as it arrived. Only what the act is
    /// *called* leaves out the part two honest parties can write two ways.
    #[must_use]
    pub fn of(
        operation: &crate::operation::Operation,
        sequence: u64,
        subject: Option<Did>,
    ) -> Self {
        Self {
            sequence,
            hash: operation.called(),
            object: operation.object.clone(),
            previous: operation.previous.clone(),
            kind: operation.kind,
            version: operation.version,
            subject,
            critical: critical_fields_of(&operation.payload),
        }
    }
}

/// The odd field numbers a payload carries, in ascending order.
///
/// Taken from the payload's keys and from nothing this build knows about them: the parity is the
/// mark, and the map is already in canonical order, so the list is the same whoever writes it.
#[must_use]
pub fn critical_fields_of(payload: &BTreeMap<u64, Value>) -> Vec<u64> {
    payload
        .keys()
        .copied()
        .filter(|number| crate::field::Field::new(*number).is_critical())
        .collect()
}

/// Read an entry back from the bytes it was written in.
///
/// **What lets a node come back to a record holding entries whose acts it does not have.** The tree
/// over the entries is what a node has put its name to, so it has to be rebuilt exactly — and an
/// entry that had to be derived from its act could only be rebuilt where the act was still held.
///
/// [`None`] when the value is not an entry: a field missing, or one of the wrong shape.
#[must_use]
pub fn read(value: &Value) -> Option<Entry> {
    let Value::Map(fields) = value else {
        return None;
    };
    let (&Value::Uint(sequence), Value::Text(hash), Value::Text(object)) = (
        fields.get(&key::SEQUENCE)?,
        fields.get(&key::HASH)?,
        fields.get(&key::OBJECT)?,
    ) else {
        return None;
    };
    let previous = match fields.get(&key::PREVIOUS)? {
        Value::Null => None,
        Value::Text(name) => Some(Name::parse(name).ok()?),
        _ => return None,
    };
    let (&Value::Uint(kind), &Value::Uint(version)) =
        (fields.get(&key::KIND)?, fields.get(&key::VERSION)?)
    else {
        return None;
    };
    // Absent is absent, never null: an entry that wrote it as null would be one written by
    // something that does not agree with this about how absence is spelled.
    let subject = match fields.get(&key::SUBJECT) {
        None => None,
        Some(Value::Text(did)) => Some(Did::parse(did).ok()?),
        Some(_) => return None,
    };
    let critical = critical_read(fields.get(&key::CRITICAL)?)?;

    Some(Entry {
        sequence,
        hash: Name::parse(hash).ok()?,
        object: Did::parse(object).ok()?,
        previous,
        kind,
        version,
        subject,
        critical,
    })
}

/// The critical-field list read back, held to the one shape it is written in.
///
/// Odd numbers, strictly ascending, and nothing else: a list in any other order would be a second
/// encoding of one entry, and an even number in it would be a writer that does not agree with this
/// about what the mark is. [`None`] for anything else.
fn critical_read(value: &Value) -> Option<Vec<u64>> {
    let Value::Array(listed) = value else {
        return None;
    };
    let mut critical = Vec::with_capacity(listed.len());
    for number in listed {
        let &Value::Uint(number) = number else {
            return None;
        };
        if !crate::field::Field::new(number).is_critical() {
            return None;
        }
        if critical.last().is_some_and(|last| *last >= number) {
            return None;
        }
        critical.push(number);
    }
    Some(critical)
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
    fn what_an_act_is_called_does_not_depend_on_how_it_was_signed() {
        // **An ECDSA signature has two valid forms for one message.** A name taken over the
        // signatures would give one act two of them, and anybody who merely saw it go past could
        // reprint it in the other form and have it read as a second act on the same chain — a fork
        // made by somebody holding nothing and forging nothing.
        //
        // It also lets two nodes holding one act in different forms check each other's proofs,
        // which they could not if the entry were about the encoding.
        let signed = operation();
        let mut resigned = signed.clone();
        resigned.signatures[0].signature = [6; 64];

        assert_eq!(
            Entry::of(&signed, 0, None).hash,
            Entry::of(&resigned, 0, None).hash,
            "one act, one name"
        );
        assert_eq!(signed.name(), resigned.name(), "and one object");
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

    #[test]
    fn an_entry_reads_back_as_what_was_written() {
        let subject = Did::new(Network::Development, Name::of(b"somebody else"));
        for about in [None, Some(subject)] {
            let entry = Entry::of(&operation(), 7, about);
            let value = read(&entry.to_bytes()).expect("canonical");
            assert_eq!(super::read(&value), Some(entry));
        }
    }

    #[test]
    fn something_that_is_not_an_entry_does_not_read_as_one() {
        let mut written = match read(&Entry::of(&operation(), 0, None).to_bytes()) {
            Ok(Value::Map(fields)) => fields,
            other => panic!("an entry is a map, got {other:?}"),
        };
        written.remove(&key::HASH);
        assert_eq!(super::read(&Value::Map(written)), None);

        assert_eq!(
            super::read(&Value::Uint(9)),
            None,
            "and neither does a number"
        );
    }

    #[test]
    fn an_entry_lists_the_critical_fields_its_act_carries_and_only_those() {
        // What lets a reader holding the log and not the act say *this build could not have
        // applied that* — the numbers, never the values, so that two builds write one list.
        let mut operation = operation();
        operation.payload = BTreeMap::from([
            (4, Value::Uint(0)),
            (3, Value::Uint(0)),
            (1, Value::Uint(0)),
            (101, Value::Uint(0)),
            (100, Value::Uint(0)),
        ]);
        let entry = Entry::of(&operation, 0, None);
        assert_eq!(
            entry.critical,
            vec![1, 3, 101],
            "odd, ascending, even ones left out"
        );
        assert_eq!(
            fields(&entry.to_bytes()).get(&key::CRITICAL),
            Some(&Value::Array(vec![
                Value::Uint(1),
                Value::Uint(3),
                Value::Uint(101)
            ]))
        );

        let mut plain = operation.clone();
        plain.payload = BTreeMap::from([(2, Value::Uint(0))]);
        assert_eq!(
            fields(&Entry::of(&plain, 0, None).to_bytes()).get(&key::CRITICAL),
            Some(&Value::Array(Vec::new())),
            "and an act any build can apply says so with an empty list, never by leaving it out"
        );
    }

    #[test]
    fn an_entry_without_the_list_or_with_a_list_in_another_shape_does_not_read() {
        // One shape per entry. A missing list would be an entry back to judging by kind alone;
        // one out of order, or carrying an even number, is written by something that does not
        // agree with this about what the mark is.
        let written = || match read(&Entry::of(&operation(), 0, None).to_bytes()) {
            Ok(Value::Map(fields)) => fields,
            other => panic!("an entry is a map, got {other:?}"),
        };

        let mut missing = written();
        missing.remove(&key::CRITICAL);
        assert_eq!(super::read(&Value::Map(missing)), None);

        let mut backwards = written();
        backwards.insert(
            key::CRITICAL,
            Value::Array(vec![Value::Uint(3), Value::Uint(1)]),
        );
        assert_eq!(super::read(&Value::Map(backwards)), None);

        let mut twice = written();
        twice.insert(
            key::CRITICAL,
            Value::Array(vec![Value::Uint(1), Value::Uint(1)]),
        );
        assert_eq!(super::read(&Value::Map(twice)), None);

        let mut even = written();
        even.insert(key::CRITICAL, Value::Array(vec![Value::Uint(2)]));
        assert_eq!(super::read(&Value::Map(even)), None);

        let mut not_a_number = written();
        not_a_number.insert(
            key::CRITICAL,
            Value::Array(vec![Value::Text("1".to_owned())]),
        );
        assert_eq!(super::read(&Value::Map(not_a_number)), None);
    }

    #[test]
    fn an_absent_subject_read_back_is_absent_and_not_null() {
        // Two ways to write absence would be two encodings of one entry, so one of them has to be
        // refused rather than quietly understood.
        let mut written = match read(&Entry::of(&operation(), 0, None).to_bytes()) {
            Ok(Value::Map(fields)) => fields,
            other => panic!("an entry is a map, got {other:?}"),
        };
        written.insert(key::SUBJECT, Value::Null);
        assert_eq!(super::read(&Value::Map(written)), None);
    }
}
