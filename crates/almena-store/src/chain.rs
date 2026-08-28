//! The chain each object advances along, and what its history says is true of it now.
//!
//! Every object has a chain of its own, and every operation on it names the one it follows. That
//! is what makes an operation that nobody understands cost so little: it spoils **that object**
//! and no other. There is no shared state to corrupt, so a node running an old version is behind
//! on some objects rather than wrong about everything.
//!
//! # What authorises an operation
//!
//! **The key the previous state authorised, and nothing else.** Not what a DID document says — a
//! document is a projection for other people's tools, and two sources of truth for one thing
//! diverge until somebody believes the weaker one. The chain decides.
//!
//! A holder's account is governed by one key and operated by others, and the two are kept apart on
//! purpose: the words behind the control key are the last resort, so a device that has been taken
//! must not be able to rotate them, and a control key signing alone must not be able to act as if
//! it were a device in somebody's hand.
//!
//! # Two operations claiming the same predecessor
//!
//! The object becomes one this node **declines to resolve**, and both operations are kept. No node
//! picks a branch — not the first one it saw, not the one in more roots. Choosing would put two
//! honest nodes in different states with nobody having lied, which is the one outcome this design
//! cannot afford. Somebody with the right to sign on that object settles it.

use std::collections::{BTreeMap, BTreeSet};

use almena_format::identifier::Name;
use almena_format::operation::Operation;
use almena_suite::{ed25519, p256};
use almena_time::{Clock, Epoch};

use crate::kind::Kind;

/// Where an operation carries the key it is about — a holder's control key, a node's own.
///
/// One field, odd because a reader that does not understand it cannot claim to have applied the
/// operation: an `add_device` whose key was skipped would read as an act that added nothing.
const KEY: u64 = 1;

/// Where the genesis carries the key it establishes Almena Government with.
///
/// Odd for the same reason: a reader that skipped it would be reading an act that opened a network
/// and created nothing to trust.
const GOVERNMENT_KEY: u64 = 3;

/// What a node can say when somebody asks about an object.
///
/// **Four answers, and none may be mistaken for another.** Saying *it does not exist* about
/// something that does is a lie; so is serving the state from before an operation nobody
/// understood. What is not resolved is said to whoever is going without it, along with which of
/// these happened to them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// No creation with that name has been seen.
    DoesNotExist,
    /// It exists, and this node will not say what it is.
    CannotResolve(Reason),
    /// It exists and is held elsewhere — the object is asleep and its state lives at the shared
    /// level, so this is one more query and not an absence.
    ///
    /// Nothing produces it yet, and it is here on purpose. A contract without this answer breeds
    /// clients that meet it for the first time in production and treat it as an error.
    NotHere,
    /// Here it is.
    Here(State),
}

/// What an object is, once its history has been read.
///
/// Two kinds so far. Every other object arrives with the work that builds it, and until then a
/// creation this build cannot apply is refused rather than stored as an object nobody could say
/// anything about — which would be worse than never having taken it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// A person's account.
    Holder(Holder),
    /// Almena Government, as the act that opened the network created it.
    ///
    /// It holds one key here and it will hold much more: owners, a threshold, the things an entity
    /// is governed by. Those arrive with entities, and putting a guess in their place now would be
    /// inventing state nobody decided.
    Government {
        /// The key the genesis established it with.
        key: [u8; ed25519::PUBLIC_KEY_WIDTH],
    },
    /// Two things one key signed that cannot both be true.
    ///
    /// The only thing that can be proved against a node, and it is proved by what the act carries
    /// rather than by anybody's word. Whether that key belongs to a node the network has heard of
    /// is a different question, answered by resolving that node's own name.
    Contradiction {
        /// Whose key said both.
        against: [u8; ed25519::PUBLIC_KEY_WIDTH],
    },
    /// A node, as the act that introduced it created it.
    ///
    /// One key, because a node has one and signs everything it says with it. What a node offers
    /// and what version it runs belong here too and are not here yet: they arrive with the mesh
    /// that has to read them, and standing in for them now would be inventing state nobody
    /// decided.
    Node {
        /// The key it announced itself with, which is what its word is checked against.
        key: [u8; ed25519::PUBLIC_KEY_WIDTH],
    },
}

