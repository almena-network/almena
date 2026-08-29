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

use std::collections::{BTreeSet, HashSet};

use almena_format::entry::Entry;
use almena_format::identifier::{Did, Name};
use almena_format::operation::Operation;
use almena_suite::ed25519;
use almena_time::Epoch;

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
    /// Where the record says that node can be reached.
    ///
    /// **Resolved, like the key.** It is what the node said about itself in the record, which is
    /// what anybody else reading the record would also see — never what the answer being counted
    /// claims about where it came from.
    pub reachable: BTreeSet<String>,
    /// Where whoever is counting actually reached that node.
    ///
    /// **Empty is *I did not reach it myself*, not *it is nowhere*.** A counter working from acts
    /// somebody handed it has reached nobody, and reading that as a node being unreachable would
    /// turn having stayed at home into a finding about somebody else.
    pub found_at: BTreeSet<String>,
    /// What other nodes wrote down about this one over the window being looked at.
    ///
    /// **Nobody says this about themselves**, which is what makes it worth having — and it is per
    /// node and never averaged, because an average hides that a figure came from one observer, and
    /// one observer's day is one node's word.
    pub watched: Watched,
    /// Whether the share-out puts this act's history on that node.
    ///
    /// Worked out from the record by anybody, asking nobody.
    pub dealt: bool,
    /// Whether whoever is counting got that history from it when they asked.
    pub serving: Serving,
    /// Whether the record proves that key signed two things that cannot both be true.
    ///
    /// **Resolved by whoever is counting, and it is their decision.** A network without permission
    /// can impose one consequence and this is not it; what this is, is the counter saying it will
    /// not treat a tree kept by somebody demonstrably willing to sign two histories as one more
    /// independent tree. Somebody who wants to count it anyway simply does not set this.
    pub contradicted: bool,
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
    /// The record proves that node signed two things that cannot both be true.
    ///
    /// The point of counting is that several trees would all have had to be wrong in the same way,
    /// by people who never had to agree on anything. One kept by somebody who has already signed
    /// two histories is not evidence of that.
    Contradicted,
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

/// What other nodes said about one node over a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watched {
    /// No summary in the window names it.
    ///
    /// **Not nought.** Nobody having written anything down about a node is a fact about the
    /// observers, and reading it as *it answered nothing* would blame a node for a day nobody spent
    /// watching it.
    Nobody,
    /// What the observers who did name it wrote down.
    By {
        /// How many observers it is drawn from. One observer's day is one node's word.
        observers: usize,
        /// What they saw: how often it was asked, how often it answered, how far behind it got.
        seen: crate::summary::Seen,
    },
}

/// Whether whoever is counting got an act's history from a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Serving {
    /// Nobody asked it.
    ///
    /// **A fact about the counter and not about the node.** Reading it as a failure to serve would
    /// turn having stayed at home into a finding about somebody else.
    NotAsked,
    /// It was asked, and this is how much came back and checked out.
    Asked {
        /// How many things it was asked to hand over.
        asked_for: u64,
        /// How many came back and were what they were said to be.
        handed_over: u64,
    },
}

/// What is behind one act, and nothing at all about what that is worth.
///
/// **The platform does not decide whether an act is firm enough**, and this is the shape of it not
/// deciding: counted facts, each carrying the denominator that makes it readable, and every absence
/// said as an absence rather than rendered as a nought that reads as health. Whoever is relying on
/// the act compares these against what they are willing to accept — a shop at its own door and a
/// register whose entries are checked across a country do not want the same thing, and one number
/// would mean opposite things to the two of them.
///
/// **Nothing here is weighted.** Multiplying a tree by how available its keeper has been would turn
/// a count of trees into a score, and let one very available node outweigh three — which is scoring
/// under another name, and scoring is the thing this must not do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Footing {
    /// How many nodes the record names at all.
    ///
    /// **The denominator without which the count is unreadable.** Three trees out of a census of
    /// three and three out of forty are different situations, and only one of them is short.
    pub census: usize,
    /// How many nodes were put the question, and how many said anything back.
    ///
    /// Nought asked is *nobody went*, never *nothing carries it*.
    pub asked: usize,
    /// How many of those answered at all.
    pub answered: usize,
    /// How many independent trees carry it.
    pub trees: usize,
    /// Every answer that did not count, and why.
    ///
    /// A count of two from five is a different situation from a count of two from two, and whoever
    /// is deciding should be able to tell them apart.
    pub refused: Vec<(Did, NotCounted)>,
    /// How spread out those trees are, as far as the record and the counter can say.
    pub spread: Spread,
    /// The oldest and newest epoch among the roots that counted, if any did.
    ///
    /// Both ends, because one root closed late is not the same situation as every root being stale.
    pub roots: Option<(Epoch, Epoch)>,
    /// One entry per node whose tree counted, never summarised into an average.
    pub keepers: Vec<Keeper>,
}

