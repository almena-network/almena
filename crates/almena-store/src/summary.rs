//! What one node saw of the others, once a day, in its own chain.
//!
//! **Nobody says anything about themselves.** A node's own account of its uptime is worth nothing;
//! what is worth something is that other nodes, who gain nothing by it, kept asking it questions
//! and wrote down whether it answered. So a summary is always about somebody else, and a node that
//! put itself in one would be doing the one thing this is designed to make unnecessary.
//!
//! # Why a day, and why this day
//!
//! Raw observations do not go in the record. Fifty nodes watching each other every five minutes is
//! two hundred and fifty million entries a year in a log nobody ever deletes — the record would be
//! almost entirely telemetry. A daily aggregate grows with the number of nodes rather than with how
//! often they look, so measuring more often costs nothing permanent.
//!
//! And the day is a **UTC day of twenty-four closed epochs**, never the machine's own midnight. Two
//! observers of one event who each summarised their own midnight would file it in different
//! windows, and comparing summaries is the only thing they are for.
//!
//! # The observations are behind it, not in it
//!
//! The act carries the **hash** of the observations it was drawn from. The observations themselves
//! stay off the record and are served by whoever made them, for as long as they keep them.
//!
//! **What that buys, said exactly, because it is less than it looks.** It does not make a summary
//! impossible to fabricate: nobody signs an observation but the observer, so an observer willing to
//! invent a day's watching can invent the thing behind it just as easily. What it does is **pin the
//! observer to one account of what it saw** — having published the hash, it cannot later produce a
//! different set of observations to justify the same figures, and it cannot produce any at all if
//! it never had them. That is worth having and it is not proof of anything.
//!
//! Which is also why the hash has to be over the observations and **not over the summary itself**.
//! A hash of the figures being published commits to nothing: checking it against the act it travels
//! in always succeeds, and an observer that watched nobody would pass exactly as well as one that
//! watched everybody.

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::Did;
use almena_format::operation::{Operation, Signed};
use almena_suite::digest::Digest;
use almena_suite::ed25519;
use almena_time::{Day, Epoch};

use crate::kind::Kind;

/// Where the day being summarised sits.
///
/// Odd: a summary without one covers no window, and two summaries covering no window cannot be
/// compared with each other or with anything else.
const DAY: u64 = 1;

/// Where the hash of the observations behind it sits.
///
/// Odd. Without it a summary is an assertion instead of a claim anybody can go and check.
const BEHIND: u64 = 3;

/// Where what was seen of each node sits.
const SEEN: u64 = 5;

/// Where what the observer went looking for sits.
///
/// Even, so it may be ignored: a summary without it went looking for nothing, which is a true and
/// complete statement. A reader that skips it has misread nothing — unlike the day or the hash,
/// where skipping would leave a claim about a window nobody could name.
const LOOKED: u64 = 6;

/// What one observer saw of one node over a day.
///
/// Small on purpose. Every figure here is something the observer did itself and can be held to —
/// not a judgement about the node, and nothing that needs the node's cooperation to establish.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Seen {
    /// How many times it was asked something.
    pub asked: u64,
    /// How many of those it answered.
    ///
    /// **Availability, as somebody else measured it.** Not a percentage: a fraction of a known
    /// denominator, so that a node asked twice and a node asked ten thousand times do not look
    /// alike.
    pub answered: u64,
    /// The furthest behind the observer ever saw it, in acts.
    ///
    /// **A node that is up and behind is worse than one that is down**, because whoever asks it
    /// gets an answer and has no way to tell it is stale. Down is visible; behind is not, unless
    /// somebody says so.
    pub behind: u64,
}

/// What an observer went looking for over a day, and how much of it it found.
///
/// **About the looking, and never about anybody looked at.** Which things fall to which node is
/// worked out from a census, and an observer behind on the record has a smaller one — so a miss
/// filed against a node would be a figure about the observer's own position wearing somebody else's
/// name. Measured against a half-read record, half of those misses land on nodes the thing never
/// fell to, who did nothing and can prove nothing.
///
/// Filed against the observer it is what it is: how much of what it went looking for it found.
/// Looking in the wrong place costs the observer its own denominator and costs nobody else
/// anything — which is the same direction the share-out itself leans, where being behind costs a
/// node its own disk.
///
/// **And there is deliberately no column here naming who was short.** Per node, per day, in a
/// record nobody deletes, that would be a ranked list of where to attack, assembled by the network
/// about itself. Whether one node is short is a question anybody can answer on the day they care,
/// by asking; it is not a thing to write down for ever.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Looked {
    /// How many copies of things it asked somebody to hand over.
    pub asked_for: u64,
    /// How many came back and were what they were said to be.
    pub found: u64,
}

