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
//! stay off the record and are served by whoever made them, for as long as they keep them. So a
//! summary can be checked against what it was drawn from by anybody who cares enough to ask —
//! and a summary drawn from nothing at all cannot pretend otherwise, because the hash of nothing
//! is a hash somebody can compute.

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
pub fn read(operation: &Operation) -> Option<(Day, Digest, BTreeMap<Did, Seen>)> {
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
    ))
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
    use super::{Observer, Seen, publish, read, worth_writing};
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
            Digest::of(b"the observations"),
        );

        let (day, behind, back) = read(&written.operation).expect("a summary");
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
            Digest::of(b"the observations"),
        );

        let (_, _, back) = read(&written.operation).expect("a summary");
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
            Digest::of(b"observations"),
        );

        let (_, _, back) = read(&written.operation).expect("a summary");
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
            Digest::of(b"observations"),
        );
        written.operation.payload.remove(&3);
        assert!(read(&written.operation).is_none(), "it cites nothing");
    }
}
