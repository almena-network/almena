//! Which nodes are expected to hold a thing, worked out by anybody from what everybody has.
//!
//! Every node keeping everything is correct and does not last: the record only grows, and the day
//! it is too big for a laptop, a network whose only plan was *everybody keeps everything* has no
//! plan. What replaces it cannot be *each node keeps what it likes* either — if every node chooses,
//! every node chooses the popular things and the long tail is quietly lost, with nobody having
//! decided to lose it and nobody able to say it happened.
//!
//! So the share is **assigned, deterministic, and computable by anybody**: given the record, which
//! everyone holds, anybody works out which nodes are expected to hold any given thing. A node that
//! does not have what falls to it is not caught by an audit — it is visibly short, to whoever
//! bothers to look, from public data.
//!
//! # Why the place depends on the thing, the node, and the moment
//!
//! Membership is open, and a node's identity is a key it generated. So **making identities until
//! one lands on the thing you want is cheap** — which is a targeted attack on the assignment
//! itself, and whoever wins it can sit on exactly the piece they wanted to bury.
//!
//! That is why every node's place is scored **for each thing separately**, under a seed that
//! changes. Two consequences, and the second is the one that pays:
//!
//! - Landing on a chosen thing costs about as many attempts as there are nodes over copies, which
//!   is microseconds. This is conceded rather than defended.
//! - **The share that comes with it cannot be shrunk.** The same draw that puts a node on the thing
//!   it wanted puts it on its full share of everything else, and no choice of key makes that
//!   smaller. Fabricating an identity therefore costs real storage and real bandwidth, every month,
//!   for as long as the camping lasts — and that, not the grinding, is the price.
//!
//! The second point is exactly what a scheme that placed nodes on a circle would give away. There,
//! a node's place does not depend on the thing, so an identity can be ground to sit right behind
//! the target **and** right behind another node — covering what it wanted while owing almost
//! nothing. The share would be chosen after all.
//!
//! # The seed is public, and predictable on purpose
//!
//! Anybody can compute next year's seed and grind keys years ahead. Nothing is lost by that: an
//! attacker regenerating identities every rotation was already conceded, so unpredictability bought
//! no security — it would only have bought the problem of agreeing on something unpredictable. And
//! there is nothing here to agree on: the seed comes from the name of the act that opened the
//! network and from the count of periods, which every node reaches without asking.
//!
//! **It cannot come from a root.** There is no global log — each node keeps its own tree — so a
//! seed drawn from any one node's root would have two honest nodes sharing the record out
//! differently.
//!
//! # Things do not all move on the same day
//!
//! A fresh seed re-draws every placement independently, so what a node keeps from one period to the
//! next is about *copies over nodes* — on a large network, nearly nothing. Rotating everything at
//! one instant would therefore have the whole network move nearly all of its replicated bytes on
//! the same hour, every thirty days, and during the scramble a node that is merely busy looks
//! exactly like one that is not bothering — which is the moment the measurement is least able to
//! tell them apart and most needed.
//!
//! So each thing gets **its own hour of the month to move on**, taken from its own name. Every
//! thing still moves exactly every thirty days, anybody still computes when, and the traffic is
//! spread across the month instead of arriving as one wave.

use almena_format::identifier::Name;
use almena_suite::digest::Digest;
use almena_time::{EPOCHS_PER_PERIOD, Epoch, Period};

use crate::parameter::Parameter;

/// What each thing tells apart inside a hash.
///
/// Two hashes over different things must not be able to come out the same by having the same bytes
/// mean different things in different places. It costs one byte and removes the question.
mod tag {
    /// The seed of a period.
    pub const SEED: u8 = 1;
    /// Where a node stands for one thing.
    pub const PLACE: u8 = 2;
    /// Which hour of the month a thing moves on.
    pub const WHEN: u8 = 3;
}

/// How many nodes are expected to hold a status list.
///
/// **Ten, because they are kilobytes and their availability *is* the path a revocation travels.**
/// Copying them further over costs almost nothing, and the thing being protected is somebody
/// finding out that a credential was withdrawn.
pub const COPIES_OF_A_STATUS_LIST: Parameter = Parameter::from(&[(Epoch::GENESIS, 10)]);

