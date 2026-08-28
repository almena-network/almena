//! What can be asked of a node, and what comes back.
//!
//! **Nothing here knows there is a network.** No socket, no status code, no header, no stream: a
//! question arrives as a method and a path, an answer leaves as bytes, and whatever carried them
//! is somebody else's problem. That is not tidiness — it is what stops the transport from
//! deciding anything. A transport that could decide would be a second place where the node's
//! behaviour lives, and the two would drift.
//!
//! # The four rules this obeys, and where each one shows up
//!
//! | | Where it is visible here |
//! |---|---|
//! | **Reading is not authenticated** | There is no caller in any signature. Nothing can be denied on the basis of who is asking, because nothing knows |
//! | **Writing is handing over a signed act** | [`deliver`] takes bytes and nothing else. The signature is the authorisation, so there is no session to be in and none to be thrown out of |
//! | **What comes back is the author's own bytes** | An act is handed back exactly as it arrived, carried as a byte string. Nothing is re-encoded and nothing is signed by the node |
//! | **Limits are self-protection, not access control** | A throttle is a state with a number, said out loud, and [`Ask::Limits`] publishes what the limits are so that *what a node said* and *what it did* are two facts anybody can compare |
//!
//! # Everything said carries the epoch and root it was true at
//!
//! Every response, including a refusal and including a throttle. Two answers that do not say what
//! they were computed against cannot be compared, and a node that stamps its successes but not its
//! refusals is a node whose refusals cannot be audited — which is precisely the ones worth
//! auditing.
//!
//! # Errors are states, never prose
//!
//! What comes back is a number from one closed vocabulary, and the sentence a person reads is
//! drawn wherever the reading happens. Two operators running the node in different languages
//! compare the same number.

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name};
use almena_node::{Answered, Node};
use almena_store::chain::{Answer, Reason, Refused};
use almena_time::Epoch;

/// Where each part of a response sits.
mod field {
    /// The epoch it was answered in.
    pub const EPOCH: u64 = 1;
    /// The root over everything the node had written down.
    pub const ROOT: u64 = 2;
    /// What happened, from one closed vocabulary.
    pub const STATE: u64 = 3;
    /// What was asked for, when there is anything.
    pub const PAYLOAD: u64 = 4;
    /// Which rule, when the state is one that has rules.
    pub const WHICH: u64 = 5;
}

/// What a proof of inclusion is made of.
///
/// All three odd: a reader that skipped any of them would hold a path with no position to count it
/// from, no size to count it against, or no signature to hold anybody to.
mod proving {
    /// Where in that node's record the act sits.
    pub const AT: u64 = 1;
    /// The hashes that carry it up to the root.
    pub const PATH: u64 = 3;
    /// The signed root it is a proof against.
    pub const ROOT: u64 = 5;
}

/// What a published root is made of, when one is asked for.
///
/// All three odd, and none of them weighable: a reader that skipped any one of them would hold a
/// root it cannot check, which is the same as holding bytes somebody handed it.
mod published {
    /// The root's own bytes, which are what was signed.
    pub const ROOT: u64 = 1;
    /// The key that signed it, which is the node's.
    pub const KEY: u64 = 3;
    /// The signature over the root's bytes.
    pub const SIGNATURE: u64 = 5;
}

/// What a node says happened. One vocabulary for everything, and a number rather than a sentence.
///
/// A number is what an operator pastes into a support channel and somebody else searches for; two
/// people running the node in different languages could not compare a translated phrase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Here it is.
    Here = 1,
    /// No creation with that name has been seen.
    DoesNotExist = 2,
    /// It exists and this node will not say what it is. `WHICH` says why.
    CannotResolve = 3,
    /// It exists and is held elsewhere. Not an absence: one more query.
    NotHere = 4,
    /// There is nothing at that path. About the request, not about the object.
    NoSuchQuestion = 5,
    /// The request could not be read.
    Malformed = 6,
    /// A limit was reached. Self-protection, and the node says which limit.
    Throttled = 7,
    /// An act was handed over and not taken. `WHICH` names the rule it broke.
    NotTaken = 8,
    /// An act was handed over and written down.
    Taken = 9,
    /// The question cannot be asked of this build yet, which is not the same as its answer being
    /// empty. Nothing has looked, so nothing may be reported as having found nothing.
    NotYetAskable = 10,
}

