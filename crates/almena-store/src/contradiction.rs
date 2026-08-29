//! Two things one node signed that cannot both be true.
//!
//! A node keeps one tree and signs where it stood at the end of each epoch. Saying two different
//! things about **one** epoch is therefore not a disagreement, a race, or a matter of opinion: it
//! is one party stating two incompatible facts over its own name. It is the only thing that can be
//! proved against a node, and this is how the proof gets written down.
//!
//! # It carries its own proof, so nobody has to be believed
//!
//! The act holds both signed roots. Anybody who reads it can check, without asking anybody
//! anything, that one key signed two roots for one epoch of one network that do not match. Whoever
//! published it is not vouching for anything — they are handing over evidence, and the evidence is
//! what convinces.
//!
//! That is why it is worth putting in the record at all: everything else a node might say about
//! another node would only be its word, and would travel no further than people willing to take it.
//!
//! # Everybody who finds it publishes the same object
//!
//! An object is named by the hash of the act that created it, **without the signatures**. Two nodes
//! that find the same pair therefore write the same name, and the second one to try is refused for
//! being a thing that already exists. One contradiction is one object, however many people noticed.
//!
//! # What it does not say
//!
//! Only that this **key** signed both. Whether that key belongs to a node the network has heard of
//! is a different question, answered by resolving that node's own name — and it does not change the
//! evidence, which stands on the two signatures whatever anybody turns out to be.

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::Did;
use almena_format::operation::{Operation, Signed, create};
use almena_suite::ed25519;
use almena_time::Epoch;

use crate::kind::Kind;
use crate::root::Published;

/// Where the first of the two roots sits.
///
/// Odd, and there is nothing to weigh: an act that carried one root would be an accusation with
/// nothing behind it, which is worse than none.
const ONE: u64 = 1;

/// Where the other sits.
const OTHER: u64 = 3;

/// A contradiction written down, and the name it will be known by.
#[derive(Debug, Clone)]
pub struct Written {
    /// The act to be admitted, which is what names the contradiction.
    pub operation: Operation,
    /// What it is called from now on.
    pub named: Did,
}

/// Write down that a node said two incompatible things about one epoch.
///
/// `by` is whoever is publishing it. They vouch for nothing: the evidence is the two signatures
/// inside, and a signature here only says who bothered to write it down.
///
/// The order the two are given in does not matter to whether it is true, but it does change the
/// bytes — so a pair found by two people in different orders is two objects saying the same thing.
/// They are put in the order their own bytes sort in, which nobody has to remember and everybody
/// arrives at.
#[must_use]
pub fn publish(one: &Published, other: &Published, at: Epoch, by: &ed25519::SigningKey) -> Written {
    let (first, second) = {
        let (a, b) = (one.to_bytes(), other.to_bytes());
        if a <= b { (a, b) } else { (b, a) }
    };

    let payload = BTreeMap::from([(ONE, Value::Bytes(first)), (OTHER, Value::Bytes(second))]);
    let network = one.root.node.network();
    let mut operation = create(
        network,
        Kind::CONTRADICTION_PUBLISH.number(),
        1,
        at,
        payload,
    );

    let signature = by.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: by.verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });

    let named = operation.object.clone();
    Written { operation, named }
}

/// Who a contradiction is against, if it really is one.
///
/// [`None`] for anything that is not: an act that carries one root, two roots by different keys,
/// roots for different epochs or networks, or two that are simply the same. **None of those is a
/// contradiction**, and treating them as one would let anybody write an accusation against anybody.
#[must_use]
pub fn against(operation: &Operation) -> Option<[u8; ed25519::PUBLIC_KEY_WIDTH]> {
    if Kind::new(operation.kind) != Some(Kind::CONTRADICTION_PUBLISH) {
        return None;
    }
    let (Some(Value::Bytes(one)), Some(Value::Bytes(other))) =
        (operation.payload.get(&ONE), operation.payload.get(&OTHER))
    else {
        return None;
    };

    let (one, other) = (Published::read(one)?, Published::read(other)?);
    if one.key != other.key {
        return None;
    }
    // Each really is that key's word. Whether the key belongs to a node anybody has heard of is a
    // different question and does not change what these two signatures say.
    let network = one.root.network.clone();
    one.accept(&network, &one.key).ok()?;
    other.accept(&network, &other.key).ok()?;

    crate::root::contradict(&one.root, &other.root).then_some(one.key)
}

