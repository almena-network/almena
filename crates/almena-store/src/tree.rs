//! The append-only tree over everything this node has written down, and the root it publishes.
//!
//! It is the Merkle tree of RFC 6962 — Certificate Transparency's — and using somebody else's
//! shape rather than inventing one is the point: it is analysed, it is implemented in a dozen
//! languages, and an auditor arrives already knowing how to check it.
//!
//! **The tree validates nothing.** Validity lives in the chain each object advances along, and
//! whoever consumes an operation checks it there. What the tree gives is narrower and cannot be
//! got any other way: **a verifiable position in time**, and the impossibility of showing
//! different histories to different people without leaving a trace.
//!
//! # There is no global tree
//!
//! Each node has its own. That is not a compromise, it is the most that can be asked for without a
//! set of validators to agree on one — and it makes what cross-signing detects precise: **two
//! incompatible roots from the same node for the same epoch** prove that node contradicting itself
//! about its own history. Two nodes with different roots prove nothing, and demanding otherwise
//! would be demanding agreement nobody is in a position to produce.
//!
//! # The two prefixes are the whole security argument
//!
//! A leaf is hashed with `0x00` in front and a pair of branches with `0x01`. Without that, a
//! well-chosen leaf could be read as a pair of branches, and one tree would have two shapes with
//! the same root — which is how a second history gets shown to somebody with the same root
//! everybody else checked.

use almena_suite::digest::{Digest, WIDTH};

/// What goes in front of a leaf before it is hashed.
const LEAF: u8 = 0x00;

/// What goes in front of a pair of branches before they are hashed.
const BRANCH: u8 = 0x01;

/// Everything this node has written down, in the order it wrote it.
#[derive(Debug, Clone, Default)]
pub struct Tree {
    leaves: Vec<Digest>,
}

/// The hashes that carry one leaf up to the root, from the leaf's own level upwards.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Path(Vec<Digest>);

impl Path {
    /// The hashes, from the leaf's own level upwards.
    #[must_use]
    pub fn hashes(&self) -> &[Digest] {
        &self.0
    }

    /// How many levels it climbs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether it climbs nothing, which is a tree of one.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Tree {
    /// A tree with nothing in it.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Write one entry down. It goes at the end and nothing moves.
    pub fn append(&mut self, entry: &[u8]) {
        self.leaves.push(leaf(entry));
    }

    /// How many entries it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    /// Whether nothing has been written down.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    /// The root over everything written down so far.
    ///
    /// **An empty tree has a root too** — the hash of nothing — and a node publishes one every
    /// epoch whether or not anything happened. Without that, a gap says nothing about whether
    /// there was nothing to say or nobody there to say it, and a gap that means both means
    /// neither.
    #[must_use]
    pub fn root(&self) -> Digest {
        root_of(&self.leaves)
    }

    /// The root of this tree as it was when it held `size` entries.
    ///
    /// A tree only ever grows, so the tree it had is the first `size` of the tree it has.
    ///
    /// [`None`] for a size this tree never reached — a root nobody signed, and answering would be
    /// inventing one.
    #[must_use]
    pub fn root_at(&self, size: usize) -> Option<Digest> {
        if size > self.leaves.len() {
            return None;
        }
        Some(root_of(&self.leaves[..size]))
    }

    /// The path that carries the entry at `index` up to the root, or nothing if it is not here.
    #[must_use]
    pub fn inclusion(&self, index: usize) -> Option<Path> {
        self.inclusion_at(index, self.leaves.len())
    }

    /// The same, against the tree as it was when it held `size` entries.
    ///
    /// **This is what makes a proof checkable by whoever receives it.** A path is only a proof
    /// against a root, a root is only worth anything signed, and the only roots a node signs are
    /// the ones it publishes at the end of an epoch — so a proof against the tree as it stands now
    /// is a proof against a root nobody ever put their name to.
    ///
    /// A tree only ever grows, so the tree it had is the first `size` of the tree it has.
    #[must_use]
    pub fn inclusion_at(&self, index: usize, size: usize) -> Option<Path> {
        if index >= size || size > self.leaves.len() {
            return None;
        }
        let mut path = Vec::new();
        climb(&self.leaves[..size], index, &mut path);
        Some(Path(path))
    }
}