/// Why a node will not resolve something, as a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// Two acts claim the same predecessor.
    Forked = 1,
    /// Its history contains an act this build does not know.
    Unintelligible = 2,
}

/// What this node will and will not do, published so that saying and doing can be compared.
///
/// **A limit is self-protection and censorship is not**, and the difference has to be checkable by
/// somebody outside rather than asserted by the operator. Publishing the numbers is half of it:
/// what a node said and what it did become two facts a third party can hold up against each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// How many questions one connection may ask in a window.
    pub per_connection: u64,
    /// How long that window is, in seconds.
    pub window: u64,
    /// The largest act this node will read.
    pub largest_act: u64,
    /// How many connections it will hold at once, across everybody.
    ///
    /// **The one limit that is not per-connection**, and it is here because the others cannot
    /// bound anything: connections times streams times bytes has no ceiling. It also means a fresh
    /// connection can fail — so *try again from another connection* is not on its own a test for
    /// censorship, and this number is what lets somebody tell the two apart instead.
    pub connections: u64,
}

/// A question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// What an object is now.
    Object(Name),
    /// One act, by its hash.
    Act(Name),
    /// What has been said about somebody by somebody else.
    About(Did),
    /// Where an act sits in this node's tree, and the path that proves it.
    /// Where a node wrote an act down, proved against a root it signed.
    ///
    /// **The epoch is not optional.** A path proves an entry against a root of a stated size, and
    /// the only roots with a node's name on them are the ones it published at the ends of epochs —
    /// so a proof without one named is a proof against nothing.
    Inclusion(Epoch, Name),
    /// What this node said about an epoch.
    Root(Epoch),
    /// What this node will and will not do.
    Limits,
}

/// A response: what happened, and the bytes that say so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    /// What happened, so that a transport can map it to whatever it maps things to **without
    /// computing anything**. Deciding here is the whole point; the transport writes it down.
    pub state: State,
    /// The canonical bytes of the response.
    pub body: Vec<u8>,
}

/// Why a question could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unreadable {
    /// Nothing is served at that path.
    NoSuchQuestion,
    /// The right path, with something unusable in it.
    Malformed,
}

/// Read a question.
///
/// The vocabulary is small and fixed. Nothing about it grows without a version, which is why a
/// router earns nothing here that a list of names does not.
///
/// # Errors
///
/// [`Unreadable`], telling apart *there is nothing at that path* from *the path is right and what
/// is in it is not*.
pub fn parse(method: &str, path: &str) -> Result<Ask, Unreadable> {
    if method != "GET" {
        return Err(Unreadable::NoSuchQuestion);
    }
    let mut parts = path.trim_matches('/').split('/');
    let (Some(what), rest, more, None) = (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(Unreadable::NoSuchQuestion);
    };

    match (what, rest, more) {
        ("limits", None, None) => Ok(Ask::Limits),
        ("object", Some(name), None) => named(name).map(Ask::Object),
        ("act", Some(name), None) => named(name).map(Ask::Act),
        // Two segments and both required: a proof is against a root, and the epoch is which root.
        ("inclusion", Some(epoch), Some(name)) => {
            let at = numbered(epoch)?;
            named(name).map(|name| Ask::Inclusion(at, name))
        }
        ("about", Some(did), None) => Did::parse(did)
            .map(Ask::About)
            .map_err(|_| Unreadable::Malformed),
        ("root", Some(epoch), None) => numbered(epoch).map(Ask::Root),
        _ => Err(Unreadable::NoSuchQuestion),
    }
}

/// An epoch from a path segment.
fn numbered(text: &str) -> Result<Epoch, Unreadable> {
    text.parse::<u64>()
        .map(Epoch::new)
        .map_err(|_| Unreadable::Malformed)
}