/// Which node a contradiction is against, if it really is one.
///
/// **Out of the act and nothing else.** Two roots are only a contradiction when they name the same
/// node, so there is exactly one answer and it is in the bytes — no census, no resolving, and no
/// dependence on what the node reading it happens to know. An index that turned on that would put
/// the same act under two different names on two honest nodes.
#[must_use]
pub fn against_whom(operation: &Operation) -> Option<Did> {
    against(operation)?;
    let Some(Value::Bytes(one)) = operation.payload.get(&ONE) else {
        return None;
    };
    Some(Published::read(one)?.root.node)
}

#[cfg(test)]
mod tests {
    use super::{against, publish};
    use crate::root::Root;
    use almena_format::identifier::{Did, Name, Network};
    use almena_suite::digest::Digest;
    use almena_suite::ed25519;
    use almena_time::Epoch;

    fn key(seed: u8) -> ed25519::SigningKey {
        ed25519::SigningKey::from_secret([seed; 32])
    }

    fn network() -> Name {
        Name::of(b"one network")
    }

    /// A root that node would have signed for that epoch over that tree.
    fn root(seed: u8, epoch: u64, over: &[u8]) -> crate::root::Published {
        Root {
            network: network(),
            node: Did::new(Network::Development, Name::of(&[seed])),
            epoch: Epoch::new(epoch),
            size: 4,
            root: Digest::of(over),
        }
        .publish(&key(seed))
    }

    #[test]
    fn two_things_one_key_said_about_one_epoch_are_a_contradiction() {
        let published = publish(
            &root(3, 7, b"one history"),
            &root(3, 7, b"another history"),
            Epoch::GENESIS,
            &key(9),
        );

        assert_eq!(
            against(&published.operation),
            Some(key(3).verifying_key().bytes()),
            "and it says whose key said them"
        );
    }

    #[test]
    fn whoever_writes_it_down_vouches_for_nothing() {
        // The evidence is the two signatures inside. Who bothered to publish it changes nothing
        // about whether it is true.
        let one = root(3, 7, b"one history");
        let other = root(3, 7, b"another history");

        for publisher in [key(9), key(10), key(3)] {
            let published = publish(&one, &other, Epoch::GENESIS, &publisher);
            assert!(against(&published.operation).is_some());
        }
    }

    #[test]
    fn the_same_pair_found_by_two_people_is_one_object() {
        // An object is named by the hash of its creation without the signatures, so two people who
        // find the same pair write the same name — and the second is refused for already existing.
        let one = root(3, 7, b"one history");
        let other = root(3, 7, b"another history");

        let mine = publish(&one, &other, Epoch::GENESIS, &key(9));
        let theirs = publish(&other, &one, Epoch::GENESIS, &key(10));
        assert_eq!(
            mine.named, theirs.named,
            "including when they found the two in the other order"
        );
    }

    #[test]
    fn the_record_takes_one_and_resolves_to_whose_key_it_is_against() {
        let mut objects = crate::chain::Objects::new();
        let published = publish(
            &root(3, 7, b"one history"),
            &root(3, 7, b"another history"),
            Epoch::GENESIS,
            &key(9),
        );

        objects
            .admit(&published.operation, Epoch::GENESIS)
            .expect("evidence anybody can check");

        assert_eq!(
            objects.resolve(published.named.name()),
            crate::chain::Answer::Here(crate::chain::State::Contradiction {
                against: key(3).verifying_key().bytes()
            })
        );
    }

