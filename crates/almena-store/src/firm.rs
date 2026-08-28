//! How firm an act is: how many independent trees carry it.
//!
//! **Nothing here asks anybody anything.** It is handed what several nodes said and works out what
//! that adds up to — which is what makes it testable without a network, and what makes the same
//! rule usable by whoever is counting, whether that is a node, a portal or somebody's wallet.
//!
//! # Why counting is the answer at all
//!
//! There is no vote and nobody decides. An act is written down by whoever accepted it, each into a
//! tree of their own, and each of them signs where their tree stood. So *how sure am I* is not a
//! question anybody answers — it is a number: **how many separate trees, kept by separate people,
//! can be shown to carry this**.
//!
//! One node saying so is one node's word. Five is five trees that would all have had to be wrong in
//! the same way, by people who never had to agree on anything.
//!
//! # What is checked before anything counts
//!
//! | | Because |
//! | --- | --- |
//! | The root is signed by the key that node's record says it has | Otherwise it is somebody's bytes, and anybody can send anybody bytes |
//! | The root is this network's | A development root and a production one say the same things about themselves |
//! | The path carries the act to that root, at the size inside the signature | A path against a size nobody stated proves nothing |
//! | The node is one that has not been counted already | Two answers from one node are one tree, however many times it is asked |
//!
//! **Nothing here believes a node about who it is.** The key it is held to has to come from the
//! record — resolving the node's own name — and not from the answer being counted, which is exactly
//! the thing under suspicion.

use std::collections::HashSet;

use almena_format::entry::Entry;
use almena_format::identifier::{Did, Name};
use almena_format::operation::Operation;
use almena_suite::ed25519;

use crate::root::Published;
use crate::tree::{Path, included};

/// One node's answer about one act.
#[derive(Debug, Clone)]
pub struct Carried {
    /// Which node said it, as the record names it.
    pub node: Did,
    /// The key that node's own record says it has.
    ///
    /// **Resolved, never taken from the answer.** A root that vouched for the key it was checked
    /// against would vouch for anybody.
    pub key: [u8; ed25519::PUBLIC_KEY_WIDTH],
    /// The root it signed, and whoever countersigned it.
    pub published: Published,
    /// Where the act sits in that node's record. Its own position, meaning nothing anywhere else.
    pub at: u64,
    /// The hashes that carry it up to that root.
    pub path: Path,
}

/// Why one node's answer did not count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotCounted {
    /// The root is not this network's.
    AnotherNetwork,
    /// The root is not signed by the key that node's record says it has.
    NotThatNode,
    /// The path does not carry the act to that root at that size.
    NotInThatTree,
    /// That node has already been counted. Two answers from one node are one tree.
    CountedAlready,
}

/// How many independent trees carry `act`, and what was thrown out on the way.
///
/// **The refusals matter as much as the number.** A count of two from five answers is a different
/// situation from a count of two from two, and whoever is deciding on it should be able to see
/// which — a node that answered wrongly is a thing to go and look at.
#[must_use]
pub fn carried_by(
    act: &Operation,
    network: &Name,
    answers: &[Carried],
) -> (usize, Vec<(Did, NotCounted)>) {
    let mut counted: HashSet<Did> = HashSet::new();
    let mut refused = Vec::new();

    for answer in answers {
        if let Err(why) = weighs(act, network, answer, &counted) {
            refused.push((answer.node.clone(), why));
            continue;
        }
        counted.insert(answer.node.clone());
    }
    (counted.len(), refused)
}

/// Whether one node's answer counts, given who has been counted already.
fn weighs(
    act: &Operation,
    network: &Name,
    answer: &Carried,
    counted: &HashSet<Did>,
) -> Result<(), NotCounted> {
    if counted.contains(&answer.node) {
        return Err(NotCounted::CountedAlready);
    }
    // Held to the key the record says that node has, which is the whole reason this is worth
    // counting rather than tallying.
    answer
        .published
        .accept(network, &answer.key)
        .map_err(|why| match why {
            crate::root::Rejected::AnotherNetwork => NotCounted::AnotherNetwork,
            _ => NotCounted::NotThatNode,
        })?;

    // The entry is rebuilt here rather than taken from the answer: it is a function of the act and
    // the position, and taking it from whoever is being checked would be checking their arithmetic
    // against itself.
    let entry = Entry::of(act, answer.at, None);
    let size =
        usize::try_from(answer.published.root.size).map_err(|_| NotCounted::NotInThatTree)?;
    let at = usize::try_from(answer.at).map_err(|_| NotCounted::NotInThatTree)?;

    if included(
        &entry.to_bytes(),
        at,
        size,
        &answer.path,
        &answer.published.root.root,
    ) {
        Ok(())
    } else {
        Err(NotCounted::NotInThatTree)
    }
}

#[cfg(test)]
mod tests {
    use super::{Carried, NotCounted, carried_by};
    use crate::log::Log;
    use crate::root::Root;
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::{Operation, Signed, create};
    use almena_suite::ed25519;
    use almena_time::Epoch;
    use std::collections::BTreeMap;