/// How many nodes are expected to hold a stretch of history.
///
/// **Five, because this is the expensive one.** It is asked for rarely — auditing rather than
/// operating — and five survives failures and small collusions without multiplying what it costs to
/// run a node by ten.
pub const COPIES_OF_HISTORY: Parameter = Parameter::from(&[(Epoch::GENESIS, 5)]);

/// The number every node draws the same share from.
///
/// From the name of the act that opened the network, and the count of periods. Nothing else: not a
/// root, not a majority, not anybody's word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seed(Digest);

impl Seed {
    /// The seed a network is on in a period.
    #[must_use]
    pub fn of(network: &Name, period: Period) -> Self {
        // The number is fixed-width and comes last, so one string of bytes can be read one way
        // only. A network name run straight into a decimal count would let two different pairs
        // produce one seed, and two honest nodes would share the record out differently.
        let mut bytes = vec![tag::SEED];
        bytes.extend_from_slice(network.as_str().as_bytes());
        bytes.extend_from_slice(&period.number().to_be_bytes());
        Self(Digest::of(&bytes))
    }
}

/// One share-out: a network, a moment, and the nodes it is drawn across.
///
/// The three travel together because none of them means anything alone — a share is *of this
/// network, at this moment, among these nodes* — and because keeping them apart invites a caller to
/// answer two questions under two different draws and compare the answers.
#[derive(Debug, Clone, Copy)]
pub struct Drawn<'a> {
    /// The name of the act that opened the network, which is what the seed comes from.
    network: &'a Name,
    /// When the share is being worked out for.
    at: Epoch,
    /// Every node the record names, one entry per node.
    census: &'a [&'a Name],
}

impl<'a> Drawn<'a> {
    /// The share-out in force at a moment.
    #[must_use]
    pub const fn at(network: &'a Name, at: Epoch, census: &'a [&'a Name]) -> Self {
        Self {
            network,
            at,
            census,
        }
    }

    /// How many nodes are expected to hold anything, given how many there are.
    ///
    /// **Never more than there are nodes.** On day one there are three and not ten, and asking for
    /// ten would make the shortfall a number about the network's size rather than about anything
    /// anybody could fix. It is a floor for a network too small to reach the figure, and nothing
    /// more: somebody willing to run identities removes it for the price of a signature each.
    #[must_use]
    pub fn wanted(&self, copies: Parameter) -> usize {
        let asked = usize::try_from(copies.at(self.at)).unwrap_or(usize::MAX);
        asked.min(self.census.len())
    }

    /// The nodes a thing falls to, best placed first.
    ///
    /// Everything is scored and the best are taken, rather than the nearest on a circle: it is what
    /// makes a node's share something it is dealt rather than something it can pick.
    #[must_use]
    pub fn holders(&self, thing: &Name, copies: Parameter) -> Vec<&'a Name> {
        let seed = self.seed_for(thing);
        let mut placed: Vec<(Digest, &'a Name)> = self
            .census
            .iter()
            .map(|node| (place(thing, &seed, node), *node))
            .collect();
        // By place, and by name where two places are the same. A tie broken by anything a node
        // could arrange — the order it was heard of, say — would be a tie it could win.
        placed.sort_unstable_by(|one, other| {
            other.0.bytes().cmp(one.0.bytes()).then(one.1.cmp(other.1))
        });
        placed
            .into_iter()
            .take(self.wanted(copies))
            .map(|(_, node)| node)
            .collect()
    }

    /// Whether a thing falls to one particular node.
    #[must_use]
    pub fn falls_to(&self, thing: &Name, node: &Name, copies: Parameter) -> bool {
        self.holders(thing, copies)
            .into_iter()
            .any(|held| held == node)
    }

    /// How many copies short a thing is, given who was found to be serving it.
    ///
    /// **Measured against who serves it, never against who was assigned it.** The assignment always
    /// names as many nodes as it can, so a shortfall counted against it would be nought by
    /// construction — a number that says everything is fine because it cannot say anything else.
    /// What the figure is for is answering whether what the record says existed still exists, and
    /// only somebody who went and asked can answer that.
    #[must_use]
    pub fn short_by(&self, serving: usize, copies: Parameter) -> usize {
        self.wanted(copies).saturating_sub(serving)
    }

    /// The period a thing is in, on its own hour of the month.
    #[must_use]
    pub fn period_of(&self, thing: &Name) -> Period {
        Period::new(
            self.at
                .number()
                .saturating_add(moves_on(thing))
                .wrapping_div(EPOCHS_PER_PERIOD),
        )
    }

    /// The seed a thing is placed under right now.
    fn seed_for(&self, thing: &Name) -> Seed {
        Seed::of(self.network, self.period_of(thing))
    }
}

/// Which hour of the month a thing moves on.
///
/// From the thing's own name, so everybody computes the same one and nobody has to be told. It is
/// what stops the whole network moving nearly everything it holds on one hour every thirty days —
/// a wave during which a node that is merely busy is indistinguishable from one that is not
/// bothering, which is when being able to tell them apart matters most.
#[must_use]
pub fn moves_on(thing: &Name) -> u64 {
    let mut bytes = vec![tag::WHEN];
    bytes.extend_from_slice(thing.as_str().as_bytes());
    let picked = u64::from_be_bytes(Digest::of(&bytes).bytes()[..8].try_into().unwrap_or([0; 8]));
    picked % EPOCHS_PER_PERIOD
}

/// Where a node stands for one thing under one seed. Greatest wins.
///
/// Everything that goes in is a fixed width, so one string of bytes reads one way. Names are hashed
/// rather than run in as they are: they are one width today because the hash inside them is, and
/// that hash exists in order to be able to change.
fn place(thing: &Name, seed: &Seed, node: &Name) -> Digest {
    let mut bytes = Vec::with_capacity(1 + 3 * almena_suite::digest::WIDTH);
    bytes.push(tag::PLACE);
    bytes.extend_from_slice(seed.0.bytes());
    bytes.extend_from_slice(Digest::of(thing.as_str().as_bytes()).bytes());
    bytes.extend_from_slice(Digest::of(node.as_str().as_bytes()).bytes());
    Digest::of(&bytes)
}

#[cfg(test)]
mod tests {
    use super::{COPIES_OF_HISTORY, Drawn, Seed, moves_on};
    use almena_format::identifier::Name;
    use almena_time::{EPOCHS_PER_PERIOD, Epoch, Period};

