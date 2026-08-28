//! The one act that opens a network, and the one a node most has to be stopped from doing twice.
//!
//! It does three things at once, and they belong together because none of them can be done twice
//! or undone:
//!
//! 1. **Opens the record**, with its first entry, fixing the instant epoch zero begins.
//! 2. **Declares which network this is** — inside the record, not only in the zone the node read
//!    it from. Where something was found is the weak proof, and two networks that get merged do
//!    not come apart again.
//! 3. **Creates Almena Government, self-signed**, which is what everything else is trusted
//!    against.
//!
//! # Its hash is what the network is called
//!
//! And that is what actually keeps two networks apart, because the label *production* or
//! *development* **does not tell one production network from another** created by accident: both
//! would say exactly the same word. A genesis hash does not repeat.
//!
//! It follows that **the network's identifier and Almena Government's name are the same hash** —
//! they come out of the same act, and everything here is named by the hash of its creation. There
//! is no ambiguity to it: a network is never resolved as an identity, and Almena Government never
//! appears in the name of a protocol. What it buys is that two networks have two different trust
//! anchors, which is right, because one network's anchor is nothing to the other.
//!
//! # A node refuses to open a network that already has seeds
//!
//! This is the only defence against the accident that costs the most: creating, by carelessness, a
//! second production network that nobody can tell from the first until it is far too late. If
//! there is somebody to join, a node joins. Opening only ever happens when there is nobody.

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, Signed, create};
use almena_suite::ed25519;
use almena_time::Epoch;

use crate::kind::Kind;

/// Where the network this genesis opens is declared.
///
/// Odd, and it is the clearest case there is: a reader that skipped it would be reading an act
/// that opens *a* network without knowing which, which is the confusion this field exists to make
/// impossible.
const NETWORK: u64 = 1;

/// Where the key Almena Government is created with sits.
const KEY: u64 = 3;

/// Where it carries the instant epoch zero begins, in seconds since the Unix epoch.
///
/// **Without it nobody can say what epoch it is.** An epoch is the count of whole hours since a
/// fixed instant, and that instant is fixed by this act — so a node that read the record and could
/// not find it would hold a history it cannot place in time, and two nodes that guessed would
/// disagree about when everything happened.
///
/// Odd, and there is nothing to weigh: an act that opened a network without saying when is not an
/// act anybody can use.
const BEGAN: u64 = 5;

/// What a network is called in the act that opens it.
///
/// Written out rather than left as a number so that somebody reading the bytes can see which one
/// they are looking at without a table.
const PRODUCTION: &str = "production";

/// The other one.
const DEVELOPMENT: &str = "development";

/// Why a node would not open a network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The zone already publishes seeds, so there is somebody to join.
    ///
    /// **A node joins when there is anybody to join.** Opening a second network beside an existing
    /// one produces two that say the same thing about themselves and cannot be told apart by
    /// anyone reading a label.
    ThereIsAlreadyANetwork(Vec<String>),
    /// This node has already opened or joined one. A node is a directory with a key in it, and a
    /// second genesis over the same directory would be a second history for one identity.
    ThisNodeAlreadyHasOne,
    /// An act built here to start the record was not accepted into it.
    ///
    /// **It must not be reachable**, since the acts in question are built a few lines away to be
    /// exactly what accepts them. It exists so that if it ever happens the node says so, instead
    /// of borrowing a reason that is not true and sending somebody looking for a network that is
    /// not there.
    TheRecordWouldNotStart,
}

/// A network that has just been opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    /// The act itself, ready to be written down as the first entry.
    pub operation: Operation,
    /// The instant epoch zero begins, in seconds since the Unix epoch.
    pub began: u64,
    /// What the network is called: the hash of that act.
    pub network: Name,
    /// Almena Government, which is the same hash wearing a method in front of it.
    pub government: Did,
}

/// What opening a network settles, all of which is settled once and never again.
///
/// Together rather than as separate arguments because they are one decision: which network this
/// is, and where its clock starts. Passing them apart would invite passing them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opening {
    /// Production or development.
    pub which: Which,
    /// The epoch it counts from, which for a network being opened is its first.
    pub beginning: Epoch,
    /// The instant that epoch begins, in seconds since the Unix epoch.
    pub began: u64,
}

/// Which network an act opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    /// The real one.
    Production,
    /// Development, which may be opened again as often as it needs to be.
    Development,
}

impl Which {
    /// What it is called in the bytes.
    const fn word(self) -> &'static str {
        match self {
            Self::Production => PRODUCTION,
            Self::Development => DEVELOPMENT,
        }
    }

    /// How identifiers on this network are written. Only development is marked, so that a test
    /// identifier somewhere it does not belong is the thing that stands out.
    pub(crate) const fn marking(self) -> Network {
        match self {
            Self::Production => Network::Production,
            Self::Development => Network::Development,
        }
    }
}