/// One node whose tree carries an act, and what is known about that node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keeper {
    /// Which node, as the record names it.
    pub node: Did,
    /// What other nodes wrote down about it.
    pub watched: Watched,
    /// Whether the share-out puts this act's history on it.
    pub dealt: bool,
    /// Whether whoever is counting got that history from it.
    pub serving: Serving,
}

/// Everything that bears on how well an act is carried, and no judgement about it.
///
/// `census` is how many nodes the record names; the answers are what the counter collected.
#[must_use]
pub fn footing(act: &Operation, network: &Name, answers: &[Carried], census: usize) -> Footing {
    let (trees, refused) = carried_by(act, network, answers);
    let counted: BTreeSet<&Did> = refused.iter().map(|(node, _)| node).collect();
    let keeping: Vec<&Carried> = answers
        .iter()
        .filter(|answer| !counted.contains(&answer.node))
        .collect();

    let epochs: Vec<Epoch> = keeping
        .iter()
        .map(|answer| answer.published.root.epoch)
        .collect();
    let roots = epochs
        .iter()
        .min()
        .zip(epochs.iter().max())
        .map(|(first, last)| (*first, *last));

    Footing {
        census,
        asked: answers.len(),
        answered: answers.len(),
        trees,
        refused,
        spread: spread_of(answers),
        roots,
        keepers: keeping
            .iter()
            .map(|answer| Keeper {
                node: answer.node.clone(),
                watched: answer.watched,
                dealt: answer.dealt,
                serving: answer.serving,
            })
            .collect(),
    }
}

/// How spread out the trees that carry an act are.
///
/// **It does not prove independence, and nothing can.** Nodes are open and nobody says who runs
/// one, so a swarm belonging to one operator is outside any figure drawn from the record. What this
/// does is let somebody who wants independence **ask for it instead of assuming it**: five trees
/// from one place are one place's word, whoever signed them, and that is worth being able to see.
///
/// Facts, and no ranking. It says how many and from where; what is enough is the asker's to decide.
#[must_use]
pub fn spread_of(answers: &[Carried]) -> Spread {
    let counted: BTreeSet<&Did> = answers.iter().map(|answer| &answer.node).collect();
    let places: BTreeSet<String> = answers
        .iter()
        .flat_map(|answer| answer.reachable.iter())
        .filter_map(|address| crate::capability::place(address))
        .collect();
    let silent = answers
        .iter()
        .filter(|answer| answer.reachable.is_empty())
        .count();
    let found: BTreeSet<String> = answers
        .iter()
        .flat_map(|answer| answer.found_at.iter())
        .filter_map(|address| crate::capability::place(address))
        .collect();
    let elsewhere = answers.iter().filter(|answer| moved(answer)).count();

    Spread {
        nodes: counted.len(),
        places: places.len(),
        nowhere: silent,
        found: found.len(),
        elsewhere,
    }
}

/// Whether a node was reached only in places it never said it was.
///
/// False for one nobody reached, which is *nothing was looked at* rather than *it was not there* —
/// the two are only the same to somebody who has forgotten they did not go.
fn moved(answer: &Carried) -> bool {
    if answer.found_at.is_empty() {
        return false;
    }
    let said: BTreeSet<String> = answer
        .reachable
        .iter()
        .filter_map(|address| crate::capability::place(address))
        .collect();

    !answer
        .found_at
        .iter()
        .filter_map(|address| crate::capability::place(address))
        .any(|found| said.contains(&found))
}

