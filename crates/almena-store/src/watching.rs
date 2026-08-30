//! What one node actually saw, asking by asking, which is what a day's summary is drawn from.
//!
//! A summary (`SPECS.md §5.1`) carries a day's figures and the **hash of the observations behind
//! them**. The observations stay off the record — fifty nodes watching each other every five
//! minutes would make the record almost entirely telemetry — and are served by whoever made them,
//! for as long as they keep them.
//!
//! # What the hash buys, and what it does not
//!
//! It does not make a summary impossible to fabricate: nobody signs an observation but the
//! observer, so an observer willing to invent a day's watching can invent the thing behind it just
//! as easily. What it does is **pin the observer to one account of what it saw** — having published
//! the hash it cannot later produce a different set of observations to justify the same figures,
//! and it cannot produce any at all if it never had them.
//!
//! **Which only works if the hash is over the observations and not over the summary.** A hash of
//! the figures being published commits to nothing: it checks out against the act carrying them
//! whatever they say, and an observer that watched nobody passes exactly as well as one that
//! watched everybody. That is what this module exists to make true rather than to claim — the
//! figures are **derived from this list** and the hash is over this list, so the two cannot come
//! apart.
//!
//! # Bounded, and it says how much it took
//!
//! A day's watching is unbounded in principle: nothing stops a node asking as often as it likes,
//! and nothing here is authorised. So a day takes [`AT_MOST`] askings and stops taking them, and
//! because the figures come from this same list, a day that filled up summarises what it recorded
//! rather than claiming a denominator it did not keep. `asked` is that denominator, which is why
//! `SPECS.md §5.1` publishes a fraction and never a percentage.

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::Did;
use almena_suite::digest::Digest;
use almena_time::Epoch;

use crate::summary::Seen;

/// How many askings one day of watching keeps.
///
/// **Eight thousand, which is a node asking every neighbour every few minutes all day and having
/// room left.** It is a ceiling on memory and on what a node offers to serve, not a target: a day
/// that reaches it summarises what it recorded, and the denominator it publishes is that count.
pub const AT_MOST: usize = 8_192;

/// What one node saw of another, once.
///
/// **An event and not a pairing.** A question going out and an answer coming back are two moments,
/// and an observer that had to join them before writing either down would be inventing the join —
/// which is what the totals already are. Written as events, the day's figures are counts over them
/// and nothing has to be reconstructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Saw {
    /// Something was asked of them.
    Asked,
    /// Something came back from them.
    Answered,
    /// They were seen to be this far behind, in acts.
    Behind(u64),
}

impl Saw {
    /// What it is called in the bytes, which is what the hash is over.
    const fn number(self) -> u64 {
        match self {
            Self::Asked => 1,
            Self::Answered => 2,
            Self::Behind(_) => 3,
        }
    }

    /// The figure it carries, where it carries one.
    const fn carrying(self) -> u64 {
        match self {
            Self::Asked | Self::Answered => 0,
            Self::Behind(by) => by,
        }
    }
}

/// One thing this node saw of one peer, at one moment.
///
/// **The unit an observation is really made of.** Totals are what a summary publishes; this is what
/// it is drawn from, and the difference is the whole of what the hash is worth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Noted {
    /// Whose key it was about.
    ///
    /// **The key and not the name.** What a connection proves somebody holds is a key; what the
    /// record calls them is a second question, answered by resolving it — and one the record may
    /// not be able to answer at the moment. Writing down the name would be writing down an answer
    /// that was not this node's to give.
    pub of: Vec<u8>,
    /// When.
    pub at: Epoch,
    /// What was seen.
    pub saw: Saw,
}

/// A day's worth of them, in the order they happened.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Watching {
    /// What was seen, oldest first.
    noted: Vec<Noted>,
}

impl Watching {
    /// A day nobody has watched yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Take note of one thing seen, if there is still room for it today.
    ///
    /// Returns whether it was kept. A day that has filled up is not an error and not something to
    /// hide: what it costs is a smaller denominator, and the denominator is published.
    pub fn wrote(&mut self, noted: Noted) -> bool {
        if self.noted.len() >= AT_MOST {
            return false;
        }
        self.noted.push(noted);
        true
    }

    /// How many things were kept.
    #[must_use]
    pub fn len(&self) -> usize {
        self.noted.len()
    }

    /// Whether nothing was watched at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.noted.is_empty()
    }

    /// The canonical bytes of the whole day, which are what is hashed and what is served.
    ///
    /// **The two have to be the same bytes.** A hash over one encoding and an answer in another
    /// would be a promise nobody could check, which is the state this replaces.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        Value::Array(
            self.noted
                .iter()
                .map(|noted| {
                    Value::Array(vec![
                        Value::Bytes(noted.of.clone()),
                        Value::Uint(noted.at.number()),
                        Value::Uint(noted.saw.number()),
                        Value::Uint(noted.saw.carrying()),
                    ])
                })
                .collect(),
        )
        .to_bytes()
    }

    /// The hash a summary carries.
    #[must_use]
    pub fn digest(&self) -> Digest {
        Digest::of(&self.to_bytes())
    }

    /// The day's figures, per node, counted over what was seen.
    ///
    /// `called` turns a peer's key into the name the record knows it by. **A peer it cannot name is left out**: a figure filed against no name is a figure about
    /// nobody, and one filed against a guess is worse.
    #[must_use]
    pub fn seen(&self, called: &dyn Fn(&[u8]) -> Option<Did>) -> BTreeMap<Did, Seen> {
        let mut totals: BTreeMap<Did, Seen> = BTreeMap::new();
        for noted in &self.noted {
            let Some(node) = called(&noted.of) else {
                continue;
            };
            let seen = totals.entry(node).or_default();
            match noted.saw {
                Saw::Asked => seen.asked += 1,
                Saw::Answered => seen.answered += 1,
                // **The furthest behind it was ever seen**, because a node that is up and behind is
                // worse than one that is down: whoever asks it gets an answer and cannot tell it is
                // stale, and a figure that averaged that away would say the opposite.
                Saw::Behind(by) => seen.behind = seen.behind.max(by),
            }
        }
        totals
    }
}