/// A name from a path segment.
fn named(text: &str) -> Result<Name, Unreadable> {
    Name::parse(text).map_err(|_| Unreadable::Malformed)
}

/// Answer a question.
///
/// Takes the node by shared reference, which is what makes it obvious that reading changes
/// nothing — and why handing over an act is [`deliver`] and not one more arm of this.
#[must_use]
pub fn answer(node: &Node, ask: &Ask, now: Epoch, limits: &Limits) -> Said {
    match ask {
        Ask::Limits => said(node, now, State::Here, Some(limits_of(limits)), None),
        Ask::Object(name) => resolved(node, name, now),
        Ask::Act(name) => match node.act(name, now).answer {
            Some(bytes) => said(node, now, State::Here, Some(Value::Bytes(bytes)), None),
            None => said(node, now, State::DoesNotExist, None, None),
        },
        Ask::About(subject) => match node.about(subject, now).answer {
            Some(hashes) => {
                let listed = hashes
                    .into_iter()
                    .map(|hash| Value::Text(hash.as_str().to_owned()))
                    .collect();
                said(node, now, State::Here, Some(Value::Array(listed)), None)
            }
            None => said(node, now, State::NotYetAskable, None, None),
        },
        Ask::Inclusion(epoch, name) => match node.inclusion_in(name, *epoch, now).answer {
            Some((at, path, published)) => {
                let hashes = path
                    .hashes()
                    .iter()
                    .map(|hash| Value::Bytes(hash.bytes().to_vec()))
                    .collect();
                // The root goes with it, signed. Without it the path is counted against a size
                // nobody stated and a root nobody put their name to — which is not a proof, and
                // whoever received it would have no way of finding that out.
                let proof = Value::Map(BTreeMap::from([
                    (proving::AT, Value::Uint(at)),
                    (proving::PATH, Value::Array(hashes)),
                    (proving::ROOT, Value::Bytes(published.to_bytes())),
                ]));
                said(node, now, State::Here, Some(proof), None)
            }
            None => said(node, now, State::DoesNotExist, None, None),
        },
        Ask::Root(epoch) => match node.root_at(*epoch) {
            // Signed on the way out, not stored signed: the signature is over bytes that never
            // change, so making it here costs one signature per question and saves keeping a
            // second copy of every root that could drift from the first.
            //
            // Handing back the bare root instead would be handing back a claim with nothing to
            // check it against — anything answering on this address could make the same bytes.
            Some(root) => {
                let stamped = node.publish(root);
                let carried = Value::Map(BTreeMap::from([
                    (published::ROOT, Value::Bytes(stamped.root.to_bytes())),
                    (published::KEY, Value::Bytes(stamped.key.to_vec())),
                    (
                        published::SIGNATURE,
                        Value::Bytes(stamped.signature.to_vec()),
                    ),
                ]));
                said(node, now, State::Here, Some(carried), None)
            }
            None => said(node, now, State::DoesNotExist, None, None),
        },
    }
}

/// Hand over a signed act.
///
/// **The signature is the authorisation.** Nothing about who delivered it, or over what, enters
/// into whether it is taken.
///
/// It takes the node by exclusive reference because it changes the record, and that is worth
/// seeing in the signature rather than discovering. Reading and writing are not two arms of one
/// function here for the same reason.
#[must_use]
pub fn deliver(node: &mut Node, act: &[u8], now: Epoch) -> Said {
    let Ok(value) = almena_format::cbor::read(act) else {
        let (epoch, root) = (now, node.root_now());
        return raw(epoch, root, State::Malformed, None, None);
    };
    let Some(operation) = almena_format::operation::read(&value) else {
        let (epoch, root) = (now, node.root_now());
        return raw(epoch, root, State::Malformed, None, None);
    };

    match node.submit(&operation, now) {
        Ok(_) => {
            let root = node.root_now();
            raw(now, root, State::Taken, None, None)
        }
        Err(refused) => {
            let root = node.root_now();
            raw(now, root, State::NotTaken, None, Some(rule(refused)))
        }
    }
}