    fn network() -> Name {
        Name::of(b"the act that opened it")
    }

    fn nodes(how_many: usize) -> Vec<Name> {
        (0..how_many)
            .map(|which| Name::of(format!("node {which}").as_bytes()))
            .collect()
    }

    fn census(nodes: &[Name]) -> Vec<&Name> {
        nodes.iter().collect()
    }

    fn things(how_many: usize) -> Vec<Name> {
        (0..how_many)
            .map(|which| Name::of(format!("thing {which}").as_bytes()))
            .collect()
    }

    #[test]
    fn everybody_works_out_the_same_share() {
        // **The whole of it.** A share nobody else computes the same way is not an assignment; it is
        // an opinion, and a node short of its share could always say somebody else had it.
        let nodes = nodes(12);
        let thing = Name::of(b"a status list");

        let network = network();
        let held = census(&nodes);
        let one = Drawn::at(&network, Epoch::new(5_000), &held);
        let other = Drawn::at(&network, Epoch::new(5_000), &held);
        assert_eq!(
            one.holders(&thing, COPIES_OF_HISTORY),
            other.holders(&thing, COPIES_OF_HISTORY)
        );
    }

    #[test]
    fn the_order_the_census_is_given_in_changes_nothing() {
        // Two nodes hold the same record and list it however their own storage happens to.
        let nodes = nodes(12);
        let network = network();
        let held = census(&nodes);
        let mut backwards = held.clone();
        backwards.reverse();

        let forwards = Drawn::at(&network, Epoch::new(5_000), &held);
        let reversed = Drawn::at(&network, Epoch::new(5_000), &backwards);
        let thing = Name::of(b"a status list");
        assert_eq!(
            forwards.holders(&thing, COPIES_OF_HISTORY),
            reversed.holders(&thing, COPIES_OF_HISTORY)
        );
    }

    #[test]
    fn a_share_is_dealt_and_not_picked() {
        // **What makes fabricating an identity cost something.** Every node ends up holding roughly
        // its share of everything, and no node can arrange to hold less.
        let nodes = nodes(20);
        let things = things(2_000);
        let network = network();
        let held = census(&nodes);
        let drawn = Drawn::at(&network, Epoch::new(0), &held);

        let mut carried = vec![0usize; nodes.len()];
        for thing in &things {
            for node in drawn.holders(thing, COPIES_OF_HISTORY) {
                let at = nodes
                    .iter()
                    .position(|named| named == node)
                    .expect("a node");
                carried[at] += 1;
            }
        }

        let fair = things.len() * 5 / nodes.len();
        for (which, count) in carried.iter().enumerate() {
            assert!(
                *count > fair / 2 && *count < fair * 2,
                "node {which} carries {count}, fair share is {fair}"
            );
        }
    }