/// Why a node will not say what an object is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Two operations claim the same predecessor.
    Forked,
    /// Its history contains an act this build does not know.
    Unintelligible,
}

/// What a holder's chain says about them right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    /// The key that governs the account. It comes from the words and never operates.
    pub control: [u8; ed25519::PUBLIC_KEY_WIDTH],
    /// The keys that operate it, one per device, each born inside that device.
    pub devices: BTreeSet<Vec<u8>>,
}

/// Why an operation was not admitted.
///
/// Being refused is not the same as being unintelligible: a refused operation was **never valid**
/// and is not stored, while one this node cannot read is valid as far as it knows and is kept and
/// passed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// A creation whose object is not the name its own bytes give it.
    DoesNotNameItself,
    /// A creation for an object that already exists.
    AlreadyExists,
    /// It follows an operation this node has never seen.
    NoSuchPredecessor,
    /// It declares a moment more than one epoch ahead of now.
    FromTheFuture,
    /// It carries no signature at all.
    Unsigned,
    /// It is signed by a key the previous state did not authorise for this act.
    NotAuthorised,
    /// The signature does not check out against the key that claims to have made it.
    SignatureDoesNotCheck,
    /// It is missing a field this act cannot be performed without, or one is the wrong shape.
    Malformed,
    /// An accusation that is not one.
    ///
    /// Two roots by different keys, for different epochs or networks, or two that are the same —
    /// none of those is a contradiction, and admitting one would let anybody write an accusation
    /// against anybody.
    NotAContradiction,
    /// This node could not write it down, so it did not take it.
    ///
    /// Nothing about the act: it is about the machine underneath. Answering *taken* for something
    /// that only reached memory would be telling somebody their act is kept when the next power
    /// cut takes it, and they would have no way of finding out.
    NotKept,
}

/// What happened to an operation that was accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admitted {
    /// It advanced the chain.
    Extended,
    /// It claimed a predecessor that already had a successor. Both are kept and the object is now
    /// one this node declines to resolve.
    Forked,
}

/// One object's chain.
#[derive(Debug, Clone)]
struct Chain {
    /// Every operation of this object that has arrived.
    seen: BTreeSet<Name>,
    /// Every operation that already has a successor. A second one is a fork.
    followed: BTreeSet<Name>,
    /// The latest operation, which the next one has to follow.
    head: Name,
    /// What the history says, when the history can be read.
    state: State,
    /// Two operations claimed the same predecessor.
    forked: bool,
    /// An act this build does not know is somewhere in the history.
    opaque: bool,
}

/// Every object this node holds.
#[derive(Debug, Clone, Default)]
pub struct Objects {
    chains: BTreeMap<Name, Chain>,
    /// Which node each key belongs to.
    ///
    /// **The census, and the only direction it is needed in.** A connection proves who holds a key;
    /// what the record has to supply is the name that key answers to, because everything written
    /// down about a node is written about its name and not about its key.
    nodes: BTreeMap<[u8; ed25519::PUBLIC_KEY_WIDTH], Name>,
}

impl Objects {
    /// A node holding nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many objects have been seen.
    #[must_use]
    pub fn len(&self) -> usize {
        self.chains.len()
    }

