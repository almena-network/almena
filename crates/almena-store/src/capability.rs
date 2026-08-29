//! What a node offers to do, and which version of the protocol it speaks.
//!
//! A node is one program with parts that can be switched on separately, and which parts are on is
//! not a detail of how it was installed: it is what the network has, and what somebody deciding
//! where to send a request needs to know. So it goes in the record, where anybody can count it —
//! not in a directory somebody keeps.
//!
//! # It is a closed list, and that is the whole point
//!
//! The mark that says *you cannot claim to have applied this act without understanding it* is on
//! **fields**, and it is blind to values: a field that ships on day one is one every reader knows,
//! so what grows there is the vocabulary and the mark never fires. A reader meeting a capability it
//! has never heard of would quietly drop it and count a node as offering less than it does — and
//! the count is the whole reason the field exists.
//!
//! So the list is **closed**: a value a reader has no meaning for stops it reading the act at all,
//! rather than being passed over. It keeps the act, passes it on, and declines to say what that node
//! is — which is the same answer it gives to anything else it cannot read, and the only one that is
//! not a quiet lie.
//!
//! # Why the first announcement carries none of this
//!
//! A node's first announcement is its creation and its name. What it offers and what version it
//! runs change over its life and its name must not, so they belong to the announcements that follow
//! — and that is why this is here and not in the act that names anything.

use almena_format::cbor::Value;
use almena_format::field::{Field, Vocabulary};

/// Something a node can be running.
///
/// **Four, and each is a separate thing to have and to measure.** A node that answers questions
/// about the record and one that carries messages for people whose phones are off are both nodes
/// and share almost nothing else; counting them as one number would say nothing about either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    /// Answering questions about the record, over the interface anybody may ask on.
    Interface,
    /// Holding messages for somebody whose device is not on.
    Mailbox,
    /// Keeping copies of the lists that say what has been withdrawn.
    Withdrawals,
    /// Carrying other nodes' traffic, so that one behind a household router is reachable.
    Relay,
}

impl Capability {
    /// Every one this build knows.
    pub const ALL: [Self; 4] = [
        Self::Interface,
        Self::Mailbox,
        Self::Withdrawals,
        Self::Relay,
    ];

    /// How it travels.
    ///
    /// Numbered and never renumbered, for the reason acts are: a number that changed meaning would
    /// make an old announcement say something its author never said. Zero is no capability at all.
    #[must_use]
    pub const fn number(self) -> u64 {
        match self {
            Self::Interface => 1,
            Self::Mailbox => 2,
            Self::Withdrawals => 3,
            Self::Relay => 4,
        }
    }

    /// The capability a number names, if this build knows it.
    #[must_use]
    pub const fn new(number: u64) -> Option<Self> {
        match number {
            1 => Some(Self::Interface),
            2 => Some(Self::Mailbox),
            3 => Some(Self::Withdrawals),
            4 => Some(Self::Relay),
            _ => None,
        }
    }
}

/// Where an announcement carries what the node offers.
///
/// Odd, and it has to be. A reader that passed over it would conclude the node offers **nothing** —
/// not that it does not know, but that there is nothing there — and would then count it that way.
/// That is the quiet wrong conclusion the mark exists to prevent, arriving through the field a
/// measurement is drawn from.
pub const OFFERS: u64 = 3;

/// Where an announcement carries which version of the protocol the node speaks.
///
/// Odd for the same reason and one more: what fraction of the network understands each version is
/// the figure that decides when something new may start being used, and a reader that skipped this
/// would leave that node out of the fraction while believing it had counted everybody.
pub const SPEAKS: u64 = 5;

/// Where an announcement carries the addresses the node can be reached at.
///
/// Odd, and for the plainest of the three reasons: an announcement is for saying **where and what**,
/// so a reader that passed over the where has not applied it. It would also count the node as
/// reachable nowhere, which is a claim and not an absence.
pub const WHERE: u64 = 7;

/// How far into an address the place goes.
///
/// An address says where something is and which door to knock on. **Only the first answers whether
/// two nodes are in the same place**, and that is the whole of what this is for — two doors on one
/// building are one building.
const PLACE: usize = 2;

/// The place an address is in, as far as anybody can tell from the address.
///
/// **It does not prove two nodes are apart, and nothing can.** What it does is let somebody asking
/// for independence ask for it, instead of assuming it: two roots from one place are one place's
/// word, whoever signed them, and that is worth being able to see.
#[must_use]
pub fn place(address: &str) -> Option<String> {
    let parts: Vec<&str> = address.trim_matches('/').split('/').collect();
    (parts.len() >= PLACE).then(|| format!("/{}", parts[..PLACE].join("/")))
}