/// Who is doing the observing, and what they are adding it to.
///
/// The three travel together because they are one thing: a summary is an entry in one node's own
/// account, so it needs the name that account is under, where that account had got to, and the key
/// that speaks for it. Any two without the third would be a summary belonging to nobody.
///
/// It does not print. A signing key has no `Debug` and should not get one: a secret that can be
/// formatted is a secret that ends up in a log somebody forgot they were writing.
#[derive(Clone, Copy)]
pub struct Observer<'a> {
    /// The node doing the observing, as the record names it.
    pub observer: &'a Did,
    /// Where its chain had got to.
    pub head: &'a almena_format::identifier::Name,
    /// The key that speaks for it.
    pub by: &'a ed25519::SigningKey,
}

/// A summary written down, and the name it will be known by.
#[derive(Debug, Clone)]
pub struct Written {
    /// The act to be admitted.
    pub operation: Operation,
    /// What it is called from now on.
    pub named: Did,
}

/// Write down what an observer saw of others over a day.
///
/// `observations` is the hash of what it was drawn from, which stays off the record.
///
/// **Anything the observer says about itself is left out**, silently and by construction: a node's
/// own account of its uptime is exactly what cross-observation exists to replace.
#[must_use]
pub fn publish(
    who: Observer<'_>,
    day: Day,
    seen: &BTreeMap<Did, Seen>,
    looked: Looked,
    observations: Digest,
) -> Written {
    let Observer { observer, head, by } = who;
    let watched = seen
        .iter()
        .filter(|(node, _)| *node != observer)
        .map(|(node, seen)| {
            Value::Array(vec![
                Value::Text(node.to_string()),
                Value::Uint(seen.asked),
                Value::Uint(seen.answered),
                Value::Uint(seen.behind),
            ])
        })
        .collect();

    let payload = BTreeMap::from([
        (DAY, Value::Uint(day.number())),
        (BEHIND, Value::Bytes(observations.bytes().to_vec())),
        (SEEN, Value::Array(watched)),
        (
            LOOKED,
            Value::Array(vec![
                Value::Uint(looked.asked_for),
                Value::Uint(looked.found),
            ]),
        ),
    ]);

    // **On the observer's own chain**, not as an object of its own. What a node saw is part of
    // what that node has said, and putting it anywhere else would make it somebody's free-floating
    // opinion rather than an entry in the account they are answerable for.
    let mut operation = Operation {
        object: observer.clone(),
        previous: Some(head.clone()),
        kind: Kind::NODE_SUMMARY.number(),
        version: 1,
        issued: day.begins(),
        payload,
        signatures: Vec::new(),
    };
    let signature = by.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: by.verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });

    let named = operation.object.clone();
    Written { operation, named }
}

/// What a summary says, if it is one.
///
/// [`None`] for anything that is not: a different kind of act, or one missing the day or the hash
/// of what it was drawn from. A summary that covers no window and cites nothing is not a weaker
/// summary — it is not one.
#[must_use]
pub fn read(operation: &Operation) -> Option<(Day, Digest, BTreeMap<Did, Seen>, Looked)> {
    if Kind::new(operation.kind) != Some(Kind::NODE_SUMMARY) {
        return None;
    }
    let (Some(&Value::Uint(day)), Some(Value::Bytes(behind))) =
        (operation.payload.get(&DAY), operation.payload.get(&BEHIND))
    else {
        return None;
    };

    let mut seen = BTreeMap::new();
    if let Some(Value::Array(watched)) = operation.payload.get(&SEEN) {
        for one in watched {
            let Value::Array(parts) = one else {
                return None;
            };
            let [
                Value::Text(node),
                Value::Uint(asked),
                Value::Uint(answered),
                Value::Uint(behind),
            ] = parts.as_slice()
            else {
                return None;
            };
            // A node it cannot name is a figure about nobody, and keeping it would mean a summary
            // that adds up to more than it says.
            let named = Did::parse(node).ok()?;
            seen.insert(
                named,
                Seen {
                    asked: *asked,
                    answered: *answered,
                    behind: *behind,
                },
            );
        }
    }

    Some((
        Day::new(day),
        Digest::from_bytes(behind.as_slice().try_into().ok()?),
        seen,
        looked(operation)?,
    ))
}