    /// Whether nothing has been seen at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chains.is_empty()
    }

    /// What this node says about an object.
    #[must_use]
    pub fn resolve(&self, name: &Name) -> Answer {
        let Some(chain) = self.chains.get(name) else {
            return Answer::DoesNotExist;
        };
        if chain.forked {
            return Answer::CannotResolve(Reason::Forked);
        }
        if chain.opaque {
            return Answer::CannotResolve(Reason::Unintelligible);
        }
        Answer::Here(chain.state.clone())
    }

    /// Take an operation in, or say why not.
    ///
    /// # Errors
    ///
    /// [`Refused`], naming which rule it broke.
    pub fn admit(&mut self, operation: &Operation, now: Epoch) -> Result<Admitted, Refused> {
        if !Clock::accepts(operation.issued, now) {
            return Err(Refused::FromTheFuture);
        }
        match operation.previous.clone() {
            None => self.create(operation),
            Some(previous) => self.advance(operation, &previous),
        }
    }

    /// A first operation, which brings an object into existence.
    fn create(&mut self, operation: &Operation) -> Result<Admitted, Refused> {
        if !operation.names_itself() {
            return Err(Refused::DoesNotNameItself);
        }
        let name = operation.object.name().clone();
        if self.chains.contains_key(&name) {
            return Err(Refused::AlreadyExists);
        }
        let state = match Kind::new(operation.kind) {
            Some(Kind::HOLDER_CREATE) => {
                let control = fixed(operation, KEY)?;
                // Nothing else could have signed it: the account does not exist until this act
                // does, so the only key its own state authorises is the one it establishes.
                check(operation, &control)?;
                State::Holder(Holder {
                    control,
                    devices: BTreeSet::new(),
                })
            }
            Some(Kind::GENESIS) => {
                // Self-signed, because there is nothing earlier for it to be signed against: the
                // anchor everything else is trusted from cannot be vouched for by something before
                // it.
                let key = fixed(operation, GOVERNMENT_KEY)?;
                check(operation, &key)?;
                State::Government { key }
            }
            Some(Kind::NODE_ANNOUNCE) => {
                // Self-signed, like the act that opens a network and for the same reason: nothing
                // earlier can vouch for something that did not exist until now. What it settles is
                // that this name and this key belong together, which is what a reader needs before
                // it can tell one node's word from another's.
                let key = fixed(operation, KEY)?;
                check(operation, &key)?;
                self.nodes.insert(key, name.clone());
                State::Node { key }
            }
            Some(Kind::CONTRADICTION_PUBLISH) => {
                // **It carries its own proof, so nobody has to be believed.** Whoever wrote it down
                // is not vouching for anything: what convinces is that one key signed two roots for
                // one epoch that cannot both be true, which anybody reading it can check.
                //
                // So the signature on the act only says who bothered, and is checked as any
                // creation's is — while what makes it admissible at all is the evidence inside.
                let against =
                    crate::contradiction::against(operation).ok_or(Refused::NotAContradiction)?;
                let publisher = first_key(operation)?;
                check(operation, &publisher)?;
                State::Contradiction { against }
            }
            // Every other object arrives with the work that builds it. Until then a creation this
            // node cannot apply is refused rather than stored as an object with no state.
            _ => return Err(Refused::Malformed),
        };

        let head = Name::of(&operation.to_bytes());
        self.chains.insert(
            name,
            Chain {
                seen: BTreeSet::from([head.clone()]),
                followed: BTreeSet::new(),
                head,
                state,
                forked: false,
                opaque: false,
            },
        );
        Ok(Admitted::Extended)
    }

    /// A later operation, which follows one already here.
    fn advance(&mut self, operation: &Operation, previous: &Name) -> Result<Admitted, Refused> {
        let name = operation.object.name().clone();
        let Some(chain) = self.chains.get(&name) else {
            return Err(Refused::NoSuchPredecessor);
        };
        if !chain.seen.contains(previous) {
            return Err(Refused::NoSuchPredecessor);
        }

        // A fork is kept rather than refused: both operations are valid as far as anybody can
        // tell, and it is the object that becomes unresolvable, not the second signer who becomes
        // wrong.
        if chain.followed.contains(previous) {
            let held = self
                .chains
                .get_mut(&name)
                .ok_or(Refused::NoSuchPredecessor)?;
            held.seen.insert(Name::of(&operation.to_bytes()));
            held.forked = true;
            return Ok(Admitted::Forked);
        }

        let kind = Kind::new(operation.kind);
        let state = chain.state.clone();
        let applied = match kind {
            Some(known) if known.known() => Some(apply(operation, &state, known)?),
            // Stored, propagated, and the object stops resolving. Refusing it would split the
            // record between versions, and nothing can tell an out-of-date node from a hostile
            // one — so nothing is ever given the chance to confuse them.
            _ => None,
        };

        let head = Name::of(&operation.to_bytes());
        let held = self
            .chains
            .get_mut(&name)
            .ok_or(Refused::NoSuchPredecessor)?;
        held.followed.insert(previous.clone());
        held.seen.insert(head.clone());
        held.head = head;
        match applied {
            Some(next) => held.state = next,
            None => held.opaque = true,
        }
        Ok(Admitted::Extended)
    }

    /// What the record calls the node that holds this key.
    ///
    /// [`None`] for a key no node ever announced itself with — which is what somebody speaking the
    /// protocol without being anybody looks like, and is a thing to be able to say rather than to
    /// guess around.
    #[must_use]
    pub fn node_called(&self, key: &[u8; ed25519::PUBLIC_KEY_WIDTH]) -> Option<&Name> {
        self.nodes.get(key)
    }

    /// What the chain of an object points at now, for whoever is building the next operation.
    #[must_use]
    pub fn head(&self, name: &Name) -> Option<&Name> {
        self.chains.get(name).map(|chain| &chain.head)
    }
}