/// Which rule an act broke, as a number.
///
/// Written out one by one rather than cast from the type it names. A cast would make these numbers
/// the order somebody happened to declare the variants in, and reordering a list nobody thought
/// was load-bearing would silently change what every node in the network says about a refusal.
const fn rule(refused: Refused) -> u64 {
    match refused {
        Refused::DoesNotNameItself => 1,
        Refused::AlreadyExists => 2,
        Refused::NoSuchPredecessor => 3,
        Refused::FromTheFuture => 4,
        Refused::Unsigned => 5,
        Refused::NotAuthorised => 6,
        Refused::SignatureDoesNotCheck => 7,
        Refused::Malformed => 8,
        Refused::NotKept => 9,
        Refused::NotAContradiction => 10,
    }
}

/// A throttle, said out loud and stamped like everything else.
///
/// Stamped because a node that stamps what it served and not what it refused is a node whose
/// refusals cannot be checked — and those are the ones worth checking.
#[must_use]
pub fn throttled(node: &Node, now: Epoch, limits: &Limits) -> Said {
    said(node, now, State::Throttled, Some(limits_of(limits)), None)
}

/// What comes back when a question could not be read.
#[must_use]
pub fn unreadable(node: &Node, now: Epoch, why: Unreadable) -> Said {
    let state = match why {
        Unreadable::NoSuchQuestion => State::NoSuchQuestion,
        Unreadable::Malformed => State::Malformed,
    };
    said(node, now, state, None, None)
}

/// What this node says about an object, as one of the four answers.
fn resolved(node: &Node, name: &Name, now: Epoch) -> Said {
    let Answered { answer, .. } = node.resolve(name, now);
    match answer {
        Answer::DoesNotExist => said(node, now, State::DoesNotExist, None, None),
        Answer::NotHere => said(node, now, State::NotHere, None, None),
        Answer::CannotResolve(reason) => {
            let why = match reason {
                Reason::Forked => Why::Forked,
                Reason::Unintelligible => Why::Unintelligible,
            };
            said(node, now, State::CannotResolve, None, Some(why as u64))
        }
        Answer::Here(_) => {
            // The state itself is not served: whoever asked composes it from the acts, checking
            // the signatures on the way. A node that handed over a finished answer would be a
            // source somebody had to believe, which is the one thing it must not become.
            let head = node
                .head(name)
                .map(|hash| Value::Text(hash.as_str().to_owned()));
            said(node, now, State::Here, head, None)
        }
    }
}

/// What the limits look like as an answer.
fn limits_of(limits: &Limits) -> Value {
    Value::Map(BTreeMap::from([
        (1, Value::Uint(limits.per_connection)),
        (2, Value::Uint(limits.window)),
        (3, Value::Uint(limits.largest_act)),
        (4, Value::Uint(limits.connections)),
    ]))
}

/// Build a response, stamped with what the node was at.
fn said(node: &Node, now: Epoch, state: State, payload: Option<Value>, which: Option<u64>) -> Said {
    raw(now, node.root_now(), state, payload, which)
}