/// What a reader of this build can make of an announcement.
///
/// The capabilities are a **closed** list here, so a value from a newer version stops the reader
/// rather than being passed over.
#[must_use]
pub fn vocabulary() -> Vocabulary<'static> {
    Vocabulary::with_closed(KNOWN, CLOSED)
}

/// The fields an announcement may carry that this build has a meaning for.
const KNOWN: &[Field] = &[
    Field::new(1),
    Field::new(OFFERS),
    Field::new(SPEAKS),
    Field::new(WHERE),
    // A node's own chain can split like anything else's, so it can be settled like anything else's.
    Field::new(crate::resolution::FIELD),
];

/// The capability numbers this build knows, as they appear inside the field.
const CLOSED: &[(Field, &[Value])] = &[(
    Field::new(OFFERS),
    &[
        Value::Uint(1),
        Value::Uint(2),
        Value::Uint(3),
        Value::Uint(4),
    ],
)];

#[cfg(test)]
mod tests {
    use super::{CLOSED, Capability, OFFERS, SPEAKS, vocabulary};
    use almena_format::cbor::Value;
    use almena_format::field::{Field, Unintelligible, understood};
    use std::collections::BTreeMap;

    #[test]
    fn the_numbers_run_from_one_with_no_gap_and_no_repeat() {
        let numbers: Vec<u64> = Capability::ALL.iter().map(|what| what.number()).collect();
        assert_eq!(numbers, (1..=4).collect::<Vec<u64>>());
        for what in Capability::ALL {
            assert_eq!(Capability::new(what.number()), Some(what));
        }
    }

    #[test]
    fn nothing_is_no_capability_at_all() {
        assert_eq!(Capability::new(0), None);
    }

    #[test]
    fn what_a_node_offers_is_a_field_a_reader_may_not_pass_over() {
        // A reader that skipped it would conclude the node offers **nothing** — not that it does
        // not know — and would then count it that way.
        assert!(Field::new(OFFERS).is_critical());
        assert!(Field::new(SPEAKS).is_critical());
    }

    #[test]
    fn a_capability_from_a_newer_version_stops_the_reader_rather_than_being_dropped() {
        // **The reason the list is closed.** The mark on a field is blind to values, and a field
        // that shipped on day one never fires it — so without this an old reader would pass over a
        // capability it had never heard of and count a node as offering less than it does.
        let newer = BTreeMap::from([(
            OFFERS,
            Value::Array(vec![Value::Uint(1), Value::Uint(9_999)]),
        )]);
        assert_eq!(
            understood(&newer, vocabulary()),
            Err(Unintelligible::Value(Field::new(OFFERS)))
        );
    }

    #[test]
    fn everything_this_build_knows_is_read_without_complaint() {
        let all = Value::Array(
            Capability::ALL
                .iter()
                .map(|what| Value::Uint(what.number()))
                .collect(),
        );
        let announced = BTreeMap::from([(OFFERS, all), (SPEAKS, Value::Uint(1))]);
        assert_eq!(understood(&announced, vocabulary()), Ok(()));
    }

    #[test]
    fn the_closed_list_is_the_list() {
        // Two places to say which capabilities exist would be two places to get it wrong, and the
        // one that decides what a reader accepts is this one.
        let (_, known) = CLOSED[0];
        let numbered: Vec<Value> = Capability::ALL
            .iter()
            .map(|what| Value::Uint(what.number()))
            .collect();
        assert_eq!(known, numbered.as_slice());
    }

    #[test]
    fn where_a_node_can_be_reached_is_a_field_a_reader_may_not_pass_over() {
        // An announcement is for saying where and what. A reader that passed over the where would
        // count the node as reachable nowhere, which is a claim and not an absence.
        assert!(Field::new(super::WHERE).is_critical());
    }

    #[test]
    fn two_doors_on_one_building_are_one_building() {
        // What the figure is for: two roots from one place are one place's word, whoever signed
        // them. It does not prove two nodes are apart — nothing can — but it lets somebody ask.
        assert_eq!(
            super::place("/ip4/198.51.100.7/tcp/4001"),
            super::place("/ip4/198.51.100.7/tcp/4002")
        );
        assert_ne!(
            super::place("/ip4/198.51.100.7/tcp/4001"),
            super::place("/ip4/198.51.100.8/tcp/4001")
        );
        assert_eq!(
            super::place("/ip4/198.51.100.7/tcp/4001"),
            Some("/ip4/198.51.100.7".to_owned())
        );
    }

    #[test]
    fn something_that_is_not_an_address_is_nowhere() {
        assert_eq!(super::place("/ip4"), None);
        assert_eq!(super::place(""), None);
    }
}
