//! A status list, as the record holds it: the hash of each version, and nothing else.
//!
//! # Only the hash goes in the log
//!
//! `SPECS.md §10.2`. The bytes are hosted on the network and addressed by hash; what the record
//! carries is the hash of the current version, signed by the issuer and with a position in time.
//! Putting the bytes in would be putting sixteen kilobytes of mostly noughts into a log every node
//! keeps for ever, to say a thing one digest says.
//!
//! And it is the hash that makes the whole scheme work: **any source will do**, because whoever
//! serves the bytes is indifferent — either they match the version the record names or they do not.
//!
//! # Signed by the issuer's own key, not by its owners
//!
//! `SPECS.md §10.2` says *signed by the issuer*, and it has to mean the element's own key. An
//! issuer that had to convene its owners to flip a bit would be one that does not revoke: revoking
//! has to cost what issuing costs, or it will not happen at the speed a revocation is for. What the
//! owners decide is a different question and they have already decided it — authorising the element
//! and its issuance key is a sealing act (`SPECS.md §8.2`), and this is the element doing what it
//! was authorised to do.
//!
//! # The cohort is on the chain, so a reader knows when it may stop caring
//!
//! **Only the current version is kept, and a list whose window has passed is dropped entirely**
//! (`SPECS.md §10.2`). Working that out has to be possible from the record alone, without fetching
//! the bytes — otherwise letting go of a list would mean downloading it first.

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name};
use almena_format::operation::Operation;
use almena_time::{Epoch, cohort::Cohort};

use crate::chain::Refused;
use crate::kind::Kind;

/// Where each part of a status list act sits.
pub mod field {
    /// The hash of the version's bytes, which is the whole of what the record carries.
    pub const VERSION: u64 = 1;
    /// Which window of expiries the list covers, as the cohort is written.
    pub const COHORT: u64 = 3;
    /// The issuer whose list it is.
    pub const BY: u64 = 5;
}

/// One version of a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    /// The act that published it.
    pub called: Name,
    /// The epoch it was published in, which is the position in time `SPECS.md §10.2` asks for.
    pub at: Epoch,
    /// The hash of the bytes, which is what a verifier compares against what it downloaded.
    pub hash: Vec<u8>,
}

/// A status list, as its chain says it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusList {
    /// The issuer whose list it is.
    pub by: Did,
    /// Which window of expiries it covers.
    ///
    /// **Read here rather than left as text**, so that whoever holds the record can work out when
    /// the list may be let go of without fetching a byte of it — which is the whole point of the
    /// obligation ending by expiry with no operation at all.
    pub cohort: Cohort,
    /// Every version published, oldest first.
    ///
    /// **The record keeps the history of hashes and the network keeps only the current bytes.** The
    /// two are different things: a log cannot forget, and `SPECS.md §10.2` says no history of
    /// contents is kept — so what is here is the trail of *when* it changed, which is what makes a
    /// verifier able to tell a stale copy from a current one.
    pub versions: Vec<Version>,
}

impl StatusList {
    /// The version in force, which is the last one published.
    #[must_use]
    pub fn latest(&self) -> Option<&Version> {
        self.versions.last()
    }
}

/// How long a hash is here: a SHA-256 digest and nothing else.
const HASH_WIDTH: usize = 32;

/// Whose list it is, read from the act.
#[must_use]
pub fn publishing(operation: &Operation) -> Option<Did> {
    match operation.payload.get(&field::BY) {
        Some(Value::Text(by)) => Did::parse(by).ok(),
        _ => None,
    }
}

/// The fields a status list act may carry that this build has a meaning for.
#[must_use]
pub fn vocabulary() -> almena_format::field::Vocabulary<'static> {
    use almena_format::field::Field;
    const FIELDS: &[Field] = &[
        Field::new(field::VERSION),
        Field::new(field::COHORT),
        Field::new(field::BY),
        Field::new(crate::resolution::FIELD),
    ];
    almena_format::field::Vocabulary::of(FIELDS)
}

/// One version's hash, read from the act.
fn version(operation: &Operation) -> Result<Vec<u8>, Refused> {
    match operation.payload.get(&field::VERSION) {
        // **A width and not a length.** A hash of another width is a hash from another suite, and
        // reading it as this one's would be comparing two things that are not the same thing.
        Some(Value::Bytes(hash)) if hash.len() == HASH_WIDTH => Ok(hash.clone()),
        _ => Err(Refused::Malformed),
    }
}

/// A status list, as the act that published its first version made it.
///
/// # Errors
///
/// [`Refused::Malformed`] for an act missing the hash, the cohort or the issuer.
pub fn born(operation: &Operation) -> Result<StatusList, Refused> {
    let cohort = match operation.payload.get(&field::COHORT) {
        Some(Value::Text(cohort)) => Cohort::read(cohort).ok_or(Refused::Malformed)?,
        _ => return Err(Refused::Malformed),
    };
    Ok(StatusList {
        by: publishing(operation).ok_or(Refused::Malformed)?,
        cohort,
        versions: vec![Version {
            called: operation.called(),
            at: operation.issued,
            hash: version(operation)?,
        }],
    })
}