    #[test]
    fn the_record_refuses_an_accusation_that_is_not_one() {
        // Otherwise anybody could write one against anybody by making up the evidence.
        let mut objects = crate::chain::Objects::new();
        let published = publish(
            &root(3, 7, b"what one saw"),
            &root(4, 7, b"what the other saw"),
            Epoch::GENESIS,
            &key(9),
        );

        assert_eq!(
            objects.admit(&published.operation, Epoch::GENESIS),
            Err(crate::chain::Refused::NotAContradiction)
        );
    }

    #[test]
    fn the_second_person_to_find_it_writes_the_same_object() {
        // One contradiction is one object, however many people noticed.
        let mut objects = crate::chain::Objects::new();
        let one = root(3, 7, b"one history");
        let other = root(3, 7, b"another history");

        objects
            .admit(
                &publish(&one, &other, Epoch::GENESIS, &key(9)).operation,
                Epoch::GENESIS,
            )
            .expect("the first");
        assert_eq!(
            objects.admit(
                &publish(&other, &one, Epoch::GENESIS, &key(10)).operation,
                Epoch::GENESIS
            ),
            Err(crate::chain::Refused::AlreadyExists)
        );
    }

    #[test]
    fn two_nodes_saying_different_things_is_not_a_contradiction() {
        // They have different trees by design. That is what having more than one node is for.
        let published = publish(
            &root(3, 7, b"what one saw"),
            &root(4, 7, b"what the other saw"),
            Epoch::GENESIS,
            &key(9),
        );
        assert_eq!(against(&published.operation), None);
    }

    #[test]
    fn one_node_talking_about_two_epochs_is_not_a_contradiction() {
        let published = publish(
            &root(3, 7, b"epoch seven"),
            &root(3, 8, b"epoch eight"),
            Epoch::GENESIS,
            &key(9),
        );
        assert_eq!(against(&published.operation), None);
    }

    #[test]
    fn the_same_root_twice_is_not_a_contradiction() {
        let one = root(3, 7, b"one history");
        let published = publish(&one, &one, Epoch::GENESIS, &key(9));
        assert_eq!(against(&published.operation), None);
    }

    #[test]
    fn an_accusation_carrying_something_that_is_not_a_root_is_not_one() {
        // Otherwise anybody could write an accusation against anybody by making up the evidence.
        let one = root(3, 7, b"one history");
        let mut published = publish(&one, &root(3, 7, b"another"), Epoch::GENESIS, &key(9));
        published.operation.payload.insert(
            3,
            almena_format::cbor::Value::Bytes(b"not a root at all".to_vec()),
        );
        assert_eq!(against(&published.operation), None);
    }

    #[test]
    fn a_contradiction_says_which_node_it_is_against() {
        // **How anybody finds it.** It is looked for by the party affected, not by whoever bothered
        // to write it down — and it comes out of the act itself, so two honest nodes reading the
        // same bytes file it under the same name whatever else they know.
        let published = publish(
            &root(3, 7, b"one history"),
            &root(3, 7, b"another history"),
            Epoch::GENESIS,
            &key(9),
        );

        assert_eq!(
            super::against_whom(&published.operation),
            Some(Did::new(Network::Development, Name::of(&[3])))
        );
    }

    #[test]
    fn the_same_pair_found_in_either_order_is_against_the_same_node() {
        // The two are put in the order their own bytes sort, so which of them is read for the name
        // must not change the answer.
        let one = root(3, 7, b"one history");
        let other = root(3, 7, b"another history");

        assert_eq!(
            super::against_whom(&publish(&one, &other, Epoch::GENESIS, &key(9)).operation),
            super::against_whom(&publish(&other, &one, Epoch::GENESIS, &key(10)).operation)
        );
    }

    #[test]
    fn something_that_is_not_a_contradiction_is_against_nobody() {
        // Otherwise anything at all could be filed against anybody.
        let published = publish(
            &root(3, 7, b"what one saw"),
            &root(4, 7, b"what the other saw"),
            Epoch::GENESIS,
            &key(9),
        );
        assert_eq!(super::against_whom(&published.operation), None);
    }
}