/// Open a network, if there is nobody to join and this node has not already got one.
///
/// `seeds` is what the zone publishes — whatever a node found when it looked for somebody to join.
/// It is passed in rather than read here so that the rule can be tested, and so that where the
/// answer comes from stays the business of whatever reads the zone.
///
/// `beginning` is the epoch this network counts from, which for a network being opened is its
/// first. Nothing is signed against a wall clock: the instant epoch zero begins is what the act
/// fixes, and every deadline afterwards is counted in epochs from it.
///
/// # Errors
///
/// [`Refused`], and both reasons are worth telling apart: one says somebody else is already here,
/// the other says you are.
pub fn open(
    opening: &Opening,
    seeds: &[String],
    already: bool,
    government: &ed25519::SigningKey,
) -> Result<Opened, Refused> {
    let Opening {
        which,
        beginning,
        began,
    } = *opening;

    if already {
        return Err(Refused::ThisNodeAlreadyHasOne);
    }
    if !seeds.is_empty() {
        return Err(Refused::ThereIsAlreadyANetwork(seeds.to_vec()));
    }

    let payload = BTreeMap::from([
        (NETWORK, Value::Text(which.word().to_owned())),
        (
            KEY,
            Value::Bytes(government.verifying_key().bytes().to_vec()),
        ),
        (BEGAN, Value::Uint(began)),
    ]);

    let mut operation = create(
        which.marking(),
        Kind::GENESIS.number(),
        1,
        beginning,
        payload,
    );

    // Self-signed, because there is nothing else it could be signed against: the anchor everything
    // is trusted from cannot itself be vouched for by something earlier.
    let signature = government.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: government.verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });

    let network = operation.object.name().clone();
    Ok(Opened {
        government: operation.object.clone(),
        network,
        began,
        operation,
    })
}

/// When a network's epoch zero began, if this act opened one.
///
/// It is what lets a node holding only the record work out what epoch it is, which is what every
/// deadline in the protocol is counted in.
#[must_use]
pub fn began(operation: &Operation) -> Option<u64> {
    if Kind::new(operation.kind) != Some(Kind::GENESIS) {
        return None;
    }
    match operation.payload.get(&BEGAN) {
        Some(&Value::Uint(seconds)) => Some(seconds),
        _ => None,
    }
}