/// How far apart the trees behind an act are, as far as the record can say.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Spread {
    /// How many distinct nodes answered.
    pub nodes: usize,
    /// How many distinct places those nodes say they are in.
    pub places: usize,
    /// How many of them have never said where they are.
    ///
    /// **Said rather than swallowed.** A node that published no address is not a node in its own
    /// place — it is one nobody can tell, and folding it into either figure would turn *unknown*
    /// into an answer.
    pub nowhere: usize,
    /// How many distinct places whoever is counting actually reached these nodes in.
    ///
    /// **The half that could not be written down for free.** Publishing an address costs nothing;
    /// answering on one had to work. So where a node was really found is the figure a swarm cannot
    /// improve by declaring things, and it is kept beside the declared one rather than instead of
    /// it — because a counter that reached nobody has not discovered that nobody is anywhere.
    pub found: usize,
    /// How many were reached somewhere they never said they were.
    ///
    /// Not an accusation: the counter may be behind on the record, or the node may have moved and
    /// said so in an act this counter has not got. It is worth seeing and not worth concluding
    /// from, which is why it is a count here and nothing anywhere else.
    pub elsewhere: usize,
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
    if answer.contradicted {
        return Err(NotCounted::Contradicted);
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

    // The entry is rebuilt here rather than taken from the answer: it is a function of the act, the
    // position and what the act is about, and taking it from whoever is being checked would be
    // checking their arithmetic against itself.
    //
    // **What it is about comes from the same place the log gets it.** Deciding it separately here
    // would refuse honest proofs for honest acts, with nothing to look at and nobody at fault.
    let entry = Entry::of(act, answer.at, crate::subject_of(act));
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
    use super::{Carried, NotCounted, Serving, Watched, carried_by, footing, spread_of};
    use crate::log::Log;
    use crate::root::Root;
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::{Operation, Signed, create};
    use almena_suite::ed25519;
    use almena_time::Epoch;
    use std::collections::{BTreeMap, BTreeSet};

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
        let named = act.called();
        let (at, path) = log.inclusion(&named).expect("it is in there");

        Carried {
            node,
            key: key(seed).verifying_key().bytes(),
            published: root.publish(&key(seed)),
            at,
            path,
            reachable: BTreeSet::new(),
            found_at: BTreeSet::new(),
            watched: Watched::Nobody,
            dealt: false,
            serving: Serving::NotAsked,
            contradicted: false,
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

    #[test]
    fn a_tree_kept_by_somebody_who_signed_two_histories_does_not_count() {
        // **What counting is for.** Several trees would all have had to be wrong in the same way,
        // by people who never had to agree on anything. One kept by somebody already shown to sign
        // two histories is not evidence of that, and counting it would be counting the thing the
        // count exists to survive.
        let act = an_act(9);
        let good = a_node(3, 0, &act);
        let mut caught = a_node(4, 1, &act);
        caught.contradicted = true;

        let (count, refused) = carried_by(&act, &network(), &[good, caught]);
        assert_eq!(count, 1);
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].1, NotCounted::Contradicted);
    }

    #[test]
    fn whoever_is_counting_decides_whether_to_hold_it_against_them() {
        // A network without permission imposes one consequence, and this is not it. Somebody who
        // wants to count a contradicted node's tree simply does not say it is contradicted.
        let act = an_act(9);
        let counted_anyway = a_node(4, 1, &act);

        let (count, refused) = carried_by(&act, &network(), &[counted_anyway]);
        assert_eq!(count, 1);
        assert!(refused.is_empty());
    }

    /// The same node, saying where it is.
    fn found_at(mut carried: Carried, addresses: &[&str]) -> Carried {
        carried.reachable = addresses.iter().map(|one| (*one).to_owned()).collect();
        carried
    }

    #[test]
    fn five_trees_from_one_place_are_one_places_word() {
        // **What the figure is for.** It does not prove independence — nobody can — but somebody who
        // wants it can ask for it instead of assuming it, and this is what they read.
        let act = an_act(9);
        let answers: Vec<Carried> = [(3u8, 0u8), (4, 1), (5, 7)]
            .into_iter()
            .map(|(seed, before)| {
                found_at(a_node(seed, before, &act), &["/ip4/198.51.100.7/tcp/4001"])
            })
            .collect();

        let spread = spread_of(&answers);
        assert_eq!(spread.nodes, 3, "three separate trees, kept by three keys");
        assert_eq!(spread.places, 1, "and all of them in one place");
        assert_eq!(spread.nowhere, 0);
    }

    #[test]
    fn three_places_are_three_places() {
        let act = an_act(9);
        let answers: Vec<Carried> = [
            (3u8, 0u8, "/ip4/198.51.100.7/tcp/4001"),
            (4, 1, "/ip4/203.0.113.9/tcp/4001"),
            (5, 7, "/ip6/2001:db8::1/tcp/4001"),
        ]
        .into_iter()
        .map(|(seed, before, at)| found_at(a_node(seed, before, &act), &[at]))
        .collect();

        let spread = spread_of(&answers);
        assert_eq!(spread.nodes, 3);
        assert_eq!(spread.places, 3);
    }

    #[test]
    fn two_doors_on_one_building_do_not_make_two_places() {
        let act = an_act(9);
        let answers = vec![
            found_at(a_node(3, 0, &act), &["/ip4/198.51.100.7/tcp/4001"]),
            found_at(a_node(4, 1, &act), &["/ip4/198.51.100.7/tcp/4002"]),
        ];
        assert_eq!(spread_of(&answers).places, 1);
    }

    #[test]
    fn a_node_that_has_never_said_where_it_is_is_counted_as_that() {
        // **Said rather than swallowed.** Folding it into either figure would turn *nobody can tell*
        // into an answer — either inventing a place or hiding that one is missing.
        let act = an_act(9);
        let answers = vec![
            found_at(a_node(3, 0, &act), &["/ip4/198.51.100.7/tcp/4001"]),
            a_node(4, 1, &act),
        ];

        let spread = spread_of(&answers);
        assert_eq!(spread.nodes, 2);
        assert_eq!(spread.places, 1, "only one of them said");
        assert_eq!(spread.nowhere, 1);
    }

    #[test]
    fn a_node_in_more_than_one_place_is_in_all_of_them() {
        // A node reachable two ways is reachable two ways, and saying so is the honest reading:
        // what the figure asks is how many places the answers came from, not how many nodes there
        // are per place.
        let act = an_act(9);
        let answers = vec![found_at(
            a_node(3, 0, &act),
            &["/ip4/198.51.100.7/tcp/4001", "/ip6/2001:db8::1/tcp/4001"],
        )];

        let spread = spread_of(&answers);
        assert_eq!(spread.nodes, 1);
        assert_eq!(spread.places, 2);
    }

    #[test]
    fn nothing_answering_is_nowhere_and_nobody() {
        assert_eq!(spread_of(&[]), super::Spread::default());
    }

    /// The same node, and where whoever is counting actually reached it.
    fn reached_at(mut carried: Carried, addresses: &[&str]) -> Carried {
        carried.found_at = addresses.iter().map(|one| (*one).to_owned()).collect();
        carried
    }

    #[test]
    fn where_a_node_was_really_found_is_counted_apart_from_where_it_said() {
        // **The half that could not be written down for free.** Publishing an address costs
        // nothing; answering on one had to work. So a swarm cannot improve this figure by declaring
        // things, which is the whole reason it is kept beside the declared one.
        let act = an_act(9);
        let answers = vec![
            reached_at(
                found_at(a_node(3, 0, &act), &["/ip4/198.51.100.7/tcp/4001"]),
                &["/ip4/198.51.100.7/tcp/4001"],
            ),
            reached_at(
                found_at(a_node(4, 1, &act), &["/ip4/203.0.113.9/tcp/4001"]),
                &["/ip4/203.0.113.9/tcp/4002"],
            ),
        ];

        let spread = spread_of(&answers);
        assert_eq!(spread.places, 2, "two places said");
        assert_eq!(spread.found, 2, "and two places actually reached");
        assert_eq!(spread.elsewhere, 0, "each where it said it would be");
    }

    #[test]
    fn a_node_reached_only_where_it_never_said_is_counted_as_that() {
        // Not an accusation: whoever is counting may be behind on the record, or the node may have
        // moved and said so in an act they have not got. Worth seeing, not worth concluding from.
        let act = an_act(9);
        let answers = vec![reached_at(
            found_at(a_node(3, 0, &act), &["/ip4/198.51.100.7/tcp/4001"]),
            &["/ip4/203.0.113.9/tcp/4001"],
        )];

        let spread = spread_of(&answers);
        assert_eq!(spread.elsewhere, 1);
        assert_eq!(spread.places, 1, "where it said");
        assert_eq!(spread.found, 1, "and where it was");
    }

    #[test]
    fn having_stayed_at_home_is_not_a_finding_about_anybody() {
        // **Empty is *I did not reach it myself*, not *it is nowhere*.** A counter working from acts
        // somebody handed it has reached nobody, and reading that as unreachability would turn not
        // having gone into a fact about somebody else.
        let act = an_act(9);
        let answers = vec![found_at(
            a_node(3, 0, &act),
            &["/ip4/198.51.100.7/tcp/4001"],
        )];

        let spread = spread_of(&answers);
        assert_eq!(spread.found, 0, "nobody was reached, by anybody");
        assert_eq!(
            spread.elsewhere, 0,
            "and nobody was found somewhere they had not said"
        );
        assert_eq!(spread.places, 1, "what it said is still what it said");
    }

    #[test]
    fn a_door_it_did_not_name_in_a_building_it_did_is_the_building_it_said() {
        // The place is the address without the door. A node reached on another port of the machine
        // it published is where it said it would be.
        let act = an_act(9);
        let answers = vec![reached_at(
            found_at(a_node(3, 0, &act), &["/ip4/198.51.100.7/tcp/4001"]),
            &["/ip4/198.51.100.7/tcp/9999"],
        )];
        assert_eq!(spread_of(&answers).elsewhere, 0);
    }

    #[test]
    fn an_honest_proof_for_an_act_about_somebody_else_counts() {
        // **The entry has to be rebuilt the way the log wrote it.** An act that says who it is about
        // gets that into its log entry, so a rebuild that decided otherwise would refuse an honest
        // proof for an honest act — with nothing to look at and nobody at fault. Today the only act
        // that says is a contradiction, which is the one the record exists to make provable.
        let network = network();
        let against = Did::new(Network::Development, Name::of(&[3]));
        let a_root = |over: &[u8]| {
            crate::root::Root {
                network: network.clone(),
                node: against.clone(),
                epoch: Epoch::GENESIS,
                size: 4,
                root: almena_suite::digest::Digest::of(over),
            }
            .publish(&key(3))
        };
        let act = crate::contradiction::publish(
            &a_root(b"one history"),
            &a_root(b"another history"),
            Epoch::GENESIS,
            &key(9),
        )
        .operation;
        assert!(
            crate::subject_of(&act).is_some(),
            "it says who it is against"
        );

        // Written down exactly as a node writes it, and proved against that node's own root.
        let mut log = Log::new();
        log.append(&act, crate::subject_of(&act));
        let (at, path) = log.inclusion(&act.called()).expect("it is in there");
        let node = Did::new(Network::Development, Name::of(&[7]));
        let answer = Carried {
            node: node.clone(),
            key: key(7).verifying_key().bytes(),
            published: crate::root::Root {
                network: network.clone(),
                node,
                epoch: Epoch::GENESIS,
                size: log.len() as u64,
                root: log.root(),
            }
            .publish(&key(7)),
            at,
            path,
            reachable: BTreeSet::new(),
            found_at: BTreeSet::new(),
            watched: Watched::Nobody,
            dealt: false,
            serving: Serving::NotAsked,
            contradicted: false,
        };

        let (count, refused) = carried_by(&act, &network, &[answer]);
        assert!(refused.is_empty(), "{refused:?}");
        assert_eq!(count, 1);
    }

    #[test]
    fn what_is_behind_an_act_is_said_without_saying_what_it_is_worth() {
        // **The shape of the platform not deciding.** A shop at its own door and a register whose
        // entries are checked across a country do not want the same thing, and one number would
        // mean opposite things to the two of them.
        let act = an_act(9);
        let answers: Vec<Carried> = [(3u8, 0u8), (4, 1), (5, 7)]
            .into_iter()
            .map(|(seed, before)| a_node(seed, before, &act))
            .collect();

        let footing = footing(&act, &network(), &answers, 40);
        assert_eq!(footing.trees, 3);
        assert_eq!(
            footing.census, 40,
            "the denominator, without which three is unreadable"
        );
        assert_eq!(footing.asked, 3);
        assert_eq!(footing.keepers.len(), 3, "one entry each, never averaged");
        assert!(footing.refused.is_empty());
        assert_eq!(footing.roots, Some((Epoch::GENESIS, Epoch::GENESIS)));
    }

    #[test]
    fn nobody_having_watched_a_node_is_not_the_node_answering_nothing() {
        // Reading it as nought would blame a node for a day nobody spent watching it.
        let act = an_act(9);
        let answer = a_node(3, 0, &act);
        assert_eq!(answer.watched, Watched::Nobody);

        let footing = footing(&act, &network(), &[answer], 3);
        assert_eq!(footing.keepers[0].watched, Watched::Nobody);
        assert_eq!(
            footing.keepers[0].serving,
            Serving::NotAsked,
            "and nobody having asked it is a fact about the counter"
        );
    }

    #[test]
    fn a_tree_that_did_not_count_is_not_a_keeper_and_the_reason_is_kept() {
        // A count of two from five is a different situation from a count of two from two.
        let act = an_act(9);
        let good = a_node(3, 0, &act);
        let mut caught = a_node(4, 1, &act);
        caught.contradicted = true;

        let footing = footing(&act, &network(), &[good, caught], 9);
        assert_eq!(footing.trees, 1);
        assert_eq!(footing.asked, 2, "two were asked");
        assert_eq!(footing.keepers.len(), 1, "and one of them counts");
        assert_eq!(footing.refused.len(), 1);
        assert_eq!(footing.refused[0].1, NotCounted::Contradicted);
    }

    #[test]
    fn nothing_here_is_weighted() {
        // Multiplying a tree by how available its keeper has been would turn a count of trees into
        // a score, and let one very available node outweigh three. The count is of trees.
        let act = an_act(9);
        let mut busy = a_node(3, 0, &act);
        busy.watched = Watched::By {
            observers: 40,
            seen: crate::summary::Seen {
                asked: 10_000,
                answered: 10_000,
                behind: 0,
            },
        };
        let quiet = a_node(4, 1, &act);

        let footing = footing(&act, &network(), &[busy, quiet], 5);
        assert_eq!(
            footing.trees, 2,
            "two trees, whatever is known about either"
        );
    }
}