    fn key(seed: u8) -> ed25519::SigningKey {
        ed25519::SigningKey::from_secret([seed; 32])
    }

    fn network() -> Name {
        Name::of(b"one network")
    }

    /// An act somebody signed.
    fn an_act(seed: u8) -> Operation {
        let control = key(seed);
        let public = control.verifying_key().bytes();
        let mut operation = create(
            Network::Development,
            crate::kind::Kind::HOLDER_CREATE.number(),
            1,
            Epoch::GENESIS,
            BTreeMap::from([(1, Value::Bytes(public.to_vec()))]),
        );
        let signature = control.sign(&operation.signing_bytes());
        operation.signatures.push(Signed {
            by: operation.object.clone(),
            key: public.to_vec(),
            signature: signature.bytes(),
        });
        operation
    }

    /// One node that wrote `before` acts down, then the one being counted, and what it says of it.
    ///
    /// The padding is what makes the nodes independent: each puts the act at a different position
    /// in a differently shaped tree, which is exactly the situation a count has to survive.
    fn a_node(seed: u8, before: u8, act: &Operation) -> Carried {
        let mut log = Log::new();
        for pad in 0..before {
            log.append(&an_act(200 + pad), None);
        }
        log.append(act, None);

        let node = Did::new(Network::Development, Name::of(&[seed]));
        let root = Root {
            network: network(),
            node: node.clone(),
            epoch: Epoch::GENESIS,
            size: log.len() as u64,
            root: log.root(),
        };
        let named = Name::of(&act.to_bytes());
        let (at, path) = log.inclusion(&named).expect("it is in there");

        Carried {
            node,
            key: key(seed).verifying_key().bytes(),
            published: root.publish(&key(seed)),
            at,
            path,
        }
    }

    #[test]
    fn every_node_that_can_show_it_counts_once() {
        // Three trees of different shapes, each with the act at a different position, each signed
        // by a different key. That is what *independent* means and what a count is counting.
        let act = an_act(9);
        let answers: Vec<Carried> = [(3u8, 0u8), (4, 1), (5, 7)]
            .into_iter()
            .map(|(seed, before)| a_node(seed, before, &act))
            .collect();

        let (count, refused) = carried_by(&act, &network(), &answers);
        assert_eq!(count, 3);
        assert!(refused.is_empty());
    }

    #[test]
    fn one_node_asked_twice_is_one_tree() {
        // However many times it is asked. A count that could be run up by asking again would be a
        // count of questions rather than of trees.
        let act = an_act(9);
        let one = a_node(3, 0, &act);
        let (count, refused) = carried_by(&act, &network(), &[one.clone(), one]);

        assert_eq!(count, 1);
        assert_eq!(
            refused,
            vec![(refused[0].0.clone(), NotCounted::CountedAlready)]
        );
    }

    #[test]
    fn a_root_not_signed_by_that_node_does_not_count() {
        // **The check the whole thing rests on.** The key comes from the record, so a node that
        // signed with something else is somebody sending bytes.
        let act = an_act(9);
        let mut answer = a_node(3, 0, &act);
        answer.key = key(99).verifying_key().bytes();

        let (count, refused) = carried_by(&act, &network(), &[answer]);
        assert_eq!(count, 0);
        assert_eq!(refused[0].1, NotCounted::NotThatNode);
    }

    #[test]
    fn a_root_from_another_network_does_not_count() {
        let act = an_act(9);
        let answer = a_node(3, 0, &act);
        let (count, refused) = carried_by(&act, &Name::of(b"somewhere else"), &[answer]);

        assert_eq!(count, 0);
        assert_eq!(refused[0].1, NotCounted::AnotherNetwork);
    }

    #[test]
    fn a_path_that_does_not_carry_the_act_does_not_count() {
        // A node that signed a real root and handed over a proof of something else.
        let act = an_act(9);
        let other = an_act(11);
        let answer = a_node(3, 0, &act);

        let (count, refused) = carried_by(&other, &network(), &[answer]);
        assert_eq!(count, 0);
        assert_eq!(refused[0].1, NotCounted::NotInThatTree);
    }

    #[test]
    fn a_position_from_one_node_does_not_count_against_another() {
        // A position belongs to the record it is a position in. Reading one node's position into
        // another's tree is the mistake that would make a count meaningless.
        let act = an_act(9);
        let mut answer = a_node(4, 1, &act);
        answer.at = 0;

        let (count, refused) = carried_by(&act, &network(), &[answer]);
        assert_eq!(count, 0);
        assert_eq!(refused[0].1, NotCounted::NotInThatTree);
    }

    #[test]
    fn what_was_thrown_out_is_said_and_not_swallowed() {
        // A count of one from three is a different situation from a count of one from one, and
        // whoever is deciding on it has to be able to tell them apart.
        let act = an_act(9);
        let good = a_node(3, 0, &act);
        let mut bad = a_node(4, 1, &act);
        bad.key = key(99).verifying_key().bytes();

        let (count, refused) = carried_by(&act, &network(), &[good, bad]);
        assert_eq!(count, 1);
        assert_eq!(refused.len(), 1);
    }
}
