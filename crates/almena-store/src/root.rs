//! What a node says about its own tree at the close of an epoch, and how it says it.
//!
//! A root is **not a log entry**, and that is arithmetic rather than taste: one root per node per
//! epoch, with a thousand nodes, is twenty-four thousand entries a day of pure bookkeeping in
//! something everybody stores and nothing ever deletes. So roots are handled the way status lists
//! are — **signed artefacts that get published and served**, asked for by hash like everything
//! else, and travelling with the countersignatures of whoever has seen them.
//!
//! What does enter the log is the **contradiction**, if one ever appears: two roots from the same
//! node for the same epoch, each signed by it. That proof needs nothing else to be believed.
//!
//! # What is signed, and why each part of it
//!
//! | | Why it is inside the signature |
//! |---|---|
//! | The network | A root published on the development network must not be replayable as one from production. The two say the same thing about everything else, and confusing them is the accident that costs the most |
//! | The node | So that *whose root is this* is not a question the reader answers by guessing from the key |
//! | The epoch | Because *two incompatible roots for the same epoch* is the whole of what cross-signing detects, and it cannot detect it if the epoch is outside the signature |
//! | How many entries | An inclusion proof is checked against a size. A size somebody could change is a proof somebody could bend |
//! | The root itself | The point |
//!
//! # A root is published every epoch, empty or not
//!
//! If a node only published when something happened, a gap would mean either *nothing happened*
//! or *I was not there* — and a gap that means both means neither. The tree of no entries has a
//! root like any other.

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name};
use almena_suite::digest::Digest;
use almena_suite::ed25519;
use almena_time::Epoch;
use std::collections::BTreeMap;

/// Where each part of a root sits in the map it is signed as.
mod key {
    /// The network this root belongs to, which is the hash of its genesis.
    pub const NETWORK: u64 = 1;
    /// The node that published it.
    pub const NODE: u64 = 2;
    /// Which epoch it closes.
    pub const EPOCH: u64 = 3;
    /// How many entries the tree held.
    pub const SIZE: u64 = 4;
    /// The root over them.
    pub const ROOT: u64 = 5;
}

/// What one node says its tree looked like at the close of one epoch.
///
/// Its shape is fixed, so the criticality parity that governs an operation's payload does not
/// apply here: there is nothing optional in it, and a reader that did not understand a part of it
/// would not be reading a root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    /// The network, which is the hash of the operation that opened it.
    pub network: Name,
    /// The node saying it.
    pub node: Did,
    /// The epoch this closes.
    pub epoch: Epoch,
    /// How many entries the tree held.
    pub size: u64,
    /// The root over them.
    pub root: Digest,
}

/// A root as it travels: what the node said, and its signature over it.
///
/// Countersignatures from other nodes ride along with it and are not part of what the node signed
/// — a witness saw a root; it did not help make one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    /// What was said.
    pub root: Root,
    /// The key that said it, which is the node's.
    pub key: [u8; ed25519::PUBLIC_KEY_WIDTH],
    /// Its signature over [`Root::to_bytes`].
    pub signature: [u8; 64],
    /// Who else has signed the same bytes as having seen them.
    ///
    /// They ride along and are **not part of what the node signed**: a node cannot sign a list that
    /// grows after it signed. Each one stands on its own.
    pub witnesses: Vec<Witness>,
}

/// Why a published root was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejected {
    /// It belongs to another network. Nothing about it is worth reading here.
    AnotherNetwork,
    /// The key that signed it is not the one it names as the node's.
    NotTheNode,
    /// The signature does not check out.
    SignatureDoesNotCheck,
}

/// Two roots from one node for one epoch that are not the same root.
///
/// This is the whole of what cross-signing detects, and it is deliberately narrow. Two *different*
/// nodes with different roots prove nothing at all: they have different trees by design, and
/// asking them to agree would be asking for the consensus this design does without.
#[must_use]
pub fn contradict(one: &Root, other: &Root) -> bool {
    one.network == other.network
        && one.node == other.node
        && one.epoch == other.epoch
        && (one.root != other.root || one.size != other.size)
}