/// What the observer went looking for, as the act records it.
///
/// Absent is *it went looking for nothing* — which is what a summary from before anybody looked
/// says, and what a summary written by a build that had no such field says. Neither is an error,
/// and reading absence as nought is the only reading that says the same thing about both.
///
/// [`None`] only for a field that is there and is not a pair, which is a summary claiming to have
/// looked in a way nobody can read.
fn looked(operation: &Operation) -> Option<Looked> {
    match operation.payload.get(&LOOKED) {
        None => Some(Looked::default()),
        Some(Value::Array(pair)) => match pair.as_slice() {
            [Value::Uint(asked_for), Value::Uint(found)] => Some(Looked {
                asked_for: *asked_for,
                found: *found,
            }),
            _ => None,
        },
        Some(_) => None,
    }
}

/// What the whole network went looking for on a day, and how much of it it found.
///
/// **A fraction over a stated denominator, and never a percentage.** A network that asked twice and
/// a network that asked ten thousand times must not look alike, and the number of observers behind
/// it is part of the figure rather than a footnote: one observer's day is one node's word.
///
/// **Nought asked for is *nobody looked*, and not *everything is well*.** The two are the easiest
/// figures in this design to confuse, and confusing them would report health nobody measured — so
/// the denominator is here to be read, not divided away.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Kept {
    /// How many copies of things were asked for, across every observer that wrote a summary.
    pub asked_for: u64,
    /// How many came back and were what they were said to be.
    pub found: u64,
    /// How many observers the figure is drawn from.
    pub observers: usize,
}

/// Add up what every observer said about one day.
///
/// **Anybody can do this**, from summaries anybody can get: it is a sum over signed acts in a record
/// everyone holds, not an assertion by whoever runs anything. That is what makes the shortfall a
/// figure a third party can arrive at rather than a number somebody publishes about themselves.
///
/// Acts that are not summaries, or are for another day, are skipped. **One summary per observer**:
/// a second for the same day by the same author is not counted, because an observer that wrote two
/// accounts of one window has said two things and adding both would let it weigh twice.
#[must_use]
pub fn kept(day: Day, summaries: &[&Operation]) -> Kept {
    let mut counted: BTreeMap<Did, Looked> = BTreeMap::new();
    for act in summaries {
        let Some((said, _, _, looked)) = read(act) else {
            continue;
        };
        if said != day {
            continue;
        }
        counted.entry(act.object.clone()).or_insert(looked);
    }

    Kept {
        asked_for: counted.values().map(|looked| looked.asked_for).sum(),
        found: counted.values().map(|looked| looked.found).sum(),
        observers: counted.len(),
    }
}

/// Whether this summary is one an observer may write today.
///
/// A day still happening cannot be summarised, and neither can a node summarise itself. Both are
/// refusals rather than corrections: what a summary is for is being compared, and one drawn over
/// half a window or about its own author compares with nothing.
#[must_use]
pub fn worth_writing(observer: &Did, day: Day, seen: &BTreeMap<Did, Seen>, now: Epoch) -> bool {
    day.over(now) && seen.keys().any(|node| node != observer)
}

#[cfg(test)]
mod tests {
    use super::{Looked, Observer, Seen, publish, read, worth_writing};
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_suite::digest::Digest;
    use almena_suite::ed25519;
    use almena_time::{Day, Epoch};
    use std::collections::BTreeMap;

    fn key(seed: u8) -> ed25519::SigningKey {
        ed25519::SigningKey::from_secret([seed; 32])
    }

    fn node(seed: u8) -> Did {
        Did::new(Network::Development, Name::of(&[seed]))
    }

    fn seen(asked: u64, answered: u64, behind: u64) -> Seen {
        Seen {
            asked,
            answered,
            behind,
        }
    }

    #[test]
    fn what_was_seen_survives_the_wire() {
        let watched = BTreeMap::from([(node(4), seen(100, 97, 3)), (node(5), seen(100, 100, 0))]);
        let written = publish(
            Observer {
                observer: &node(3),
                head: &Name::of(b"the act before it"),
                by: &key(3),
            },
            Day::new(9),
            &watched,
            Looked::default(),
            Digest::of(b"the observations"),
        );

        let (day, behind, back, _) = read(&written.operation).expect("a summary");
        assert_eq!(day, Day::new(9));
        assert_eq!(behind, Digest::of(b"the observations"));
        assert_eq!(back, watched);
    }