/// The same, for the two callers that already hold the stamp.
fn raw(
    epoch: Epoch,
    root: almena_suite::digest::Digest,
    state: State,
    payload: Option<Value>,
    which: Option<u64>,
) -> Said {
    let mut fields = BTreeMap::from([
        (field::EPOCH, Value::Uint(epoch.number())),
        (field::ROOT, Value::Bytes(root.bytes().to_vec())),
        (field::STATE, Value::Uint(state as u64)),
    ]);
    if let Some(payload) = payload {
        fields.insert(field::PAYLOAD, payload);
    }
    if let Some(which) = which {
        fields.insert(field::WHICH, Value::Uint(which));
    }
    Said {
        state,
        body: Value::Map(fields).to_bytes(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Ask, Limits, Said, State, Unreadable, Why, answer, deliver, field, parse, rule, throttled,
        unreadable,
    };
    use almena_format::cbor::{Value, read};
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::{Signed, create};
    use almena_node::Node;
    use almena_store::chain::Refused;
    use almena_store::genesis::Which;
    use almena_store::kind::Kind;
    use almena_suite::ed25519;
    use almena_time::{Epoch, Epochs};
    use std::collections::BTreeMap;

    /// A development network with a fixed clock, so that a test is never about what time it is.
    fn at() -> almena_node::Opening {
        almena_node::Opening {
            which: Which::Development,
            beginning: Epoch::GENESIS,
            began: 1_800_000_000,
        }
    }

    fn key(seed: u8) -> ed25519::SigningKey {
        ed25519::SigningKey::from_secret([seed; 32])
    }

    fn limits() -> Limits {
        Limits {
            per_connection: 60,
            window: 60,
            largest_act: 65_536,
            connections: 256,
        }
    }

    fn a_node() -> Node {
        Node::open(&at(), &[], &key(5), key(6)).expect("nobody to join")
    }

    /// The three parts of a published root, out of what was answered.
    fn published_root(said: &Said) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let Value::Map(body) = read(&said.body).expect("readable") else {
            panic!("an answer is a map")
        };
        let Some(Value::Map(carried)) = body.get(&field::PAYLOAD) else {
            panic!("it carried a root")
        };
        let part = |number: u64| match carried.get(&number) {
            Some(Value::Bytes(bytes)) => bytes.clone(),
            _ => panic!("a root has all three parts"),
        };
        (
            part(super::published::ROOT),
            part(super::published::KEY),
            part(super::published::SIGNATURE),
        )
    }

    #[test]
    fn a_proof_of_inclusion_comes_back_with_the_root_it_is_against() {
        // **What makes a proof one.** A path proves an entry against a root of a stated size, so
        // without the root and the size the receiver is counting hashes against nothing.
        let mut node = a_node();
        let account = an_account(&key(9), Epoch::GENESIS);
        let name = Name::of(&account.to_bytes());
        node.submit(&account, Epoch::GENESIS).expect("taken");
        let next = Epoch::GENESIS.plus(Epochs(1)).expect("no overflow");
        node.close(next);

        let said = answer(
            &node,
            &Ask::Inclusion(next, name),
            Epoch::GENESIS,
            &limits(),
        );
        let Value::Map(body) = read(&said.body).expect("readable") else {
            panic!("an answer is a map")
        };
        let Some(Value::Map(proof)) = body.get(&field::PAYLOAD) else {
            panic!("it carried a proof")
        };

        assert!(matches!(
            proof.get(&super::proving::AT),
            Some(Value::Uint(_))
        ));
        assert!(matches!(
            proof.get(&super::proving::PATH),
            Some(Value::Array(_))
        ));
        let Some(Value::Bytes(root)) = proof.get(&super::proving::ROOT) else {
            panic!("and the signed root it is against")
        };
        let published = almena_store::root::Published::read(root).expect("a published root");
        assert_eq!(
            published.accept(node.network(), &node.key()),
            Ok(()),
            "which is this node's own word and not anybody's bytes"
        );
    }

    #[test]
    fn a_proof_is_not_served_without_saying_which_root() {
        // A path against whatever the tree happens to be now is a path against a root nobody ever
        // put their name to, so there is no route that offers one.
        assert_eq!(
            parse("GET", "/inclusion/zQmSomething"),
            Err(Unreadable::NoSuchQuestion)
        );
    }

    #[test]
    fn a_root_comes_back_signed_by_the_node_that_answered() {
        // Without this a root is a claim: anything answering on this address could produce the
        // same bytes, and a reader would have nothing to hold it to.
        let node = a_node();
        let said = answer(&node, &Ask::Root(Epoch::GENESIS), Epoch::GENESIS, &limits());

        let (root, said_key, signature) = published_root(&said);
        assert_eq!(said_key, node.key(), "the key it names is this node's");

        let verifying =
            ed25519::VerifyingKey::from_bytes(said_key.as_slice().try_into().expect("a key"))
                .expect("a key");
        assert!(
            verifying
                .verify(
                    &root,
                    &ed25519::Signature::from_bytes(
                        signature.as_slice().try_into().expect("a signature")
                    )
                )
                .is_ok(),
            "and the signature is over exactly the root it handed back"
        );
    }

    #[test]
    fn a_root_says_which_node_published_it_and_not_which_network() {
        // The two are both hashes this node holds, and stamping the wrong one costs nothing until
        // roots are compared — when it makes every honest pair of nodes look like misconduct.
        let node = a_node();
        let said = answer(&node, &Ask::Root(Epoch::GENESIS), Epoch::GENESIS, &limits());
        let (root, _, _) = published_root(&said);

        let Value::Map(fields) = read(&root).expect("readable") else {
            panic!("a root is a map")
        };
        let named = fields
            .values()
            .any(|value| matches!(value, Value::Text(text) if text == &node.did().to_string()));
        assert!(named, "the node is in the bytes that were signed");
        assert!(
            !fields.values().any(|value| {
                matches!(value, Value::Text(text) if text == &node.government().to_string())
            }),
            "and Almena Government is not standing in for it"
        );
    }

    #[test]
    fn a_root_signed_by_another_key_does_not_check_out() {
        // The property is only worth anything if the signature can fail.
        let node = a_node();
        let said = answer(&node, &Ask::Root(Epoch::GENESIS), Epoch::GENESIS, &limits());
        let (root, _, _) = published_root(&said);

        let stranger = key(200);
        let forged = stranger.sign(&root);
        assert!(
            ed25519::VerifyingKey::from_bytes(node.key())
                .expect("a key")
                .verify(&root, &ed25519::Signature::from_bytes(forged.bytes()))
                .is_err()
        );
    }

    fn an_account(control: &ed25519::SigningKey, at: Epoch) -> almena_format::operation::Operation {
        let public = control.verifying_key().bytes();
        let mut operation = create(
            Network::Development,
            Kind::HOLDER_CREATE.number(),
            1,
            at,
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

    /// The response, read back as the map it is.
    fn body(said: &Said) -> BTreeMap<u64, Value> {
        match read(&said.body) {
            Ok(Value::Map(fields)) => fields,
            other => panic!("a response is a canonical map, got {other:?}"),
        }
    }

    #[test]
    fn everything_said_carries_the_epoch_and_root_it_was_true_at() {
        // Including the refusals and the throttle. A node that stamps what it served and not what
        // it refused is a node whose refusals cannot be checked, and those are the ones worth
        // checking.
        let mut node = a_node();
        let asked = Epoch::GENESIS.plus(Epochs(9)).expect("no overflow");
        let missing = Name::of(b"never happened");

        let responses = [
            answer(&node, &Ask::Limits, asked, &limits()),
            answer(&node, &Ask::Object(missing.clone()), asked, &limits()),
            answer(&node, &Ask::Act(missing.clone()), asked, &limits()),
            answer(&node, &Ask::Root(asked), asked, &limits()),
            throttled(&node, asked, &limits()),
            unreadable(&node, asked, Unreadable::NoSuchQuestion),
            unreadable(&node, asked, Unreadable::Malformed),
            deliver(&mut node, b"not an act at all", asked),
        ];

        for said in &responses {
            let fields = body(said);
            assert_eq!(
                fields.get(&field::EPOCH),
                Some(&Value::Uint(asked.number())),
                "{:?} carried no epoch",
                said.state
            );
            assert!(
                matches!(fields.get(&field::ROOT), Some(Value::Bytes(_))),
                "{:?} carried no root",
                said.state
            );
        }
    }

    #[test]
    fn the_four_answers_about_an_object_are_four_different_states() {
        // None may be mistaken for another. Saying *it does not exist* about something that does
        // is a lie, and so is serving the state from before an act nobody understood.
        let mut node = a_node();
        let now = Epoch::GENESIS;

        let unknown = answer(&node, &Ask::Object(Name::of(b"nothing")), now, &limits());
        assert_eq!(unknown.state, State::DoesNotExist);

        let account = an_account(&key(9), now);
        let name = account.object.name().clone();
        node.submit(&account, now).expect("taken");
        assert_eq!(
            answer(&node, &Ask::Object(name.clone()), now, &limits()).state,
            State::Here
        );

        // And an act nobody understands puts the same object into a different state entirely.
        let mut newer = almena_format::operation::Operation {
            object: account.object.clone(),
            previous: Some(Name::of(&account.to_bytes())),
            kind: 9_999,
            version: 1,
            issued: now,
            payload: BTreeMap::new(),
            signatures: Vec::new(),
        };
        let signature = key(9).sign(&newer.signing_bytes());
        newer.signatures.push(Signed {
            by: account.object.clone(),
            key: key(9).verifying_key().bytes().to_vec(),
            signature: signature.bytes(),
        });
        node.submit(&newer, now).expect("stored anyway");

        let opaque = answer(&node, &Ask::Object(name), now, &limits());
        assert_eq!(opaque.state, State::CannotResolve);
        assert_eq!(
            body(&opaque).get(&field::WHICH),
            Some(&Value::Uint(Why::Unintelligible as u64)),
            "and it says which of the two it is"
        );
    }

    #[test]
    fn resolving_hands_over_materials_and_never_a_finished_answer() {
        // A node that handed over the composed state would be a source somebody has to believe,
        // which is the one thing it must not become. What comes back is where to start reading.
        let mut node = a_node();
        let account = an_account(&key(9), Epoch::GENESIS);
        let name = account.object.name().clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let said = answer(&node, &Ask::Object(name), Epoch::GENESIS, &limits());
        let payload = body(&said).get(&field::PAYLOAD).cloned();
        assert!(
            matches!(payload, Some(Value::Text(_))),
            "the head of the chain, not a composed state"
        );
    }

    #[test]
    fn an_act_comes_back_byte_for_byte() {
        let mut node = a_node();
        let account = an_account(&key(9), Epoch::GENESIS);
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let hash = Name::of(&account.to_bytes());
        let said = answer(&node, &Ask::Act(hash), Epoch::GENESIS, &limits());
        assert_eq!(
            body(&said).get(&field::PAYLOAD),
            Some(&Value::Bytes(account.to_bytes())),
            "the author's own bytes, not a re-encoding"
        );
    }

    #[test]
    fn nobody_has_looked_is_not_reported_as_nothing_was_found() {
        // An empty list here would read as *nobody has certified this entity* — a claim, and a
        // false one, because no act can carry a subject yet.
        let node = a_node();
        let said = answer(
            &node,
            &Ask::About(node.government().clone()),
            Epoch::GENESIS,
            &limits(),
        );
        assert_eq!(said.state, State::NotYetAskable);
        assert_eq!(body(&said).get(&field::PAYLOAD), None);
    }

    #[test]
    fn handing_over_a_good_act_writes_it_down() {
        let mut node = a_node();
        let before = node.written();
        let account = an_account(&key(9), Epoch::GENESIS);

        let said = deliver(&mut node, &account.to_bytes(), Epoch::GENESIS);
        assert_eq!(said.state, State::Taken);
        assert_eq!(node.written(), before + 1);
    }

    #[test]
    fn an_act_that_breaks_a_rule_says_which_rule() {
        // Errors are states, never prose: an operator pastes the number into a support channel
        // and somebody running the node in another language searches for the same number.
        let mut node = a_node();
        let ahead = Epoch::GENESIS.plus(Epochs(9)).expect("no overflow");
        let account = an_account(&key(9), ahead);

        let said = deliver(&mut node, &account.to_bytes(), Epoch::GENESIS);
        assert_eq!(said.state, State::NotTaken);
        assert_eq!(
            body(&said).get(&field::WHICH),
            Some(&Value::Uint(rule(Refused::FromTheFuture)))
        );
    }

    #[test]
    fn bytes_that_are_not_an_act_are_malformed_rather_than_refused() {
        // Two different things: one is *I could not read this*, the other is *I read it and it
        // breaks a rule*. A caller that cannot tell them apart cannot fix either.
        let mut node = a_node();
        for rubbish in [b"".as_slice(), b"not cbor at all", &[0xa1, 0x01]] {
            let said = deliver(&mut node, rubbish, Epoch::GENESIS);
            assert_eq!(said.state, State::Malformed, "{rubbish:?}");
        }
    }

    #[test]
    fn the_rule_numbers_are_not_the_order_somebody_declared_them_in() {
        // Cast from the enum, these would be whatever position each variant happens to sit at, and
        // reordering a list nobody thought was load-bearing would change what every node in the
        // network says about a refusal.
        let numbered = [
            (Refused::DoesNotNameItself, 1),
            (Refused::AlreadyExists, 2),
            (Refused::NoSuchPredecessor, 3),
            (Refused::FromTheFuture, 4),
            (Refused::Unsigned, 5),
            (Refused::NotAuthorised, 6),
            (Refused::SignatureDoesNotCheck, 7),
            (Refused::Malformed, 8),
        ];
        for (refused, number) in numbered {
            assert_eq!(rule(refused), number, "{refused:?}");
        }
    }

    #[test]
    fn a_question_is_read_or_it_is_said_why_not() {
        assert_eq!(parse("GET", "/limits"), Ok(Ask::Limits));

        let name = Name::of(b"an act");
        assert_eq!(
            parse("GET", &format!("/object/{}", name.as_str())),
            Ok(Ask::Object(name.clone()))
        );
        assert_eq!(
            parse("GET", &format!("/act/{}", name.as_str())),
            Ok(Ask::Act(name))
        );
        assert_eq!(parse("GET", "/root/42"), Ok(Ask::Root(Epoch::new(42))));

        // Nothing at that path, versus the right path with something unusable in it.
        assert_eq!(parse("GET", "/nothing"), Err(Unreadable::NoSuchQuestion));
        assert_eq!(parse("GET", "/"), Err(Unreadable::NoSuchQuestion));
        assert_eq!(
            parse("GET", "/object/not-a-name"),
            Err(Unreadable::Malformed)
        );
        assert_eq!(parse("GET", "/root/soon"), Err(Unreadable::Malformed));
    }

    #[test]
    fn handing_something_over_is_not_a_question() {
        // Writing has its own way in, because it changes the record and that is worth seeing in
        // the signature rather than discovering.
        assert_eq!(parse("POST", "/limits"), Err(Unreadable::NoSuchQuestion));
        assert_eq!(parse("PUT", "/object/x"), Err(Unreadable::NoSuchQuestion));
    }

    #[test]
    fn what_a_node_will_do_is_published_without_being_asked_who_wants_to_know() {
        // Half of telling a limit from censorship: what a node said and what it did become two
        // facts a third party can hold up against each other.
        let node = a_node();
        let said = answer(&node, &Ask::Limits, Epoch::GENESIS, &limits());
        assert_eq!(said.state, State::Here);

        let Some(Value::Map(published)) = body(&said).get(&field::PAYLOAD).cloned() else {
            panic!("the limits are published");
        };
        assert_eq!(published.get(&1), Some(&Value::Uint(60)));
        assert_eq!(published.get(&4), Some(&Value::Uint(256)));
    }

    #[test]
    fn an_epoch_nothing_was_said_about_is_not_an_error() {
        let node = a_node();
        let far = Epoch::GENESIS.plus(Epochs(9_999)).expect("no overflow");
        assert_eq!(
            answer(&node, &Ask::Root(far), Epoch::GENESIS, &limits()).state,
            State::DoesNotExist
        );
    }

    #[test]
    fn nothing_in_a_question_names_who_is_asking() {
        // Reading is not authenticated, and the way that is kept true is that there is nowhere to
        // put a caller: no signature here takes one, so nothing can be denied on that basis.
        let node = a_node();
        let stranger = Did::new(Network::Development, Name::of(b"somebody"));
        let one = answer(&node, &Ask::About(stranger), Epoch::GENESIS, &limits());
        let two = answer(
            &node,
            &Ask::About(node.government().clone()),
            Epoch::GENESIS,
            &limits(),
        );
        assert_eq!(one.state, two.state, "the same answer for anybody");
    }
}