impl Root {
    /// The canonical bytes a node signs, and a witness countersigns.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        Value::Map(BTreeMap::from([
            (key::NETWORK, Value::Text(self.network.as_str().to_owned())),
            (key::NODE, Value::Text(self.node.to_string())),
            (key::EPOCH, Value::Uint(self.epoch.number())),
            (key::SIZE, Value::Uint(self.size)),
            (key::ROOT, Value::Bytes(self.root.bytes().to_vec())),
        ]))
        .to_bytes()
    }

    /// A root read back from the bytes it was signed as.
    ///
    /// The mirror of [`Root::to_bytes`], and it has to exist because a node reads its own roots
    /// back when it comes up: what a root said about an epoch cannot be worked out again from a
    /// record that has moved on since.
    ///
    /// Every field is required. A root missing one of them is not a root a signature could have
    /// been made over, so there is nothing to be lenient about.
    #[must_use]
    pub fn read(bytes: &[u8]) -> Option<Self> {
        let Value::Map(fields) = almena_format::cbor::read(bytes).ok()? else {
            return None;
        };
        let (Some(Value::Text(network)), Some(Value::Text(node))) =
            (fields.get(&key::NETWORK), fields.get(&key::NODE))
        else {
            return None;
        };
        let (Some(&Value::Uint(epoch)), Some(&Value::Uint(size))) =
            (fields.get(&key::EPOCH), fields.get(&key::SIZE))
        else {
            return None;
        };
        let Some(Value::Bytes(root)) = fields.get(&key::ROOT) else {
            return None;
        };

        Some(Self {
            network: Name::parse(network).ok()?,
            node: Did::parse(node).ok()?,
            epoch: Epoch::new(epoch),
            size,
            root: Digest::from_bytes(root.as_slice().try_into().ok()?),
        })
    }

    /// What this root is called, so that it can be asked for like anything else.
    #[must_use]
    pub fn name(&self) -> Name {
        Name::of(&self.to_bytes())
    }

    /// Sign it as somebody who has seen it.
    ///
    /// The same bytes the node itself signed, because a witness saying it saw something else would
    /// be a witness to nothing.
    #[must_use]
    pub fn countersign(&self, witness: &ed25519::SigningKey) -> Witness {
        Witness {
            key: witness.verifying_key().bytes(),
            signature: witness.sign(&self.to_bytes()).bytes(),
        }
    }

    /// Sign it as the node it names.
    #[must_use]
    pub fn publish(&self, node: &ed25519::SigningKey) -> Published {
        Published {
            root: self.clone(),
            key: node.verifying_key().bytes(),
            signature: node.sign(&self.to_bytes()).bytes(),
            witnesses: Vec::new(),
        }
    }
}

/// Where each part of a published root sits when it travels.
///
/// Its own numbering, separate from the root's, because these are two different things: one is
/// what a node said, and this is that plus who said it and their signature over it.
mod carried {
    /// The root's own bytes, which are what was signed.
    pub const ROOT: u64 = 1;
    /// The key that signed it.
    pub const KEY: u64 = 3;
    /// The signature over the root's bytes.
    pub const SIGNATURE: u64 = 5;
    /// Who else has signed it as having seen it.
    ///
    /// **Even, and that is the whole design of it.** A reader that skipped these still holds a
    /// root its author signed; what it loses is other people's word that they saw the same one,
    /// which is evidence to be counted rather than something to be understood. A node that counted
    /// nought where a newer one counts three is cautious, and being cautious is safe.
    pub const WITNESSES: u64 = 2;
}

/// Somebody else's word that they saw a root.
///
/// **Not agreement.** A witness says *this is what that node showed me*, and nothing about whether
/// what it showed is right — nobody can check another node's tree without holding it. What it buys
/// is that a node cannot quietly show one root to one person and another to somebody else: the two
/// carry different witnesses, and the pair is the proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Witness {
    /// The key that saw it.
    pub key: [u8; ed25519::PUBLIC_KEY_WIDTH],
    /// Its signature over the same bytes the node itself signed.
    pub signature: [u8; 64],
}