/// The thirty-two byte key an act carries at `field`.
fn fixed(operation: &Operation, field: u64) -> Result<[u8; ed25519::PUBLIC_KEY_WIDTH], Refused> {
    let almena_format::cbor::Value::Bytes(bytes) =
        operation.payload.get(&field).ok_or(Refused::Malformed)?
    else {
        return Err(Refused::Malformed);
    };
    bytes.as_slice().try_into().map_err(|_| Refused::Malformed)
}

/// The device key a holder operation carries, whatever width the curve gives it.
fn device(operation: &Operation) -> Result<Vec<u8>, Refused> {
    let almena_format::cbor::Value::Bytes(bytes) =
        operation.payload.get(&KEY).ok_or(Refused::Malformed)?
    else {
        return Err(Refused::Malformed);
    };
    if bytes.len() != p256::PUBLIC_KEY_WIDTH {
        return Err(Refused::Malformed);
    }
    Ok(bytes.clone())
}

/// What the state becomes once this operation is applied to it.
fn apply(operation: &Operation, state: &State, kind: Kind) -> Result<State, Refused> {
    // A node saying what it saw of others changes nothing about what the node **is** — its key is
    // its key whatever it observed — so the state comes through untouched. What the act is for is
    // being in the record: the summary is the thing, and the chain is where it lives.
    if let (State::Node { key }, Kind::NODE_SUMMARY) = (state, kind) {
        check(operation, key)?;
        return Ok(state.clone());
    }

    // Only a holder's chain advances otherwise. Almena Government's acts arrive with entities —
    // what governs one is owners, thresholds and continuity — and the rest of a node's arrive with
    // what has to read them. Until then a later act is refused rather than applied against a state
    // that has no room for it.
    let State::Holder(holder) = state else {
        return Err(Refused::Malformed);
    };
    let mut next = holder.clone();
    match kind {
        Kind::HOLDER_ADD_DEVICE => {
            let added = device(operation)?;
            check_any(operation, holder)?;
            next.devices.insert(added);
        }
        Kind::HOLDER_REMOVE_DEVICE => {
            let removed = device(operation)?;
            check_any(operation, holder)?;
            // Devices take each other out; the control key is only ever replaced by the words. A
            // removal that could reach it would make a stolen device enough to take the account.
            if !next.devices.remove(&removed) {
                return Err(Refused::Malformed);
            }
        }
        Kind::HOLDER_ROTATE => {
            let control = fixed(operation, KEY)?;
            check(operation, &holder.control)?;
            next.control = control;
        }
        _ => return Err(Refused::Malformed),
    }
    Ok(State::Holder(next))
}

