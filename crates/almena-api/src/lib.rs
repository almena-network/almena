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

pub mod post;

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

/// What is in the catalogue, by what each object is.
///
/// All odd, and every one of them present even when empty: a reader that found no `tags` key would
/// have no way of telling *no tags have been added* from *this build does not list them*, and a
/// catalogue is a comparison — the difference decides whether an empty page is about the network or
/// about the reader.
mod catalogued {
    /// The places definitions are copied from.
    pub const SOURCES: u64 = 1;
    /// The pieces of data a credential can carry.
    pub const ATTRIBUTES: u64 = 3;
    /// The closed list of what a request may be for.
    pub const TAGS: u64 = 5;
    /// The shapes of what is issued and of what is asked for.
    pub const TEMPLATES: u64 = 7;
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

/// How much of an object's history goes into one answer.
///
/// Bounded by weight as well as by count, because they are not the same bound: one act may be as
/// large as whatever the node that took it was willing to accept, so a count alone puts no ceiling
/// on the message.
const PAGE: almena_node::Page = almena_node::Page {
    at_most: 256,
    weighing_at_most: 4 * 1024 * 1024,
};

/// A question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// What an object is now.
    Object(Name),
    /// One act, by its hash.
    Act(Name),
    /// The acts somebody needs to work out what an object is now.
    ///
    /// **Not what it is** — that would be a finished answer somebody had to believe. What comes
    /// back is the last summary this node could read and everything after it, in the bytes their
    /// authors signed, so that whoever asked works out the state and checks it on the way.
    State(Did, Option<Name>),
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
    /// What the network says it is running, and how many nodes speak each version.
    ///
    /// **Counted, never declared**, so that what is missing is visible before it is a problem. It
    /// is what nodes *say*; what they do is measured by asking, and the two are kept apart.
    Capacity,
    /// The bytes of one status list version, by the hash of those bytes.
    ///
    /// **Any node will do, because the hash decides** (`SPECS.md §10.2`). Whoever asks already
    /// holds the version the record names; what a node adds is a copy of the bytes, and either they
    /// match it or they do not. That is what lets a verifier ask a replica first and go to the
    /// issuer's own node only when the replica comes back stale — asking the source every time
    /// would tell an issuer when and how often its credentials are verified.
    List(Vec<u8>),
    /// What is in the catalogue, by what each object is.
    ///
    /// **The whole of it, because it is the whole of it that is comparable.** `SPECS.md §9.4` has
    /// no private template and no arrangement outside the catalogue precisely so that this answer
    /// is complete rather than a sample — a listing that paged would make *what is asked for in
    /// this network* a question with a different answer depending on where somebody stopped
    /// reading. What bounds it is governance: sources and tags are Almena Government's and
    /// attributes and templates need the seal.
    Catalogue,
    /// How the network's trust anchor stands against what `SPECS.md §7.1` asks of it.
    ///
    /// **Because it has to be said.** A network opens with its anchor self-signed by one key
    /// (`SPECS.md §7.9`), and there is nothing wrong with that — what would be wrong is leaving it
    /// that way with nobody able to find out. So the numbers are served like every other figure
    /// here: as facts, with no verdict attached, for whoever is deciding whether to rely on that
    /// seal.
    Anchor,
    /// What one node saw on a day, asking by asking — the observations a summary's hash pins.
    ///
    /// **Without this the hash is a promise that could be kept rather than one that is.** A summary
    /// carries the hash of what it was drawn from so that, having published it, an observer cannot
    /// later produce a different account of what it saw. That is worth nothing if nobody can get
    /// the account.
    Watching(u64),
    /// What the network went looking for on a day, and how much of it it found.
    ///
    /// **Nobody's assertion about themselves.** It is a sum over signed acts in a record everybody
    /// holds, so two nodes holding the same acts answer the same thing and whoever asked can work
    /// it out again without asking anybody.
    Kept(u64),
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
        ("state", Some(did), None) => Did::parse(did)
            .map(|object| Ask::State(object, None))
            .map_err(|_| Unreadable::Malformed),
        // **Where the last page stopped.** A caller that folded a page and could not tell it from
        // the whole chain would land on a state from earlier with nothing saying so, so every
        // paged answer here carries a cursor and this is how the next page is asked for.
        ("state", Some(did), Some(after)) => {
            let object = Did::parse(did).map_err(|_| Unreadable::Malformed)?;
            named(after).map(|cursor| Ask::State(object, Some(cursor)))
        }
        // Two segments and both required: a proof is against a root, and the epoch is which root.
        ("inclusion", Some(epoch), Some(name)) => {
            let at = numbered(epoch)?;
            named(name).map(|name| Ask::Inclusion(at, name))
        }
        ("about", Some(did), None) => Did::parse(did)
            .map(Ask::About)
            .map_err(|_| Unreadable::Malformed),
        ("root", Some(epoch), None) => numbered(epoch).map(Ask::Root),
        ("capacity", None, None) => Ok(Ask::Capacity),
        ("anchor", None, None) => Ok(Ask::Anchor),
        ("catalogue", None, None) => Ok(Ask::Catalogue),
        ("list", Some(version), None) => written_out(version).map(Ask::List),
        ("watching", Some(day), None) => day
            .parse::<u64>()
            .map(Ask::Watching)
            .map_err(|_| Unreadable::Malformed),
        ("kept", Some(day), None) => day
            .parse::<u64>()
            .map(Ask::Kept)
            .map_err(|_| Unreadable::Malformed),
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

/// Bytes from a path segment, written the way a person pastes them: lower-case hexadecimal.
///
/// **One spelling and no other.** Two spellings of one hash would be two addresses for one version,
/// and a cache that treated them as different would serve one of them stale for ever.
fn written_out(text: &str) -> Result<Vec<u8>, Unreadable> {
    if !text.len().is_multiple_of(2) || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Unreadable::Malformed);
    }
    if text.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(Unreadable::Malformed);
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).map_err(|_| Unreadable::Malformed))
        .collect()
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
        Ask::State(object, after) => composing(node, object, after.as_ref(), now),
        Ask::Kept(day) => measured(node, *day, now),
        Ask::Watching(day) => watching(node, almena_time::Day::new(*day), now),
        Ask::Capacity => running(node, now),
        Ask::Anchor => anchoring(node, now),
        Ask::Catalogue => listed(node, now),
        Ask::List(version) => match node.list(version) {
            Some(bytes) => said(node, now, State::Here, Some(Value::Bytes(bytes)), None),
            // **Not here, never *does not exist*.** A node that does not hold a copy of a public
            // list is saying something about itself; saying the list is not there would be a claim
            // about the issuer, made out of this node's own share of the work.
            None => said(node, now, State::NotHere, None, None),
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
        Ask::Inclusion(epoch, name) => proved(node, name, *epoch, now),
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
        Refused::BeforeItsPredecessor => 11,
        Refused::TooManyWaiting => 12,
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

/// Where a node wrote an act down, proved against a root it signed, as an answer.
fn proved(node: &Node, name: &Name, epoch: Epoch, now: Epoch) -> Said {
    let Some((at, path, published)) = node.inclusion_in(name, epoch, now).answer else {
        return said(node, now, State::DoesNotExist, None, None);
    };
    let hashes = path
        .hashes()
        .iter()
        .map(|hash| Value::Bytes(hash.bytes().to_vec()))
        .collect();
    // The root goes with it, signed. Without it the path is counted against a size nobody stated
    // and a root nobody put their name to — which is not a proof, and whoever received it would
    // have no way of finding that out.
    let proof = Value::Map(BTreeMap::from([
        (proving::AT, Value::Uint(at)),
        (proving::PATH, Value::Array(hashes)),
        (proving::ROOT, Value::Bytes(published.to_bytes())),
    ]));
    said(node, now, State::Here, Some(proof), None)
}

/// What is in the catalogue, as an answer.
///
/// **Names and never entries.** Whoever asked composes each object from its own acts, which is what
/// keeps a catalogue page a reading of the record rather than a report from whichever node was
/// asked (`SPECS.md §13.1`, `§13.6`).
fn listed(node: &Node, now: Epoch) -> Said {
    let held = node.catalogue(now).answer;
    let named = |names: Vec<almena_format::identifier::Name>| {
        Value::Array(
            names
                .into_iter()
                .map(|name| Value::Text(name.as_str().to_owned()))
                .collect(),
        )
    };
    let catalogue = Value::Map(BTreeMap::from([
        (catalogued::SOURCES, named(held.sources)),
        (catalogued::ATTRIBUTES, named(held.attributes)),
        (catalogued::TAGS, named(held.tags)),
        (catalogued::TEMPLATES, named(held.templates)),
    ]));
    said(node, now, State::Here, Some(catalogue), None)
}

/// What the network says it is running, as an answer.
fn running(node: &Node, now: Epoch) -> Said {
    let counted = node.running(now).answer;
    let offered = counted
        .offering
        .into_iter()
        .map(|(what, count)| (what.number(), Value::Uint(count as u64)))
        .collect();
    let spoken = counted
        .speaking
        .into_iter()
        .map(|(version, count)| (version, Value::Uint(count as u64)))
        .collect();

    // How much of each capability falls to one declared operator, and how much of it nobody has
    // claimed. **Numbers and never a name**: who holds the largest share, served on request, would
    // be a list of where to attack assembled by the network about itself.
    let concentrated = counted
        .concentration
        .into_iter()
        .map(|(what, held)| {
            (
                what.number(),
                Value::Array(vec![
                    Value::Uint(held.offering as u64),
                    Value::Uint(held.most as u64),
                    Value::Uint(held.unclaimed as u64),
                ]),
            )
        })
        .collect();

    // **Nought is said rather than left out** for a capability nobody offers: a figure that
    // omitted it would read as a capability nobody had thought of, and the whole point is that
    // what is missing is visible. The nodes this record cannot read travel with it for the same
    // reason — dropping them would make the figures look tidier than the network is.
    let figure = Value::Map(BTreeMap::from([
        (1, Value::Map(offered)),
        (2, Value::Map(spoken)),
        (3, Value::Uint(counted.unreadable as u64)),
        (4, Value::Map(concentrated)),
        (5, Value::Uint(counted.operators as u64)),
        (6, Value::Uint(counted.unclaimed as u64)),
    ]));
    said(node, now, State::Here, Some(figure), None)
}

/// How the trust anchor stands, as an answer.
///
/// **Codes and never sentences** (`SPECS.md §13.9`): what is missing travels as an identifier with
/// its numbers, and whoever received it draws it in their own reader's language. A node has no idea
/// what language anybody reads in, and asking it to would be asking it the wrong question.
fn anchoring(node: &Node, now: Epoch) -> Said {
    let Some(anchor) = node.anchor(now).answer else {
        // This node cannot resolve its own government, which is its own ignorance and not an
        // answer about the anchor — so it declines rather than reporting an anchor of nobody.
        return said(node, now, State::CannotResolve, None, None);
    };

    let wanting = anchor
        .wanting
        .iter()
        .map(|one| Value::Map(missing(one)))
        .collect();
    let figure = Value::Map(BTreeMap::from([
        (1, Value::Text(node.government().to_string())),
        (2, Value::Uint(anchor.owners as u64)),
        (3, Value::Uint(anchor.thresholds.routine)),
        (4, Value::Uint(anchor.thresholds.sealing)),
        (5, Value::Uint(anchor.thresholds.governance)),
        (6, Value::Uint(u64::from(anchor.one_pair_of_hands))),
        (7, Value::Array(wanting)),
    ]));
    said(node, now, State::Here, Some(figure), None)
}

/// One thing `SPECS.md §7.1` asks that the record does not show, as a code and its numbers.
fn missing(one: &almena_store::government::Wanting) -> BTreeMap<u64, Value> {
    use almena_store::government::Wanting;
    let (code, numbers) = match one {
        Wanting::NobodyIsAnOwnerYet => (1, Vec::new()),
        Wanting::TooFewOwners { has } => (2, vec![*has]),
        Wanting::SealingTooLow { is } => (3, vec![*is]),
        Wanting::GovernanceIsNotAMajority { is, of } => (4, vec![*is, *of]),
        // **The one that never reaches here**, because it is a declaration and not something in the
        // record — an owner's organisation is not a thing a node can read. It is answered for so
        // that adding it later is an addition rather than a shape somebody has to guess at.
        Wanting::TooManyInOne { has, of, .. } => (5, vec![*has, *of]),
    };
    BTreeMap::from([
        (1, Value::Uint(code)),
        (
            2,
            Value::Array(numbers.into_iter().map(Value::Uint).collect()),
        ),
    ])
}

/// What this node saw on a day, as an answer.
///
/// **The bytes it hashed, and not a rendering of them.** A hash over one encoding and an answer in
/// another would be a promise nobody could check, which is the state this exists to leave.
///
/// A day it no longer keeps is *not here* rather than an absence: the observations existed, this
/// node has aged them out, and saying they never did would be telling somebody the summary was
/// drawn from nothing.
fn watching(node: &Node, day: almena_time::Day, now: Epoch) -> Said {
    match node.watching(day) {
        Some(watching) => said(
            node,
            now,
            State::Here,
            Some(Value::Bytes(watching.to_bytes())),
            None,
        ),
        None => said(node, now, State::NotHere, None, None),
    }
}

/// What the network went looking for on a day, as an answer.
fn measured(node: &Node, day: u64, now: Epoch) -> Said {
    let kept = node.kept(almena_time::Day::new(day), now).answer;
    // **The denominator travels with it**, and so does how many observers it came from. Nought
    // found out of nought asked for is nobody having looked, and a figure that arrived without its
    // denominator would be read as a network in good order on the strength of nobody having checked.
    let figure = Value::Map(BTreeMap::from([
        (1, Value::Uint(kept.asked_for)),
        (2, Value::Uint(kept.found)),
        (3, Value::Uint(kept.observers as u64)),
    ]));
    said(node, now, State::Here, Some(figure), None)
}

/// The acts somebody needs to work out what an object is, as an answer.
fn composing(node: &Node, object: &Did, after: Option<&Name>, now: Epoch) -> Said {
    // A page, because a chain that never summarised can be as long as it likes, and an answer with
    // no bound is one a node cannot promise to give.
    let Some(composing) = node.state_of(object, after, PAGE, now).answer else {
        // A cursor this node cannot place on that object's branch. Answered as an absence about
        // the cursor rather than by starting again, because handing back the first page to
        // somebody who asked for the fourth would look like an answer and be a different one.
        return said(node, now, State::DoesNotExist, None, None);
    };
    let acts: Vec<Value> = composing.acts.into_iter().map(Value::Bytes).collect();

    // **Empty is not the same as absent.** Saying *here, nothing* about an object that exists would
    // be telling somebody it does not, which is the one thing a node may never do.
    if acts.is_empty() {
        return said(node, now, State::DoesNotExist, None, None);
    }

    // **Where it stopped, beside what it handed over.** A page and the whole of a chain look
    // identical without it, and a caller that folded the first thinking it was the second would
    // land on a state from earlier and have nothing to tell it so.
    let mut body = BTreeMap::from([(1, Value::Array(acts))]);
    if let Some(more) = composing.more {
        body.insert(3, Value::Text(more.as_str().to_owned()));
    }
    said(node, now, State::Here, Some(Value::Map(body)), None)
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
        let name = account.called();
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

    /// One more device on that account, so that its chain has two acts to page over.
    fn a_device_on(
        account: &almena_format::operation::Operation,
        control: &ed25519::SigningKey,
    ) -> almena_format::operation::Operation {
        let public = control.verifying_key().bytes();
        let mut operation = almena_format::operation::Operation {
            object: account.object.clone(),
            previous: Some(account.called()),
            kind: Kind::HOLDER_ADD_DEVICE.number(),
            version: 1,
            issued: Epoch::GENESIS,
            payload: BTreeMap::from([(1, Value::Bytes(vec![2; 33]))]),
            signatures: Vec::new(),
        };
        let signature = control.sign(&operation.signing_bytes());
        operation.signatures.push(Signed {
            by: account.object.clone(),
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
            previous: Some(account.called()),
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

        let hash = account.called();
        let said = answer(&node, &Ask::Act(hash), Epoch::GENESIS, &limits());
        assert_eq!(
            body(&said).get(&field::PAYLOAD),
            Some(&Value::Bytes(account.to_bytes())),
            "the author's own bytes, not a re-encoding"
        );
    }

    #[test]
    fn what_has_been_said_about_somebody_comes_back_as_a_list_and_not_as_a_refusal() {
        // It used to come back as *not askable*, because no act could say who it was about. Now a
        // contradiction does, so an empty list is a fact — nothing has been said — rather than a
        // silence dressed as one.
        let node = a_node();
        let said = answer(
            &node,
            &Ask::About(node.government().clone()),
            Epoch::GENESIS,
            &limits(),
        );
        assert_eq!(said.state, State::Here);
        assert_eq!(
            body(&said).get(&field::PAYLOAD),
            Some(&Value::Array(Vec::new())),
            "nothing said about it, said as nothing rather than as a refusal to say"
        );
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
            (Refused::NotKept, 9),
            (Refused::NotAContradiction, 10),
            (Refused::BeforeItsPredecessor, 11),
            (Refused::TooManyWaiting, 12),
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
        assert_eq!(parse("GET", "/catalogue"), Ok(Ask::Catalogue));

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

    #[test]
    fn a_node_hands_over_the_acts_to_work_a_state_out_from() {
        // **Not the state.** A finished answer would be a source somebody had to believe; what
        // comes back is materials, each carrying the signature that makes it check out.
        let mut node = a_node();
        let control = ed25519::SigningKey::from_secret([9; 32]);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let said = answer(&node, &Ask::State(object, None), Epoch::GENESIS, &limits());
        assert_eq!(said.state, State::Here);

        let Ok(Value::Map(fields)) = almena_format::cbor::read(&said.body) else {
            panic!("a response is a canonical map");
        };
        let Some(Value::Map(carried)) = fields.get(&4) else {
            panic!("it carries the acts, got {fields:?}");
        };
        assert_eq!(
            carried.get(&1),
            Some(&Value::Array(vec![Value::Bytes(account.to_bytes())]))
        );
        assert_eq!(
            carried.get(&3),
            None,
            "and says nothing about carrying on, because there is nothing after it"
        );
    }

    #[test]
    fn carrying_on_from_where_a_page_stopped_hands_over_what_came_after_it() {
        // **What the cursor is for.** How big a page is belongs to the node; that a caller can say
        // *I have up to here, go on* belongs to the interface, and without it a page and the whole
        // of a chain are the same answer.
        let mut node = a_node();
        let control = ed25519::SigningKey::from_secret([9; 32]);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");
        let device = a_device_on(&account, &control);
        node.submit(&device, Epoch::GENESIS).expect("taken");

        let next = carried(&answer(
            &node,
            &Ask::State(object.clone(), Some(account.called())),
            Epoch::GENESIS,
            &limits(),
        ));
        assert_eq!(
            next.get(&1),
            Some(&Value::Array(vec![Value::Bytes(device.to_bytes())])),
            "everything after the act the cursor named"
        );
        assert_eq!(next.get(&3), None, "and nothing left after that");

        // A cursor this node cannot place on that branch is an absence about the cursor, never the
        // first page again: handing back the beginning to somebody who asked for the middle would
        // look like an answer and be a different one.
        let said = answer(
            &node,
            &Ask::State(object, Some(Name::of(b"an act nobody wrote"))),
            Epoch::GENESIS,
            &limits(),
        );
        assert_eq!(said.state, State::DoesNotExist);
    }

    /// What an answer about an object's history carried.
    fn carried(said: &Said) -> std::collections::BTreeMap<u64, Value> {
        assert_eq!(said.state, State::Here);
        let Ok(Value::Map(fields)) = almena_format::cbor::read(&said.body) else {
            panic!("a response is a canonical map");
        };
        match fields.get(&4) {
            Some(Value::Map(carried)) => carried.clone(),
            other => panic!("it carries the acts, got {other:?}"),
        }
    }

    #[test]
    fn the_observations_a_summary_was_drawn_from_can_be_asked_for() {
        // **What turns the hash into a promise that is kept.** A summary carries the hash of what
        // it was drawn from so that, having published it, an observer cannot later produce a
        // different account of what it saw — which is worth nothing if nobody can get the account.
        let mut node = a_node();
        let day = almena_time::Day::new(1);
        let during = Epoch::new(almena_time::EPOCHS_PER_DAY);
        let key = ed25519::SigningKey::from_secret([7; 32]);
        let announced = almena_store::announce::announce(
            almena_store::genesis::Which::Development,
            during,
            &key,
        );
        node.submit(&announced.operation, during)
            .expect("announced");
        node.watched(
            day,
            almena_store::watching::Noted {
                of: key.verifying_key().bytes().to_vec(),
                at: during,
                saw: almena_store::watching::Saw::Asked,
            },
        );

        let said = answer(&node, &Ask::Watching(day.number()), during, &limits());
        assert_eq!(said.state, State::Here);
        let Some(Value::Bytes(bytes)) = body(&said).get(&field::PAYLOAD).cloned() else {
            panic!("it hands over the bytes it hashed")
        };
        assert_eq!(
            almena_suite::digest::Digest::of(&bytes),
            node.watching(day).expect("it kept them").digest(),
            "and they are the bytes the hash is over, not a rendering of them"
        );

        // A day it no longer keeps is *not here*: the observations existed and this node has aged
        // them out. Saying they never did would be telling somebody the summary came from nothing.
        let said = answer(&node, &Ask::Watching(9), during, &limits());
        assert_eq!(said.state, State::NotHere);
    }

    #[test]
    fn a_network_that_has_just_opened_says_its_anchor_is_in_one_pair_of_hands() {
        // **The answer that has to exist** (`SPECS.md §7.1`). It is the true state of a network on
        // its first day and there is nothing wrong with it — what would be wrong is there being no
        // way to find out, which is what this question is for.
        let node = a_node();
        let said = answer(&node, &Ask::Anchor, Epoch::GENESIS, &limits());
        assert_eq!(said.state, State::Here);

        let Value::Map(body) = read(&said.body).expect("readable") else {
            panic!("an answer is a map")
        };
        let Some(Value::Map(figure)) = body.get(&field::PAYLOAD) else {
            panic!("it carried the figure")
        };
        assert_eq!(
            figure.get(&1),
            Some(&Value::Text(node.government().to_string()))
        );
        assert_eq!(figure.get(&2), Some(&Value::Uint(0)), "nobody yet");
        assert_eq!(figure.get(&6), Some(&Value::Uint(1)), "one pair of hands");

        // And what is missing travels as a code with its numbers, never as a sentence: a node has
        // no idea what language whoever asked reads in.
        let Some(Value::Array(wanting)) = figure.get(&7) else {
            panic!("it said what is missing")
        };
        assert_eq!(
            wanting,
            &vec![Value::Map(BTreeMap::from([
                (1, Value::Uint(1)),
                (2, Value::Array(Vec::new())),
            ]))],
            "one thing to say, and it is the one worth saying"
        );
    }

    #[test]
    fn asking_for_the_state_of_something_nobody_has_heard_of_says_so() {
        // Empty is not the same as absent: saying *here, nothing* about an object that exists would
        // be telling somebody it does not.
        let node = a_node();
        let nobody = Did::new(
            almena_format::identifier::Network::Development,
            Name::of(b"never happened"),
        );
        let said = answer(&node, &Ask::State(nobody, None), Epoch::GENESIS, &limits());
        assert_eq!(said.state, State::DoesNotExist);
    }

    #[test]
    fn the_state_of_an_object_is_a_question_this_node_takes() {
        let object = Did::new(
            almena_format::identifier::Network::Development,
            Name::of(b"something"),
        );
        assert_eq!(
            parse("GET", &format!("/state/{object}")),
            Ok(Ask::State(object.clone(), None))
        );
        let after = Name::of(b"where the last page stopped");
        assert_eq!(
            parse("GET", &format!("/state/{object}/{}", after.as_str())),
            Ok(Ask::State(object, Some(after)))
        );
        assert_eq!(
            parse("GET", "/state/not-a-name"),
            Err(Unreadable::Malformed)
        );
    }

    #[test]
    fn what_the_network_went_looking_for_is_a_question_this_node_takes() {
        assert_eq!(parse("GET", "/kept/9"), Ok(Ask::Kept(9)));
        assert_eq!(parse("GET", "/kept/today"), Err(Unreadable::Malformed));
    }

    #[test]
    fn the_figure_comes_back_with_its_denominator_and_how_many_looked() {
        // **Nought found out of nought asked for is nobody having looked.** A figure that arrived
        // without its denominator would be read as a network in good order on the strength of
        // nobody having checked, which is the easiest mistake this design offers.
        let node = a_node();
        let said = answer(&node, &Ask::Kept(0), Epoch::GENESIS, &limits());
        assert_eq!(said.state, State::Here);

        let Ok(Value::Map(fields)) = almena_format::cbor::read(&said.body) else {
            panic!("a response is a canonical map");
        };
        let Some(Value::Map(figure)) = fields.get(&4) else {
            panic!("it carries the figure, got {fields:?}");
        };
        assert_eq!(figure.get(&1), Some(&Value::Uint(0)), "asked for");
        assert_eq!(figure.get(&2), Some(&Value::Uint(0)), "found");
        assert_eq!(
            figure.get(&3),
            Some(&Value::Uint(0)),
            "and nobody it was drawn from, said rather than left out"
        );
    }

    #[test]
    fn what_the_network_is_running_is_a_question_this_node_takes() {
        assert_eq!(parse("GET", "/capacity"), Ok(Ask::Capacity));
    }

    #[test]
    fn an_empty_catalogue_lists_every_shelf_rather_than_none_of_them() {
        // **A missing key and an empty list are different facts.** Somebody comparing what is asked
        // for across a network needs to know whether nothing has been published or whether this
        // build does not list that kind — and an answer that left the shelf out would look the same
        // either way.
        let node = a_node();
        let said = answer(&node, &Ask::Catalogue, Epoch::GENESIS, &limits());
        assert_eq!(said.state, State::Here);

        let Ok(Value::Map(fields)) = almena_format::cbor::read(&said.body) else {
            panic!("a response is a canonical map");
        };
        let Some(Value::Map(listed)) = fields.get(&4) else {
            panic!("it carries the catalogue, got {fields:?}");
        };
        assert_eq!(listed.len(), 4, "every shelf, said rather than left out");
        assert!(
            listed.values().all(|shelf| *shelf == Value::Array(vec![])),
            "and nothing published on any of them yet"
        );
    }

    #[test]
    fn a_capability_nobody_offers_comes_back_as_nought_rather_than_left_out() {
        // **The whole point of counting it.** A figure that omitted it would read as a capability
        // nobody had thought of, when what it means is that nobody is running it — which is exactly
        // what somebody deciding what to contribute needs to see.
        let node = a_node();
        let said = answer(&node, &Ask::Capacity, Epoch::GENESIS, &limits());
        assert_eq!(said.state, State::Here);

        let Ok(Value::Map(fields)) = almena_format::cbor::read(&said.body) else {
            panic!("a response is a canonical map");
        };
        let Some(Value::Map(figure)) = fields.get(&4) else {
            panic!("it carries the figure, got {fields:?}");
        };
        let Some(Value::Map(offered)) = figure.get(&1) else {
            panic!("what is offered, got {figure:?}");
        };
        assert_eq!(offered.len(), 4, "every capability there is, counted");
        assert!(
            offered.values().all(|count| *count == Value::Uint(0)),
            "nobody has said what they are running"
        );
        assert_eq!(
            figure.get(&3),
            Some(&Value::Uint(0)),
            "and no node this record cannot read, said rather than left out"
        );
    }
}