    #[test]
    fn a_thing_falls_to_as_many_nodes_as_there_are_when_there_are_few() {
        // On day one there are three and not ten. Asking for ten would make the shortfall a number
        // about how small the network is rather than about anything anybody could fix.
        let network = network();
        for how_many in 0..8 {
            let nodes = nodes(how_many);
            let held = census(&nodes);
            let drawn = Drawn::at(&network, Epoch::new(0), &held);
            assert_eq!(
                drawn
                    .holders(&Name::of(b"a thing"), COPIES_OF_HISTORY)
                    .len(),
                how_many.min(5),
                "{how_many} nodes"
            );
        }
    }

    #[test]
    fn a_node_that_knows_of_fewer_peers_keeps_more_and_never_less() {
        // **The direction that makes disagreement survivable.** A node behind on the record knows a
        // smaller census, and in a smaller field its own place can only improve — so it keeps a
        // superset of what it really owes. Being behind makes a node over-keep, which costs disk;
        // the other direction would drop things nobody knew were dropped.
        let everybody = nodes(30);
        let behind: Vec<&Name> = everybody.iter().take(18).collect();
        let whole = census(&everybody);
        let network = network();

        for thing in things(400) {
            let all = Drawn::at(&network, Epoch::new(0), &whole);
            let some = Drawn::at(&network, Epoch::new(0), &behind);
            let owed = some.holders(&thing, COPIES_OF_HISTORY);

            for node in all.holders(&thing, COPIES_OF_HISTORY) {
                if behind.contains(&node) {
                    assert!(
                        owed.contains(&node),
                        "a node behind must not think it owes less"
                    );
                }
            }
        }
    }

    #[test]
    fn things_do_not_all_move_on_the_same_hour() {
        // Otherwise the whole network moves nearly everything it holds on one hour every thirty
        // days, and during the scramble a busy node and an idle one look the same.
        let things = things(7_200);
        let mut carried = std::collections::BTreeMap::new();
        for thing in &things {
            let hour = moves_on(thing);
            assert!(hour < EPOCHS_PER_PERIOD);
            *carried.entry(hour).or_insert(0usize) += 1;
        }

        assert!(
            carried.len() > 700,
            "only {} of the month's hours are used",
            carried.len()
        );
        let busiest = carried.values().max().copied().unwrap_or_default();
        assert!(
            busiest * 100 < things.len(),
            "the busiest hour carries {busiest} of {}, which is not spreading it",
            things.len()
        );
    }

    #[test]
    fn a_thing_moves_exactly_once_every_thirty_days() {
        // Its own hour, and no oftener: the point of staggering is to spread the traffic, not to
        // rotate anybody more than the design says.
        let nodes = nodes(9);
        let thing = Name::of(b"a status list");
        let held = census(&nodes);

        let network = network();
        let hours = 3 * EPOCHS_PER_PERIOD;
        let mut moves = 0;
        let mut before = Drawn::at(&network, Epoch::new(0), &held).period_of(&thing);
        for hour in 1..=hours {
            let now = Drawn::at(&network, Epoch::new(hour), &held).period_of(&thing);
            if now != before {
                moves += 1;
                before = now;
            }
        }
        assert_eq!(
            moves, 3,
            "once per period, over three periods' worth of hours"
        );
    }

    #[test]
    fn the_share_moves_when_the_seed_does() {
        // What the rotation is for. If the same nodes held a thing for ever, an identity ground onto
        // it once would sit there for ever too.
        let nodes = nodes(40);
        let held = census(&nodes);
        let network = network();
        let mut moved = 0;

        for thing in things(200) {
            let before =
                Drawn::at(&network, Epoch::new(0), &held).holders(&thing, COPIES_OF_HISTORY);
            let after = Drawn::at(&network, Epoch::new(4 * EPOCHS_PER_PERIOD), &held)
                .holders(&thing, COPIES_OF_HISTORY);
            if before != after {
                moved += 1;
            }
        }
        assert!(moved > 150, "only {moved} of 200 moved across four periods");
    }