/// Whether this operation carries a good signature by exactly the key given.
/// The key on the first signature, for an act whose author is whoever signed it.
fn first_key(operation: &Operation) -> Result<[u8; ed25519::PUBLIC_KEY_WIDTH], Refused> {
    operation
        .signatures
        .first()
        .ok_or(Refused::Unsigned)?
        .key
        .as_slice()
        .try_into()
        .map_err(|_| Refused::Malformed)
}

fn check(operation: &Operation, control: &[u8; ed25519::PUBLIC_KEY_WIDTH]) -> Result<(), Refused> {
    let signature = operation.signatures.first().ok_or(Refused::Unsigned)?;
    if signature.key.as_slice() != control.as_slice() {
        return Err(Refused::NotAuthorised);
    }
    verify_control(operation, control)
}

/// Whether this operation carries a good signature by the control key or by a current device.
///
/// Which curve to check with is never guessed from the length of the key: the state says which key
/// this is, and therefore which curve made it.
fn check_any(operation: &Operation, holder: &Holder) -> Result<(), Refused> {
    let signature = operation.signatures.first().ok_or(Refused::Unsigned)?;
    if signature.key.as_slice() == holder.control.as_slice() {
        return verify_control(operation, &holder.control);
    }
    if !holder.devices.contains(&signature.key) {
        return Err(Refused::NotAuthorised);
    }
    let key: [u8; p256::PUBLIC_KEY_WIDTH] = signature
        .key
        .as_slice()
        .try_into()
        .map_err(|_| Refused::Malformed)?;
    let verifying = p256::VerifyingKey::from_bytes(key).map_err(|_| Refused::Malformed)?;
    let made = p256::Signature::from_bytes(signature.signature).map_err(|_| Refused::Malformed)?;
    verifying
        .verify(&operation.signing_bytes(), &made)
        .map_err(|_| Refused::SignatureDoesNotCheck)
}

/// Whether the control key made this signature.
fn verify_control(
    operation: &Operation,
    control: &[u8; ed25519::PUBLIC_KEY_WIDTH],
) -> Result<(), Refused> {
    let signature = operation.signatures.first().ok_or(Refused::Unsigned)?;
    let verifying = ed25519::VerifyingKey::from_bytes(*control).map_err(|_| Refused::Malformed)?;
    let made = ed25519::Signature::from_bytes(signature.signature);
    verifying
        .verify(&operation.signing_bytes(), &made)
        .map_err(|_| Refused::SignatureDoesNotCheck)
}

#[cfg(test)]
mod tests {
    use super::{Admitted, Answer, KEY, Objects, Reason, Refused, State};
    use crate::kind::Kind;
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::{Operation, Signed, create};
    use almena_suite::{ed25519, p256};
    use almena_time::{Epoch, Epochs};
    use std::collections::BTreeMap;

    /// The moment every one of these tests happens at.
    fn now() -> Epoch {
        Epoch::GENESIS.plus(Epochs(100)).expect("no overflow")
    }

    fn control_key(seed: u8) -> ed25519::SigningKey {
        ed25519::SigningKey::from_secret([seed; 32])
    }

    fn device_key(seed: u8) -> p256::SigningKey {
        p256::SigningKey::from_secret([seed; 32]).expect("a valid scalar")
    }

    fn carrying(key: &[u8]) -> BTreeMap<u64, Value> {
        BTreeMap::from([(KEY, Value::Bytes(key.to_vec()))])
    }