/// Whether `entry` really sat at `index` of a tree of `size` entries whose root was `root`.
///
/// This is what an auditor runs, and it is the reason a node cannot quietly drop something it
/// already published: whoever holds the entry and the path can show the root everybody else has.
#[must_use]
pub fn included(entry: &[u8], index: usize, size: usize, path: &Path, root: &Digest) -> bool {
    if index >= size {
        return false;
    }

    // RFC 6962's own verification, which climbs from the leaf. An earlier attempt here walked
    // down from the root instead, which reads the path in the opposite order to the one it was
    // built in — and passed a tree of two, because at that size the two orders coincide.
    let mut at = index;
    let mut last = size - 1;
    let mut hash = leaf(entry);

    for step in path.hashes() {
        if last == 0 {
            return false;
        }
        if !at.is_multiple_of(2) || at == last {
            hash = branch(step, &hash);
            while at != 0 && at.is_multiple_of(2) {
                at >>= 1;
                last >>= 1;
            }
        } else {
            hash = branch(&hash, step);
        }
        at >>= 1;
        last >>= 1;
    }

    last == 0 && hash == *root
}

/// The hash of one entry, as a leaf.
fn leaf(entry: &[u8]) -> Digest {
    let mut input = Vec::with_capacity(1 + entry.len());
    input.push(LEAF);
    input.extend_from_slice(entry);
    Digest::of(&input)
}

/// The hash of two branches, in order.
fn branch(left: &Digest, right: &Digest) -> Digest {
    let mut input = Vec::with_capacity(1 + WIDTH * 2);
    input.push(BRANCH);
    input.extend_from_slice(left.bytes());
    input.extend_from_slice(right.bytes());
    Digest::of(&input)
}

/// Where a run of `count` leaves splits: the largest power of two below it.
fn split(count: usize) -> usize {
    let mut half = 1;
    while half * 2 < count {
        half *= 2;
    }
    half
}

/// The root over a run of leaves.
fn root_of(leaves: &[Digest]) -> Digest {
    match leaves {
        [] => Digest::of(&[]),
        [only] => *only,
        _ => {
            let half = split(leaves.len());
            branch(&root_of(&leaves[..half]), &root_of(&leaves[half..]))
        }
    }
}

/// The sibling hashes between one leaf and the root, collected from the bottom up.
fn climb(leaves: &[Digest], index: usize, path: &mut Vec<Digest>) {
    if leaves.len() <= 1 {
        return;
    }
    let half = split(leaves.len());
    if index < half {
        climb(&leaves[..half], index, path);
        path.push(root_of(&leaves[half..]));
    } else {
        climb(&leaves[half..], index - half, path);
        path.push(root_of(&leaves[..half]));
    }
}

#[cfg(test)]
mod tests {
    use super::{Tree, included};
    use almena_suite::digest::Digest;

    fn of(entries: &[&[u8]]) -> Tree {
        let mut tree = Tree::new();
        for entry in entries {
            tree.append(entry);
        }
        tree
    }

    #[test]
    fn an_empty_tree_still_has_a_root() {
        // And a node publishes one every epoch whether anything happened or not: a gap that could
        // mean either *nothing happened* or *nobody was there* means neither.
        assert_eq!(Tree::new().root(), Digest::of(&[]));
        assert!(Tree::new().is_empty());
    }

    #[test]
    fn one_entry_hashes_as_a_leaf_and_nothing_else() {
        let tree = of(&[b"an entry"]);
        assert_eq!(tree.root(), Digest::of(b"\x00an entry"));
    }

    #[test]
    fn two_entries_hash_as_a_pair_of_leaves() {
        let left = Digest::of(b"\x00one");
        let right = Digest::of(b"\x00two");
        let mut expected = vec![0x01];
        expected.extend_from_slice(left.bytes());
        expected.extend_from_slice(right.bytes());

        assert_eq!(of(&[b"one", b"two"]).root(), Digest::of(&expected));
    }

    #[test]
    fn a_leaf_can_never_be_read_as_a_pair_of_branches() {
        // The whole reason for the two prefixes. Without them somebody could choose an entry whose
        // bytes happen to be two hashes, and one tree would have two shapes with one root — which
        // is how a second history gets shown to somebody checking the root everybody else has.
        let pair = of(&[b"one", b"two"]);
        let mut forged = vec![0x01];
        forged.extend_from_slice(Digest::of(b"\x00one").bytes());
        forged.extend_from_slice(Digest::of(b"\x00two").bytes());

        assert_ne!(of(&[&forged]).root(), pair.root());
    }

