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
    /// The act behind each entry, byte for byte as it arrived — where this node holds it.
    ///
    /// **[`None`] is an act this node knows about and does not have.** The entries are universal
    /// and the acts are not: everybody carries the line saying an act happened, and only the nodes
    /// it was dealt to carry what it said. A node that could not tell the two apart would have to
    /// keep everything for ever, which is the arrangement this replaces.
    acts: Vec<Option<Vec<u8>>>,
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

        // **An act already written down is not written again**, whoever asked. One act with two
        // leaves would put two nodes handed the same acts in different numbers of copies on
        // different roots — and, because what an act is called leaves out how it was signed, the
        // second copy could be one carrying a signature nobody made, quietly taking over the name.
        //
        // Every caller is supposed to have decided this already. This is here so that the next one
        // does not have to remember, which is how both of the ways it went wrong got in.
        if let Some(&held) = self.at.get(&entry.hash) {
            return self.entries[held].clone();
        }

        self.at.insert(entry.hash.clone(), self.entries.len());
        if let Some(subject) = subject {
            self.about
                .entry(subject.to_string())
                .or_default()
                .push(self.entries.len());
        }
        self.tree.append(&entry.to_bytes());
        self.acts.push(Some(bytes));
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

    /// Every act this record holds, in the order it took them.
    ///
    /// **What was taken, not what was handed over.** A node that came back from a record of what it
    /// was offered would replay copies it never accepted, and one act written twice is one act with
    /// two leaves in the tree.
    #[must_use]
    pub fn everything(&self) -> Vec<Vec<u8>> {
        self.acts.iter().flatten().cloned().collect()
    }

    /// The entry at a position in this node's record.
    #[must_use]
    pub fn at_sequence(&self, sequence: u64) -> Option<&Entry> {
        self.entries.get(usize::try_from(sequence).ok()?)
    }

    /// The act behind an entry, in the bytes it arrived in.
    #[must_use]
    pub fn act(&self, hash: &Name) -> Option<&[u8]> {
        self.acts.get(*self.at.get(hash)?)?.as_deref()
    }

    /// Take note that an act happened without holding what it said.
    ///
    /// **This is what a shared-out history looks like from the inside.** The line saying an act
    /// happened is universal — everybody carries it, and it is what lets anybody check a chain's
    /// shape, find what was said about somebody, and prove where something sits. What it said is
    /// carried by the nodes it was dealt to.
    ///
    /// The entry goes into the tree exactly as it would have: **an entry is never dropped and never
    /// skipped**, because the tree over them is what this node has put its name to, and a tree that
    /// changed shape would make a node contradict its own past roots.
    pub fn noted(&mut self, entry: &Entry) -> Entry {
        if let Some(&held) = self.at.get(&entry.hash) {
            return self.entries[held].clone();
        }

        self.at.insert(entry.hash.clone(), self.entries.len());
        if let Some(subject) = &entry.subject {
            self.about
                .entry(subject.to_string())
                .or_default()
                .push(self.entries.len());
        }
        self.tree.append(&entry.to_bytes());
        self.acts.push(None);
        self.chains
            .entry(entry.object.to_string())
            .or_default()
            .push(self.entries.len());
        self.entries.push(entry.clone());
        entry.clone()
    }

    /// Take in what an act said, at the position its entry already holds.
    ///
    /// **The entry does not move and the tree does not change.** A node that was told an act
    /// happened and later got what it said has learned something about an act it already had a
    /// place for — appending it again would put two leaves in a tree it has signed over.
    ///
    /// Returns whether there was a place for it. [`false`] is an act whose entry this node has not
    /// got, which is not something to fill in: an act without its line in the record is one nobody
    /// has said happened.
    pub fn keep(&mut self, operation: &Operation) -> bool {
        let Some(&at) = self.at.get(&operation.called()) else {
            return false;
        };
        if let Some(held) = self.acts.get_mut(at) {
            *held = Some(operation.to_bytes());
            return true;
        }
        false
    }

    /// Let go of what an act said, keeping the line that says it happened.
    ///
    /// **The entry stays and the tree does not move.** What a node has put its name to is the tree
    /// over its entries, so an entry is never dropped — what is dealt out is what the acts said,
    /// and that is what this lets go of.
    ///
    /// Returns whether there was anything to let go of.
    pub fn let_go(&mut self, hash: &Name) -> bool {
        let Some(&at) = self.at.get(hash) else {
            return false;
        };
        self.acts
            .get_mut(at)
            .is_some_and(|held| held.take().is_some())
    }

    /// Everything this node holds, as the act's name and the object it is on.
    ///
    /// The object comes with it because what a node keeps of its own chain is not a matter of the
    /// share-out: a node that let go of what it had itself said would be one nobody could check
    /// anything it said against.
    #[must_use]
    pub fn everything_held(&self) -> Vec<(Name, Name)> {
        self.entries
            .iter()
            .zip(&self.acts)
            .filter(|(_, held)| held.is_some())
            .map(|(entry, _)| (entry.hash.clone(), entry.object.name().clone()))
            .collect()
    }

    /// The acts this node knows happened and has not got.
    ///
    /// **What turns *held elsewhere* into one more question.** Without it that answer is honest and
    /// a dead end: the node knows the thing exists, knows nobody can use it through here, and has
    /// no way to go and get it.
    #[must_use]
    pub fn missing(&self) -> Vec<Name> {
        self.entries
            .iter()
            .zip(&self.acts)
            .filter(|(_, held)| held.is_none())
            .map(|(entry, _)| entry.hash.clone())
            .collect()
    }

    /// The same, with the object each one is on.
    ///
    /// **Because what is owed depends on whose chain it is.** A node keeps its own chain and what
    /// everybody keeps whatever the share-out says, so a caller deciding what to go and fetch needs
    /// the object beside the act — the mirror of what [`Log::everything_held`] hands the caller
    /// deciding what to let go of.
    #[must_use]
    pub fn missing_on(&self) -> Vec<(Name, Name)> {
        self.entries
            .iter()
            .zip(&self.acts)
            .filter(|(_, held)| held.is_none())
            .map(|(entry, _)| (entry.hash.clone(), entry.object.name().clone()))
            .collect()
    }

    /// Whether this node has a line saying that act happened, whether or not it holds what it said.
    #[must_use]
    pub fn knows(&self, hash: &Name) -> bool {
        self.at.contains_key(hash)
    }

    /// Whether this node holds what an act said, as against knowing that it happened.
    #[must_use]
    pub fn holds(&self, hash: &Name) -> bool {
        self.at
            .get(hash)
            .and_then(|at| self.acts.get(*at))
            .is_some_and(Option::is_some)
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
    use almena_format::entry::Entry;
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

    #[test]
    fn an_act_already_written_down_is_not_written_again() {
        // **One act, one leaf.** Two nodes handed the same acts in different numbers of copies
        // would otherwise sign different roots — and since what an act is called leaves out how it
        // was signed, a second copy could be one carrying a signature nobody made, taking over the
        // name the first holds.
        let mut log = Log::new();
        let act = act(1);
        let first = log.append(&act, None);
        let root = log.root();

        let again = log.append(&act, None);
        assert_eq!(again, first, "the entry it already had");
        assert_eq!(log.len(), 1, "and one entry, not two");
        assert_eq!(log.root(), root, "so the tree did not move");
    }

    #[test]
    fn what_is_served_under_a_name_is_what_was_written_under_it_first() {
        // The bytes a second copy carried are not kept, so nothing it said can be handed out under
        // a name this node already vouches for.
        let mut log = Log::new();
        let act = act(1);
        log.append(&act, None);

        let mut other_form = act.clone();
        other_form
            .signatures
            .push(almena_format::operation::Signed {
                by: act.object.clone(),
                key: vec![2; 33],
                signature: [9; 64],
            });
        assert_eq!(other_form.called(), act.called(), "one act, one name");

        log.append(&other_form, None);
        assert_eq!(log.act(&act.called()), Some(act.to_bytes().as_slice()));
    }

    #[test]
    fn an_entry_can_be_held_without_what_its_act_said() {
        // The entries are universal and the acts are not. A node carries the line saying an act
        // happened — which is what lets anybody check a chain's shape and prove where something
        // sits — and what it said is carried by the nodes it was dealt to.
        let mut log = Log::new();
        let act = act(1);
        let entry = Entry::of(&act, 0, None);

        log.noted(&entry);
        assert_eq!(log.len(), 1, "it is in the record");
        assert!(!log.holds(&act.called()), "and what it said is not");
        assert_eq!(log.act(&act.called()), None);
        assert_eq!(
            log.chain_of(&act.object).len(),
            1,
            "the chain's shape is still there to be checked"
        );
        assert!(
            log.inclusion(&act.called()).is_some(),
            "and where it sits is still provable"
        );
    }

    #[test]
    fn an_entry_noted_and_then_held_is_one_entry() {
        // Otherwise a node that was told an act happened and later got it would put two leaves in
        // its own tree for one act — and stop being able to reproduce a root it had signed.
        let mut log = Log::new();
        let act = act(1);
        log.noted(&Entry::of(&act, 0, None));
        let root = log.root();

        log.append(&act, None);
        assert_eq!(log.len(), 1, "one act, one leaf");
        assert_eq!(log.root(), root, "and the tree did not move");
    }
}