    #[test]
    fn nobody_says_anything_about_themselves() {
        // **The whole reason cross-observation exists.** A node's own account of its uptime is
        // worth nothing, and putting it in would be doing the thing this replaces.
        let watched = BTreeMap::from([(node(3), seen(100, 100, 0)), (node(4), seen(100, 97, 3))]);
        let written = publish(
            Observer {
                observer: &node(3),
                head: &Name::of(b"the act before it"),
                by: &key(3),
            },
            Day::new(9),
            &watched,
            Looked::default(),
            Digest::of(b"the observations"),
        );

        let (_, _, back, _) = read(&written.operation).expect("a summary");
        assert_eq!(back.len(), 1);
        assert!(!back.contains_key(&node(3)), "and it left itself out");
    }

    #[test]
    fn availability_is_a_fraction_and_not_a_percentage() {
        // A node asked twice and a node asked ten thousand times must not look alike.
        let watched =
            BTreeMap::from([(node(4), seen(2, 2, 0)), (node(5), seen(10_000, 10_000, 0))]);
        let written = publish(
            Observer {
                observer: &node(3),
                head: &Name::of(b"the act before it"),
                by: &key(3),
            },
            Day::new(1),
            &watched,
            Looked::default(),
            Digest::of(b"observations"),
        );

        let (_, _, back, _) = read(&written.operation).expect("a summary");
        assert_eq!(back[&node(4)].asked, 2);
        assert_eq!(back[&node(5)].asked, 10_000);
    }

    #[test]
    fn a_day_still_happening_is_not_summarised() {
        // A summary drawn over half a window compares with nothing, which is the one thing a
        // summary is for.
        let watched = BTreeMap::from([(node(4), seen(1, 1, 0))]);

        assert!(!worth_writing(
            &node(3),
            Day::new(0),
            &watched,
            Epoch::new(23)
        ));
        assert!(worth_writing(
            &node(3),
            Day::new(0),
            &watched,
            Epoch::new(24)
        ));
    }

    #[test]
    fn a_summary_about_nobody_but_its_own_author_is_not_worth_writing() {
        let only_itself = BTreeMap::from([(node(3), seen(1, 1, 0))]);
        assert!(!worth_writing(
            &node(3),
            Day::new(0),
            &only_itself,
            Epoch::new(24)
        ));
    }

    #[test]
    fn something_that_is_not_a_summary_is_not_read_as_one() {
        let mut written = publish(
            Observer {
                observer: &node(3),
                head: &Name::of(b"the act before it"),
                by: &key(3),
            },
            Day::new(1),
            &BTreeMap::from([(node(4), seen(1, 1, 0))]),
            Looked::default(),
            Digest::of(b"observations"),
        );
        written.operation.payload.remove(&3);
        assert!(read(&written.operation).is_none(), "it cites nothing");
    }

    #[test]
    fn what_an_observer_went_looking_for_survives_the_wire() {
        let written = publish(
            Observer {
                observer: &node(3),
                head: &Name::of(b"the act before it"),
                by: &key(3),
            },
            Day::new(9),
            &BTreeMap::from([(node(4), seen(1, 1, 0))]),
            Looked {
                asked_for: 40,
                found: 37,
            },
            Digest::of(b"the observations"),
        );

        let (_, _, _, looked) = read(&written.operation).expect("a summary");
        assert_eq!(looked.asked_for, 40);
        assert_eq!(looked.found, 37);
    }

    #[test]
    fn a_summary_names_nobody_for_being_short() {
        // **What must not be here.** Which things fall to which node comes from a census, and an
        // observer behind on the record has a smaller one — so a miss filed against a node would be
        // a figure about the observer's own position wearing somebody else's name. Per node, per
        // day, in a record nobody deletes, it would also be a ranked list of where to attack.
        let written = publish(
            Observer {
                observer: &node(3),
                head: &Name::of(b"the act before it"),
                by: &key(3),
            },
            Day::new(9),
            &BTreeMap::from([(node(4), seen(100, 90, 0)), (node(5), seen(100, 100, 0))]),
            Looked {
                asked_for: 40,
                found: 3,
            },
            Digest::of(b"the observations"),
        );

        let (_, _, watched, looked) = read(&written.operation).expect("a summary");
        assert_eq!(
            looked.found, 3,
            "it found almost nothing it went looking for"
        );
        for (named, seen) in &watched {
            assert!(
                seen.asked >= seen.answered,
                "{named} answered more than it was asked"
            );
        }
        // And nothing in what it says about anybody carries where a thing was missing.
        assert_eq!(watched.len(), 2);
    }