impl Witness {
    /// Whether this really is that key's word about that root.
    #[must_use]
    pub fn checks(&self, root: &Root) -> bool {
        ed25519::VerifyingKey::from_bytes(self.key).is_ok_and(|verifying| {
            verifying
                .verify(
                    &root.to_bytes(),
                    &ed25519::Signature::from_bytes(self.signature),
                )
                .is_ok()
        })
    }
}

impl Published {
    /// The bytes of a published root, for putting one on a wire.
    ///
    /// The root travels as **its own signed bytes** rather than as fields to be reassembled: a
    /// signature is over exactly those bytes, and a reader that rebuilt them from parts would be
    /// checking a signature against something it made up itself.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let seen = self
            .witnesses
            .iter()
            .map(|witness| {
                Value::Array(vec![
                    Value::Bytes(witness.key.to_vec()),
                    Value::Bytes(witness.signature.to_vec()),
                ])
            })
            .collect();

        Value::Map(BTreeMap::from([
            (carried::ROOT, Value::Bytes(self.root.to_bytes())),
            (carried::KEY, Value::Bytes(self.key.to_vec())),
            (carried::SIGNATURE, Value::Bytes(self.signature.to_vec())),
            (carried::WITNESSES, Value::Array(seen)),
        ]))
        .to_bytes()
    }

    /// A published root read back off a wire.
    ///
    /// Nothing here is checked beyond being the right shape. Whether the signature is any good is
    /// [`Published::accept`], which needs to be told whose key to expect — and being told is the
    /// point, because a root that vouched for itself would vouch for anybody.
    #[must_use]
    pub fn read(bytes: &[u8]) -> Option<Self> {
        let Value::Map(fields) = almena_format::cbor::read(bytes).ok()? else {
            return None;
        };
        let (Some(Value::Bytes(root)), Some(Value::Bytes(key)), Some(Value::Bytes(signature))) = (
            fields.get(&carried::ROOT),
            fields.get(&carried::KEY),
            fields.get(&carried::SIGNATURE),
        ) else {
            return None;
        };

        let mut witnesses = Vec::new();
        if let Some(Value::Array(seen)) = fields.get(&carried::WITNESSES) {
            for one in seen {
                // A witness that cannot be read is left out rather than spoiling the root: they are
                // separate pieces of evidence and one being unreadable says nothing about the rest.
                if let Value::Array(pair) = one
                    && let [Value::Bytes(key), Value::Bytes(signature)] = pair.as_slice()
                    && let (Ok(key), Ok(signature)) =
                        (key.as_slice().try_into(), signature.as_slice().try_into())
                {
                    witnesses.push(Witness { key, signature });
                }
            }
        }

        Some(Self {
            root: Root::read(root)?,
            key: key.as_slice().try_into().ok()?,
            signature: signature.as_slice().try_into().ok()?,
            witnesses,
        })
    }

    /// Whether this root is worth keeping: it is this network's, and the node really said it.
    ///
    /// # Errors
    ///
    /// [`Rejected`], naming which of the three it failed.
    pub fn accept(&self, network: &Name, node_key: &[u8]) -> Result<(), Rejected> {
        if self.root.network != *network {
            return Err(Rejected::AnotherNetwork);
        }
        if self.key.as_slice() != node_key {
            return Err(Rejected::NotTheNode);
        }
        let verifying =
            ed25519::VerifyingKey::from_bytes(self.key).map_err(|_| Rejected::NotTheNode)?;
        verifying
            .verify(
                &self.root.to_bytes(),
                &ed25519::Signature::from_bytes(self.signature),
            )
            .map_err(|_| Rejected::SignatureDoesNotCheck)
    }
}

/// The roots one node has published, one per epoch.
#[derive(Debug, Clone, Default)]
pub struct Roots {
    published: BTreeMap<u64, Root>,
    /// Who has said they saw each of them.
    ///
    /// Kept beside the roots rather than inside them, because a root is fixed the moment it is
    /// signed and this list is not: it grows for as long as anybody is still looking.
    seen: BTreeMap<u64, Vec<Witness>>,
}