/// How many independent trees a reader wants before it treats a concession as firm.
///
/// # This is client policy and not protocol, and it has to stay that way
///
/// `SPECS.md §4.4` is explicit: there is no instant of finality, firmness is progressive, and **how
/// many roots are demanded is configurable with conservative defaults**. So nothing here decides
/// for anybody — a node serves [`Footing`], which is facts, and this is the shape a reader's own
/// answer takes.
///
/// **Lowering it has to be a deliberate act**, which is why it is a named value with a named
/// default rather than a number somebody can pass in without noticing, and why the number a reader
/// used belongs on the screen beside what it decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wanted {
    /// How many independent trees have to carry it.
    pub trees: usize,
}

impl Wanted {
    /// The conservative default: three trees that do not answer to each other.
    ///
    /// Three rather than two because two can be one operator with two machines, which
    /// [`Spread`] can sometimes see and cannot always. And it is a **default**: a development
    /// network with one node reaches it never, which is correct — a concession is not firm there,
    /// and saying so is better than pretending a single tree is agreement.
    pub const CONSERVATIVE: Self = Self { trees: 3 };

    /// Whether that footing satisfies this reader.
    ///
    /// **Only trees that counted.** `SPECS.md §4.4` says a threshold anybody could reach by
    /// standing up nodes is no threshold, and nodes are free — so what is counted is already
    /// filtered to those with measured history and a served portion, and the refusals travel
    /// beside the count so that *two out of five* and *two out of two* can be told apart.
    #[must_use]
    pub const fn met_by(self, footing: &Footing) -> bool {
        footing.trees >= self.trees
    }
}