    /// A holder creation, signed by the control key it establishes.
    fn creation(control: &ed25519::SigningKey) -> Operation {
        let public = control.verifying_key().bytes();
        let mut operation = create(
            Network::Development,
            Kind::HOLDER_CREATE.number(),
            1,
            now(),
            carrying(&public),
        );
        let signature = control.sign(&operation.signing_bytes());
        operation.signatures.push(Signed {
            by: operation.object.clone(),
            key: public.to_vec(),
            signature: signature.bytes(),
        });
        operation
    }

    /// A later operation on the same object, unsigned.
    fn following(object: &Did, head: &Name, kind: Kind, key: &[u8]) -> Operation {
        Operation {
            object: object.clone(),
            previous: Some(head.clone()),
            kind: kind.number(),
            version: 1,
            issued: now(),
            payload: carrying(key),
            signatures: Vec::new(),
        }
    }

    fn signed_by_control(mut operation: Operation, control: &ed25519::SigningKey) -> Operation {
        let signature = control.sign(&operation.signing_bytes());
        operation.signatures.push(Signed {
            by: operation.object.clone(),
            key: control.verifying_key().bytes().to_vec(),
            signature: signature.bytes(),
        });
        operation
    }

    fn signed_by_device(mut operation: Operation, device: &p256::SigningKey) -> Operation {
        let signature = device.sign(&operation.signing_bytes());
        operation.signatures.push(Signed {
            by: operation.object.clone(),
            key: device.verifying_key().bytes().to_vec(),
            signature: signature.bytes(),
        });
        operation
    }

    /// An account with one device on it, and everything needed to act on it again.
    fn an_account() -> (Objects, Did, ed25519::SigningKey, p256::SigningKey) {
        let control = control_key(7);
        let device = device_key(9);
        let mut objects = Objects::new();

        let creation = creation(&control);
        let object = creation.object.clone();
        assert_eq!(objects.admit(&creation, now()), Ok(Admitted::Extended));

        let head = objects.head(object.name()).expect("a head").clone();
        let add = signed_by_control(
            following(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device.verifying_key().bytes(),
            ),
            &control,
        );
        assert_eq!(objects.admit(&add, now()), Ok(Admitted::Extended));

        (objects, object, control, device)
    }