    #[test]
    fn a_summary_from_before_anybody_looked_says_it_looked_for_nothing() {
        // A build with no such field and an observer that looked at nothing say the same thing, and
        // reading absence as nought is the only reading that says the same thing about both.
        let mut written = publish(
            Observer {
                observer: &node(3),
                head: &Name::of(b"the act before it"),
                by: &key(3),
            },
            Day::new(1),
            &BTreeMap::from([(node(4), seen(1, 1, 0))]),
            Looked::default(),
            Digest::of(b"observations"),
        );
        written.operation.payload.remove(&6);

        let (_, _, _, looked) = read(&written.operation).expect("a summary");
        assert_eq!(looked, Looked::default());
    }

    #[test]
    fn a_summary_that_claims_to_have_looked_in_a_way_nobody_can_read_is_not_one() {
        let mut written = publish(
            Observer {
                observer: &node(3),
                head: &Name::of(b"the act before it"),
                by: &key(3),
            },
            Day::new(1),
            &BTreeMap::from([(node(4), seen(1, 1, 0))]),
            Looked::default(),
            Digest::of(b"observations"),
        );
        written
            .operation
            .payload
            .insert(6, Value::Text("as much as I felt like".to_owned()));

        assert!(read(&written.operation).is_none());
    }

    /// A summary by that observer, saying what it went looking for.
    fn said(seed: u8, day: u64, looked: Looked) -> almena_format::operation::Operation {
        publish(
            Observer {
                observer: &node(seed),
                head: &Name::of(b"whatever came before"),
                by: &key(seed),
            },
            Day::new(day),
            &BTreeMap::from([(node(200), seen(1, 1, 0))]),
            looked,
            Digest::of(b"the observations"),
        )
        .operation
    }

    fn looking(asked_for: u64, found: u64) -> Looked {
        Looked { asked_for, found }
    }

    #[test]
    fn what_the_network_went_looking_for_is_the_sum_of_what_its_observers_did() {
        let acts = [
            said(3, 9, looking(40, 38)),
            said(4, 9, looking(30, 30)),
            said(5, 9, looking(10, 2)),
        ];
        let held: Vec<&almena_format::operation::Operation> = acts.iter().collect();

        let kept = super::kept(Day::new(9), &held);
        assert_eq!(kept.asked_for, 80);
        assert_eq!(kept.found, 70);
        assert_eq!(kept.observers, 3, "and how many it is drawn from");
    }

    #[test]
    fn one_observer_weighs_once_however_many_times_it_wrote() {
        // An observer that wrote two accounts of one window has said two things, and adding both
        // would let it weigh twice.
        let acts = [
            said(3, 9, looking(40, 38)),
            said(3, 9, looking(1_000_000, 1_000_000)),
        ];
        let held: Vec<&almena_format::operation::Operation> = acts.iter().collect();

        let kept = super::kept(Day::new(9), &held);
        assert_eq!(kept.observers, 1);
        assert_eq!(kept.asked_for, 40);
    }

    #[test]
    fn another_day_is_another_figure() {
        let acts = [said(3, 9, looking(40, 38)), said(4, 10, looking(30, 30))];
        let held: Vec<&almena_format::operation::Operation> = acts.iter().collect();

        assert_eq!(super::kept(Day::new(9), &held).asked_for, 40);
        assert_eq!(super::kept(Day::new(10), &held).asked_for, 30);
    }

    #[test]
    fn nobody_having_looked_is_not_everything_being_well() {
        // **The easiest two figures in this design to confuse.** Nought found out of nought asked
        // for is an absence of evidence, and reading it as health would report a network in good
        // order on the strength of nobody having checked.
        let nothing = super::kept(Day::new(9), &[]);
        assert_eq!(nothing.asked_for, 0);
        assert_eq!(nothing.observers, 0, "and it says so, rather than dividing");

        let acts = [said(3, 9, looking(0, 0))];
        let held: Vec<&almena_format::operation::Operation> = acts.iter().collect();
        let looked_at_nothing = super::kept(Day::new(9), &held);
        assert_eq!(looked_at_nothing.observers, 1, "somebody wrote a summary");
        assert_eq!(
            looked_at_nothing.asked_for, 0,
            "and went looking for nothing in it"
        );
    }

    #[test]
    fn something_that_is_not_a_summary_adds_nothing() {
        let mut not_one = said(3, 9, looking(40, 38));
        not_one.kind = crate::kind::Kind::HOLDER_CREATE.number();
        let held = vec![&not_one];

        assert_eq!(super::kept(Day::new(9), &held), super::Kept::default());
    }
}
