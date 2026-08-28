//! Everything this node has accepted, in the order it accepted it.
//!
//! **The entry is what everybody holds; the operation is spread out.** An entry is on the order of
//! a hundred bytes and never goes away, which is why it carries only what is needed to place an
//! act in time and to know whether it can be read at all — and why the act itself, which is far
//! larger, is not something every node is expected to keep.
//!
//! # The position is this node's, and nothing may be decided against it
//!
//! `seq` is where an act sits in **this** node's record. Another node that accepted the same act
//! at a different moment gives it a different position, and both are right. That is why nothing
//! about validity is ever decided against a position: two honest nodes would disagree, which is
//! the outcome this whole design is arranged to avoid. Time is decided against the epoch, which
//! is the same number everywhere without anybody coordinating.
//!
//! # The bytes are kept as they arrived
//!
//! A signature covers the bytes that were signed. This keeps those, and hands them back
//! unchanged: a node that tidied an act before storing it would break the signature over it, and
//! most surely over the fields it did not understand well enough to tidy.

use std::collections::BTreeMap;

use almena_format::entry::Entry;
use almena_format::identifier::{Did, Name};
use almena_format::operation::Operation;
use almena_suite::digest::Digest;

use crate::tree::{Path, Tree};

/// The record of what this node has accepted.
#[derive(Debug, Clone, Default)]
pub struct Log {
    entries: Vec<Entry>,
    /// The act behind each entry, byte for byte as it arrived.
    acts: Vec<Vec<u8>>,
    /// The tree over the entries, which is what gives them a position in time.
    tree: Tree,
    /// Where to find an act by its hash.
    at: BTreeMap<Name, usize>,
    /// Which entries speak about somebody other than their author.
    about: BTreeMap<String, Vec<usize>>,
    /// Which entries advance each object's own chain.
    ///
    /// **What makes a summary checkable by anybody.** A checkpoint says which act last set each
    /// part of an object, and the way to find out whether it left something out is to look at that
    /// object's acts — which every node holds, so nobody has to be asked and nobody has to be
    /// believed.
    chains: BTreeMap<String, Vec<usize>>,
}

impl Log {
    /// A log that has not been opened yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many acts have been written down.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing has been written down.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Write an act down, and say where it landed.
    ///
    /// `subject` names what the act is about when that is not its author — a certification, a
    /// vote, a contradiction. It is what makes *is X certified?* answerable without walking
    /// everything, and a daily summary carries none because it speaks about many nodes at once.
    ///
    /// Nothing is validated here. Whether an act may be written down at all is
    /// [`crate::chain`]'s question, and this happens after it has answered.
    pub fn append(&mut self, operation: &Operation, subject: Option<Did>) -> Entry {
        let bytes = operation.to_bytes();
        let sequence = self.entries.len() as u64;
        let entry = Entry::of(operation, sequence, subject.clone());

        self.at.insert(entry.hash.clone(), self.entries.len());
        if let Some(subject) = subject {
            self.about
                .entry(subject.to_string())
                .or_default()
                .push(self.entries.len());
        }
        self.tree.append(&entry.to_bytes());
        self.acts.push(bytes);
        self.chains
            .entry(operation.object.to_string())
            .or_default()
            .push(self.entries.len());
        self.entries.push(entry.clone());
        entry
    }