    #[test]
    fn a_proof_can_be_made_against_the_tree_as_it_was() {
        // What makes a proof checkable by whoever receives it: the only roots a node signs are the
        // ones it publishes at the end of an epoch, so a proof has to be against one of those and
        // not against whatever the tree happens to be now.
        let mut tree = Tree::new();
        for entry in [b"one".as_slice(), b"two", b"three", b"four"] {
            tree.append(entry);
        }

        let mut then = Tree::new();
        for entry in [b"one".as_slice(), b"two"] {
            then.append(entry);
        }

        for at in 0..2 {
            let path = tree.inclusion_at(at, 2).expect("a path");
            assert_eq!(path, then.inclusion(at).expect("the same path"));
            assert!(
                included(
                    match at {
                        0 => b"one".as_slice(),
                        _ => b"two",
                    },
                    at,
                    2,
                    &path,
                    &then.root()
                ),
                "and it checks out against the root of the tree it was taken against"
            );
        }
    }

    #[test]
    fn nothing_can_be_proved_against_a_tree_that_did_not_hold_it_yet() {
        let mut tree = Tree::new();
        for entry in [b"one".as_slice(), b"two", b"three"] {
            tree.append(entry);
        }
        assert!(tree.inclusion_at(2, 2).is_none(), "it was not there yet");
    }

    #[test]
    fn nothing_can_be_proved_against_a_tree_bigger_than_this_one() {
        // A size nobody reached is a root nobody signed, and answering would be inventing one.
        let mut tree = Tree::new();
        tree.append(b"one");
        assert!(tree.inclusion_at(0, 9).is_none());
    }

    #[test]
    fn every_entry_can_prove_it_was_there() {
        // The property an auditor depends on, at every size where the shape of the tree changes:
        // powers of two, one past them, and one short.
        for size in [1usize, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 64, 100] {
            let entries: Vec<Vec<u8>> = (0..size).map(|n| format!("entry {n}").into()).collect();
            let mut tree = Tree::new();
            for entry in &entries {
                tree.append(entry);
            }
            let root = tree.root();

            for (index, entry) in entries.iter().enumerate() {
                let path = tree.inclusion(index).expect("it is in there");
                assert!(
                    included(entry, index, size, &path, &root),
                    "size {size}, index {index}"
                );
            }
        }
    }

    #[test]
    fn a_proof_does_not_carry_an_entry_that_was_never_there() {
        let tree = of(&[b"one", b"two", b"three", b"four"]);
        let root = tree.root();
        let path = tree.inclusion(2).expect("it is in there");

        assert!(included(b"three", 2, 4, &path, &root));
        assert!(!included(b"forged", 2, 4, &path, &root), "another entry");
        assert!(!included(b"three", 1, 4, &path, &root), "another position");
        assert!(!included(b"three", 2, 5, &path, &root), "another size");
    }

    #[test]
    fn a_path_from_one_tree_does_not_open_another() {
        let mine = of(&[b"one", b"two", b"three"]);
        let theirs = of(&[b"one", b"two", b"other"]);
        let path = mine.inclusion(0).expect("it is in there");

        assert!(included(b"one", 0, 3, &path, &mine.root()));
        assert!(!included(b"one", 0, 3, &path, &theirs.root()));
    }

    #[test]
    fn appending_moves_nothing_that_was_already_there() {
        // Append-only means the entry that was third is still third, with the same bytes under it.
        let mut tree = of(&[b"one", b"two", b"three"]);
        let before = tree.inclusion(1).expect("it is in there");
        tree.append(b"four");
        let after = tree.inclusion(1).expect("still in there");

        assert!(included(
            b"two",
            1,
            3,
            &before,
            &of(&[b"one", b"two", b"three"]).root()
        ));
        assert!(included(b"two", 1, 4, &after, &tree.root()));
    }

    #[test]
    fn nothing_can_prove_an_index_that_is_not_there() {
        assert!(of(&[b"one", b"two"]).inclusion(2).is_none());
        assert!(Tree::new().inclusion(0).is_none());
    }
}