    #[test]
    fn an_account_starts_with_its_control_key_and_no_devices() {
        let control = control_key(7);
        let mut objects = Objects::new();
        let creation = creation(&control);
        let object = creation.object.clone();

        assert_eq!(objects.admit(&creation, now()), Ok(Admitted::Extended));
        match objects.resolve(object.name()) {
            Answer::Here(State::Holder(holder)) => {
                assert_eq!(holder.control, control.verifying_key().bytes());
                assert!(
                    holder.devices.is_empty(),
                    "an account arrives with nothing on it"
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_object_nobody_has_seen_does_not_exist() {
        // And *does not exist* is a different answer from every other one. Saying it about
        // something that does exist would be a lie.
        let objects = Objects::new();
        assert_eq!(
            objects.resolve(&Name::of(b"never happened")),
            Answer::DoesNotExist
        );
    }

    #[test]
    fn a_creation_that_does_not_name_itself_is_refused() {
        let control = control_key(7);
        let mut lying = creation(&control);
        lying.object = Did::new(Network::Development, Name::of(b"some other operation"));

        let mut objects = Objects::new();
        assert_eq!(
            objects.admit(&lying, now()),
            Err(Refused::DoesNotNameItself)
        );
    }

    #[test]
    fn a_device_can_be_added_and_taken_out_again() {
        let (mut objects, object, _control, device) = an_account();
        let public = device.verifying_key().bytes().to_vec();

        match objects.resolve(object.name()) {
            Answer::Here(State::Holder(holder)) => assert!(holder.devices.contains(&public)),
            other => panic!("{other:?}"),
        }

        // And a device can take itself out, which is what somebody does with a phone they are
        // about to give away.
        let head = objects.head(object.name()).expect("a head").clone();
        let remove = signed_by_device(
            following(&object, &head, Kind::HOLDER_REMOVE_DEVICE, &public),
            &device,
        );
        assert_eq!(objects.admit(&remove, now()), Ok(Admitted::Extended));

        match objects.resolve(object.name()) {
            Answer::Here(State::Holder(holder)) => assert!(holder.devices.is_empty()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_key_the_account_never_authorised_cannot_act_on_it() {
        // The whole rule: what authorises is the key the previous state authorised. A stranger
        // with a perfectly good signature is still a stranger.
        let (mut objects, object, _control, _device) = an_account();
        let stranger = device_key(200);
        let head = objects.head(object.name()).expect("a head").clone();

        let add = signed_by_device(
            following(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(201).verifying_key().bytes(),
            ),
            &stranger,
        );
        assert_eq!(objects.admit(&add, now()), Err(Refused::NotAuthorised));
    }

    #[test]
    fn only_the_control_key_rotates_the_control_key() {
        // A device that has been taken must not be able to replace the words. That asymmetry is
        // the whole of what makes the words the last resort.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let fresh = control_key(11).verifying_key().bytes();

        let by_device = signed_by_device(
            following(&object, &head, Kind::HOLDER_ROTATE, &fresh),
            &device,
        );
        assert_eq!(
            objects.admit(&by_device, now()),
            Err(Refused::NotAuthorised)
        );

        let by_control = signed_by_control(
            following(&object, &head, Kind::HOLDER_ROTATE, &fresh),
            &control,
        );
        assert_eq!(objects.admit(&by_control, now()), Ok(Admitted::Extended));

        match objects.resolve(object.name()) {
            Answer::Here(State::Holder(holder)) => assert_eq!(holder.control, fresh),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rotating_leaves_the_name_alone() {
        // What recovery promises: the account is still the same account, only the key that
        // controls it changed. A name that moved would strand everything pointing at the old one.
        let (mut objects, object, control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let rotate = signed_by_control(
            following(
                &object,
                &head,
                Kind::HOLDER_ROTATE,
                &control_key(11).verifying_key().bytes(),
            ),
            &control,
        );
        objects.admit(&rotate, now()).expect("admitted");

        assert!(matches!(objects.resolve(object.name()), Answer::Here(_)));
        assert_eq!(objects.len(), 1);
    }

    #[test]
    fn a_signature_that_does_not_check_is_refused() {
        let (mut objects, object, control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let mut tampered = signed_by_control(
            following(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(21).verifying_key().bytes(),
            ),
            &control,
        );
        tampered.signatures[0].signature[0] ^= 0xff;

        assert_eq!(
            objects.admit(&tampered, now()),
            Err(Refused::SignatureDoesNotCheck)
        );
    }

    #[test]
    fn an_operation_following_nothing_this_node_has_is_refused() {
        let (mut objects, object, control, _device) = an_account();
        let add = signed_by_control(
            following(
                &object,
                &Name::of(b"an operation that never arrived"),
                Kind::HOLDER_ADD_DEVICE,
                &device_key(21).verifying_key().bytes(),
            ),
            &control,
        );
        assert_eq!(objects.admit(&add, now()), Err(Refused::NoSuchPredecessor));
    }

    #[test]
    fn a_moment_more_than_one_epoch_ahead_is_refused() {
        // The operation always declares `now()`. What moves is the clock of the node reading it,
        // so this is one node meeting the same operation from four positions in time.
        let control = control_key(7);
        let reading = |epochs| Epoch::GENESIS.plus(Epochs(epochs)).expect("no overflow");
        let cases = [
            (reading(150), true, "long past"),
            (reading(100), true, "this moment"),
            (
                reading(99),
                true,
                "a node one epoch slow, which is the drift the tolerance is for",
            ),
            (reading(98), false, "two epochs of future is not drift"),
        ];

        for (reading, accepted, why) in cases {
            let mut objects = Objects::new();
            let outcome = objects.admit(&creation(&control), reading);
            assert_eq!(outcome.is_ok(), accepted, "{why}");
            if !accepted {
                assert_eq!(outcome, Err(Refused::FromTheFuture), "{why}");
            }
        }
    }

    #[test]
    fn two_operations_claiming_the_same_predecessor_leave_the_object_unresolvable() {
        // Neither is refused, and no branch is chosen. Choosing would put two honest nodes in
        // different states with nobody having lied.
        let (mut objects, object, control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let one = signed_by_control(
            following(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(31).verifying_key().bytes(),
            ),
            &control,
        );
        let other = signed_by_control(
            following(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(32).verifying_key().bytes(),
            ),
            &control,
        );

        assert_eq!(objects.admit(&one, now()), Ok(Admitted::Extended));
        assert_eq!(objects.admit(&other, now()), Ok(Admitted::Forked));
        assert_eq!(
            objects.resolve(object.name()),
            Answer::CannotResolve(Reason::Forked)
        );
    }

    #[test]
    fn an_act_this_build_does_not_know_is_kept_and_stops_the_object_resolving() {
        // The two halves of the same rule: replicate what you do not understand, and never serve
        // the state from before it as though it were current.
        let (mut objects, object, control, _device) = an_account();
        let before = objects.resolve(object.name());
        assert!(matches!(before, Answer::Here(_)));

        let head = objects.head(object.name()).expect("a head").clone();
        let mut newer = following(
            &object,
            &head,
            Kind::HOLDER_ADD_DEVICE,
            &device_key(41).verifying_key().bytes(),
        );
        newer.kind = 9_999;
        let newer = signed_by_control(newer, &control);

        assert_eq!(
            objects.admit(&newer, now()),
            Ok(Admitted::Extended),
            "it is stored, not refused"
        );
        assert_eq!(
            objects.resolve(object.name()),
            Answer::CannotResolve(Reason::Unintelligible),
            "and the state from before it is never served as current"
        );
    }

    #[test]
    fn one_object_going_dark_leaves_every_other_alone() {
        // Why chains are per object: an operation nobody understands spoils that object and
        // nothing else. A node on an old version is behind on some accounts, never wrong about
        // all of them.
        let (mut objects, first, control, _device) = an_account();
        let second = creation(&control_key(77));
        let second_name = second.object.name().clone();
        objects.admit(&second, now()).expect("admitted");

        let head = objects.head(first.name()).expect("a head").clone();
        let mut newer = following(
            &first,
            &head,
            Kind::HOLDER_ADD_DEVICE,
            &device_key(41).verifying_key().bytes(),
        );
        newer.kind = 9_999;
        objects
            .admit(&signed_by_control(newer, &control), now())
            .expect("stored");

        assert!(matches!(
            objects.resolve(first.name()),
            Answer::CannotResolve(_)
        ));
        assert!(matches!(objects.resolve(&second_name), Answer::Here(_)));
    }

    #[test]
    fn taking_out_a_device_that_is_not_there_is_refused() {
        let (mut objects, object, control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let remove = signed_by_control(
            following(
                &object,
                &head,
                Kind::HOLDER_REMOVE_DEVICE,
                &device_key(99).verifying_key().bytes(),
            ),
            &control,
        );
        assert_eq!(objects.admit(&remove, now()), Err(Refused::Malformed));
    }

    #[test]
    fn an_operation_with_no_signature_is_refused() {
        let (mut objects, object, _control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let bare = following(
            &object,
            &head,
            Kind::HOLDER_ADD_DEVICE,
            &device_key(51).verifying_key().bytes(),
        );
        assert_eq!(objects.admit(&bare, now()), Err(Refused::Unsigned));
    }

    #[test]
    fn the_same_account_cannot_be_created_twice() {
        let control = control_key(7);
        let mut objects = Objects::new();
        let creation = creation(&control);
        objects.admit(&creation, now()).expect("admitted");
        assert_eq!(objects.admit(&creation, now()), Err(Refused::AlreadyExists));
    }
}