    /// The entries that advance an object's own chain, in the order this node wrote them.
    ///
    /// Not the same as what speaks *about* it: a certification is somebody else's act about this
    /// object and lives in their chain, not in its own.
    #[must_use]
    pub fn chain_of(&self, object: &Did) -> Vec<&Entry> {
        self.chains
            .get(&object.to_string())
            .map(|positions| {
                positions
                    .iter()
                    .filter_map(|at| self.entries.get(*at))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The entry at a position in this node's record.
    #[must_use]
    pub fn at_sequence(&self, sequence: u64) -> Option<&Entry> {
        self.entries.get(usize::try_from(sequence).ok()?)
    }

    /// The act behind an entry, in the bytes it arrived in.
    #[must_use]
    pub fn act(&self, hash: &Name) -> Option<&[u8]> {
        self.acts.get(*self.at.get(hash)?).map(Vec::as_slice)
    }

    /// The entry an act got, by its hash.
    #[must_use]
    pub fn entry(&self, hash: &Name) -> Option<&Entry> {
        self.entries.get(*self.at.get(hash)?)
    }

    /// Everything written down about somebody other than its author.
    ///
    /// This is what the `sujeto` field exists for: answering *what has been said about X* without
    /// reading the whole record.
    #[must_use]
    pub fn about(&self, subject: &Did) -> Vec<&Entry> {
        self.about
            .get(&subject.to_string())
            .map(|positions| {
                positions
                    .iter()
                    .filter_map(|at| self.entries.get(*at))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The root over everything written down so far.
    #[must_use]
    pub fn root(&self) -> Digest {
        self.tree.root()
    }

    /// The path that proves an act was written down where this node says it was.
    #[must_use]
    pub fn inclusion(&self, hash: &Name) -> Option<(u64, Path)> {
        let at = *self.at.get(hash)?;
        let path = self.tree.inclusion(at)?;
        Some((at as u64, path))
    }

    /// The root of this record as it was when it held `size` entries.
    ///
    /// What a node checks its own record against when it comes back: the root it signed then has to
    /// be the root the record still gives now, or acts it vouched for have gone.
    #[must_use]
    pub fn root_at(&self, size: u64) -> Option<Digest> {
        self.tree.root_at(usize::try_from(size).ok()?)
    }

    /// The same, against this record as it was when it held `size` entries.
    ///
    /// **What makes a proof worth handing to somebody.** A path proves nothing on its own: it
    /// proves an entry against a root of a stated size, and the only roots anybody has this node's
    /// name on are the ones it published at the ends of epochs.
    #[must_use]
    pub fn inclusion_at(&self, hash: &Name, size: u64) -> Option<(u64, Path)> {
        let at = *self.at.get(hash)?;
        let path = self.tree.inclusion_at(at, usize::try_from(size).ok()?)?;
        Some((at as u64, path))
    }
}

#[cfg(test)]
mod tests {
    use super::Log;
    use crate::kind::Kind;
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::{Operation, create};
    use almena_time::Epoch;
    use std::collections::BTreeMap;

    fn act(what: u64) -> Operation {
        create(
            Network::Development,
            Kind::HOLDER_CREATE.number(),
            1,
            Epoch::GENESIS,
            BTreeMap::from([(1, Value::Uint(what))]),
        )
    }

    fn somebody(seed: u8) -> Did {
        Did::new(Network::Development, Name::of(&[seed; 4]))
    }

    #[test]
    fn a_log_starts_empty_and_grows_in_order() {
        let mut log = Log::new();
        assert!(log.is_empty());

        for n in 0..3 {
            let entry = log.append(&act(n), None);
            assert_eq!(entry.sequence, n);
        }
        assert_eq!(log.len(), 3);
        assert_eq!(log.at_sequence(1).expect("there").sequence, 1);
        assert!(log.at_sequence(3).is_none());
    }

    #[test]
    fn an_act_comes_back_in_the_bytes_it_arrived_in() {
        // A signature covers the bytes that were signed. Handing back anything else would break
        // it, and most surely over fields this node did not understand well enough to tidy.
        let mut log = Log::new();
        let operation = act(7);
        let entry = log.append(&operation, None);

        assert_eq!(log.act(&entry.hash), Some(operation.to_bytes().as_slice()));
    }

    #[test]
    fn what_is_said_about_somebody_is_found_without_reading_everything() {
        let mut log = Log::new();
        let about = somebody(9);
        log.append(&act(1), None);
        log.append(&act(2), Some(about.clone()));
        log.append(&act(3), Some(somebody(11)));
        log.append(&act(4), Some(about.clone()));

        let said = log.about(&about);
        assert_eq!(said.len(), 2);
        assert_eq!(said[0].sequence, 1);
        assert_eq!(said[1].sequence, 3);
        assert!(log.about(&somebody(200)).is_empty());
    }

    #[test]
    fn every_act_can_prove_where_it_was_written_down() {
        let mut log = Log::new();
        let mut written = Vec::new();
        for n in 0..9 {
            written.push(log.append(&act(n), None));
        }
        let root = log.root();

        for entry in &written {
            let (at, path) = log.inclusion(&entry.hash).expect("it is in there");
            assert_eq!(at, entry.sequence);
            assert!(
                crate::tree::included(&entry.to_bytes(), at as usize, log.len(), &path, &root),
                "sequence {at}"
            );
        }
    }

    #[test]
    fn nothing_proves_an_act_that_was_never_written_down() {
        let mut log = Log::new();
        log.append(&act(1), None);
        assert!(log.inclusion(&Name::of(b"never happened")).is_none());
        assert!(log.act(&Name::of(b"never happened")).is_none());
    }

    #[test]
    fn the_position_is_this_nodes_and_two_nodes_need_not_agree_on_it() {
        // Which is why nothing about validity is ever decided against it: two honest nodes would
        // disagree, and that is the one outcome this design cannot afford.
        let operation = act(42);

        let mut here = Log::new();
        here.append(&act(1), None);
        here.append(&act(2), None);
        let mine = here.append(&operation, None);

        let mut there = Log::new();
        let theirs = there.append(&operation, None);

        assert_eq!(mine.hash, theirs.hash, "the same act");
        assert_ne!(mine.sequence, theirs.sequence, "in different places");
        assert_ne!(here.root(), there.root(), "and different trees");
    }
}