impl Roots {
    /// A node that has published nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record what this node is saying about an epoch.
    ///
    /// # Errors
    ///
    /// The root already published for that epoch, when it is not this one. Saying two different
    /// things about one epoch is the misconduct the whole mechanism exists to catch, so a node
    /// refuses to do it to itself rather than discovering later that somebody else noticed.
    pub fn publish(&mut self, root: Root) -> Result<(), Root> {
        match self.published.get(&root.epoch.number()) {
            Some(already) if *already != root => Err(already.clone()),
            Some(_) => Ok(()),
            None => {
                self.published.insert(root.epoch.number(), root);
                Ok(())
            }
        }
    }

    /// Take in somebody's word that they saw one of these roots.
    ///
    /// Returns whether it was kept. A witness for an epoch this node never closed, or one whose
    /// signature is not over the root it names, is dropped — and proves nothing about whoever sent
    /// it either, since anybody can send anybody bytes.
    ///
    /// The same witness twice is not two witnesses. It arrives twice all the time.
    pub fn saw(&mut self, epoch: Epoch, witness: Witness) -> bool {
        let Some(root) = self.published.get(&epoch.number()) else {
            return false;
        };
        if !witness.checks(root) {
            return false;
        }

        let seen = self.seen.entry(epoch.number()).or_default();
        if seen.contains(&witness) {
            return true;
        }
        seen.push(witness);
        true
    }

    /// Who has said they saw what this node published for that epoch.
    #[must_use]
    pub fn witnesses(&self, epoch: Epoch) -> &[Witness] {
        self.seen
            .get(&epoch.number())
            .map_or(&[], |seen| seen.as_slice())
    }

    /// The last epoch this node has said anything about.
    ///
    /// [`None`] before it has closed one, which is a node that has just started rather than one
    /// with nothing to say.
    #[must_use]
    pub fn last(&self) -> Option<Epoch> {
        self.published.keys().next_back().copied().map(Epoch::new)
    }

    /// What this node said about an epoch, if it said anything.
    #[must_use]
    pub fn at(&self, epoch: Epoch) -> Option<&Root> {
        self.published.get(&epoch.number())
    }

    /// Every epoch between the first published and `through` that has no root.
    ///
    /// A node publishes one every epoch whether anything happened or not, so anything this returns
    /// is a hole in its own record — which is exactly what somebody measuring its coverage looks
    /// for, and what a node should notice about itself first.
    #[must_use]
    pub fn missing(&self, through: Epoch) -> Vec<u64> {
        let Some(&first) = self.published.keys().next() else {
            return Vec::new();
        };
        (first..=through.number())
            .filter(|epoch| !self.published.contains_key(epoch))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Published, Rejected, Root, Roots, Witness, contradict};
    use almena_format::identifier::{Did, Name, Network};
    use almena_suite::digest::Digest;
    use almena_suite::ed25519;
    use almena_time::{Epoch, Epochs};

    fn node_key(seed: u8) -> ed25519::SigningKey {
        ed25519::SigningKey::from_secret([seed; 32])
    }

    fn network() -> Name {
        Name::of(b"the genesis operation of a development network")
    }

    fn node(seed: u8) -> Did {
        Did::new(Network::Development, Name::of(&[seed; 8]))
    }

    fn root(epoch: u64, size: u64, over: &[u8]) -> Root {
        Root {
            network: network(),
            node: node(1),
            epoch: Epoch::GENESIS.plus(Epochs(epoch)).expect("no overflow"),
            size,
            root: Digest::of(over),
        }
    }

    #[test]
    fn a_node_signs_its_own_root_and_it_checks_out() {
        let key = node_key(3);
        let published = root(10, 4, b"four entries").publish(&key);
        assert_eq!(
            published.accept(&network(), &key.verifying_key().bytes()),
            Ok(())
        );
    }

