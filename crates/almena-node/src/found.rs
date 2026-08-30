//! Where this node has really reached the people it has reached, and why that is kept small.
//!
//! **This node's own observation, and it never goes into the record.** Where a node says it is, is
//! its own word in a place everybody holds; where this one found it is one node's experience, and
//! putting the second into the first would make somebody's experience everybody's truth.
//!
//! # It has to be bounded, and it was not
//!
//! One entry per key per address, kept for as long as the process runs, is a set that only grows:
//! an address a peer used once and left stays for ever, and a peer that rotates its addresses adds
//! one every time it does. Neither is an attack — both are ordinary — and a node that ran for a
//! year would be holding a year of addresses that answer nothing.
//!
//! And there is an attack under it. Nothing about this is authorised: a connection is enough to be
//! remembered, and a key costs nothing to make, so somebody who wanted to could dial once per key
//! for as long as they liked and this node would keep every one of them.
//!
//! So both halves are bounded, and both drop **the least recently seen**, which is the right end:
//! what this is for is having somewhere to reach a peer that answers now, and an address that has
//! not been seen in a while is the one least likely to be that.
//!
//! # Losing one costs nothing that cannot be recovered
//!
//! An address dropped here is one this node will observe again the next time it reaches that peer,
//! and until then the record still says where they claim to be. That is the difference between this
//! and everything in the log: forgetting a signed act would be losing history, and forgetting an
//! observation is forgetting where somebody was last seen.

use std::collections::BTreeMap;

/// How many addresses are remembered for one peer.
///
/// **Four, because that is roughly how many ways one machine is reachable**: an IPv4 address, an
/// IPv6 one, a circuit through a relay, and one spare for the moment a machine is moving between
/// two of those. A peer with more than four live addresses is unusual; a peer with a hundred is one
/// that has rotated ninety-six times.
const ADDRESSES_EACH: usize = 4;

/// How many peers are remembered at all.
///
/// **A thousand, which is far above the number of nodes any test of this network has run and far
/// below where the memory matters.** It is the ceiling on the other half of the growth: a key costs
/// nothing to make and a connection is all it takes to be remembered here, so without it whoever
/// wanted to could hand this node an unbounded set one dial at a time.
const PEERS_AT_MOST: usize = 1_024;

/// Where this node has reached people, most recently seen first.
///
/// Ordered rather than a set on purpose: what it drops is the least recently seen, and a set has no
/// opinion about which that is.
#[derive(Debug, Clone, Default)]
pub struct Found {
    /// Per peer, the addresses it was reached at, most recent first.
    at: BTreeMap<Vec<u8>, Vec<String>>,
    /// The peers, most recently seen first. Its head is what a new one pushes out of its tail.
    seen: Vec<Vec<u8>>,
}

impl Found {
    /// Nothing seen yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Take note of having actually reached somebody somewhere.
    ///
    /// By key, because that is what a connection proves somebody holds — the name it answers to is
    /// a separate question, and one the record may not be able to answer yet.
    pub fn reached(&mut self, key: &[u8], at: String) {
        let addresses = self.at.entry(key.to_vec()).or_default();
        addresses.retain(|held| held != &at);
        addresses.insert(0, at);
        addresses.truncate(ADDRESSES_EACH);

        self.seen.retain(|held| held != key);
        self.seen.insert(0, key.to_vec());
        // **The peer that goes is the one longest unseen**, and its addresses go with it: an entry
        // left behind by a peer nobody is tracking any more would be the growth this bounds,
        // wearing a different name.
        for gone in self.seen.split_off(PEERS_AT_MOST.min(self.seen.len())) {
            self.at.remove(&gone);
        }
    }

    /// Where this node has really reached whoever holds that key, most recent first.
    ///
    /// Empty is *this node has not reached them*, which is not *they are nowhere*.
    #[must_use]
    pub fn at(&self, key: &[u8]) -> Vec<String> {
        self.at.get(key).cloned().unwrap_or_default()
    }

    /// How many peers are being remembered, which is what the ceiling is on.
    #[must_use]
    pub fn peers(&self) -> usize {
        self.seen.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{ADDRESSES_EACH, Found, PEERS_AT_MOST};

    fn key(seed: u8) -> Vec<u8> {
        vec![seed; 32]
    }

    #[test]
    fn where_somebody_was_last_reached_comes_back_first() {
        // What this is for is having somewhere to reach a peer that answers *now*, so the order is
        // the answer and not a detail of the container.
        let mut found = Found::new();
        found.reached(&key(1), "/ip6/one/tcp/4001".to_owned());
        found.reached(&key(1), "/ip4/two/tcp/4001".to_owned());
        assert_eq!(
            found.at(&key(1)),
            vec![
                "/ip4/two/tcp/4001".to_owned(),
                "/ip6/one/tcp/4001".to_owned()
            ]
        );
    }

    #[test]
    fn reaching_somebody_at_an_address_again_moves_it_up_and_does_not_repeat_it() {
        let mut found = Found::new();
        for at in ["a", "b", "a"] {
            found.reached(&key(1), at.to_owned());
        }
        assert_eq!(found.at(&key(1)), vec!["a".to_owned(), "b".to_owned()]);
    }

    #[test]
    fn a_peer_that_rotates_its_addresses_does_not_grow_without_end() {
        // **The ordinary case that used to be unbounded**, and it is not an attack: a machine that
        // moves between networks writes one more address here every time it does.
        let mut found = Found::new();
        for at in 0..50 {
            found.reached(&key(1), format!("/ip4/{at}/tcp/4001"));
        }
        assert_eq!(found.at(&key(1)).len(), ADDRESSES_EACH);
        assert_eq!(
            found.at(&key(1))[0],
            "/ip4/49/tcp/4001",
            "and what it keeps is the most recent"
        );
    }

    #[test]
    fn a_key_costs_nothing_to_make_and_this_is_what_that_cannot_buy() {
        // Nothing here is authorised: a connection is all it takes to be remembered, so without a
        // ceiling somebody could hand this node an unbounded set one dial at a time.
        let mut found = Found::new();
        for seed in 0..(PEERS_AT_MOST + 500) {
            let mut made = seed.to_le_bytes().to_vec();
            made.resize(32, 0);
            found.reached(&made, "/ip4/one/tcp/4001".to_owned());
        }
        assert_eq!(found.peers(), PEERS_AT_MOST);

        // And the one pushed out is the one longest unseen, addresses and all — an entry left
        // behind by a peer nobody tracks any more would be the same growth under another name.
        let mut first = 0_usize.to_le_bytes().to_vec();
        first.resize(32, 0);
        assert!(found.at(&first).is_empty());
    }

    #[test]
    fn somebody_never_reached_is_not_somebody_who_is_nowhere() {
        let found = Found::new();
        assert!(found.at(&key(9)).is_empty());
    }
}