#[cfg(test)]
mod tests {
    use super::{AT_MOST, Noted, Saw, Watching};
    use almena_format::identifier::{Did, Name, Network};
    use almena_time::Epoch;

    fn node(seed: u8) -> Did {
        Did::new(Network::Development, Name::of(&[seed]))
    }

    fn noted(of: u8, saw: Saw) -> Noted {
        Noted {
            of: vec![of; 32],
            at: Epoch::GENESIS,
            saw,
        }
    }

    /// One key is node one, and nothing else is anybody.
    fn only_one(of: &[u8]) -> Option<Did> {
        (of == vec![1; 32]).then(|| node(1))
    }

    #[test]
    fn the_figures_are_counted_over_what_was_seen_and_not_kept_beside_it() {
        // **The whole point.** The hash is over these and the figures come out of them, so an
        // observer cannot publish one account and stand behind another.
        let mut watching = Watching::new();
        for saw in [
            Saw::Asked,
            Saw::Answered,
            Saw::Behind(3),
            Saw::Asked,
            Saw::Answered,
            Saw::Asked,
            Saw::Behind(11),
        ] {
            watching.wrote(noted(1, saw));
        }

        let seen = watching.seen(&only_one);
        let held = seen.get(&node(1)).expect("watched");
        assert_eq!(held.asked, 3);
        assert_eq!(held.answered, 2);
        assert_eq!(held.behind, 11, "the furthest it was ever seen behind");
    }

    #[test]
    fn an_observer_that_watched_nobody_cannot_pretend_otherwise() {
        // The failure the old mechanism could not catch: a hash over the published figures checks
        // out whatever they say. A hash over what was seen does not — an empty day has an empty
        // day's hash, and nothing produces it but an empty day.
        let nothing = Watching::new();
        let mut something = Watching::new();
        something.wrote(noted(1, Saw::Asked));
        assert_ne!(nothing.digest(), something.digest());
        assert!(nothing.seen(&only_one).is_empty());
    }

    #[test]
    fn asking_and_being_answered_are_two_moments_and_are_written_as_two() {
        // An observer that had to join them before writing either down would be inventing the
        // join, which is what a total already is. Written as events, nothing is reconstructed.
        let mut watching = Watching::new();
        watching.wrote(noted(1, Saw::Asked));
        assert_eq!(watching.len(), 1);
        let asked_only = watching.seen(&only_one);
        assert_eq!(asked_only.get(&node(1)).expect("watched").answered, 0);

        watching.wrote(noted(1, Saw::Answered));
        assert_eq!(
            watching
                .seen(&only_one)
                .get(&node(1))
                .expect("watched")
                .answered,
            1
        );
    }

    #[test]
    fn what_is_hashed_is_what_would_be_served() {
        // A hash over one encoding and an answer in another would be a promise nobody could check.
        let mut watching = Watching::new();
        watching.wrote(noted(1, Saw::Behind(2)));
        assert_eq!(
            watching.digest(),
            almena_suite::digest::Digest::of(&watching.to_bytes())
        );
    }

    #[test]
    fn a_peer_the_record_cannot_name_is_left_out_of_the_figures() {
        // A figure filed against no name is a figure about nobody, and one filed against a guess is
        // worse. It stays in what was seen, which is what this node really saw.
        let mut watching = Watching::new();
        watching.wrote(noted(1, Saw::Asked));
        watching.wrote(noted(2, Saw::Asked));

        assert_eq!(watching.len(), 2);
        let seen = watching.seen(&only_one);
        assert_eq!(seen.len(), 1);
        assert_eq!(seen.get(&node(1)).expect("watched").asked, 1);
    }

    #[test]
    fn a_day_that_fills_up_stops_taking_and_summarises_what_it_kept() {
        // Nothing here is authorised and nothing stops a node asking as often as it likes, so a day
        // is bounded. What it costs is a smaller denominator — and the denominator is published,
        // which is why a fraction is published and never a percentage.
        let mut watching = Watching::new();
        for _ in 0..AT_MOST {
            assert!(watching.wrote(noted(1, Saw::Asked)));
        }
        assert!(!watching.wrote(noted(1, Saw::Asked)));
        assert_eq!(watching.len(), AT_MOST);
        assert_eq!(
            watching
                .seen(&only_one)
                .get(&node(1))
                .expect("watched")
                .asked,
            AT_MOST as u64,
            "and the figures are of what was kept, because they come from the same list"
        );
    }
}