    #[test]
    fn a_root_from_another_network_is_refused_before_anything_else() {
        // The accident that costs the most: development and production say the same thing about
        // everything except the hash of the genesis that opened them.
        let key = node_key(3);
        let published = root(10, 4, b"four entries").publish(&key);
        let elsewhere = Name::of(b"the genesis operation of another network");

        assert_eq!(
            published.accept(&elsewhere, &key.verifying_key().bytes()),
            Err(Rejected::AnotherNetwork)
        );
    }

    #[test]
    fn a_root_signed_by_somebody_else_is_not_that_nodes_root() {
        let published = root(10, 4, b"four entries").publish(&node_key(3));
        assert_eq!(
            published.accept(&network(), &node_key(9).verifying_key().bytes()),
            Err(Rejected::NotTheNode)
        );
    }

    #[test]
    fn a_signature_that_does_not_check_is_refused() {
        let key = node_key(3);
        let mut published = root(10, 4, b"four entries").publish(&key);
        published.signature[0] ^= 0xff;
        assert_eq!(
            published.accept(&network(), &key.verifying_key().bytes()),
            Err(Rejected::SignatureDoesNotCheck)
        );
    }

    #[test]
    fn the_size_is_inside_the_signature() {
        // An inclusion proof is checked against a size. A size somebody could change on the way is
        // a proof somebody could bend.
        let key = node_key(3);
        let mut published = root(10, 4, b"four entries").publish(&key);
        published.root.size = 5;
        assert_eq!(
            published.accept(&network(), &key.verifying_key().bytes()),
            Err(Rejected::SignatureDoesNotCheck)
        );
    }

    #[test]
    fn two_roots_from_one_node_for_one_epoch_contradict_each_other() {
        assert!(contradict(
            &root(10, 4, b"one history"),
            &root(10, 4, b"another history")
        ));
        assert!(
            contradict(&root(10, 4, b"one history"), &root(10, 5, b"one history")),
            "a different size is a different claim about the same epoch"
        );
    }

    #[test]
    fn saying_the_same_thing_twice_is_not_a_contradiction() {
        assert!(!contradict(
            &root(10, 4, b"one history"),
            &root(10, 4, b"one history")
        ));
    }

    #[test]
    fn a_published_root_survives_the_wire_and_still_checks_out() {
        // The root travels as its own signed bytes. A reader that rebuilt them from fields would
        // be checking a signature against something it had made up itself.
        let key = node_key(3);
        let published = root(10, 4, b"what this node saw").publish(&key);

        let back = Published::read(&published.to_bytes()).expect("readable");
        assert_eq!(back, published);
        assert_eq!(
            back.accept(&network(), &key.verifying_key().bytes()),
            Ok(()),
            "and it is still the same signature over the same bytes"
        );
    }

    #[test]
    fn a_published_root_that_was_altered_on_the_way_does_not_check_out() {
        let key = node_key(3);
        let mut published = root(10, 4, b"what this node saw").publish(&key);
        published.root.size += 1;

        let back = Published::read(&published.to_bytes()).expect("still readable");
        assert_eq!(
            back.accept(&network(), &key.verifying_key().bytes()),
            Err(Rejected::SignatureDoesNotCheck)
        );
    }

    #[test]
    fn bytes_that_are_not_a_published_root_are_not_read_as_one() {
        assert!(Published::read(b"not this").is_none());
        assert!(Published::read(&root(10, 4, b"bare").to_bytes()).is_none());
    }

    #[test]
    fn a_witness_is_that_key_s_word_about_that_root() {
        let saw = root(10, 4, b"what this node saw");
        let seen = saw.countersign(&node_key(8));

        assert!(seen.checks(&saw));
        assert!(
            !seen.checks(&root(10, 5, b"what this node saw")),
            "and says nothing about a root it did not see"
        );
    }

    #[test]
    fn witnesses_ride_along_and_survive_the_wire() {
        let key = node_key(3);
        let mut published = root(10, 4, b"what this node saw").publish(&key);
        published
            .witnesses
            .push(published.root.countersign(&node_key(8)));
        published
            .witnesses
            .push(published.root.countersign(&node_key(9)));

        let back = Published::read(&published.to_bytes()).expect("readable");
        assert_eq!(back.witnesses.len(), 2);
        assert!(back.witnesses.iter().all(|seen| seen.checks(&back.root)));
    }

