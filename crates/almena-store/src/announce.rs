//! The act a node exists by, and the one that gives it its name.
//!
//! A node is a directory with a key in it, but a key is not a name: everything here is called by
//! the hash of the act that created it, and until a node performs one it is a machine answering on
//! a port rather than somebody the record knows.
//!
//! **So a node's first announcement is its creation.** Its hash is the node's identifier, the same
//! way every other object gets one, and the key inside it is what anything the node signs is
//! checked against.
//!
//! Announcing is meant to happen again — what a node offers and what version it runs change over
//! its life, and none of that may rename it, which is why only the first one names anything.
//! **Nothing applies a second one yet**, so today a node's chain is its creation and no more.
//!
//! # Why the census comes from here and not from the zone
//!
//! Whoever can answer for a zone can put anything in it, so a directory of nodes kept there would
//! be believed on the authority of whoever holds the domain. An announcement is an entry in the
//! record every node already has, signed by the key it names, and it does not need to be trusted:
//! it can be checked. The zone says where to call first; this says who anybody turned out to be.
//!
//! # Self-signed, and that is not a weakness
//!
//! Nothing earlier can vouch for a node — it is new, and the act that introduces it is the first
//! thing it ever says. What the signature establishes is not that the node is trustworthy but that
//! *this identifier and this key belong to each other*, which is the whole of what a reader needs
//! before it can tell one node's word from another's. Being worth listening to is earned
//! afterwards, by being bound and by being measured.

use std::collections::{BTreeMap, BTreeSet};

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name};
use almena_format::operation::{Operation, Signed, create};
use almena_suite::ed25519;
use almena_time::Epoch;

use crate::capability::Capability;
use crate::genesis::Which;
use crate::kind::Kind;

/// Where an announcement carries the key that is this node.
///
/// Odd, and there is nothing to weigh: an announcement a reader could not get the key out of would
/// introduce a node without saying how to recognise anything it says — which is the one thing the
/// act is for.
const KEY: u64 = 1;

/// A node, and the name it will be known by.
#[derive(Debug, Clone)]
pub struct Announced {
    /// The act to be admitted, which is what names the node.
    pub operation: Operation,
    /// What the node is called from now on.
    pub node: Did,
}

/// Say again what a node is running, on the chain its first announcement opened.
///
/// **This never renames anything.** What a node offers and what version it speaks change over its
/// life and its name must not, which is why they are here and not in the act that named it.
#[must_use]
pub fn offering(
    node: &Did,
    head: &Name,
    offers: &BTreeSet<Capability>,
    what: Speaking<'_>,
) -> Operation {
    let Speaking {
        version,
        reachable,
        issued,
        key,
    } = what;
    let listed = offers.iter().map(|one| Value::Uint(one.number())).collect();
    let where_ = reachable
        .iter()
        .map(|address| Value::Text(address.clone()))
        .collect();
    let mut operation = Operation {
        object: node.clone(),
        previous: Some(head.clone()),
        kind: Kind::NODE_ANNOUNCE.number(),
        version: 1,
        issued,
        payload: BTreeMap::from([
            (crate::capability::OFFERS, Value::Array(listed)),
            (crate::capability::SPEAKS, Value::Uint(version)),
            (crate::capability::WHERE, Value::Array(where_)),
        ]),
        signatures: Vec::new(),
    };
    let signature = key.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: node.clone(),
        key: key.verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });
    operation
}

/// What a node says about itself when it announces again.
#[derive(Clone, Copy)]
pub struct Speaking<'a> {
    /// Which version of the protocol it speaks.
    pub version: u64,
    /// Where it says it can be reached.
    pub reachable: &'a BTreeSet<String>,
    /// When it is saying so.
    pub issued: Epoch,
    /// The key that is this node, which is the only one that may say it.
    pub key: &'a ed25519::SigningKey,
}

/// Introduce a node to a network, naming it.
///
/// What it can do and what version it runs are not here. They change while a node's name does not,
/// so they belong to the announcements that follow this one rather than to the one act whose bytes
/// the name is taken from — a node that switched off a capability would otherwise become a
/// different node. Nothing reads them yet, and standing in for them now would put a guess inside
/// the bytes a name is taken from, where it could never be corrected.
#[must_use]
pub fn announce(which: Which, epoch: Epoch, key: &ed25519::SigningKey) -> Announced {
    let payload = BTreeMap::from([(KEY, Value::Bytes(key.verifying_key().bytes().to_vec()))]);

    let mut operation = create(
        which.marking(),
        Kind::NODE_ANNOUNCE.number(),
        1,
        epoch,
        payload,
    );

    let signature = key.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: key.verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });

    let node = operation.object.clone();
    Announced { operation, node }
}