/// Which network an act claims to open, if it is a genesis at all.
#[must_use]
pub fn declares(operation: &Operation) -> Option<Which> {
    if Kind::new(operation.kind) != Some(Kind::GENESIS) {
        return None;
    }
    match operation.payload.get(&NETWORK) {
        Some(Value::Text(word)) if word == PRODUCTION => Some(Which::Production),
        Some(Value::Text(word)) if word == DEVELOPMENT => Some(Which::Development),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Refused, Which, declares, open};
    use almena_suite::ed25519;
    use almena_time::Epoch;

    /// A fixed instant, so that a test is never about what time it is here.
    const WHEN: u64 = 1_800_000_000;

    fn at(which: Which) -> super::Opening {
        super::Opening {
            which,
            beginning: Epoch::GENESIS,
            began: WHEN,
        }
    }

    fn later(which: Which) -> super::Opening {
        super::Opening {
            began: WHEN + 3600,
            ..at(which)
        }
    }

    fn government() -> ed25519::SigningKey {
        ed25519::SigningKey::from_secret([5; 32])
    }

    fn opened(which: Which) -> super::Opened {
        open(&at(which), &[], false, &government()).expect("nobody to join")
    }

    #[test]
    fn opening_a_network_names_it_after_the_act_that_opened_it() {
        let network = opened(Which::Development);
        assert!(network.operation.names_itself());
        assert_eq!(network.network, *network.government.name());
    }

    #[test]
    fn the_network_and_almena_government_are_the_same_hash() {
        // Said out loud because somebody will trip over it: both come out of the same act, and
        // everything here is named by the hash of its creation. A network is never resolved as an
        // identity and the government never appears in a protocol name, so nothing is ambiguous.
        let network = opened(Which::Development);
        assert_eq!(network.government.name().as_str(), network.network.as_str());
        assert!(
            network
                .government
                .to_string()
                .starts_with("did:almena:dev:")
        );
    }

    #[test]
    fn two_networks_have_two_different_governments() {
        // Which is right: one network's trust anchor is nothing at all to the other.
        let development = opened(Which::Development);
        let production = opened(Which::Production);
        assert_ne!(development.network, production.network);
        assert_ne!(development.government, production.government);
    }

    #[test]
    fn the_label_alone_would_not_tell_two_production_networks_apart() {
        // The reason the hash is what identifies a network. Two production networks opened by
        // accident would say exactly the same word about themselves.
        let one = opened(Which::Production);
        let elsewhere = ed25519::SigningKey::from_secret([6; 32]);
        let other = open(&at(Which::Production), &[], false, &elsewhere).expect("nobody to join");

        assert_eq!(declares(&one.operation), declares(&other.operation));
        assert_ne!(
            one.network, other.network,
            "and yet they are not the same network"
        );
    }

    #[test]
    fn a_node_will_not_open_a_network_when_there_is_one_to_join() {
        // The only defence against the accident that costs the most.
        let seeds = vec!["/dns/madrid.example/tcp/443".to_owned()];
        assert_eq!(
            open(&at(Which::Production), &seeds, false, &government()),
            Err(Refused::ThereIsAlreadyANetwork(seeds))
        );
    }

    #[test]
    fn a_node_that_already_has_a_network_will_not_open_another() {
        // A node is a directory with a key in it, and a second genesis over the same directory
        // would be a second history for one identity.
        assert_eq!(
            open(&at(Which::Development), &[], true, &government()),
            Err(Refused::ThisNodeAlreadyHasOne)
        );
    }

    #[test]
    fn the_network_is_declared_inside_the_act_and_not_only_where_it_was_found() {
        // Where something was found is the weak proof, and two networks that get merged do not
        // come apart again.
        assert_eq!(
            declares(&opened(Which::Production).operation),
            Some(Which::Production)
        );
        assert_eq!(
            declares(&opened(Which::Development).operation),
            Some(Which::Development)
        );
    }

    #[test]
    fn an_act_that_is_not_a_genesis_declares_no_network() {
        let mut ordinary = opened(Which::Development).operation;
        ordinary.kind = crate::kind::Kind::HOLDER_CREATE.number();
        assert_eq!(declares(&ordinary), None);
    }

    #[test]
    fn opening_a_network_writes_its_first_entry_and_leaves_a_trust_anchor_that_resolves() {
        // The whole point of the act, end to end: a record with something in it, a government
        // somebody can actually ask about, and a root the node can publish for the epoch it
        // opened in — empty trees have roots, and this one is no longer empty.
        use crate::chain::{Admitted, Answer, Objects, State};
        use crate::log::Log;

        let network = opened(Which::Development);
        let mut objects = Objects::new();
        let mut log = Log::new();

        assert_eq!(
            objects.admit(&network.operation, Epoch::GENESIS),
            Ok(Admitted::Extended)
        );
        let entry = log.append(&network.operation, None);
        assert_eq!(entry.sequence, 0, "the genesis opens the record");
        assert_eq!(log.len(), 1);

        match objects.resolve(network.government.name()) {
            Answer::Here(State::Government { key }) => {
                assert_eq!(key, government().verifying_key().bytes());
            }
            other => panic!("the trust anchor has to resolve, got {other:?}"),
        }

        // And it is in the tree, provably where the node says it is.
        let (at, path) = log.inclusion(&entry.hash).expect("it is in there");
        assert!(crate::tree::included(
            &entry.to_bytes(),
            at as usize,
            log.len(),
            &path,
            &log.root()
        ));
    }

    #[test]
    fn a_second_genesis_is_a_second_network_and_not_a_second_history() {
        // Two genesis acts have different hashes, so they are two networks with two governments.
        // What must never happen is one node holding both, which is what `already` prevents.
        use crate::chain::{Admitted, Objects};

        let first = opened(Which::Development);
        let second = open(
            &at(Which::Development),
            &[],
            false,
            &ed25519::SigningKey::from_secret([8; 32]),
        )
        .expect("nobody to join");

        let mut objects = Objects::new();
        objects
            .admit(&first.operation, Epoch::GENESIS)
            .expect("admitted");
        assert_eq!(
            objects.admit(&second.operation, Epoch::GENESIS),
            Ok(Admitted::Extended),
            "as bytes they are simply two different objects"
        );
        assert_ne!(first.network, second.network);
    }

    #[test]
    fn a_network_says_when_its_first_epoch_began() {
        // Without it, a node holding the record could not work out what epoch it is — and every
        // deadline this protocol has is counted in epochs.
        let network = opened(Which::Development);
        assert_eq!(network.began, WHEN);
        assert_eq!(super::began(&network.operation), Some(WHEN));
    }

    #[test]
    fn an_act_that_is_not_a_genesis_says_when_nothing_began() {
        let mut ordinary = opened(Which::Development).operation;
        ordinary.kind = crate::kind::Kind::HOLDER_CREATE.number();
        assert_eq!(super::began(&ordinary), None);
    }

    #[test]
    fn two_networks_opened_at_different_moments_are_different_networks() {
        // The instant is inside the act, so it is inside the hash that names the network.
        let early =
            open(&at(Which::Development), &[], false, &government()).expect("nobody to join");
        let afterwards =
            open(&later(Which::Development), &[], false, &government()).expect("nobody to join");

        assert_ne!(early.network, afterwards.network);
        assert_ne!(early.began, afterwards.began);
    }

    #[test]
    fn the_genesis_is_signed_by_the_government_it_creates() {
        // Self-signed because there is nothing earlier for it to be signed against: the anchor
        // everything is trusted from cannot be vouched for by something before it.
        let network = opened(Which::Development);
        let signature = network.operation.signatures.first().expect("signed");
        assert_eq!(signature.key, government().verifying_key().bytes().to_vec());

        let verifying =
            ed25519::VerifyingKey::from_bytes(government().verifying_key().bytes()).expect("a key");
        let made = ed25519::Signature::from_bytes(signature.signature);
        assert_eq!(
            verifying.verify(&network.operation.signing_bytes(), &made),
            Ok(())
        );
    }
}