    #[test]
    fn witnesses_are_not_part_of_what_the_node_signed() {
        // A node cannot sign a list that grows after it signed. Each witness stands on its own, and
        // the root itself still checks out however many of them arrive.
        let key = node_key(3);
        let mut published = root(10, 4, b"what this node saw").publish(&key);
        assert_eq!(
            published.accept(&network(), &key.verifying_key().bytes()),
            Ok(())
        );

        published
            .witnesses
            .push(published.root.countersign(&node_key(8)));
        assert_eq!(
            published.accept(&network(), &key.verifying_key().bytes()),
            Ok(()),
            "still the node's own word about the same bytes"
        );
    }

    #[test]
    fn a_witness_that_is_not_one_is_left_out_and_does_not_spoil_the_root() {
        // They are separate pieces of evidence, and one being unreadable says nothing about the
        // rest or about the root they are attached to.
        let key = node_key(3);
        let mut published = root(10, 4, b"what this node saw").publish(&key);
        published.witnesses.push(Witness {
            key: [0xff; 32],
            signature: [0; 64],
        });

        let back = Published::read(&published.to_bytes()).expect("readable");
        assert_eq!(
            back.accept(&network(), &key.verifying_key().bytes()),
            Ok(()),
            "the root is untouched"
        );
        assert!(
            !back.witnesses.iter().any(|seen| seen.checks(&back.root)),
            "and nothing that is not a witness counts as one"
        );
    }

    #[test]
    fn two_nodes_with_different_roots_prove_nothing() {
        // They have different trees by design. Asking them to agree would be asking for the
        // consensus this whole design does without.
        let mine = root(10, 4, b"what I saw");
        let mut theirs = root(10, 7, b"what they saw");
        theirs.node = node(2);

        assert!(!contradict(&mine, &theirs));
    }

    #[test]
    fn the_same_epoch_on_two_networks_is_not_a_contradiction_either() {
        let here = root(10, 4, b"what I saw");
        let mut there = root(10, 9, b"what I saw elsewhere");
        there.network = Name::of(b"another genesis");

        assert!(!contradict(&here, &there));
    }

    #[test]
    fn a_node_refuses_to_contradict_itself() {
        // It finds out from its own records rather than from somebody else's accusation.
        let mut roots = Roots::new();
        let first = root(10, 4, b"one history");
        assert_eq!(roots.publish(first.clone()), Ok(()));
        assert_eq!(
            roots.publish(first.clone()),
            Ok(()),
            "saying it again is fine"
        );
        assert_eq!(
            roots.publish(root(10, 4, b"another history")),
            Err(first),
            "and saying something else is not"
        );
    }

    #[test]
    fn an_epoch_with_no_root_is_a_hole_the_node_can_see() {
        // A node publishes one every epoch, empty or not, so a gap is not silence: it is a gap.
        let mut roots = Roots::new();
        roots.publish(root(10, 0, b"")).expect("published");
        roots.publish(root(13, 0, b"")).expect("published");

        let through = Epoch::GENESIS.plus(Epochs(14)).expect("no overflow");
        assert_eq!(roots.missing(through), vec![11, 12, 14]);
    }

    #[test]
    fn a_node_that_has_published_nothing_is_missing_nothing() {
        // Never having started is not the same as having stopped, and the two must not look alike.
        let through = Epoch::GENESIS.plus(Epochs(14)).expect("no overflow");
        assert!(Roots::new().missing(through).is_empty());
    }

    #[test]
    fn an_empty_tree_gets_a_root_like_any_other() {
        let key = node_key(3);
        let empty = Root {
            network: network(),
            node: node(1),
            epoch: Epoch::GENESIS,
            size: 0,
            root: crate::tree::Tree::new().root(),
        };
        assert_eq!(
            empty
                .publish(&key)
                .accept(&network(), &key.verifying_key().bytes()),
            Ok(())
        );
    }
}