#[cfg(test)]
mod policy {
    use super::{Footing, Spread, Wanted};

    /// A footing with that many trees behind it.
    fn carried_by(trees: usize) -> Footing {
        Footing {
            census: 9,
            asked: 9,
            answered: trees,
            trees,
            refused: Vec::new(),
            spread: Spread::default(),
            roots: None,
            keepers: Vec::new(),
        }
    }

    #[test]
    fn the_default_is_conservative_and_lowering_it_is_something_somebody_does_on_purpose() {
        // **Client policy and not protocol** (`SPECS.md §4.4`). Nothing here decides for anybody;
        // what it does is make the number a named thing, so that a reader who lowered it did so
        // deliberately and can be asked what they set.
        assert_eq!(Wanted::CONSERVATIVE.trees, 3);
        assert!(Wanted::CONSERVATIVE.met_by(&carried_by(3)));
        assert!(!Wanted::CONSERVATIVE.met_by(&carried_by(2)));

        // And a reader that wants one is a reader that said so.
        assert!(Wanted { trees: 1 }.met_by(&carried_by(1)));
        assert!(!Wanted { trees: 1 }.met_by(&carried_by(0)));
    }

    #[test]
    fn a_network_with_one_tree_makes_nothing_firm_and_that_is_the_right_answer() {
        // A development network with one node reaches the default never. Saying so is better than
        // pretending a single tree is agreement — which is what a concession waiting for several
        // roots exists to avoid.
        assert!(!Wanted::CONSERVATIVE.met_by(&carried_by(1)));
    }
}