    #[test]
    fn a_shortfall_is_counted_against_who_serves_it_and_not_against_who_owes_it() {
        // Counted against the assignment it would be nought by construction — a number that says
        // everything is fine because it cannot say anything else.
        let nodes = nodes(9);
        let network = network();
        let held = census(&nodes);
        let drawn = Drawn::at(&network, Epoch::new(0), &held);

        assert_eq!(drawn.short_by(5, COPIES_OF_HISTORY), 0);
        assert_eq!(drawn.short_by(2, COPIES_OF_HISTORY), 3);
        assert_eq!(drawn.short_by(0, COPIES_OF_HISTORY), 5);
        assert_eq!(
            drawn.short_by(50, COPIES_OF_HISTORY),
            0,
            "more copies than asked for is not a negative shortfall"
        );
    }

    #[test]
    fn falling_to_a_node_is_the_same_question_as_being_among_its_holders() {
        let nodes = nodes(15);
        let network = network();
        let held = census(&nodes);
        let drawn = Drawn::at(&network, Epoch::new(900), &held);

        for thing in things(50) {
            let holders = drawn.holders(&thing, COPIES_OF_HISTORY);
            for node in &nodes {
                assert_eq!(
                    drawn.falls_to(&thing, node, COPIES_OF_HISTORY),
                    holders.contains(&node)
                );
            }
        }
    }

    #[test]
    fn a_seed_is_of_one_network_and_one_period_and_no_other_pair() {
        // Two pairs producing one seed would have two networks, or two moments, sharing a record
        // out identically — and the rotation would not rotate.
        let mine = Seed::of(&network(), Period::new(7));
        assert_ne!(mine, Seed::of(&network(), Period::new(8)));
        assert_ne!(mine, Seed::of(&Name::of(b"somewhere else"), Period::new(7)));
        assert_eq!(mine, Seed::of(&network(), Period::new(7)));
    }

    #[test]
    fn nothing_falls_to_anybody_when_there_is_nobody() {
        // A network with no node named in it yet. Nought copies asked for, and nought short —
        // there is nothing to be short of.
        let network = network();
        let drawn = Drawn::at(&network, Epoch::new(0), &[]);
        assert!(
            drawn
                .holders(&Name::of(b"a thing"), COPIES_OF_HISTORY)
                .is_empty()
        );
        assert_eq!(drawn.short_by(0, COPIES_OF_HISTORY), 0);
    }

    #[test]
    fn the_end_of_the_clock_does_not_wrap_a_share_round() {
        // A moment near the end of the numbers must not land a thing back in an early period.
        let nodes = nodes(5);
        let network = network();
        let held = census(&nodes);
        let drawn = Drawn::at(&network, Epoch::new(u64::MAX), &held);
        let thing = Name::of(b"a thing");
        assert!(drawn.period_of(&thing).number() > 0);
        assert_eq!(drawn.holders(&thing, COPIES_OF_HISTORY).len(), 5);
    }

    #[test]
    fn a_rotation_moves_nearly_everything_on_a_network_of_any_size() {
        // **The cost, measured rather than hoped for.** A fresh seed re-draws every placement
        // independently, so what survives a rotation is *copies over nodes* — half at ten nodes,
        // a fiftieth at five hundred. The traffic a rotation causes is therefore proportional to
        // everything the network holds, not to a slice of it, and it recurs every thirty days.
        //
        // Staggering spreads it across the month; it does not make it smaller. Anybody sizing a
        // node has to know that, and a number that drifts is a number nobody knows — so it is
        // pinned here.
        let network = network();
        let things = things(400);

        for (count, most) in [(10usize, 60u64), (50, 20), (200, 8)] {
            let nodes = nodes(count);
            let held = census(&nodes);
            let (mut kept, mut total) = (0usize, 0usize);

            for thing in &things {
                let before =
                    Drawn::at(&network, Epoch::new(0), &held).holders(thing, COPIES_OF_HISTORY);
                let after = Drawn::at(&network, Epoch::new(EPOCHS_PER_PERIOD), &held)
                    .holders(thing, COPIES_OF_HISTORY);
                total += before.len();
                kept += before.iter().filter(|node| after.contains(node)).count();
            }

            let percent = 100 * kept / total;
            assert!(
                percent as u64 <= most,
                "{count} nodes kept {percent}% across a rotation, which is more than five over {count}"
            );
        }
    }
}