/// The act that closes a node, signed by the node's own key.
///
/// **A node closes; it does not rotate** (`SPECS.md §4.1`). What a rotation preserves is an identity
/// with something behind it — credentials in that name, a seal, open relationships — and a node has
/// none of it: its name is in its own roots and in the census the share-out is drawn from, and the
/// roots it signed stay where they are, true and signed by the key that signed them. A new node
/// starts with no history and has lost none.
///
/// And it could not rotate the way an organisation does: the only thing that governs a node is its
/// own key, so a rotation would have to be signed by the key that was lost.
///
/// **Somebody holding the stolen key can write this, and that is the right way round.** Closing
/// denies and concedes nothing, so the worst they achieve is that the node stops counting — which
/// is what its operator was about to do. It is `SPECS.md §1.8` again.
#[must_use]
pub fn close(node: &Did, head: &Name, at: Epoch, by: &ed25519::SigningKey) -> Operation {
    let mut operation = Operation {
        object: node.clone(),
        previous: Some(head.clone()),
        kind: Kind::NODE_CLOSE.number(),
        version: 1,
        issued: at,
        payload: BTreeMap::new(),
        signatures: Vec::new(),
    };
    let signature = by.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: node.clone(),
        key: by.verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });
    operation
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{KEY, Speaking, announce, offering};
    use crate::chain::{Answer, Objects, State};
    use crate::genesis::Which;
    use almena_format::cbor::Value;
    use almena_suite::ed25519;
    use almena_time::Epoch;

    fn key(seed: u8) -> ed25519::SigningKey {
        ed25519::SigningKey::from_secret([seed; 32])
    }

    #[test]
    fn a_node_is_named_by_the_act_that_introduces_it() {
        let announced = announce(Which::Development, Epoch::GENESIS, &key(1));
        assert!(announced.operation.names_itself());
        assert_eq!(&announced.node, &announced.operation.object);
    }

    #[test]
    fn two_nodes_are_two_names() {
        // The reason the whole thing exists: a root that named the same object whoever published
        // it would make every honest pair of nodes look like it was contradicting the other.
        let one = announce(Which::Development, Epoch::GENESIS, &key(1));
        let other = announce(Which::Development, Epoch::GENESIS, &key(2));
        assert_ne!(one.node, other.node);
    }

    #[test]
    fn the_same_key_announcing_twice_is_the_same_node() {
        let once = announce(Which::Development, Epoch::GENESIS, &key(1));
        let again = announce(Which::Development, Epoch::GENESIS, &key(1));
        assert_eq!(once.node, again.node);
    }

    #[test]
    fn it_carries_the_key_it_is_checked_against() {
        let announced = announce(Which::Development, Epoch::GENESIS, &key(7));
        assert_eq!(
            announced.operation.payload.get(&KEY),
            Some(&Value::Bytes(key(7).verifying_key().bytes().to_vec()))
        );
    }

    #[test]
    fn it_is_admitted_and_leaves_the_node_resolvable() {
        let mut objects = Objects::new();
        let announced = announce(Which::Development, Epoch::GENESIS, &key(3));

        objects
            .admit(&announced.operation, Epoch::GENESIS)
            .expect("a node introducing itself");

        assert_eq!(
            objects.resolve(announced.node.name()),
            Answer::Here(State::Node {
                key: key(3).verifying_key().bytes(),
                offers: BTreeSet::new(),
                speaks: 0,
                claimed_by: None,
                reachable: BTreeSet::new(),
                closed: None,
            }),
            "and what it resolves to is the key to check its word against"
        );
    }

    #[test]
    fn a_node_closes_and_stops_being_counted_without_anything_it_said_being_taken_back() {
        // **The one way out of a node whose key is somebody else's** (`SPECS.md §4.1`). A node does
        // not rotate: what a rotation preserves is an identity with something behind it, and a node
        // has none — its roots stay where they are, signed by the key that signed them, and a new
        // node starts with no history and has lost none.
        let mut objects = crate::chain::Objects::new();
        let its_key = key(3);
        let announced = announce(crate::genesis::Which::Development, Epoch::GENESIS, &its_key);
        objects
            .admit(&announced.operation, Epoch::GENESIS)
            .expect("taken");
        let at = Epoch::new(100);
        assert_eq!(objects.nodes_at(at).count(), 1, "it counts while it is up");

        let head = objects.head(announced.node.name()).expect("a head").clone();
        let shut = super::close(&announced.node, &head, at, &its_key);
        objects.admit(&shut, at).expect("a node may close itself");

        // Out of the census from the epoch it said, and not before it: a share-out drawn for an
        // earlier epoch has to be the same share-out afterwards, or the past would move.
        assert_eq!(objects.nodes_at(at).count(), 0);
        assert_eq!(
            objects.nodes_at(Epoch::new(99)).count(),
            1,
            "and it was still counting the epoch before"
        );
        assert_eq!(
            objects.nodes().count(),
            1,
            "everything it said stays in the record — closing is a state and never a deletion"
        );

        // Announcing again does not bring it back: coming back means a new node, with a new key
        // and a new name. One that returned would bring whoever took its key with it.
        let head = objects.head(announced.node.name()).expect("a head").clone();
        let again = offering(
            &announced.node,
            &head,
            &BTreeSet::from([crate::capability::Capability::Interface]),
            Speaking {
                version: 1,
                reachable: &BTreeSet::new(),
                issued: at,
                key: &its_key,
            },
        );
        objects.admit(&again, at).expect("taken");
        assert_eq!(objects.nodes_at(at).count(), 0, "still closed");
    }

    #[test]
    fn an_announcement_signed_by_somebody_else_is_refused() {
        // Otherwise anybody could introduce a node carrying a key they do not hold, and every
        // signature checked against that key afterwards would be checked against a stranger's.
        let mut objects = Objects::new();
        let mut announced = announce(Which::Development, Epoch::GENESIS, &key(3));

        let impostor = key(4);
        announced.operation.signatures[0].key = impostor.verifying_key().bytes().to_vec();
        announced.operation.signatures[0].signature =
            impostor.sign(&announced.operation.signing_bytes()).bytes();

        assert!(objects.admit(&announced.operation, Epoch::GENESIS).is_err());
    }
}