/// What an act does to a status list.
///
/// # Errors
///
/// [`Refused`].
pub fn does(operation: &Operation, list: &StatusList, kind: Kind) -> Result<StatusList, Refused> {
    if kind != Kind::STATUS_LIST_PUBLISH_VERSION {
        return Err(Refused::Malformed);
    }
    let mut next = list.clone();
    // **The same list, so the cohort may not move.** A list that changed which window it covers
    // would be one whose credentials could no longer say when it may be discarded, and their
    // expiries are signed and cannot follow it.
    if operation
        .payload
        .get(&field::COHORT)
        .is_some_and(|held| held != &Value::Text(list.cohort.written()))
    {
        return Err(Refused::Malformed);
    }
    next.versions.push(Version {
        called: operation.called(),
        at: operation.issued,
        hash: version(operation)?,
    });
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::{born, does, field, publishing};
    use crate::chain::Refused;
    use crate::kind::Kind;
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::{Operation, create};
    use almena_time::Epoch;
    use std::collections::BTreeMap;

    fn issuer() -> Did {
        Did::new(Network::Development, Name::of(b"an issuer"))
    }

    fn published(hash: &[u8], extra: &[(u64, Value)]) -> Operation {
        let mut payload = BTreeMap::from([
            (field::VERSION, Value::Bytes(hash.to_vec())),
            (field::COHORT, Value::Text("2026-Q3".to_owned())),
            (field::BY, Value::Text(issuer().to_string())),
        ]);
        for (which, value) in extra {
            payload.insert(*which, value.clone());
        }
        create(
            Network::Development,
            Kind::STATUS_LIST_PUBLISH_VERSION.number(),
            1,
            Epoch::new(100),
            payload,
        )
    }

    fn then(first: &Operation, hash: &[u8], extra: &[(u64, Value)]) -> Operation {
        let mut payload = BTreeMap::from([(field::VERSION, Value::Bytes(hash.to_vec()))]);
        for (which, value) in extra {
            payload.insert(*which, value.clone());
        }
        Operation {
            object: first.object.clone(),
            previous: Some(first.called()),
            kind: Kind::STATUS_LIST_PUBLISH_VERSION.number(),
            version: 1,
            issued: Epoch::new(110),
            payload,
            signatures: Vec::new(),
        }
    }

    #[test]
    fn the_record_keeps_the_trail_of_hashes_and_never_the_bytes() {
        // **What lets a verifier tell a stale copy from a current one**: it holds the hash the
        // record names and compares whatever it downloaded against it.
        let first = published(&[1; 32], &[]);
        let again = then(&first, &[2; 32], &[]);
        let held = born(&first).expect("a list");
        let held = does(&again, &held, Kind::STATUS_LIST_PUBLISH_VERSION).expect("a version");

        assert_eq!(held.versions.len(), 2);
        assert_eq!(held.latest().expect("one").hash, vec![2; 32]);
        assert_eq!(held.latest().expect("one").at, Epoch::new(110));
        assert_eq!(held.by, issuer());
    }

    #[test]
    fn a_hash_of_another_width_is_a_hash_from_another_suite() {
        assert_eq!(
            born(&published(&[1; 20], &[])),
            Err(Refused::Malformed),
            "and comparing it against this one's would be comparing two different things"
        );
    }

    #[test]
    fn a_list_may_not_change_which_window_it_covers() {
        // The credentials it covers carry their expiry signed inside them and cannot follow it, so
        // a list that moved would be one nobody could work out when to discard.
        let first = published(&[1; 32], &[]);
        let held = born(&first).expect("a list");
        let moved = then(
            &first,
            &[2; 32],
            &[(field::COHORT, Value::Text("2027-Q1".to_owned()))],
        );
        assert_eq!(
            does(&moved, &held, Kind::STATUS_LIST_PUBLISH_VERSION),
            Err(Refused::Malformed)
        );

        // Repeating the same cohort is fine, and so is leaving it out.
        let same = then(
            &first,
            &[2; 32],
            &[(field::COHORT, Value::Text("2026-Q3".to_owned()))],
        );
        assert!(does(&same, &held, Kind::STATUS_LIST_PUBLISH_VERSION).is_ok());
    }

    #[test]
    fn a_list_with_no_cohort_or_no_issuer_is_not_one() {
        let nameless = create(
            Network::Development,
            Kind::STATUS_LIST_PUBLISH_VERSION.number(),
            1,
            Epoch::new(100),
            BTreeMap::from([(field::VERSION, Value::Bytes(vec![1; 32]))]),
        );
        assert_eq!(born(&nameless), Err(Refused::Malformed));
        assert_eq!(publishing(&nameless), None);
    }
}
