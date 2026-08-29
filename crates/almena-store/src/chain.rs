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

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name};
use almena_format::operation::Operation;
use almena_suite::{ed25519, p256};
use almena_time::{Clock, Epoch};

use crate::checkpoint::{Claim, Governs, Stated};
use crate::kind::Kind;
use crate::parameter::{CONTROL_PENDING_MOST, CONTROL_WAITS, SUMMARISE_EVERY};

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
    /// What the party a decision was taken about has to say back.
    ///
    /// **Its own object, pointing at the decision** (`SPECS.md §7.8`). Appended to the decision's
    /// chain it would mean the party affected can add to what the decider said; needing the
    /// decider's agreement it would be no reply at all. So it is neither.
    Reply(Box<crate::reply::Reply>),
    /// One party's signed statement that it has checked another.
    ///
    /// **A chain of its own, pointing at its subject** (`SPECS.md §2.2`, `§4.9`). It does not live
    /// on the chain of the entity it is about, for the same reason a vote does not live on the
    /// proposal's: nobody writes in somebody else's chain, and being certified must not mean letting
    /// another party append to your own history.
    Certification(Box<crate::certification::Certification>),
    /// An issuer or a verifier, which hangs from an entity and is governed by that entity's owners.
    ///
    /// **It has no owners of its own**, and that is the design rather than an omission: it is a
    /// thing an organisation runs, so who may change it is the organisation's question and is
    /// answered by resolving the parent (`SPECS.md §2.3`).
    ///
    /// Boxed, because it is much the largest thing a state can be and every answer a node gives
    /// carries a state. Unboxed, every `DoesNotExist` would be the size of an organisation.
    Element(Box<crate::element::Element>),
    /// An organisation, governed by its owners and their thresholds.
    ///
    /// **Not a holder** (`SPECS.md §2.2`): no seed, no guardians, and no one key that is the last
    /// resort. What keeps it alive instead is that several people have to agree, counted here
    /// against the set of owners standing at the act's own moment.
    ///
    /// Boxed, for the reason the one above is.
    Entity(Box<crate::entity::Entity>),
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
        /// What it says it is running.
        ///
        /// **Empty until it says otherwise**, and that is a fact rather than a gap: the act that
        /// names a node carries its key and nothing else, because what it offers changes over its
        /// life and its name must not.
        offers: BTreeSet<crate::capability::Capability>,
        /// Which version of the protocol it says it speaks. Nought before it has said.
        speaks: u64,
        /// Who contributed it, once a claim has been written down and checked.
        ///
        /// **[`None`] is a node nobody has claimed**, which is a machine — and a machine cannot be
        /// credited for what it serves. It is not a fault and it is not rare: a node runs perfectly
        /// well unclaimed, it simply earns nothing by it.
        claimed_by: Option<Did>,
        /// Where it says it can be reached.
        ///
        /// **What it says, and not where anybody found it.** A node reached at an address it never
        /// published is reachable; a node that published one nobody can reach is not. Which of
        /// those is true is measured by asking, and mixing the two here would make this neither.
        reachable: BTreeSet<String>,
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
    /// Whether the account is stopped.
    ///
    /// **A frozen account is one where only *no* can be said.** Freezing is what somebody does the
    /// moment something is wrong — a device gone, words read over a shoulder — and it works
    /// because it denies everything and concedes nothing: no device may act, nothing may be
    /// presented or signed, and the one act that still lands is a cancellation, because a
    /// cancellation is itself a *no*.
    pub frozen: bool,
    /// What the control key has asked for that has not yet taken effect, oldest first.
    ///
    /// **The control key signing alone always waits.** It comes from words, and words can be
    /// stolen without the account's devices going anywhere — so what those words sign enters the
    /// record at once, where every device can see it, and lands only when the wait runs out. Any
    /// current device may cancel it first. The one exception is freezing, which concedes nothing
    /// and therefore has nothing worth stealing a wait for.
    pub waiting: Vec<Waiting>,
}

impl Holder {
    /// The account once everything due by that moment has taken effect.
    ///
    /// **The record holds the asking, and the effect trails it.** What the control key asked for
    /// sits in `waiting` from the moment it was written; whether it has landed is a question about
    /// *when* — so anybody reading the account has to say when they are asking about, and two
    /// readers asking about the same moment get the same account.
    ///
    /// In the order they entered, because that is the one order every reader shares.
    #[must_use]
    pub fn come_due(&self, at: Epoch) -> Self {
        let mut settled = Self {
            waiting: Vec::new(),
            ..self.clone()
        };
        for waiting in &self.waiting {
            if at.number() < waiting.due.number() {
                settled.waiting.push(waiting.clone());
                continue;
            }
            match &waiting.does {
                Does::AddDevice(key) => {
                    settled.devices.insert(key.clone());
                }
                // Gone already is gone: a device removed by another device during the wait leaves
                // nothing for the landing to do, and that is not a fault in either act.
                Does::RemoveDevice(key) => {
                    settled.devices.remove(key);
                }
                Does::Rotate(key) => settled.control = *key,
                Does::Unfreeze => settled.frozen = false,
            }
        }
        settled
    }
}

/// One thing the control key asked for that has not yet taken effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    /// The act that asked — which is what a cancellation names.
    pub act: Name,
    /// What it will do when the wait runs out.
    pub does: Does,
    /// The first epoch it is in force.
    pub due: Epoch,
}

/// What a waiting act will do when it lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Does {
    /// A device key joins the set that operates the account.
    AddDevice(Vec<u8>),
    /// A device key leaves it.
    RemoveDevice(Vec<u8>),
    /// The control key is replaced.
    Rotate([u8; ed25519::PUBLIC_KEY_WIDTH]),
    /// The account starts moving again.
    ///
    /// The one direction that waits: freezing denies, so it lands at once — thawing concedes
    /// everything the freeze stopped, so it is the words asking for trust back, and the devices
    /// get the same window to say no that they get over anything else the words ask alone.
    Unfreeze,
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
    /// The control key has as many acts waiting as it is allowed at once.
    ///
    /// **A ceiling, so that the words alone cannot make a chain cost the square of its length.**
    /// What the control key signs alone waits, and each thing waiting is an entry every reader
    /// walks on every later act — so an unbounded queue is an unbounded cost. Nothing honest comes
    /// near the cap; what does is somebody signing thousands with only the words.
    TooManyWaiting,
    /// It is dated before the act it follows.
    ///
    /// **The move a wait cannot survive.** What the control key signs alone lands a fixed number
    /// of epochs after the moment its author wrote on it, and that moment is a field the author
    /// fills in — so without this a thief of the words could date a removal at the genesis and
    /// have it land the instant it was admitted, with no window for any device to cancel it. An
    /// act may share its predecessor's epoch, but it may not precede it.
    BeforeItsPredecessor,
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
    /// This node already held it, so nothing happened and nothing needed to.
    ///
    /// **The ordinary case, and it must not be mistaken for a fork.** Acts arrive more than once by
    /// design — two peers holding one record hand over overlapping pages, a page is asked for again
    /// after a connection drops — and the same act twice is one act. Treating the second delivery as
    /// two acts claiming one predecessor would let a node make its own objects unresolvable simply
    /// by being told the truth twice.
    AlreadyHere,
    /// It claimed a predecessor that already had a successor. Both are kept and the object is now
    /// one this node declines to resolve.
    Forked,
    /// It is a resolution: somebody with the right to sign named the branch that survives.
    ///
    /// **Nothing has been applied yet**, and that is the whole of what this answer is for. A
    /// resolution puts already-signed operations out of effect, so the state it leaves cannot be
    /// reached by carrying on from where this node happened to be — the node has to replay the
    /// branch that was named, from its own record, which is what [`Objects::resolved`] does and
    /// what only a caller holding the acts can supply.
    Resolves,
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
    /// Which act last settled each part of the state.
    ///
    /// **What a summary is built out of.** The state alone cannot be summarised: a summary says
    /// *the devices are these, and this act put them there*, and only something that watched the
    /// chain go past knows the second half.
    set_by: BTreeMap<Governs, Name>,
    /// How many acts have gone by without one that carries a summary this node could read.
    since: u64,
    /// The latest epoch any act on this chain was dated at, which the next may equal but not
    /// precede. **What makes a wait un-rewindable**: an effect held back a fixed span from an
    /// act's own date is worth nothing if the date can be set to the past.
    dated: Epoch,
}

/// What the network says it is running, and how much of it could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Running {
    /// How many nodes say they offer each thing. Every capability there is, including the ones
    /// nobody offers — what is missing is the thing the figure is for.
    pub offering: BTreeMap<crate::capability::Capability, usize>,
    /// How many nodes say they speak each version of the protocol.
    ///
    /// **Not a health figure.** It is what decides when something new may start being used: whoever
    /// writes in a way half the network cannot read is the one who pays, so what they need is the
    /// number rather than a gate somebody else holds. Nought is a node that has never said.
    pub speaking: BTreeMap<u64, usize>,
    /// How many nodes this record cannot read at all.
    ///
    /// **Said rather than left out of the denominator.** A node whose chain has split, or carries an
    /// act this build has no meaning for, is a node that exists and offers something unknown —
    /// dropping it silently would make both figures look tidier than the network is, which is the
    /// one thing a measurement must not do.
    pub unreadable: usize,
}

/// Where an object stands on summarising itself.
///
/// **Not a judgement and not an obligation enforced here.** Whether the next act has to carry a
/// summary is a rule the signer keeps, because it depends on a number the protocol can change and
/// a node that refused over it would be refusing what a node on another version keeps — which is
/// the one disagreement this design cannot have. What a node does is say where the object stands
/// and let whoever is about to sign decide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Standing {
    /// What each part of the state is and which act last set it: the summary, ready to be signed.
    pub claims: Vec<Claim>,
    /// How many acts have gone by without one.
    pub since: u64,
    /// Whether the next act this object signs should carry a summary.
    pub owed: bool,
}

/// Every object this node holds.
#[derive(Debug, Clone, Default)]
pub struct Objects {
    chains: BTreeMap<Name, Chain>,
    /// Which node each key belongs to, and which act said so.
    ///
    /// **The census, and the only direction it is needed in.** A connection proves who holds a key;
    /// what the record has to supply is the name that key answers to, because everything written
    /// down about a node is written about its name and not about its key.
    ///
    /// Announcing is meant to happen again, and only the first one names anything — so a key that
    /// announces twice must not answer to two names. **First by what the acts say, never by what
    /// arrived first**: two nodes do not receive things in the same order, and a census that
    /// depended on that would have two honest nodes disagreeing about who somebody is.
    nodes: BTreeMap<[u8; ed25519::PUBLIC_KEY_WIDTH], (Epoch, Name)>,
    /// The objects this node knows exist and whose acts it does not hold.
    ///
    /// **Knowing something exists is universal; having what it said is not.** A node that answered
    /// *it does not exist* about one of these would be lying about something it can see in its own
    /// record — and one that answered with a state would be answering from a history it has not
    /// got. What it says instead is that the object is held elsewhere, which is one more question
    /// and not an absence.
    elsewhere: BTreeSet<Name>,
    /// Which entity each verified domain is bound to, and when it was bound.
    ///
    /// **A domain in the record belongs to one entity** (`SPECS.md §7.5`). DNS takes several
    /// records at one name, so an administrator can publish two claims and both pass the
    /// bidirectional check — and without this the register would hold two entities each entitled to
    /// the same name, with no way to tell them apart from outside.
    ///
    /// **The tie-break is not the register's**, and this is not one: a second claim on a domain
    /// already bound is refused *until the first releases it*, which puts the decision back where it
    /// belongs — with whoever controls the domain, by taking down the record they did not mean to
    /// publish.
    ///
    /// **First by what the acts say, never by what arrived first**, for the same reason the census
    /// above is: two nodes do not receive things in the same order.
    domains: BTreeMap<String, (Epoch, Name)>,
    /// Which entity holds each name, and since when.
    ///
    /// **A name carries somebody's reputation**, so two entities holding one would be the confusion
    /// the whole of `SPECS.md §7.5` exists to prevent: somebody arriving looking for one party and
    /// finding another. A name still cooling after its holder gave it up or lost it counts as held.
    aliases: BTreeMap<String, (Epoch, Name)>,
    /// Every certification written about each subject, by the party it is about.
    ///
    /// **Indexed by subject and not by issuer**, because that is the question anybody asks: *what
    /// has been said about this entity*. Whether any of it is worth anything is the reader's to
    /// judge — this only makes it findable by the party affected rather than by whoever bothered to
    /// write it down.
    certified: BTreeMap<Did, BTreeSet<Name>>,
    /// Almena Government's own name, learnt from the act that opened the network.
    ///
    /// **Learnt rather than configured**, because a node that carried it would be a node whose
    /// answer about who Almena is could differ from the record's.
    government: Option<Name>,
    /// The keys shown to have signed two things that cannot both be true.
    ///
    /// **By key and not by name**, because that is what the evidence establishes: two signatures.
    /// Whether the key belongs to a node anybody has heard of is a different question, and reading
    /// the index through a census would make the same act land under two names on two honest nodes.
    contradicted: BTreeSet<[u8; ed25519::PUBLIC_KEY_WIDTH]>,
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
            // Known to exist and not held here, which is one more question rather than an absence.
            // Saying *it does not exist* about something this node can see in its own record would
            // be the plainest lie it could tell.
            return if self.elsewhere.contains(name) {
                Answer::NotHere
            } else {
                Answer::DoesNotExist
            };
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
        signed_as_required(operation)?;
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
        let governor = self.governing_the_creation(operation);
        let state = born(
            operation,
            &Speaks {
                owners: &governor.owners,
                thresholds: governor.thresholds,
                alone: governor.alone,
                claimant: None,
                answering: governor.answering.clone(),
            },
        )?;
        // **The one act that establishes who controls an object, asked the same question as every
        // act after it** (`SPECS.md §4.8`, rule 4). A creation never reaches `apply`, so without
        // this the field check begins at the *second* act — and the field it would have caught on
        // the first is the one that says who governs the whole thing. `born` has just checked the
        // signature, which is what makes going opaque something only the key it establishes can do.
        //
        // **Every class this build has a vocabulary for, and not only a holder.** A node's own
        // chain is created the same way and read the same way, and rule 4 governs the payload
        // rather than what the payload is about — so exempting it would leave the same silent
        // disaster in a place nobody would think to look for it.
        let opaque = created_vocabulary(&state)
            .is_some_and(|vocabulary| operation.understood(vocabulary).is_err());
        if let State::Contradiction { against } = &state {
            self.contradicted.insert(*against);
        }
        if matches!(state, State::Government { .. }) {
            // Learnt from the act that opened the network rather than carried, so that this node's
            // answer about who Almena is cannot differ from the record's.
            self.government = Some(name.clone());
        }
        if let State::Certification(certification) = &state {
            self.certified
                .entry(certification.subject.clone())
                .or_default()
                .insert(name.clone());
        }
        if let State::Node { key, .. } = &state {
            let announced = (operation.issued, name.clone());
            self.nodes
                .entry(*key)
                .and_modify(|held| {
                    if announced < *held {
                        *held = announced.clone();
                    }
                })
                .or_insert(announced);
        }

        self.opened(operation, name, state, opaque);
        Ok(Admitted::Extended)
    }

    /// Write down a chain that did not exist a moment ago.
    fn opened(&mut self, operation: &Operation, name: Name, state: State, opaque: bool) {
        let head = operation.called();
        // It is held here now, so it is no longer somewhere else.
        self.elsewhere.remove(&name);
        let set_by = settles(Kind::new(operation.kind))
            .map(|part| (part, head.clone()))
            .collect();
        self.chains.insert(
            name,
            Chain {
                seen: BTreeSet::from([head.clone()]),
                followed: BTreeSet::new(),
                head,
                state,
                forked: false,
                opaque,
                set_by,
                // The act that brings an object into existence is one more act to replay, so it
                // counts like any other. An account nobody has touched since is one act behind.
                since: 1,
                dated: operation.issued,
            },
        );
    }

    /// What one act does to an object whose chain has not split.
    ///
    /// **Nothing can be applied against a state that stopped being computable** — but whether
    /// somebody was entitled to write here is still a question this node can put, and it must. An
    /// object it has given up on is not one anybody at all may extend: taking unsigned acts on it
    /// would put things in this node's record that no other node holds, which is the worse
    /// divergence of the two. The cost is the other one — an act authorised by a key established in
    /// the act this node could not read is refused here.
    fn applying(
        &mut self,
        operation: &Operation,
        name: &Name,
        state: &State,
        opaque: bool,
    ) -> Result<Applied, Refused> {
        // **Before anything is applied**, because a name already bound elsewhere is not a smaller
        // act — it is one the record already answered, and applying it would give one name two
        // holders.
        self.domain_is_free(operation, name)?;
        self.alias_is_free(operation, name)?;

        // Who governs this object is that object's own chains' answer, never this act's, and it is
        // resolved here where the record is.
        let governor = self.owners_and_thresholds(operation, state);
        let speaks = Speaks {
            owners: &governor.owners,
            thresholds: governor.thresholds,
            alone: governor.alone,
            claimant: self.speaks_for_claimant(operation),
            answering: governor.answering.clone(),
        };
        if opaque {
            entitled(operation, state, &speaks)?;
            return Ok(Applied::Beyond);
        }
        apply(operation, state, Kind::new(operation.kind), &speaks)
    }

    /// What one more act does to an object whose chain has split.
    ///
    /// **One act settles it and every other one is only kept.** A resolution is the one that
    /// settles: nothing about it is decided here, because the branch it keeps is the one it names,
    /// and whether its signer was entitled is answered by replaying that branch — which only a
    /// caller holding the acts can supply.
    ///
    /// Everything else is **kept without effect**, and that is a correction rather than a nicety.
    /// Once a chain has split its state is one branch's, frozen at the moment the split was noticed;
    /// carrying on applying acts to it would move a state nobody may read, and an act extending the
    /// *other* branch would be applied against a state it never followed. What a split object owes
    /// is one answer — *ask somebody who may sign* — until somebody does.
    fn on_a_fork(
        &mut self,
        operation: &Operation,
        on: &Split<'_>,
        head: Name,
    ) -> Result<Admitted, Refused> {
        if on.already && crate::resolution::declared(operation) {
            return Ok(Admitted::Resolves);
        }
        self.split(operation, on.name, head, on.state)
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
        // **Already held is not a fork.** A fork is two *different* acts claiming one predecessor;
        // the same act arriving twice is one act, and the second delivery leaves the chain holding
        // exactly what it held before. Asked after the predecessor, so that an act nobody has the
        // start of is still refused rather than quietly accepted.
        let head = operation.called();
        if chain.seen.contains(&head) {
            return Ok(Admitted::AlreadyHere);
        }
        // Everything an act must be to belong here at all, before anything is done with it.
        // Checked after the duplicate short-circuit, so an old act heard again is still one act.
        belongs_here(operation, chain.dated)?;
        let (state, opaque, forking) = (
            chain.state.clone(),
            chain.opaque,
            chain.followed.contains(previous),
        );

        if chain.forked || forking {
            return self.on_a_fork(
                operation,
                &Split {
                    name: &name,
                    state: &state,
                    already: chain.forked,
                },
                head,
            );
        }

        // Nothing can be *applied* against a state that stopped being computable — but whether
        // somebody was entitled to write here is still a question this node can put, and it must.
        // An object it has stopped resolving is not an object anybody at all may extend: taking
        // unsigned acts on it would put things in this node's record that no other node holds,
        // which is the worse divergence of the two. The cost is the other one — an act authorised
        // by a key established in the act this node could not read is refused here.
        // Whoever is claiming this node is somebody with a chain of their own, and which key speaks
        // for them is that chain's answer — never this act's. Resolved here, where the record is.
        let applied = self.applying(operation, &name, &state, opaque)?;

        self.moved_on(
            operation,
            &Following {
                name: &name,
                previous,
                head,
                applied,
            },
        )?;
        self.domain_bound(operation, &name);
        self.alias_held(operation, &name);
        Ok(Admitted::Extended)
    }

    /// Write down what one more act did to a chain that was already here.
    ///
    /// The four things one act moves, gathered so that where they are written is one place and the
    /// order they are written in is visible.
    fn moved_on(&mut self, operation: &Operation, on: &Following<'_>) -> Result<(), Refused> {
        let Following {
            name,
            previous,
            head,
            applied,
        } = on;
        let held = self
            .chains
            .get_mut(name)
            .ok_or(Refused::NoSuchPredecessor)?;
        held.followed.insert((*previous).clone());
        held.seen.insert(head.clone());
        for part in settles(Kind::new(operation.kind)) {
            held.set_by.insert(part, head.clone());
        }
        held.head = head.clone();
        // The chain has reached this act's epoch, which the next may equal but not go under. It
        // only ever rises: the check above refused anything earlier, so the max is the latest.
        held.dated = operation.issued;
        // **A summary this node could read**, not merely one that is there. A field it cannot make
        // sense of has told it nothing, and letting that clear the count would let an object put
        // anything at all in that field and never owe a summary again.
        held.since = if matches!(crate::checkpoint::declared(operation), Ok(Some(_))) {
            0
        } else {
            held.since.saturating_add(1)
        };
        match applied {
            Applied::State(next) => held.state = next.clone(),
            Applied::Beyond => held.opaque = true,
        }
        Ok(())
    }

    /// What the network says it is running, counted across every node the record names.
    ///
    /// **The network's capacity, counted rather than declared.** A node that has never said what it
    /// offers is counted as offering nothing — which is what the record says about it, and the
    /// reason the figure is worth having: what is missing is visible before it is a problem, and
    /// whoever wants to contribute can see what to contribute.
    ///
    /// It counts what nodes **say**. What they actually do is measured separately and by asking; a
    /// figure that mixed the two would be neither.
    #[must_use]
    pub fn running(&self) -> Running {
        let mut running = Running {
            offering: crate::capability::Capability::ALL
                .into_iter()
                .map(|what| (what, 0))
                .collect(),
            speaking: BTreeMap::new(),
            unreadable: 0,
        };

        for named in self.nodes() {
            let Some(State::Node { offers, speaks, .. }) = self.state_of(named) else {
                running.unreadable += 1;
                continue;
            };
            for what in offers {
                *running.offering.entry(*what).or_insert(0) += 1;
            }
            *running.speaking.entry(*speaks).or_insert(0) += 1;
        }
        running
    }

    /// What a name resolves to, when it resolves at all.
    fn state_of(&self, name: &Name) -> Option<&State> {
        let chain = self.chains.get(name)?;
        (!chain.forked && !chain.opaque).then_some(&chain.state)
    }

    /// Take note that an object exists without holding what its acts said.
    ///
    /// **What a shared-out history looks like from the inside.** Every node carries the line saying
    /// an act happened; only the nodes it was dealt to carry what it said. This is how the first
    /// becomes an answer — *held elsewhere* — instead of the object looking like nothing at all.
    ///
    /// An object whose acts this node does hold is left alone: having the history is strictly more
    /// than knowing it exists, and this must never take anything away.
    pub fn noted(&mut self, object: &Name) {
        if !self.chains.contains_key(object) {
            self.elsewhere.insert(object.clone());
        }
    }

    /// The key that speaks for whoever a binding names, if the record names them at all.
    ///
    /// **From their own chain and never from the act.** An act that vouched for the key it was
    /// checked against would vouch for anybody — the same reason a root is held to the key the
    /// record says its node has.
    ///
    /// [`None`] for anything that is not a binding, and for one naming somebody this node cannot
    /// resolve: a claim on behalf of somebody nobody has heard of is not a weaker claim, it is the
    /// node's word about a stranger.
    fn speaks_for_claimant(
        &self,
        operation: &Operation,
    ) -> Option<[u8; ed25519::PUBLIC_KEY_WIDTH]> {
        let (approval, _) = crate::bind::claimed(operation)?;
        match self.resolve(approval.claimant.name()) {
            // The key in force at the act's own moment: a rotation still waiting has not happened
            // yet, and one that has landed has.
            Answer::Here(State::Holder(holder)) => Some(holder.come_due(operation.issued).control),
            _ => None,
        }
    }

    /// Settle a forked object by replaying the branch a resolution named.
    ///
    /// `along` is that branch whole, oldest first, ending with the resolution itself. **Replayed
    /// rather than believed**: every act is validated again in order, so what authorised the
    /// resolution is the state its own branch produced, and nobody's summary of it. A node that
    /// does not hold one of the acts cannot do this and says so, which is its own limit and not a
    /// rule about the object (`SPECS.md §4.6`).
    ///
    /// The losing branches are **kept and left without effect** (`SPECS.md §4.9`). Their authors can
    /// see that they landed nowhere, and repeat them if they still want them.
    ///
    /// # Errors
    ///
    /// [`Refused::NoSuchPredecessor`] where the acts do not join up into the branch that was named,
    /// and whatever admitting one of them refuses.
    pub fn resolved(
        &mut self,
        whose: &Name,
        along: &[Operation],
        now: Epoch,
    ) -> Result<(), Refused> {
        let (settling, rest) = along.split_last().ok_or(Refused::NoSuchPredecessor)?;
        if !crate::resolution::declared(settling) {
            return Err(Refused::NotAuthorised);
        }
        // The branch has to be the one the act named, act by act, or it is not that branch.
        let mut following: Option<Name> = None;
        for act in rest.iter().chain([settling]) {
            if act.object.name() != whose || act.previous != following {
                return Err(Refused::NoSuchPredecessor);
            }
            following = Some(act.called());
        }

        // **Replayed beside the rest of the record and not in a vacuum.** Who may sign for an
        // organisation is settled by resolving its owners' own chains, so a rebuild that could not
        // see them would refuse every act it replayed. The copy costs what it costs: settling a
        // fork is rare, and reaching the wrong answer cheaply is not a saving.
        let mut rebuilding = self.clone();
        let held = rebuilding
            .chains
            .remove(whose)
            .ok_or(Refused::NoSuchPredecessor)?;
        // The branch that survives may not carry a domain the losing one claimed, so what this
        // object holds is let go of first and taken again by the replay. Otherwise a name would
        // stay bound by an act that no longer has any effect.
        rebuilding.domains.retain(|_, (_, bound)| bound != whose);

        for act in rest.iter().chain([settling]) {
            rebuilding.admit(act, now)?;
        }

        let mut settled = rebuilding
            .chains
            .remove(whose)
            .ok_or(Refused::NoSuchPredecessor)?;
        // Everything that ever arrived is still known to have arrived — the branches that lost are
        // kept, without effect, so their authors can see where they landed.
        settled.seen.extend(held.seen);
        settled.forked = false;

        // **Nothing here was touched until the replay came through.** A node left holding half a
        // rebuilt chain would be one in a state no act produced.
        self.domains = rebuilding.domains;
        self.nodes = rebuilding.nodes;
        self.chains.insert(whose.clone(), settled);
        Ok(())
    }

    /// Whether the domain an act claims is one no other entity holds.
    ///
    /// **Refused rather than resolved.** Two entities holding one name is a state the register
    /// cannot recover from by choosing, so the second claim waits until the first releases — and who
    /// releases is decided by whoever controls the domain, not here (`SPECS.md §7.5`).
    ///
    /// An act that claims a domain this same entity already holds is not a second claim: it is the
    /// revalidation `SPECS.md §7.4` asks for every thirty days, and refusing it would make a domain
    /// impossible to keep.
    fn domain_is_free(&self, operation: &Operation, whose: &Name) -> Result<(), Refused> {
        let Some(domain) = claimed_domain(operation) else {
            return Ok(());
        };
        match self.domains.get(&domain) {
            Some((_, held)) if held != whose => Err(Refused::NotAuthorised),
            _ => Ok(()),
        }
    }

    /// Whether the name an act claims is one nobody else holds, and whether this entity may have one.
    ///
    /// **The domain decides which name; the seal decides whether it may have one at all**
    /// (`SPECS.md §7.5`). Proving a domain does not prove identity — `banco-santander-clientes.com`
    /// is registrable by anybody — so without the seal that person would hold that name with
    /// perfect technical legitimacy.
    ///
    /// # Errors
    ///
    /// [`Refused::NotAuthorised`] for an entity with no live certification from Almena, or a name
    /// somebody else holds or has not finished cooling.
    fn alias_is_free(&self, operation: &Operation, whose: &Name) -> Result<(), Refused> {
        let Some(Value::Text(name)) = operation
            .payload
            .get(&crate::entity::field::ALIAS)
            .filter(|_| Kind::new(operation.kind) == Some(Kind::ENTITY_SET_ALIAS))
        else {
            return Ok(());
        };
        // **The act's own object**, which carries the network as well as the name — rather than a
        // network this function would have had to decide, which is a decision it has no business
        // making.
        if !self.sealed_by_almena(&operation.object, operation.issued) {
            return Err(Refused::NotAuthorised);
        }
        match self.aliases.get(name.as_str()) {
            Some((_, held)) if held != whose => Err(Refused::NotAuthorised),
            _ => Ok(()),
        }
    }

    /// Write down which entity holds a name, and let go of one that has been given up.
    fn alias_held(&mut self, operation: &Operation, whose: &Name) {
        let Answer::Here(State::Entity(entity)) = self.resolve(whose) else {
            return;
        };
        if let Some(alias) = &entity.alias {
            let held = (operation.issued, whose.clone());
            self.aliases
                .entry(alias.name.clone())
                .and_modify(|there| {
                    // First by what the acts say, never by what arrived first.
                    if held < *there {
                        *there = held.clone();
                    }
                })
                .or_insert(held);
        }
        // A name still cooling is still held: nobody inherits the reputation of whoever left until
        // three months have passed (`SPECS.md §7.5`).
        if let Some(cooling) = &entity.cooling
            && operation.issued.number() >= cooling.until.number()
            && self
                .aliases
                .get(&cooling.name)
                .is_some_and(|(_, there)| there == whose)
        {
            self.aliases.remove(&cooling.name);
        }
    }

    /// Whether Almena has certified that entity, and the certification still stands.
    ///
    /// **Almena's and not anybody's.** `SPECS.md §7.3` lets anybody certify anybody, and what a
    /// certification is worth is the reader's to judge — but the alias is one of the three things
    /// `SPECS.md §7.3` says the *seal* unlocks, and the seal is Almena's.
    fn sealed_by_almena(&self, subject: &Did, at: Epoch) -> bool {
        let Some(government) = self.government.as_ref() else {
            return false;
        };
        self.certified.get(subject).is_some_and(|written| {
            written.iter().any(|about| {
                matches!(
                    self.resolve(about),
                    Answer::Here(State::Certification(held))
                        if held.by.name() == government && held.stands(at)
                )
            })
        })
    }

    /// Write down which entity a domain is bound to, or that it has been released.
    fn domain_bound(&mut self, operation: &Operation, whose: &Name) {
        let Some(domain) = claimed_domain(operation) else {
            // Giving one up frees it for whoever proves it next.
            if let Some(released) = released_domain(operation)
                && self
                    .domains
                    .get(&released)
                    .is_some_and(|(_, held)| held == whose)
            {
                self.domains.remove(&released);
            }
            return;
        };
        let bound = (operation.issued, whose.clone());
        self.domains
            .entry(domain)
            .and_modify(|held| {
                // **First by what the acts say, never by what arrived first.** Two nodes do not
                // receive things in the same order, and a binding that depended on that would have
                // two honest nodes disagreeing about whose name it is.
                if bound < *held {
                    *held = bound.clone();
                }
            })
            .or_insert(bound);
    }

    /// The keys the record says speak for each owner of that object, at that moment.
    ///
    /// **Resolved from each owner's own chain**, because an owner is a root identifier and not a
    /// key (`SPECS.md §8.5`): rotating, recovering with guardians or changing phone must not cost
    /// somebody their place in an organisation.
    ///
    /// **A frozen owner contributes none.** Freezing denies everything and concedes nothing
    /// (`SPECS.md §11.12`), and signing for an organisation is conceding — so somebody who stopped
    /// their account because a device was taken does not go on signing governance from it. What
    /// they can still do is what freezing leaves anybody: say no.
    ///
    /// An owner this node cannot resolve contributes none either, and that is this node's own
    /// ignorance rather than a rule: it means fewer signatures counted and an act refused, which is
    /// recoverable by asking a node that holds them.
    fn speaking_for(&self, owners: &BTreeSet<Did>, at: Epoch) -> crate::entity::Speaking {
        owners
            .iter()
            .filter_map(|owner| match self.resolve(owner.name()) {
                Answer::Here(State::Holder(holder)) => {
                    let holder = holder.come_due(at);
                    (!holder.frozen).then(|| (owner.clone(), holder.devices))
                }
                _ => None,
            })
            .collect()
    }

    /// The same, for an act that creates an object nothing yet says who governs.
    ///
    /// There is no state to read it out of, so it comes from the payload and is then **checked
    /// against the record like anything else**: naming somebody is not being them.
    ///
    /// - An entity names its **first owner**, who has to already exist and has to sign.
    /// - An element names the **entity it hangs from**, whose owners have to sign at the class the
    ///   creation costs — which is the second half of the bidirectional link `SPECS.md §2.3` asks
    ///   for, got without a second act and without a window in which the claim stands unanswered.
    fn governing_the_creation(&self, operation: &Operation) -> Governor {
        let empty = Governor::nobody;
        match Kind::new(operation.kind) {
            Some(Kind::ENTITY_CREATE) => {
                let Some(Value::Text(who)) = operation.payload.get(&crate::entity::field::WHO)
                else {
                    return empty();
                };
                let Ok(owner) = Did::parse(who) else {
                    return empty();
                };
                Governor {
                    owners: self.speaking_for(&BTreeSet::from([owner]), operation.issued),
                    thresholds: None,
                    alone: None,
                    answering: None,
                }
            }
            Some(Kind::CERTIFICATION_ISSUE) => {
                let Some(by) = crate::certification::issuer(operation) else {
                    return empty();
                };
                self.governed_by(&by, operation.issued)
            }
            Some(Kind::REPLY_PUBLISH) => {
                // **Who the decision was about**, resolved from the decision rather than taken from
                // the reply — an act that named its own author would let anybody answer in somebody
                // else's name. And it is that party's organisation that signs, at what its routine
                // acts cost, because saying something concedes nothing.
                let Some(answers) = crate::reply::answers(operation) else {
                    return empty();
                };
                let Answer::Here(State::Certification(decision)) = self.resolve(&answers) else {
                    return empty();
                };
                let mut governor = self.governed_by(&decision.subject, operation.issued);
                governor.answering = Some(decision.subject.clone());
                governor
            }
            Some(Kind::ISSUER_CREATE) => {
                let Some(Value::Text(of)) = operation.payload.get(&crate::element::field::OF)
                else {
                    return empty();
                };
                let Ok(parent) = Did::parse(of) else {
                    return empty();
                };
                self.governed_by(&parent, operation.issued)
            }
            _ => empty(),
        }
    }

    /// The owners of an organisation, and what each class of act costs them.
    ///
    /// Nothing where this node cannot resolve that organisation, which refuses the act rather than
    /// guessing a number — its own ignorance, and recoverable by asking a node that holds it.
    fn governed_by(&self, entity: &Did, at: Epoch) -> Governor {
        match self.resolve(entity.name()) {
            Answer::Here(State::Entity(held)) => {
                let held = held.come_due(at);
                Governor {
                    owners: self.speaking_for(&held.owners, at),
                    thresholds: Some(held.thresholds),
                    alone: None,
                    answering: None,
                }
            }
            // Almena Government, while the one key the genesis gave it is all it has.
            Answer::Here(State::Government { key }) => Governor {
                owners: crate::entity::Speaking::new(),
                thresholds: None,
                alone: Some(key),
                answering: None,
            },
            _ => Governor::nobody(),
        }
    }

    /// What the record says about everybody this act leans on, and what it costs them.
    ///
    /// **Both resolved here, where the record is.** An act that carried either would be an act
    /// deciding who signed it and how many that had to be.
    fn owners_and_thresholds(&self, operation: &Operation, state: &State) -> Governor {
        match state {
            State::Entity(entity) => {
                let entity = entity.come_due(operation.issued);
                Governor {
                    owners: self.speaking_for(&entity.owners, operation.issued),
                    thresholds: Some(entity.thresholds),
                    alone: None,
                    answering: None,
                }
            }
            // An element is governed by its parent, so both answers come from resolving the parent.
            // One this node cannot resolve gives neither, which refuses the act rather than
            // guessing a number — its own ignorance, and recoverable by asking a node that holds it.
            State::Element(element) => self.governed_by(&element.of, operation.issued),
            // A certification is its issuer's statement, so who may change it is the issuer's
            // question — never the subject's, who does not get to edit what was said about them.
            State::Certification(certification) => {
                self.governed_by(&certification.by, operation.issued)
            }
            _ => Governor::nobody(),
        }
    }

    /// Whether somebody this object authorises signed this act.
    ///
    /// **For an act this node knows happened and is being handed what it said.** The entry says an
    /// act of that name happened; the bytes claim to be it, and since the name covers everything
    /// but the signatures, matching the name means the content matches and only the signature is
    /// still open. Left unchecked, whoever handed them over could sign with a key they made that
    /// morning and have this node serve it under a name it vouches for.
    ///
    /// It is checked against the state as it stands **now**, which is the only state this node has:
    /// an act signed by a device that has since been removed is refused, though it was good when it
    /// was written. That is the conservative direction and it is recoverable — the act can be asked
    /// for again, and the object goes on resolving meanwhile.
    #[must_use]
    pub fn vouches_for(&self, operation: &Operation) -> bool {
        let name = operation.object.name();
        self.chains.get(name).is_some_and(|chain| {
            let governor = self.owners_and_thresholds(operation, &chain.state);
            entitled(
                operation,
                &chain.state,
                &Speaks {
                    owners: &governor.owners,
                    thresholds: governor.thresholds,
                    alone: governor.alone,
                    claimant: None,
                    answering: governor.answering.clone(),
                },
            )
            .is_ok()
        })
    }

    /// Who contributed a node, if anybody has claimed it and the record checked out.
    ///
    /// [`None`] is a node nobody has claimed, which is a machine — and a machine cannot be credited
    /// for what it serves. Not a fault and not rare: a node runs perfectly well unclaimed.
    #[must_use]
    pub fn claimed_by(&self, node: &Name) -> Option<Did> {
        match self.resolve(node) {
            Answer::Here(State::Node { claimed_by, .. }) => claimed_by,
            _ => None,
        }
    }

    /// Whether an object is one every node has to keep whatever it was dealt.
    ///
    /// **What the share-out is drawn from cannot itself be shared out.** The share is worked out
    /// from the network's name and from who the nodes are, so a node that let go of those could no
    /// longer work out what it was supposed to keep — and would have let go of the answer to the
    /// question it needed the answer to. The act that opened the network is the same case: it is
    /// where a node reads which network it is on.
    #[must_use]
    pub fn everybody_keeps(&self, name: &Name) -> bool {
        matches!(
            self.chains.get(name).map(|chain| &chain.state),
            Some(State::Node { .. } | State::Government { .. })
        )
    }

    /// Whether the record holds proof that this key signed two things that cannot both be true.
    ///
    /// **It is not a verdict and nothing here acts on it.** What a network without permission can
    /// impose is one thing only — that whoever contradicts themselves stops earning the right to
    /// write — and everything else is decided by whoever is relying on the answer. This is how they
    /// get to decide it: it says what the record proves, and leaves what to do about it alone.
    #[must_use]
    pub fn contradicted(&self, key: &[u8; ed25519::PUBLIC_KEY_WIDTH]) -> bool {
        self.contradicted.contains(key)
    }

    /// Every node the record names, in the order their keys sort.
    ///
    /// **An order nobody chose**, so that two nodes holding the same record hand back the same
    /// list — anything worked out from this has to come out the same everywhere or it is not a
    /// shared rule at all.
    ///
    /// It is every node that ever announced itself, including ones that announced once and were
    /// never heard from again. Telling those apart needs measurement this does not have.
    pub fn nodes(&self) -> impl Iterator<Item = &Name> {
        self.nodes.values().map(|(_, name)| name)
    }

    /// What the record calls the node that holds this key.
    ///
    /// [`None`] for a key no node ever announced itself with — which is what somebody speaking the
    /// protocol without being anybody looks like, and is a thing to be able to say rather than to
    /// guess around.
    #[must_use]
    pub fn node_called(&self, key: &[u8; ed25519::PUBLIC_KEY_WIDTH]) -> Option<&Name> {
        self.nodes.get(key).map(|(_, name)| name)
    }

    /// Where an object stands on summarising itself, and the summary it would sign.
    ///
    /// [`None`] for an object this node will not resolve, and for one whose state it has no parts
    /// for. **An object it cannot read is one it cannot summarise**: a summary drawn from a state
    /// that stopped being computable would be a statement about a history nobody could follow.
    #[must_use]
    pub fn standing(&self, name: &Name, at: Epoch) -> Option<Standing> {
        let chain = self.chains.get(name)?;
        if chain.forked || chain.opaque {
            return None;
        }
        let claims: Vec<Claim> = stating(&chain.state, at)
            .into_iter()
            .filter_map(|(about, stated)| {
                chain.set_by.get(&about).map(|set_by| Claim {
                    about,
                    stated,
                    set_by: set_by.clone(),
                })
            })
            .collect();
        if claims.is_empty() {
            return None;
        }
        Some(Standing {
            claims,
            since: chain.since,
            // What it is under the newest setting, because this is for somebody about to sign and
            // not for judging anything already written.
            owed: chain.since >= SUMMARISE_EVERY.now(),
        })
    }

    /// A second act claiming a predecessor that already had one.
    ///
    /// A fork is kept rather than refused: both acts are valid as far as anybody can tell, and it is
    /// the object that becomes unresolvable rather than the second signer who becomes wrong. **Which
    /// is why the signature is checked first.** A fork is two acts somebody had the right to sign;
    /// without that, anybody could make any account unresolvable for ever by sending one act nobody
    /// signed against a predecessor that already had a successor.
    fn split(
        &mut self,
        operation: &Operation,
        name: &Name,
        head: Name,
        state: &State,
    ) -> Result<Admitted, Refused> {
        // Checked whether or not this node can still read the object: one it has given up on is not
        // one anybody may split further, and *cannot read* is not *anything goes*.
        let governor = self.owners_and_thresholds(operation, state);
        entitled(
            operation,
            state,
            &Speaks {
                owners: &governor.owners,
                thresholds: governor.thresholds,
                alone: governor.alone,
                claimant: None,
                answering: governor.answering.clone(),
            },
        )?;
        let held = self
            .chains
            .get_mut(name)
            .ok_or(Refused::NoSuchPredecessor)?;
        held.seen.insert(head);
        held.forked = true;
        Ok(Admitted::Forked)
    }

    /// How many acts an object has added since the last one carrying a summary this node could
    /// read.
    ///
    /// Counted for **every** object, including ones this node will not resolve: what it hands over
    /// is acts, and it can say where the last summary sits without being able to say what the
    /// object is. [`None`] only for an object it has never seen.
    ///
    /// Where the count reaches the whole chain there is no summary, and whoever arrives replays
    /// from the creation — which is what happens to everything that never wrote enough to owe one.
    #[must_use]
    pub fn since_summarising(&self, name: &Name) -> Option<u64> {
        self.chains.get(name).map(|chain| chain.since)
    }

    /// What the chain of an object points at now, for whoever is building the next operation.
    #[must_use]
    pub fn head(&self, name: &Name) -> Option<&Name> {
        self.chains.get(name).map(|chain| &chain.head)
    }
}

/// Whether an act carries the signatures its object's rule calls for, and nothing else.
///
/// **One, for every object this build models**, each of which has a single controller. It matters
/// far more than its size: what an act is *called on a chain* covers its signatures, while what a
/// signature covers does not — so an act with one more signature stapled to it is a new name on the
/// same chain, still carrying the original signature, which still checks out. Without this, anybody
/// who saw an act go past could send it back with a few bytes added and split its object for ever,
/// holding no key and forging nothing.
///
/// The day an object is governed by several keys this becomes *the signatures its threshold calls
/// for, all distinct and all authorised*. The reason does not change.
fn signed_as_required(operation: &Operation) -> Result<(), Refused> {
    // **An organisation and everything it speaks through are signed by several names**, because a
    // threshold is met by several people (`SPECS.md §8.5`). Each of them says which owner they are,
    // and that claim is worth nothing here: `entitled` counts only the ones whose key their own
    // chain authorises, so a name attached to somebody else's key counts as nobody. What is checked
    // here is only that there is somebody to count.
    if Kind::new(operation.kind).is_some_and(|kind| {
        concerns_an_entity(kind)
            || concerns_an_element(kind)
            || concerns_a_certification(kind)
            || kind == Kind::REPLY_PUBLISH
    }) {
        return match operation.signatures.is_empty() {
            true => Err(Refused::Unsigned),
            false => Ok(()),
        };
    }

    let [signature] = operation.signatures.as_slice() else {
        return match operation.signatures.len() {
            0 => Err(Refused::Unsigned),
            _ => Err(Refused::Malformed),
        };
    };

    // **And whose it says it is has to be checked, or it is one more thing to rewrite.** A
    // signature covers everything but the signature list, so the name inside it is not covered by
    // it — while the act's name on the chain is. Left unchecked, anybody who merely saw an act go
    // past could rewrite that one name and send it back: a new name on the same chain, carrying a
    // signature that still verifies, splitting the object for ever, with no key and nothing forged.
    //
    // Every object this build models is signed by itself. The day an entity is signed by its owners
    // this becomes *one of the names the state authorises*, and the reason does not change.
    if signature.by != operation.object {
        return Err(Refused::NotAuthorised);
    }
    Ok(())
}

/// The fields a holder act may carry that this build has a meaning for.
///
/// **One list for every kind, and deliberately not a table per kind.** All seven basic holder acts
/// carry exactly one field — the `1`, the key the act is about — and `SPECS.md` says so where it
/// numbers them: *"Las operaciones básicas del holder llevan un solo campo, el `1`, y en las cuatro
/// es la clave de la que trata el acto"*. A table would be seven chances for this list and the
/// holder app's own copy to drift, and a drift here is not a disagreement — it is an account that
/// one side resolves and the other declares opaque for ever.
///
/// The checkpoint rides at 100 and is **even**, so it passes here without being named, which is
/// exactly the property `SPECS.md §4.6` chose it for: a node one version behind must not declare
/// unresolvable every object that grew enough to owe a summary.
/// The fields the act that creates an object may carry, by what it created.
///
/// **One list per class, and [`None`] where this build has none.** A holder's creation carries the
/// control key at `1`; a node announces itself with the vocabulary `capability` declares. For a
/// class with no list here nothing is checked, which is a gap named as one rather than closed by
/// guessing: an empty vocabulary would refuse every field those creations do carry, and bricking a
/// genesis in the name of thoroughness is not thoroughness.
fn created_vocabulary(state: &State) -> Option<almena_format::field::Vocabulary<'static>> {
    match state {
        State::Holder(_) => Some(holder_vocabulary()),
        State::Entity(_) => Some(crate::entity::vocabulary()),
        State::Element(_) => Some(crate::element::vocabulary()),
        State::Certification(_) => Some(crate::certification::vocabulary()),
        State::Reply(_) => Some(crate::reply::vocabulary()),
        State::Node { .. } => Some(crate::capability::vocabulary()),
        _ => None,
    }
}

pub(crate) fn holder_vocabulary() -> almena_format::field::Vocabulary<'static> {
    /// The key the act is about: the control key a creation establishes, the device key an
    /// addition or a removal names, the new control key a rotation carries, or the act a
    /// cancellation strikes out.
    const SUBJECT: &[almena_format::field::Field] = &[
        almena_format::field::Field::new(KEY),
        almena_format::field::Field::new(crate::resolution::FIELD),
    ];

    almena_format::field::Vocabulary::of(SUBJECT)
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

/// What became of an operation this node was handed.
#[derive(Debug, Clone)]
enum Applied {
    /// The state it leaves behind.
    State(State),
    /// Somebody entitled to write here signed it, and this build has no meaning for it.
    ///
    /// **Kept, passed on, and the object stops resolving** — never refused. Refusing would split
    /// the record between versions, and nothing can tell an out-of-date node from a hostile one, so
    /// nothing is ever given the chance to confuse them.
    Beyond,
}

/// What the state becomes once this operation is applied to it.
fn apply(
    operation: &Operation,
    state: &State,
    kind: Option<Kind>,
    speaks: &Speaks<'_>,
) -> Result<Applied, Refused> {
    let claimant = speaks.claimant;
    match (state, kind) {
        // **Enough owners first, then what the act does** (`SPECS.md §8.5`). The two are separate
        // questions and stay separate: how many signed is about the entity's own configuration,
        // and what the act means is about the act.
        (State::Element(_) | State::Entity(_) | State::Certification(_), Some(kind))
            if concerns_an_element(kind)
                || concerns_an_entity(kind)
                || concerns_a_certification(kind) =>
        {
            governed(operation, state, kind, speaks)
        }
        // A node saying what it saw of others changes nothing about what the node **is** — its key
        // is its key whatever it observed — so the state comes through untouched. What the act is
        // for is being in the record: the summary is the thing, and the chain is where it lives.
        (State::Node { key, .. }, Some(Kind::NODE_SUMMARY)) => {
            check(operation, key)?;
            Ok(Applied::State(state.clone()))
        }
        // Announcing is meant to happen again: what a node offers and what version it runs change
        // over its life, and neither may rename it. Only the first one named anything.
        (
            State::Node {
                key, claimed_by, ..
            },
            Some(Kind::NODE_ANNOUNCE),
        ) => {
            check(operation, key)?;
            offering(operation, *key, claimed_by.clone())
        }
        // A node letting go of whoever contributed it. **The node alone**: whoever claimed it agreed
        // to be credited for what it served, and letting go of that costs them nothing they can be
        // held to, so nobody has to be asked.
        // Both sides, or it binds nothing. The node signs the act because it is the node's chain,
        // and whoever is claiming it approved a challenge naming this node and no other — checked
        // against the key their own chain authorises, resolved from the record.
        (State::Node { key, .. }, Some(Kind::NODE_BIND)) => {
            check(operation, key)?;
            let (approval, _) = crate::bind::claimed(operation).ok_or(Refused::Malformed)?;
            let speaks_for_them = claimant.ok_or(Refused::NotAuthorised)?;
            if !crate::bind::agreed(operation, &speaks_for_them) {
                return Err(Refused::NotAuthorised);
            }
            Ok(Applied::State(claimed(state, Some(approval.claimant))))
        }
        (State::Node { key, .. }, Some(Kind::NODE_UNBIND)) => {
            check(operation, key)?;
            Ok(Applied::State(claimed(state, None)))
        }
        // A summary changes nothing about the account: it restates what the chain already produces,
        // so that whoever arrives later does not have to work it out again. **The state is not
        // taken from what it declares** — a node that believed a summary would resolve differently
        // from one that replayed the chain, with nobody having lied, and checking a summary is
        // something whoever reads it does against the record rather than something a node does on
        // their behalf.
        (State::Holder(holder), Some(kind)) if concerns_a_holder(kind) => {
            // **A critical field this build has no meaning for stops it here** (`SPECS.md §4.8`,
            // rule 4): *crítico* means "if you do not understand this, you cannot claim to have
            // applied this operation", and applying it anyway is how a node ends up serving the
            // opposite of what an act said. Rule 2 fixes what to do instead — *irresoluble, nunca
            // obsoleto* — so the object goes opaque and this node stops answering for it, rather
            // than serving the state from before the act as though nothing had happened.
            //
            // **`Beyond` and never `Refused`**, for the reason rule 1 gives: replication does not
            // require understanding, and refusing would split the record between versions with no
            // way to tell an out-of-date node from a hostile one.
            //
            // **And `entitled` first**, so that going opaque is something only a key the account
            // authorises can do. The other way round, anything anybody handed over would cost
            // somebody their account — which is what the catch-all below has always guarded.
            if operation.understood(holder_vocabulary()).is_err() {
                entitled(operation, state, speaks)?;
                return Ok(Applied::Beyond);
            }
            holder_takes(operation, holder, kind).map(|next| Applied::State(State::Holder(next)))
        }
        // Everything else: an act this build has no meaning for, or one on an object whose class it
        // does not model yet. **It is still an act somebody had to be entitled to write.** That
        // question is about the state and not about the act, so it has an answer even here — and
        // without it, appending nonsense to a stranger's account would cost nothing and cost them
        // the account.
        _ => {
            entitled(operation, state, speaks)?;
            Ok(Applied::Beyond)
        }
    }
}

/// One act on an object a threshold governs, once enough of them have been counted.
///
/// **Enough owners first, then what the act does** (`SPECS.md §8.5`). The two are separate
/// questions and stay separate: how many signed is about the organisation's own configuration, and
/// what the act means is about the act.
fn governed(
    operation: &Operation,
    state: &State,
    kind: Kind,
    speaks: &Speaks<'_>,
) -> Result<Applied, Refused> {
    entitled(operation, state, speaks)?;
    match state {
        State::Element(element) => {
            // Rule 4 again, and the same shape as a holder's: a critical field this build has no
            // meaning for stops it here, and the object goes opaque rather than this node serving
            // the state from before the act as though nothing had happened.
            if operation.understood(crate::element::vocabulary()).is_err() {
                return Ok(Applied::Beyond);
            }
            crate::element::does(operation, element, kind)
                .map(|next| Applied::State(State::Element(Box::new(next))))
        }
        State::Entity(entity) => {
            if operation.understood(crate::entity::vocabulary()).is_err() {
                return Ok(Applied::Beyond);
            }
            crate::entity::does(operation, entity, kind)
                .map(|next| Applied::State(State::Entity(Box::new(next))))
        }
        State::Certification(certification) => {
            if operation
                .understood(crate::certification::vocabulary())
                .is_err()
            {
                return Ok(Applied::Beyond);
            }
            crate::certification::does(operation, certification, kind)
                .map(|next| Applied::State(State::Certification(Box::new(next))))
        }
        // Unreachable: the caller matched on these two. Beyond rather than a panic, because a
        // node that fell over on an act would be a node an act can stop.
        _ => Ok(Applied::Beyond),
    }
}

/// Who governs an object, resolved from the record before anything is applied.
struct Governor {
    /// For each owner who may sign, the keys their account authorises.
    owners: crate::entity::Speaking,
    /// What each class of act costs them.
    thresholds: Option<crate::entity::Thresholds>,
    /// The one key that governs it instead, where one key is what governs it.
    alone: Option<[u8; ed25519::PUBLIC_KEY_WIDTH]>,
    /// Who a reply may be published by, when the act is one.
    answering: Option<Did>,
}

impl Governor {
    /// Nobody, which refuses the act rather than guessing.
    fn nobody() -> Self {
        Self {
            owners: crate::entity::Speaking::new(),
            thresholds: None,
            alone: None,
            answering: None,
        }
    }
}

/// What a chain that has split looked like when one more act arrived.
struct Split<'a> {
    /// Whose chain.
    name: &'a Name,
    /// What it says, frozen at the moment the split was noticed.
    state: &'a State,
    /// Whether it had already split before this act, or is splitting because of it.
    already: bool,
}

/// Where one act lands on a chain that was already here.
struct Following<'a> {
    /// Whose chain.
    name: &'a Name,
    /// The act it follows.
    previous: &'a Name,
    /// What this act is called.
    head: Name,
    /// What applying it produced.
    applied: Applied,
}

/// What the record says about the people an act leans on, resolved before anything is applied.
///
/// **Never taken from the act.** Which keys speak for a person is their own chain's answer, and an
/// act that carried it would be an act deciding who signed it.
#[derive(Debug, Clone)]
pub(crate) struct Speaks<'a> {
    /// For each owner who may sign here, the keys their account authorises at this act's moment.
    ///
    /// For an entity those are its own owners; for an element they are its **parent's**, because an
    /// element has none.
    pub owners: &'a crate::entity::Speaking,
    /// What each class of act costs, when the object is one governed by a threshold.
    ///
    /// [`None`] where the object is not governed that way, and for an element whose parent this
    /// node cannot resolve — which refuses the act rather than guessing a number.
    pub thresholds: Option<crate::entity::Thresholds>,
    /// The one key that speaks for this object's governor, where a single key is what governs it.
    ///
    /// **Almena Government, until `SPECS.md §7.1` gives it its owners.** The act that opens a
    /// network establishes it with one key and nothing else, and inventing owners for it would be
    /// putting state in the record that nobody decided. So while it has one key, that key is what
    /// signs in its name — and a certification issued by it is checked against that rather than
    /// counted against a set that does not exist yet.
    pub alone: Option<[u8; ed25519::PUBLIC_KEY_WIDTH]>,
    /// The key that speaks for whoever is claiming a node, when that is what the act is about.
    pub claimant: Option<[u8; ed25519::PUBLIC_KEY_WIDTH]>,
    /// Who a reply may be published by: the party the decision it answers was taken about.
    ///
    /// Resolved from the decision and never taken from the act — an act that named its own author
    /// would let anybody answer in somebody else's name.
    pub answering: Option<Did>,
}

/// Whether an act can belong on this chain at all, whatever it goes on to say.
///
/// Two rules, and both are about the act's own shape against where it landed rather than about
/// what it would do — so every build, of every version, reaches the same answer.
///
/// # Errors
///
/// [`Refused::Malformed`] for a creation that arrived mid-chain, and
/// [`Refused::BeforeItsPredecessor`] for one dated behind the chain it extends. **A new act may
/// share the latest epoch on the chain but never precede it** — the one move that would let a
/// thief of the words rewind a wait to nothing.
fn belongs_here(operation: &Operation, dated: Epoch) -> Result<(), Refused> {
    misplaced_creation(operation)?;
    if operation.issued.number() < dated.number() {
        return Err(Refused::BeforeItsPredecessor);
    }
    Ok(())
}

/// Whether this act is a creation that arrived somewhere it cannot mean anything.
///
/// **Refused rather than kept**, because it is not an act from a newer version that a build merely
/// cannot read: every build, of every version, can see that a creating kind landed mid-chain.
/// Kept, it would make the object unresolvable for ever — and the two readers of a summary would
/// disagree about it, one taking it as an act that reset the account and the other as one nobody
/// can apply.
///
/// # Errors
///
/// [`Refused::Malformed`], which is what it is: an act whose kind and whose position cannot both
/// be what they say.
fn misplaced_creation(operation: &Operation) -> Result<(), Refused> {
    if Kind::new(operation.kind).is_some_and(only_ever_creates) {
        return Err(Refused::Malformed);
    }
    Ok(())
}

/// Whether an act of this kind can only ever bring an object into existence.
///
/// **A creation creates, and creates once.** These kinds establish what an object *is* — a
/// holder's control key, the network's own government, the two roots a contradiction is made of —
/// so an act of one of them following something is not a creation at all: it is an act nobody
/// could apply, and admitting it would leave the object it lands on permanently unresolvable.
/// Whoever wrote it would have bricked an account with an act that has no meaning in that place.
///
/// **A node's announcement is deliberately not here.** It creates the first time and repeats
/// afterwards, because what a node offers changes over its life and its name must not.
const fn only_ever_creates(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::HOLDER_CREATE | Kind::GENESIS | Kind::CONTRADICTION_PUBLISH
    )
}

/// Whether this build knows how to work an act into a holder's state.
///
/// Separate from whether it has a *number* for it. A number this build lists is one it can store,
/// index and pass on; only these are ones it can claim to have applied, and conflating the two is
/// how a node ends up refusing something every other node keeps.
const fn concerns_a_holder(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::HOLDER_ADD_DEVICE
            | Kind::HOLDER_REMOVE_DEVICE
            | Kind::HOLDER_ROTATE
            | Kind::HOLDER_FREEZE
            | Kind::HOLDER_UNFREEZE
            | Kind::HOLDER_CANCEL
            | Kind::HOLDER_CHECKPOINT
    )
}

/// The domain an act claims, when it is one that claims one.
fn claimed_domain(operation: &Operation) -> Option<String> {
    (Kind::new(operation.kind) == Some(Kind::ENTITY_ADD_DOMAIN))
        .then(
            || match operation.payload.get(&crate::entity::field::DOMAIN) {
                Some(Value::Text(domain)) => Some(domain.clone()),
                _ => None,
            },
        )
        .flatten()
}

/// The domain an act gives up, when it is one that gives one up.
fn released_domain(operation: &Operation) -> Option<String> {
    (Kind::new(operation.kind) == Some(Kind::ENTITY_REMOVE_DOMAIN))
        .then(
            || match operation.payload.get(&crate::entity::field::DOMAIN) {
                Some(Value::Text(domain)) => Some(domain.clone()),
                _ => None,
            },
        )
        .flatten()
}

/// Whether that act is one performed on a certification.
const fn concerns_a_certification(kind: Kind) -> bool {
    matches!(kind, Kind::CERTIFICATION_ISSUE | Kind::CERTIFICATION_REVOKE)
}

/// Whether that act is one performed on an issuer or a verifier.
const fn concerns_an_element(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::ISSUER_CREATE
            | Kind::ISSUER_SET_CONFIG
            | Kind::ISSUER_SET_ISSUANCE_KEY
            | Kind::ISSUER_ROTATE_KEY
            | Kind::ISSUER_CLOSE
    )
}

/// Whether that act is one performed on an entity.
const fn concerns_an_entity(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::ENTITY_CREATE
            | Kind::ENTITY_ADD_OWNER
            | Kind::ENTITY_REMOVE_OWNER
            | Kind::ENTITY_ADD_MANAGER
            | Kind::ENTITY_REMOVE_MANAGER
            | Kind::ENTITY_SET_THRESHOLD
            | Kind::ENTITY_ROTATE_KEY
            | Kind::ENTITY_ADD_DOMAIN
            | Kind::ENTITY_REMOVE_DOMAIN
            | Kind::ENTITY_SET_ALIAS
            | Kind::ENTITY_CONTINUITY
            | Kind::ENTITY_VETO
            | Kind::ENTITY_CLOSE
            | Kind::ENTITY_CHECKPOINT
    )
}

/// Who signed an act on a holder's chain, said by the state and never guessed from the key.
enum Signer {
    /// The key that governs the account — the one that comes from the words.
    Control,
    /// One of the keys that operate it, and which one.
    Device(Vec<u8>),
}

/// Who signed this act, checked, or why it does not count.
///
/// Which curve to verify with is never guessed from the length of the key: the state says which
/// key this is, and therefore which curve made it.
fn who_signs(operation: &Operation, holder: &Holder) -> Result<Signer, Refused> {
    let signature = operation.signatures.first().ok_or(Refused::Unsigned)?;
    if signature.key.as_slice() == holder.control.as_slice() {
        verify_control(operation, &holder.control)?;
        return Ok(Signer::Control);
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
        .map_err(|_| Refused::SignatureDoesNotCheck)?;
    Ok(Signer::Device(signature.key.clone()))
}

/// What a holder's state becomes under an act this build knows.
///
/// Everything due by the act's own moment lands first, so that what an act is judged against is
/// the account as it stood when the act was made — the same account every other reader computes.
fn holder_takes(operation: &Operation, holder: &Holder, kind: Kind) -> Result<Holder, Refused> {
    let holder = holder.come_due(operation.issued);
    let signer = who_signs(operation, &holder)?;

    // **A frozen account is one where only *no* can be said.** A device may still cancel — a
    // cancellation denies, which is what frozen means — and the control key may still ask, because
    // what it asks for waits where every device can see it and veto it. Everything else is what
    // freezing exists to stop.
    if holder.frozen {
        return frozen_takes(operation, holder, kind, &signer);
    }

    match signer {
        Signer::Control => control_asks(operation, holder, kind),
        Signer::Device(key) => device_does(operation, holder, kind, &key),
    }
}

/// What may still happen to a frozen account.
fn frozen_takes(
    operation: &Operation,
    holder: Holder,
    kind: Kind,
    signer: &Signer,
) -> Result<Holder, Refused> {
    match (signer, kind) {
        // Cancelling survives the freeze, or freezing first would kill the veto: whoever stole
        // the words would freeze — freezing is immediate — and then wait out their own theft
        // with nobody able to say no.
        (Signer::Device(key), Kind::HOLDER_CANCEL) => cancelling(operation, holder, key),
        (Signer::Device(_), _) => Err(Refused::NotAuthorised),
        // Frozen already is frozen: there is nothing for a second freeze to do, and an act that
        // does nothing is not written down.
        (Signer::Control, Kind::HOLDER_FREEZE) => Err(Refused::Malformed),
        // The words never veto. Cancelling is the counterweight the *devices* hold against the
        // words — in the words' own hands it weighs nothing.
        (Signer::Control, Kind::HOLDER_CANCEL) => Err(Refused::NotAuthorised),
        // A summary is routine, and a frozen account is the opposite of routine.
        (Signer::Control, Kind::HOLDER_CHECKPOINT) => Err(Refused::NotAuthorised),
        // Everything else the words ask for still enters — as a wait, which is a window in which
        // every device can see it and say no. That is the whole defence against stolen words, and
        // a freeze that closed it would be a freeze working for the thief.
        (Signer::Control, _) => control_asks(operation, holder, kind),
    }
}

/// What the control key alone may do, which is ask.
///
/// **Freezing lands at once and everything else waits.** Freezing denies — there is nothing in it
/// worth stealing a wait for. Everything else concedes something: a key joining, a key leaving,
/// the account changing hands or starting to move again — and what the words concede alone, the
/// devices get a window to refuse.
fn control_asks(operation: &Operation, holder: Holder, kind: Kind) -> Result<Holder, Refused> {
    let mut next = holder;
    let does = match kind {
        Kind::HOLDER_FREEZE => {
            next.frozen = true;
            return Ok(next);
        }
        // A summary restates what the chain already produces and concedes nothing, so there is
        // no effect to hold back and nothing for a wait to protect.
        //
        // **Except that it cannot restate what is waiting, because the format has nowhere to put
        // it.** A summary carries the control key and the devices; an asking in flight leaves no
        // trace in one. And a node serves the last summary and what followed, so an asking made
        // just before a summary is an asking no reader will ever see — it lands seventy-two epochs
        // later on a state nobody was shown, which is `SPECS.md §11.12`'s notice deleted by
        // arithmetic. The control key can write both acts, so it is a thing to be done on purpose.
        //
        // So a summary waits for the queue to be empty. It is the same rule the frozen account
        // above already lives under and for the same reason: a summary is routine, and an account
        // with something in flight is the opposite of routine. Everything waiting comes due within
        // one window, so what this refuses is never permanent.
        Kind::HOLDER_CHECKPOINT if !next.waiting.is_empty() => return Err(Refused::NotAuthorised),
        Kind::HOLDER_CHECKPOINT => return Ok(next),
        Kind::HOLDER_CANCEL => return Err(Refused::NotAuthorised),
        Kind::HOLDER_ADD_DEVICE => Does::AddDevice(device(operation)?),
        Kind::HOLDER_REMOVE_DEVICE => {
            let removed = device(operation)?;
            // Judged now, against the account as it stands: asking to remove what is not there
            // is not a slower way of removing it, it is a mistake being caught early.
            if !next.devices.contains(&removed) {
                return Err(Refused::Malformed);
            }
            Does::RemoveDevice(removed)
        }
        Kind::HOLDER_ROTATE => Does::Rotate(fixed(operation, KEY)?),
        // Thawing an account that is not frozen is nothing asked for; thawing one that is
        // concedes back everything the freeze stopped, so it waits like anything else the words
        // ask alone, and the devices get the same window to say no.
        Kind::HOLDER_UNFREEZE if next.frozen => Does::Unfreeze,
        Kind::HOLDER_UNFREEZE => return Err(Refused::Malformed),
        _ => return Err(Refused::Malformed),
    };
    // **A ceiling on what the words may have in flight.** Each asking is an entry every reader
    // clones and walks on every later act; without a cap, the words alone could make a chain cost
    // the square of its length to take in. Counted against the account as it stands at this act's
    // moment, so every node counts the same and refuses the same.
    if next.waiting.len() >= CONTROL_PENDING_MOST.at(operation.issued) as usize {
        return Err(Refused::TooManyWaiting);
    }
    let due = operation
        .issued
        .plus(almena_time::Epochs(CONTROL_WAITS.at(operation.issued)))
        .ok_or(Refused::Malformed)?;
    next.waiting.push(Waiting {
        act: operation.called(),
        does,
        due,
    });
    Ok(next)
}

/// What a device does, which lands at once.
///
/// A device in the hand is the strongest thing this design knows about who somebody is, and it is
/// also the thing whose theft the account can survive: devices take each other out. So a device
/// acts immediately — the wait exists for the words, not for them.
fn device_does(
    operation: &Operation,
    holder: Holder,
    kind: Kind,
    key: &[u8],
) -> Result<Holder, Refused> {
    let mut next = holder;
    match kind {
        Kind::HOLDER_ADD_DEVICE => {
            next.devices.insert(device(operation)?);
        }
        Kind::HOLDER_REMOVE_DEVICE => {
            // Devices take each other out; the control key is only ever replaced by the words. A
            // removal that could reach it would make a stolen device enough to take the account.
            if !next.devices.remove(&device(operation)?) {
                return Err(Refused::Malformed);
            }
        }
        // The account changing hands is the words' alone: a device that could rotate the control
        // key would make a stolen device the account.
        Kind::HOLDER_ROTATE => return Err(Refused::NotAuthorised),
        Kind::HOLDER_FREEZE => next.frozen = true,
        // There is nothing to thaw. On a frozen account this arm is never reached — a frozen
        // account refuses devices everything but a cancellation, so thawing is always the words
        // asking, with the wait that asking carries.
        Kind::HOLDER_UNFREEZE => return Err(Refused::Malformed),
        Kind::HOLDER_CANCEL => return cancelling(operation, next, key),
        // **Not while anything is waiting**, for the reason the control key's own arm gives: a
        // summary has nowhere to say what is in flight, and one written over an asking is an
        // asking no reader will ever see. A device may summarise as freely as the words may, and
        // is held to the same condition.
        Kind::HOLDER_CHECKPOINT if !next.waiting.is_empty() => return Err(Refused::NotAuthorised),
        Kind::HOLDER_CHECKPOINT => {}
        _ => return Err(Refused::Malformed),
    }
    Ok(next)
}

/// A device saying no to something the words asked for.
fn cancelling(operation: &Operation, holder: Holder, key: &[u8]) -> Result<Holder, Refused> {
    let Some(Value::Text(named)) = operation.payload.get(&KEY) else {
        return Err(Refused::Malformed);
    };
    let named = Name::parse(named).map_err(|_| Refused::Malformed)?;
    let mut next = holder;
    let Some(at) = next.waiting.iter().position(|waiting| waiting.act == named) else {
        // Nothing waiting under that name — never asked, already landed, or already cancelled.
        // All three are the same answer: there is nothing here to say no to.
        return Err(Refused::Malformed);
    };
    // A device cannot veto its own removal. Without this, whoever stole a phone could hold off
    // their own expulsion for ever, and the counterweight built against a thief of words would be
    // working for a thief of devices.
    if let Does::RemoveDevice(removed) = &next.waiting[at].does
        && removed.as_slice() == key
    {
        return Err(Refused::NotAuthorised);
    }
    next.waiting.remove(at);
    Ok(next)
}

/// What a creation brings into existence, or why it cannot.
///
/// Every object arrives with the work that builds it. Until then a creation this node cannot apply
/// is refused rather than stored as an object nobody could say anything about — which would be
/// worse than never having taken it.
fn born(operation: &Operation, speaks: &Speaks<'_>) -> Result<State, Refused> {
    Ok(match Kind::new(operation.kind) {
        Some(Kind::REPLY_PUBLISH) => answered(operation, speaks)?,
        Some(Kind::CERTIFICATION_ISSUE | Kind::ISSUER_CREATE) => {
            under_an_entity(operation, speaks)?
        }
        Some(Kind::ENTITY_CREATE) => {
            // **Signed by a person, because an entity has no key of its own to be signed by.** A
            // holder's creation is self-signed by the control key it establishes; an entity's key
            // is for a channel and governs nothing (`SPECS.md §2.2`), so the only thing that can
            // authorise one is somebody who already exists — checked against their own chain.
            State::Entity(Box::new(crate::entity::born(
                operation,
                speaks.owners,
                operation.issued,
            )?))
        }
        Some(Kind::HOLDER_CREATE) => {
            let control = fixed(operation, KEY)?;
            // Nothing else could have signed it: the account does not exist until this act
            // does, so the only key its own state authorises is the one it establishes.
            check(operation, &control)?;
            State::Holder(Holder {
                control,
                devices: BTreeSet::new(),
                frozen: false,
                waiting: Vec::new(),
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
            State::Node {
                key,
                offers: BTreeSet::new(),
                speaks: 0,
                claimed_by: None,
                reachable: BTreeSet::new(),
            }
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
    })
}

/// A reply to a decision, published by the party the decision was about.
///
/// **Only that party, and that is the whole of the check**: a right of reply the decider could
/// withhold would be a right the decider grants, and one anybody could exercise would let a
/// stranger answer in somebody else's name. Who that party is comes from the decision itself,
/// resolved where the record is.
///
/// It costs what that organisation's routine acts cost — saying something concedes nothing, and a
/// reply behind a governance threshold would be one its own owners could sit on.
fn answered(operation: &Operation, speaks: &Speaks<'_>) -> Result<State, Refused> {
    let by = speaks.answering.clone().ok_or(Refused::NotAuthorised)?;
    let thresholds = speaks.thresholds.ok_or(Refused::NotAuthorised)?;
    enough(
        operation,
        speaks.owners,
        thresholds.of(crate::entity::Class::Routine),
    )?;
    Ok(State::Reply(Box::new(crate::reply::born(operation, by)?)))
}

/// An object created under an organisation, which is what authorises it.
///
/// Two of them, and the same shape: the act names the organisation, and it only enters the record
/// because that organisation's owners signed it at what the act costs them.
///
/// - **A certification** costs the sealing threshold (`SPECS.md §7.10`, `§8.2`). Anybody may
///   certify anybody (`SPECS.md §7.3`), so what is checked is not who is doing it but that they
///   really are who the act says — what the statement is worth is the reader's to judge.
/// - **An issuer or a verifier** costs a routine one, and the signing is what makes its link to its
///   parent go both ways without a second act: nobody hangs one off an organisation they do not
///   govern, and the organisation never has to acknowledge it afterwards.
fn under_an_entity(operation: &Operation, speaks: &Speaks<'_>) -> Result<State, Refused> {
    Ok(match Kind::new(operation.kind) {
        Some(Kind::CERTIFICATION_ISSUE) => {
            sealed(operation, speaks)?;
            State::Certification(Box::new(crate::certification::born(operation)?))
        }
        _ => {
            let thresholds = speaks.thresholds.ok_or(Refused::NotAuthorised)?;
            let class = crate::element::class(Kind::ISSUER_CREATE).ok_or(Refused::Malformed)?;
            enough(operation, speaks.owners, thresholds.of(class))?;
            State::Element(Box::new(crate::element::born(operation)?))
        }
    })
}

/// What each part of an object's state is right now.
///
/// Empty for the objects whose state has no parts a summary may claim. A node's own chain grows
/// faster than anybody's and has nothing here yet: what a node **is** arrives with the mesh that
/// reads it, and inventing parts for it now would be summarising state nobody decided.
fn stating(state: &State, at: Epoch) -> Vec<(Governs, Stated)> {
    match state {
        State::Holder(holder) => {
            // A summary states the account **as of the act that will carry it**, so what the
            // control key asked for alone counts exactly when its wait has run out by then —
            // the same answer whoever checks the summary will compute.
            let holder = holder.come_due(at);
            vec![
                (Governs::Control, Stated::Key(holder.control.to_vec())),
                (Governs::Devices, Stated::Keys(holder.devices.clone())),
            ]
        }
        _ => Vec::new(),
    }
}

/// The parts of the state an act settles.
///
/// Read off the same table that says which acts govern which part, rather than written out a second
/// time: two lists of the same thing drift, and the drift would be silent — a summary citing an act
/// that no longer counted as having set anything, or one that stood up while hiding an act nobody
/// had remembered to list.
fn settles(kind: Option<Kind>) -> impl Iterator<Item = Governs> {
    Governs::ALL
        .into_iter()
        .filter(move |part| kind.is_some_and(|kind| part.set_by().contains(&kind)))
}

/// The same node, claimed by somebody or by nobody.
fn claimed(state: &State, by: Option<Did>) -> State {
    match state {
        State::Node {
            key,
            offers,
            speaks,
            reachable,
            ..
        } => State::Node {
            key: *key,
            offers: offers.clone(),
            speaks: *speaks,
            claimed_by: by,
            reachable: reachable.clone(),
        },
        other => other.clone(),
    }
}

/// What a node's state becomes when it says again what it is running.
///
/// A capability from a newer version is not passed over and is not refused either: the act is kept,
/// passed on, and this node stops saying what that one offers. Passing it over would mean counting
/// a node as offering less than it does, and the counting is what the field is for.
fn offering(
    operation: &Operation,
    key: [u8; ed25519::PUBLIC_KEY_WIDTH],
    claimed_by: Option<Did>,
) -> Result<Applied, Refused> {
    use almena_format::cbor::Value;

    if operation
        .understood(crate::capability::vocabulary())
        .is_err()
    {
        return Ok(Applied::Beyond);
    }
    let mut offers = BTreeSet::new();
    if let Some(Value::Array(said)) = operation.payload.get(&crate::capability::OFFERS) {
        for one in said {
            let Value::Uint(number) = one else {
                return Err(Refused::Malformed);
            };
            offers.insert(crate::capability::Capability::new(*number).ok_or(Refused::Malformed)?);
        }
    }
    let speaks = match operation.payload.get(&crate::capability::SPEAKS) {
        Some(Value::Uint(version)) => *version,
        Some(_) => return Err(Refused::Malformed),
        None => 0,
    };
    let mut reachable = BTreeSet::new();
    if let Some(Value::Array(said)) = operation.payload.get(&crate::capability::WHERE) {
        for one in said {
            let Value::Text(address) = one else {
                return Err(Refused::Malformed);
            };
            reachable.insert(address.clone());
        }
    }

    Ok(Applied::State(State::Node {
        key,
        offers,
        speaks,
        claimed_by,
        reachable,
    }))
}

/// Whether somebody the state authorises to write on this chain signed this.
///
/// **The question that can be answered without understanding the act.** What a state authorises is
/// a property of the state, so an act from a version this build has never seen is still one it can
/// tell a stranger from an owner on. Not being able to read an act is a reason to stop resolving
/// the object; it is never a reason to let anybody at all extend it.
///
/// It is the weakest true statement and not a guess at more: *a key this object authorises for
/// something signed this*. Which keys a newer act really requires is exactly what this build cannot
/// know, and it claims nothing about the result — the object stops resolving either way.
fn entitled(operation: &Operation, state: &State, speaks: &Speaks<'_>) -> Result<(), Refused> {
    match state {
        // **The issuer's owners, at the threshold sealing costs** (`SPECS.md §7.10`, `§8.2`). A
        // certification is a statement one organisation makes in its own name, so who may make it
        // and who may take it back is that organisation's question and nobody else's — including
        // the subject's, who does not get to edit what was said about them.
        State::Certification(_) => sealed(operation, speaks),
        // **Published once and never edited.** A reply somebody could revise after the fact would
        // be one whose meaning depends on when it is read, and the whole point is that the decision
        // and the answer stand side by side for ever.
        State::Reply(_) => Err(Refused::NotAuthorised),
        // Counted against the **parent's** owners and the parent's thresholds, both resolved from
        // the record. An element that could be changed by anybody who could sign for anything would
        // be an element whose link to its organisation says nothing.
        State::Element(_) => {
            let thresholds = speaks.thresholds.ok_or(Refused::NotAuthorised)?;
            let class = Kind::new(operation.kind)
                .and_then(crate::element::class)
                .ok_or(Refused::NotAuthorised)?;
            enough(operation, speaks.owners, thresholds.of(class))
        }
        // **Counted, and against the set standing at this act's own moment** (`SPECS.md §8.5`,
        // `§8.6`). Only whether enough owners signed — never what the entity's own policy would do
        // with the act, for the same reason as below: this gate also keeps a fork and extends an
        // object this build cannot read, and a node that refused one of those where another kept
        // it is the permanent divergence the whole design avoids.
        State::Entity(entity) => {
            let entity = entity.come_due(operation.issued);
            // **A resolution costs the most this object has, whatever else the act does**
            // (`SPECS.md §4.9`): it puts already-signed operations out of effect, and that is not
            // routine. For an organisation the most it has is the governance threshold — and where
            // the owners cannot reach it, this is the frozen case of `SPECS.md §12.3` and the way
            // out is the emergency continuity of `§8.3`.
            let class = if crate::resolution::declared(operation) {
                crate::entity::Class::Governance
            } else {
                Kind::new(operation.kind)
                    .and_then(crate::entity::class)
                    .ok_or(Refused::NotAuthorised)?
            };
            enough(operation, speaks.owners, entity.thresholds.of(class))
        }
        State::Holder(holder) => {
            // **Only whether the signer was allowed to write here** — never what the account's
            // policy would do with the act. Freezing decides what changes the state, not who may
            // write, and asking it here would be catastrophic: `entitled` is also the gate that
            // *keeps* a fork and *extends* an object this build cannot read, and a frozen node
            // refusing one of those where an unfrozen node kept it is exactly the permanent
            // divergence the whole design is built to avoid. Judged against the account as it
            // stood at the act's own moment — a device whose removal has landed is not one.
            let holder = holder.come_due(operation.issued);
            who_signs(operation, &holder).map(|_| ())
        }
        State::Government { key } | State::Node { key, .. } => check(operation, key),
        // Evidence, not an account. Nobody controls a contradiction and nothing extends it: it
        // says what it says because of the two signatures it carries, and a second act on it could
        // only muddy what is already settled by its own contents.
        State::Contradiction { .. } => Err(Refused::NotAuthorised),
    }
}

/// Whether whoever the act says is certifying really signed it, at what sealing costs them.
///
/// Two shapes, because a certification's issuer can be governed two ways: an organisation, whose
/// owners are counted at the sealing threshold, or **Almena Government while it still has the one
/// key the genesis gave it** — for which counting a set that does not exist would be counting
/// nothing at all.
fn sealed(operation: &Operation, speaks: &Speaks<'_>) -> Result<(), Refused> {
    if let Some(key) = speaks.alone {
        return check(operation, &key);
    }
    let thresholds = speaks.thresholds.ok_or(Refused::NotAuthorised)?;
    enough(
        operation,
        speaks.owners,
        thresholds.of(crate::entity::Class::Sealing),
    )
}

/// Whether enough of the people who may sign actually did.
fn enough(
    operation: &Operation,
    speaking: &crate::entity::Speaking,
    wanted: u64,
) -> Result<(), Refused> {
    let signed = crate::entity::counted(operation, speaking);
    if u64::try_from(signed.len()).unwrap_or(u64::MAX) < wanted {
        return Err(Refused::NotAuthorised);
    }
    Ok(())
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

    #[test]
    fn every_vocabulary_knows_the_fields_that_mean_one_thing_everywhere() {
        // **A critical common field left out of one kind's list is an act that kind can never
        // carry.** They are four lists and one rule, so the rule is checked rather than remembered:
        // adding a common field is one edit and a failing test, not four edits and a silence.
        let vocabularies = [
            ("holder", super::holder_vocabulary()),
            ("entity", crate::entity::vocabulary()),
            ("element", crate::element::vocabulary()),
            ("node", crate::capability::vocabulary()),
        ];
        for (whose, vocabulary) in vocabularies {
            for common in crate::resolution::COMMON {
                assert!(
                    vocabulary.fields.contains(common),
                    "{whose} does not know field {}",
                    common.number()
                );
            }
        }
    }

    use crate::kind::Kind;
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::{Operation, Signed, create};
    use almena_suite::{ed25519, p256};
    use almena_time::{Epoch, Epochs};
    use std::collections::{BTreeMap, BTreeSet};

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
        following_at(object, head, kind, key, now())
    }

    /// The same act, made at a chosen moment — for everything that happens after a wait.
    fn following_at(object: &Did, head: &Name, kind: Kind, key: &[u8], at: Epoch) -> Operation {
        Operation {
            object: object.clone(),
            previous: Some(head.clone()),
            kind: kind.number(),
            version: 1,
            issued: at,
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

    /// The first moment the wait on an act the control key signed at [`now`] has run out.
    ///
    /// The first device on a fresh account can only be added by the control key — there is no
    /// device yet to sign — and what the control key signs alone waits. So an account fixture is
    /// only operative from here on, which is the same thing that is true of a real account
    /// somebody opened with nothing but their words.
    fn once_due() -> Epoch {
        now().plus(Epochs(72)).expect("no overflow")
    }

    /// An account with one device on it, and everything needed to act on it again.
    ///
    /// The device was asked for by the control key at [`now`] and is operative from [`once_due`],
    /// so everything a test does with it happens there or later.
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

    /// The holder an answer carries, with everything due by that moment landed.
    fn holder_at(objects: &Objects, object: &Did, at: Epoch) -> super::Holder {
        match objects.resolve(object.name()) {
            Answer::Here(State::Holder(holder)) => holder.come_due(at),
            other => panic!("{other:?}"),
        }
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
        // Signed as being by whatever it now claims to be, so that what is under test is the name
        // it gives itself and not whose signature it says it carries.
        lying.signatures[0].by = lying.object.clone();

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

        // Asked for by the words alone, so it is not a device yet — it is a device the moment
        // the wait runs out, and any reader saying when they ask sees the same thing.
        assert!(
            !holder_at(&objects, &object, now())
                .devices
                .contains(&public)
        );
        assert!(
            holder_at(&objects, &object, once_due())
                .devices
                .contains(&public)
        );

        // And a device can take itself out, which is what somebody does with a phone they are
        // about to give away.
        let head = objects.head(object.name()).expect("a head").clone();
        let remove = signed_by_device(
            following_at(
                &object,
                &head,
                Kind::HOLDER_REMOVE_DEVICE,
                &public,
                once_due(),
            ),
            &device,
        );
        assert_eq!(objects.admit(&remove, once_due()), Ok(Admitted::Extended));

        match objects.resolve(object.name()) {
            Answer::Here(State::Holder(holder)) => assert!(holder.devices.is_empty()),
            other => panic!("{other:?}"),
        }
    }

    /// A cancellation of that waiting act, made by that device at that moment.
    fn cancel_of(object: &Did, head: &Name, waiting: &Name, at: Epoch) -> Operation {
        Operation {
            object: object.clone(),
            previous: Some(head.clone()),
            kind: Kind::HOLDER_CANCEL.number(),
            version: 1,
            issued: at,
            payload: BTreeMap::from([(KEY, Value::Text(waiting.as_str().to_owned()))]),
            signatures: Vec::new(),
        }
    }

    #[test]
    fn what_the_words_ask_alone_waits_where_every_device_can_see_it() {
        // The words can be read over a shoulder without any device going anywhere, so what they
        // sign alone does not land at once: it enters the record — which is exactly where the
        // devices will see it — and lands when the wait runs out.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let target = device.verifying_key().bytes();

        let removal = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_REMOVE_DEVICE,
                &target,
                once_due(),
            ),
            &control,
        );
        assert_eq!(objects.admit(&removal, once_due()), Ok(Admitted::Extended));

        // In the record at once, in force only later — and any reader saying when they ask
        // agrees on both.
        let holder = holder_at(&objects, &object, once_due());
        assert!(holder.devices.contains(target.as_slice()), "not yet");
        assert_eq!(holder.waiting.len(), 1);
        let landed = once_due().plus(Epochs(72)).expect("no overflow");
        assert!(
            !holder_at(&objects, &object, landed)
                .devices
                .contains(target.as_slice()),
            "and then it lands"
        );
    }

    #[test]
    fn a_device_the_words_are_removing_still_signs_until_the_wait_runs_out() {
        // The effect trails the record: during the window the device is still a device — that is
        // what gives it the standing to say no — and the moment the wait is out it is not.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let target = device.verifying_key().bytes();

        let removal = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_REMOVE_DEVICE,
                &target,
                once_due(),
            ),
            &control,
        );
        objects.admit(&removal, once_due()).expect("taken");
        let head = objects.head(object.name()).expect("a head").clone();

        // Still a device: it adds a key inside the window.
        let inside = once_due().plus(Epochs(71)).expect("no overflow");
        let adds = signed_by_device(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(21).verifying_key().bytes(),
                inside,
            ),
            &device,
        );
        assert_eq!(objects.admit(&adds, inside), Ok(Admitted::Extended));
        let head = objects.head(object.name()).expect("a head").clone();

        // And once the wait is out, it is nobody.
        let landed = once_due().plus(Epochs(72)).expect("no overflow");
        let too_late = signed_by_device(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(22).verifying_key().bytes(),
                landed,
            ),
            &device,
        );
        assert_eq!(
            objects.admit(&too_late, landed),
            Err(Refused::NotAuthorised)
        );
    }

    #[test]
    fn any_current_device_cancels_what_the_words_asked_and_it_stays_cancelled() {
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        // The words ask for a second key — a stranger's, for all the devices know.
        let planted = device_key(66).verifying_key().bytes();
        let asking = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &planted,
                once_due(),
            ),
            &control,
        );
        objects.admit(&asking, once_due()).expect("taken");
        let head = objects.head(object.name()).expect("a head").clone();

        let veto = signed_by_device(
            cancel_of(&object, &head, &asking.called(), once_due()),
            &device,
        );
        assert_eq!(objects.admit(&veto, once_due()), Ok(Admitted::Extended));

        // Struck out for good: long past the wait, the planted key is still nobody.
        let long_after = once_due().plus(Epochs(1_000)).expect("no overflow");
        let holder = holder_at(&objects, &object, long_after);
        assert!(!holder.devices.contains(planted.as_slice()));
        assert!(holder.waiting.is_empty());
    }

    #[test]
    fn a_device_cannot_cancel_the_asking_that_removes_it() {
        // Without this, whoever stole a phone could hold off their own expulsion for ever, and
        // the counterweight built against a thief of words would work for a thief of devices.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let target = device.verifying_key().bytes();

        let removal = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_REMOVE_DEVICE,
                &target,
                once_due(),
            ),
            &control,
        );
        objects.admit(&removal, once_due()).expect("taken");
        let head = objects.head(object.name()).expect("a head").clone();

        let self_serving = signed_by_device(
            cancel_of(&object, &head, &removal.called(), once_due()),
            &device,
        );
        assert_eq!(
            objects.admit(&self_serving, once_due()),
            Err(Refused::NotAuthorised)
        );

        // Another device may, because it is not the one being judged.
        let other = device_key(21);
        let joins = signed_by_device(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &other.verifying_key().bytes(),
                once_due(),
            ),
            &device,
        );
        objects.admit(&joins, once_due()).expect("taken");
        let head = objects.head(object.name()).expect("a head").clone();
        let veto = signed_by_device(
            cancel_of(&object, &head, &removal.called(), once_due()),
            &other,
        );
        assert_eq!(objects.admit(&veto, once_due()), Ok(Admitted::Extended));
    }

    #[test]
    fn a_thief_cannot_back_date_an_act_to_collapse_its_wait() {
        // **The wait is worth nothing if the thief sets the clock.** What the control key signs
        // alone lands at issued-plus-the-wait, and issued is a field the signer writes — so a
        // thief with the words, dating a removal at the genesis, would have it land the moment it
        // is admitted, with no window for any device to say no. The chain refuses to be dated
        // before the act it follows, which is what keeps the window real.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let target = device.verifying_key().bytes();

        let back_dated = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_REMOVE_DEVICE,
                &target,
                Epoch::GENESIS,
            ),
            &control,
        );
        assert_eq!(
            objects.admit(&back_dated, once_due()),
            Err(Refused::BeforeItsPredecessor),
            "dated before the act it follows, so it is refused rather than landed"
        );
        assert!(
            holder_at(&objects, &object, once_due())
                .devices
                .contains(target.as_slice()),
            "and the device the thief tried to remove is still there"
        );
    }

    #[test]
    fn an_act_may_share_its_predecessor_s_epoch_but_not_precede_it() {
        // Equality is fine — two acts in one hour is ordinary — and only going backwards is
        // refused, because that is the one move that would rewind a wait.
        let (mut objects, object, _control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        // The resident device is operative from once_due(); it adds a key there, dating the
        // chain at once_due().
        let first = signed_by_device(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(21).verifying_key().bytes(),
                once_due(),
            ),
            &device,
        );
        assert_eq!(objects.admit(&first, once_due()), Ok(Admitted::Extended));
        let head = objects.head(object.name()).expect("a head").clone();

        // A second act at the same epoch is admitted — equality is ordinary.
        let same = signed_by_device(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(22).verifying_key().bytes(),
                once_due(),
            ),
            &device,
        );
        assert_eq!(objects.admit(&same, once_due()), Ok(Admitted::Extended));
        let head = objects.head(object.name()).expect("a head").clone();

        // One dated before the chain's latest epoch is refused for that alone.
        let earlier = Epoch::new(once_due().number() - 1);
        let before = signed_by_device(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(23).verifying_key().bytes(),
                earlier,
            ),
            &device,
        );
        assert_eq!(
            objects.admit(&before, once_due()),
            Err(Refused::BeforeItsPredecessor)
        );
    }

    #[test]
    fn a_late_redelivery_of_an_old_act_is_still_the_same_act_and_not_a_rewind() {
        // The monotonic rule must not turn an honest re-delivery into a refusal: an act heard
        // again after the chain moved on has an issued now below the chain's high-water mark, and
        // it is still one act arriving twice, not one dated before its predecessor.
        let (mut objects, object, _control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let early = signed_by_device(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(21).verifying_key().bytes(),
                once_due(),
            ),
            &device,
        );
        assert_eq!(objects.admit(&early, once_due()), Ok(Admitted::Extended));
        let head = objects.head(object.name()).expect("a head").clone();

        // The chain moves on to a later epoch.
        let later = once_due().plus(Epochs(10)).expect("no overflow");
        let moved_on = signed_by_device(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(22).verifying_key().bytes(),
                later,
            ),
            &device,
        );
        assert_eq!(objects.admit(&moved_on, later), Ok(Admitted::Extended));

        // The early act, heard again, is AlreadyHere — never refused for its epoch.
        assert_eq!(
            objects.admit(&early, later),
            Ok(Admitted::AlreadyHere),
            "one act arriving twice, whatever the chain has done since"
        );
    }

    #[test]
    fn the_control_key_may_not_pile_up_more_than_the_cap_of_waiting_acts() {
        // The words queue a handful of things; the cap is for the machine that would queue a
        // thousand. Past it the asking is refused and not stored, so the cost of the chain stays
        // linear and every node refuses alike.
        let (mut objects, object, control, _device) = an_account();
        let cap = crate::parameter::CONTROL_PENDING_MOST.now() as usize;

        // Fill the queue to the brim with control-signed askings, each for a fresh key so none
        // is a duplicate of another.
        for which in 0..cap {
            let head = objects.head(object.name()).expect("a head").clone();
            let asking = signed_by_control(
                following_at(
                    &object,
                    &head,
                    Kind::HOLDER_ADD_DEVICE,
                    &device_key(100 + u8::try_from(which % 120).unwrap())
                        .verifying_key()
                        .bytes(),
                    once_due(),
                ),
                &control,
            );
            assert_eq!(objects.admit(&asking, once_due()), Ok(Admitted::Extended));
        }

        // One more is refused for the ceiling, not for anything wrong with it.
        let head = objects.head(object.name()).expect("a head").clone();
        let over = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ROTATE,
                &control_key(200).verifying_key().bytes(),
                once_due(),
            ),
            &control,
        );
        assert_eq!(
            objects.admit(&over, once_due()),
            Err(Refused::TooManyWaiting)
        );

        // And once some of the queue has landed, there is room again.
        let landed = once_due().plus(Epochs(72)).expect("no overflow");
        let head = objects.head(object.name()).expect("a head").clone();
        let after = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ROTATE,
                &control_key(200).verifying_key().bytes(),
                landed,
            ),
            &control,
        );
        assert_eq!(objects.admit(&after, landed), Ok(Admitted::Extended));
    }

    #[test]
    fn the_boundary_epoch_belongs_to_the_landing_and_not_to_the_veto() {
        // At exactly issued-plus-the-wait the asking is in force: a cancellation at that moment
        // arrives to find nothing waiting, and the two sides of the boundary agree everywhere —
        // the same rule that lands the effect is the one that retires the veto.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let planted = device_key(66).verifying_key().bytes();

        let asking = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &planted,
                once_due(),
            ),
            &control,
        );
        objects.admit(&asking, once_due()).expect("taken");
        let head = objects.head(object.name()).expect("a head").clone();

        let boundary = once_due().plus(Epochs(72)).expect("no overflow");
        let last_inside = once_due().plus(Epochs(71)).expect("no overflow");
        assert!(
            !holder_at(&objects, &object, last_inside)
                .devices
                .contains(planted.as_slice()),
            "one epoch inside the window it has not landed"
        );
        assert!(
            holder_at(&objects, &object, boundary)
                .devices
                .contains(planted.as_slice()),
            "and at the boundary it has"
        );

        let at_the_boundary = signed_by_device(
            cancel_of(&object, &head, &asking.called(), boundary),
            &device,
        );
        assert_eq!(
            objects.admit(&at_the_boundary, boundary),
            Err(Refused::Malformed),
            "so a veto at the boundary is a veto of something already in force"
        );
    }

    #[test]
    fn a_cancellation_after_the_wait_is_out_cancels_nothing() {
        // The asking landed; there is nothing left to say no to. Anything else would make a
        // cancellation a way of undoing history rather than of stopping it.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let asking = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(66).verifying_key().bytes(),
                once_due(),
            ),
            &control,
        );
        objects.admit(&asking, once_due()).expect("taken");
        let head = objects.head(object.name()).expect("a head").clone();

        let landed = once_due().plus(Epochs(72)).expect("no overflow");
        let too_late =
            signed_by_device(cancel_of(&object, &head, &asking.called(), landed), &device);
        assert_eq!(objects.admit(&too_late, landed), Err(Refused::Malformed));
    }

    #[test]
    fn the_words_never_cancel() {
        // Cancelling is the counterweight the devices hold against the words. In the words' own
        // hands it weighs nothing — and refusing it here costs the honest holder nothing, because
        // the words can simply let their own asking land or freeze the account.
        let (mut objects, object, control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let asking = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(66).verifying_key().bytes(),
                once_due(),
            ),
            &control,
        );
        objects.admit(&asking, once_due()).expect("taken");
        let head = objects.head(object.name()).expect("a head").clone();

        let veto = signed_by_control(
            cancel_of(&object, &head, &asking.called(), once_due()),
            &control,
        );
        assert_eq!(
            objects.admit(&veto, once_due()),
            Err(Refused::NotAuthorised)
        );
    }

    #[test]
    fn freezing_lands_at_once_and_stops_everything_but_saying_no() {
        let (mut objects, object, _control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        // A device freezes — immediately, because freezing concedes nothing.
        let freeze = signed_by_device(
            following_at(&object, &head, Kind::HOLDER_FREEZE, &[], once_due()),
            &device,
        );
        assert_eq!(objects.admit(&freeze, once_due()), Ok(Admitted::Extended));
        assert!(holder_at(&objects, &object, once_due()).frozen);
        let head = objects.head(object.name()).expect("a head").clone();

        // Frozen stops the devices too: adding, removing, even summarising are all refused.
        let adds = signed_by_device(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(21).verifying_key().bytes(),
                once_due(),
            ),
            &device,
        );
        assert_eq!(
            objects.admit(&adds, once_due()),
            Err(Refused::NotAuthorised)
        );

        let removes = signed_by_device(
            following_at(
                &object,
                &head,
                Kind::HOLDER_REMOVE_DEVICE,
                &device.verifying_key().bytes(),
                once_due(),
            ),
            &device,
        );
        assert_eq!(
            objects.admit(&removes, once_due()),
            Err(Refused::NotAuthorised)
        );
    }

    #[test]
    fn a_creation_cannot_follow_anything() {
        // **It would brick the object it landed on.** A creating kind means nothing anywhere but
        // at the start of a chain, so an act carrying one mid-chain is not an act from a newer
        // version this build cannot read — it is one every build can see makes no sense there.
        // Kept, it made the account unresolvable for ever; and the two readers of a summary
        // disagreed about it, one treating it as an act that reset the account and the other as
        // one nobody can apply.
        let (mut objects, object, control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let again = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_CREATE,
                &control_key(200).verifying_key().bytes(),
                once_due(),
            ),
            &control,
        );

        assert_eq!(objects.admit(&again, once_due()), Err(Refused::Malformed));
        assert!(
            matches!(objects.resolve(object.name()), Answer::Here(_)),
            "and the account is untouched"
        );
    }

    #[test]
    fn a_frozen_account_still_forks_rather_than_refusing_a_second_act() {
        // **Freezing is not a fork gate.** A fork is two acts somebody had the right to sign, and
        // whether the account is stopped does not change who had that right — so a second act on a
        // followed head makes a frozen account unresolvable exactly as it would any other. A node
        // that refused the fork because it was frozen, where a node that had not yet heard of the
        // freeze kept it, would be the permanent divergence this whole design refuses.
        let (mut objects, object, _control, device) = an_account();
        let add_head = objects.head(object.name()).expect("a head").clone();

        // The device freezes at the moment it is operative; the add's head now has a successor.
        let freeze = signed_by_device(
            following_at(&object, &add_head, Kind::HOLDER_FREEZE, &[], once_due()),
            &device,
        );
        objects.admit(&freeze, once_due()).expect("taken");
        assert!(holder_at(&objects, &object, once_due()).frozen);

        // A second act following the same pre-freeze head is a fork, and it is kept.
        let sibling = signed_by_device(
            following_at(
                &object,
                &add_head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(21).verifying_key().bytes(),
                once_due(),
            ),
            &device,
        );
        assert_eq!(objects.admit(&sibling, once_due()), Ok(Admitted::Forked));
        assert_eq!(
            objects.resolve(object.name()),
            Answer::CannotResolve(Reason::Forked),
            "kept as a fork, never quietly refused"
        );
    }

    #[test]
    fn a_critical_field_this_build_cannot_read_does_not_land_quietly() {
        // **The attack this closes, written out.** Somebody who copied the twelve words signs an
        // ordinary rotation with one extra odd field. Applying it and going on resolving would give
        // the thief a working account whose owner cannot see the asking and cannot cancel it — the
        // veto of §11.12 switched off by a field nobody has a meaning for. So the object goes
        // opaque instead: this node stops answering for it, which is *denegar servicio sí, mentir
        // no*, and every device is told to ask a node that can read the act.
        let (mut objects, object, control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let mut rotation = following_at(&object, &head, Kind::HOLDER_ROTATE, &[9; 32], once_due());
        // Odd, so critical. A number no build has a meaning for.
        rotation
            .payload
            .insert(7, almena_format::cbor::Value::Uint(1));
        let rotation = signed_by_control(rotation, &control);

        assert_eq!(
            objects.admit(&rotation, once_due()),
            Ok(Admitted::Extended),
            "kept and propagated: replication does not require understanding"
        );
        assert!(
            matches!(
                objects.resolve(object.name()),
                Answer::CannotResolve(Reason::Unintelligible)
            ),
            "and the object is unresolvable rather than resolved without it"
        );
    }

    #[test]
    fn the_act_that_establishes_who_governs_is_asked_the_same_question() {
        // A creation never reaches `apply`, so the field check has to be asked where the creation
        // is taken instead — and the field it catches there is the one that would say who governs
        // the whole account. Missing it is the silent disaster rule 4 exists for, at the root.
        let control = control_key(7);
        let public = control.verifying_key().bytes();
        let mut payload = carrying(&public);
        // Odd, so critical. On the act that says who governs everything.
        payload.insert(3, almena_format::cbor::Value::Uint(1));
        let mut creation = create(
            Network::Development,
            Kind::HOLDER_CREATE.number(),
            1,
            now(),
            payload,
        );
        let signature = control.sign(&creation.signing_bytes());
        creation.signatures.push(Signed {
            by: creation.object.clone(),
            key: public.to_vec(),
            signature: signature.bytes(),
        });

        let mut objects = Objects::new();
        assert_eq!(
            objects.admit(&creation, now()),
            Ok(Admitted::Extended),
            "kept, like any act nobody here can read"
        );
        assert!(
            matches!(
                objects.resolve(creation.object.name()),
                Answer::CannotResolve(Reason::Unintelligible)
            ),
            "and never resolved as though the field were not there"
        );
    }

    #[test]
    fn an_even_field_this_build_does_not_know_is_passed_over() {
        // The other half, and it is what makes an extension an addition rather than a migration:
        // an even number is one a later version marked safe to ignore, and ignoring it is what
        // this build is being told to do. It is also why the checkpoint rides at 100.
        let (mut objects, object, control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let mut rotation = following_at(&object, &head, Kind::HOLDER_ROTATE, &[9; 32], once_due());
        rotation
            .payload
            .insert(8, almena_format::cbor::Value::Uint(1));
        let rotation = signed_by_control(rotation, &control);

        objects.admit(&rotation, once_due()).expect("taken");
        assert!(
            matches!(objects.resolve(object.name()), Answer::Here(_)),
            "still resolving"
        );
    }

    #[test]
    fn only_a_key_the_account_authorises_can_make_it_unreadable() {
        // Asked after the signature, not before. The other way round, anybody who saw an act go
        // past could send back a copy with one odd field added and cost somebody their account
        // while holding no key at all.
        let (mut objects, object, _control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let mut rotation = following_at(&object, &head, Kind::HOLDER_ROTATE, &[9; 32], once_due());
        rotation
            .payload
            .insert(7, almena_format::cbor::Value::Uint(1));
        let stranger = ed25519::SigningKey::from_secret([200; 32]);
        let rotation = signed_by_control(rotation, &stranger);

        assert_eq!(
            objects.admit(&rotation, once_due()),
            Err(Refused::NotAuthorised),
            "refused for what it is, and the account goes on resolving"
        );
        assert!(matches!(objects.resolve(object.name()), Answer::Here(_)));
    }

    #[test]
    fn a_frozen_account_makes_an_unreadable_act_opaque_like_any_other() {
        // The other half of the same rule: an act this build cannot read is not a *no*, so it is
        // kept and the object becomes unintelligible — the honest answer every node gives — rather
        // than being refused because the account happened to be frozen.
        let (mut objects, object, _control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let freeze = signed_by_device(
            following_at(&object, &head, Kind::HOLDER_FREEZE, &[], once_due()),
            &device,
        );
        objects.admit(&freeze, once_due()).expect("taken");
        let head = objects.head(object.name()).expect("a head").clone();

        // An act of a kind this build cannot apply, by an authorised signer, on the frozen account.
        let beyond = signed_by_device(
            following_at(
                &object,
                &head,
                Kind::HOLDER_SET_GUARDIANS,
                &[7; 33],
                once_due(),
            ),
            &device,
        );
        assert_eq!(objects.admit(&beyond, once_due()), Ok(Admitted::Extended));
        assert_eq!(
            objects.resolve(object.name()),
            Answer::CannotResolve(Reason::Unintelligible)
        );
    }

    #[test]
    fn the_whole_story_of_the_stolen_words() {
        // Somebody has photographed the words. The victim still holds their phone. What the thief
        // tries enters the record — which is how the victim finds out — and every move that could
        // take the account waits long enough for one tap to stop it.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let thief_key = device_key(200);

        // The thief freezes first — freezing is immediate — hoping to lock the victim out, and
        // then asks for a device of their own.
        let freeze = signed_by_control(
            following_at(&object, &head, Kind::HOLDER_FREEZE, &[], once_due()),
            &control,
        );
        objects.admit(&freeze, once_due()).expect("taken");
        let head = objects.head(object.name()).expect("a head").clone();

        let plants = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &thief_key.verifying_key().bytes(),
                once_due(),
            ),
            &control,
        );
        assert_eq!(
            objects.admit(&plants, once_due()),
            Ok(Admitted::Extended),
            "the asking enters even on a frozen account — into the window, not past it"
        );
        let head = objects.head(object.name()).expect("a head").clone();

        // The victim's phone can still say no — a cancellation survives the freeze.
        let veto = signed_by_device(
            cancel_of(&object, &head, &plants.called(), once_due()),
            &device,
        );
        assert_eq!(objects.admit(&veto, once_due()), Ok(Admitted::Extended));

        // The attack has collapsed: long after every wait, the thief's key is nobody and the
        // victim's device is still there.
        let long_after = once_due().plus(Epochs(1_000)).expect("no overflow");
        let holder = holder_at(&objects, &object, long_after);
        assert!(
            !holder
                .devices
                .contains(thief_key.verifying_key().bytes().as_slice())
        );
        assert!(
            holder
                .devices
                .contains(device.verifying_key().bytes().as_slice())
        );
    }

    #[test]
    fn thawing_is_the_words_asking_and_the_devices_get_their_window() {
        // Freezing denies and lands at once; thawing concedes back everything the freeze stopped,
        // so it waits like anything else the words ask alone — and a device that knows the words
        // are stolen can keep saying no.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let freeze = signed_by_device(
            following_at(&object, &head, Kind::HOLDER_FREEZE, &[], once_due()),
            &device,
        );
        objects.admit(&freeze, once_due()).expect("taken");
        let head = objects.head(object.name()).expect("a head").clone();

        let thaw = signed_by_control(
            following_at(&object, &head, Kind::HOLDER_UNFREEZE, &[], once_due()),
            &control,
        );
        assert_eq!(objects.admit(&thaw, once_due()), Ok(Admitted::Extended));
        assert!(
            holder_at(&objects, &object, once_due()).frozen,
            "still frozen while the wait runs"
        );

        let landed = once_due().plus(Epochs(72)).expect("no overflow");
        assert!(
            !holder_at(&objects, &object, landed).frozen,
            "and thawed once it is out"
        );
    }

    #[test]
    fn a_cancelled_thaw_leaves_the_account_frozen() {
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let freeze = signed_by_device(
            following_at(&object, &head, Kind::HOLDER_FREEZE, &[], once_due()),
            &device,
        );
        objects.admit(&freeze, once_due()).expect("taken");
        let head = objects.head(object.name()).expect("a head").clone();

        let thaw = signed_by_control(
            following_at(&object, &head, Kind::HOLDER_UNFREEZE, &[], once_due()),
            &control,
        );
        objects.admit(&thaw, once_due()).expect("taken");
        let head = objects.head(object.name()).expect("a head").clone();

        let veto = signed_by_device(
            cancel_of(&object, &head, &thaw.called(), once_due()),
            &device,
        );
        assert_eq!(objects.admit(&veto, once_due()), Ok(Admitted::Extended));

        let long_after = once_due().plus(Epochs(1_000)).expect("no overflow");
        assert!(holder_at(&objects, &object, long_after).frozen);
    }

    #[test]
    fn what_lands_lands_in_the_order_it_was_asked() {
        // Two askings about the same key, in tension: add it, then remove it. Entry order is the
        // one order every reader shares, so everybody ends with the key gone — never a reader
        // with it present because their clock happened to see the removals first.
        let (mut objects, object, control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let key = device_key(66).verifying_key().bytes();

        let adds = signed_by_control(
            following_at(&object, &head, Kind::HOLDER_ADD_DEVICE, &key, once_due()),
            &control,
        );
        objects.admit(&adds, once_due()).expect("taken");
        let head = objects.head(object.name()).expect("a head").clone();

        // The removal is asked one epoch later, so it lands one epoch later too.
        let next = once_due().plus(Epochs(1)).expect("no overflow");
        let removes = signed_by_control(
            following_at(&object, &head, Kind::HOLDER_REMOVE_DEVICE, &key, next),
            &control,
        );
        // Not there yet — the asking to add has not landed — so removing it is refused as the
        // mistake it would be, judged against the account as it stands.
        assert_eq!(objects.admit(&removes, next), Err(Refused::Malformed));

        // Once the addition lands, the removal can be asked, and each lands in its own time.
        let landed = once_due().plus(Epochs(72)).expect("no overflow");
        let removes = signed_by_control(
            following_at(&object, &head, Kind::HOLDER_REMOVE_DEVICE, &key, landed),
            &control,
        );
        objects.admit(&removes, landed).expect("taken");
        assert!(
            holder_at(&objects, &object, landed)
                .devices
                .contains(key.as_slice())
        );
        let both_landed = landed.plus(Epochs(72)).expect("no overflow");
        assert!(
            !holder_at(&objects, &object, both_landed)
                .devices
                .contains(key.as_slice())
        );
    }

    #[test]
    fn a_summary_written_during_a_wait_honestly_leaves_the_asking_out() {
        // A summary states the account as of the act that carries it. While an asking waits, the
        // honest claim is the account without it — anything else would be jumping the wait.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let planted = device_key(66).verifying_key().bytes();

        let asking = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &planted,
                once_due(),
            ),
            &control,
        );
        objects.admit(&asking, once_due()).expect("taken");

        let devices_claim = |at: Epoch| {
            let standing = objects.standing(object.name(), at).expect("it resolves");
            standing
                .claims
                .into_iter()
                .find(|claim| claim.about == crate::checkpoint::Governs::Devices)
                .expect("a claim about the devices")
        };

        let during = devices_claim(once_due());
        assert_eq!(
            during.stated,
            crate::checkpoint::Stated::Keys(BTreeSet::from([device
                .verifying_key()
                .bytes()
                .to_vec()])),
            "the resident device, and not the one still waiting"
        );
        assert_eq!(
            during.set_by,
            asking.called(),
            "and it cites the asking, which is the newest act a reader must know about"
        );

        // Once the wait is out, the claim changes with the account.
        let landed = once_due().plus(Epochs(72)).expect("no overflow");
        assert_eq!(
            devices_claim(landed).stated,
            crate::checkpoint::Stated::Keys(BTreeSet::from([
                device.verifying_key().bytes().to_vec(),
                planted.to_vec(),
            ]))
        );
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
            following_at(&object, &head, Kind::HOLDER_ROTATE, &fresh, once_due()),
            &device,
        );
        assert_eq!(
            objects.admit(&by_device, once_due()),
            Err(Refused::NotAuthorised)
        );

        let by_control = signed_by_control(
            following_at(&object, &head, Kind::HOLDER_ROTATE, &fresh, once_due()),
            &control,
        );
        assert_eq!(
            objects.admit(&by_control, once_due()),
            Ok(Admitted::Extended)
        );

        // And even the control key does not rotate at once: replacing the account's key is the
        // account changing hands, which is exactly the thing the wait exists to hold up where
        // the devices can see it.
        let old = control.verifying_key().bytes();
        assert_eq!(holder_at(&objects, &object, once_due()).control, old);
        let landed = once_due().plus(Epochs(72)).expect("no overflow");
        assert_eq!(holder_at(&objects, &object, landed).control, fresh);
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

    #[test]
    fn a_stranger_cannot_make_an_account_unresolvable_with_an_act_nobody_signed() {
        // **What this costs if it is wrong: every account on the network, permanently.** An act of
        // a kind nobody knows is kept and passed on and stops the object resolving — which is
        // right, and which is why the act has to have been written by somebody entitled to write
        // there. Otherwise one unsigned message brings down any account it names, for ever, for
        // free.
        let (mut objects, object, _control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let intruder = Operation {
            object: object.clone(),
            previous: Some(head),
            kind: 9_999,
            version: 1,
            issued: now(),
            payload: BTreeMap::new(),
            signatures: Vec::new(),
        };

        assert_eq!(objects.admit(&intruder, now()), Err(Refused::Unsigned));
        assert!(
            matches!(objects.resolve(object.name()), Answer::Here(_)),
            "and the account is untouched"
        );
    }

    #[test]
    fn a_stranger_cannot_fork_an_account_either() {
        // The same attack through the other door. A fork is two acts **somebody had the right to
        // sign**; an act signed by nobody is not one of them, and treating it as one would make
        // any account unresolvable to anybody who bothered to replay an old head.
        let (mut objects, object, _control, device) = an_account();
        let creation_head = objects.head(object.name()).expect("a head").clone();

        // Something real, so that the predecessor has a successor and the next act forks.
        let removal = signed_by_device(
            following_at(
                &object,
                &creation_head,
                Kind::HOLDER_REMOVE_DEVICE,
                &device.verifying_key().bytes(),
                once_due(),
            ),
            &device,
        );
        assert_eq!(objects.admit(&removal, once_due()), Ok(Admitted::Extended));

        let forger = Operation {
            object: object.clone(),
            previous: Some(creation_head),
            kind: Kind::HOLDER_ADD_DEVICE.number(),
            version: 1,
            issued: now(),
            payload: carrying(&[9; 33]),
            signatures: Vec::new(),
        };

        assert_eq!(objects.admit(&forger, once_due()), Err(Refused::Unsigned));
        assert!(
            matches!(objects.resolve(object.name()), Answer::Here(_)),
            "and the account still resolves"
        );
    }

    #[test]
    fn an_act_this_build_cannot_apply_is_kept_rather_than_refused() {
        // **Refusing would split the record between versions**, which is the one thing that must
        // not happen: a refused act is not stored, so a node that refuses what another keeps has a
        // different history. A number this build lists is not the same as an act it can apply, and
        // only the second may decide what the state becomes.
        let (mut objects, object, control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let beyond = signed_by_control(
            following(&object, &head, Kind::HOLDER_SET_GUARDIANS, &[7; 33]),
            &control,
        );

        assert_eq!(objects.admit(&beyond, now()), Ok(Admitted::Extended));
        assert_eq!(
            objects.resolve(object.name()),
            Answer::CannotResolve(Reason::Unintelligible),
            "kept, passed on, and this node stops saying what the account is"
        );
    }

    #[test]
    fn an_act_from_a_newer_version_still_has_to_be_signed_by_somebody_who_may_write() {
        // The weakest true statement, and the one that can be made without understanding the act:
        // a key this account authorises for something signed it.
        let (mut objects, object, _control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let outsider = control_key(200);
        let newer = signed_by_control(
            following(&object, &head, Kind::HOLDER_SET_GUARDIANS, &[3; 33]),
            &outsider,
        );

        assert_eq!(objects.admit(&newer, now()), Err(Refused::NotAuthorised));
    }

    #[test]
    fn nothing_extends_a_contradiction() {
        // Evidence, not an account. It says what it says because of the two signatures it carries,
        // and nobody controls it — so there is nobody a later act could be authorised by.
        let mut objects = Objects::new();
        let node = |seed: u8| crate::root::Root {
            network: Name::of(b"one network"),
            node: Did::new(Network::Development, Name::of(&[seed])),
            epoch: Epoch::GENESIS,
            size: 4,
            root: almena_suite::digest::Digest::of(b"whatever"),
        };
        let one = crate::root::Root {
            root: almena_suite::digest::Digest::of(b"one history"),
            ..node(3)
        }
        .publish(&control_key(3));
        let other = crate::root::Root {
            root: almena_suite::digest::Digest::of(b"another history"),
            ..node(3)
        }
        .publish(&control_key(3));

        let published = crate::contradiction::publish(&one, &other, now(), &control_key(9));
        objects
            .admit(&published.operation, now())
            .expect("evidence");
        let head = objects
            .head(published.named.name())
            .expect("a head")
            .clone();

        let after = signed_by_control(
            following(&published.named, &head, Kind::HOLDER_ADD_DEVICE, &[1; 33]),
            &control_key(9),
        );
        assert_eq!(objects.admit(&after, now()), Err(Refused::NotAuthorised));
    }

    /// An act on that account that carries the summary the record says it would sign.
    fn summarising(
        objects: &Objects,
        object: &Did,
        control: &ed25519::SigningKey,
        at: Epoch,
    ) -> Operation {
        let head = objects.head(object.name()).expect("a head").clone();
        let standing = objects.standing(object.name(), at).expect("it resolves");
        let mut operation = following_at(object, &head, Kind::HOLDER_CHECKPOINT, &[], at);
        operation.payload = BTreeMap::from([(
            crate::checkpoint::FIELD,
            crate::checkpoint::declaration(&standing.claims),
        )]);
        signed_by_control(operation, control)
    }

    #[test]
    fn a_summary_is_built_from_what_the_chain_was_watched_doing() {
        // The state alone cannot be summarised: a summary says *the devices are these, and this act
        // put them there*, and only something that watched the chain go past knows the second half.
        // Built for the moment the carrying act will be issued: a summary written while the
        // device's wait still runs would honestly state an account with none.
        let (objects, object, control, device) = an_account();
        let standing = objects
            .standing(object.name(), once_due())
            .expect("it resolves");

        let control_claim = standing
            .claims
            .iter()
            .find(|claim| claim.about == crate::checkpoint::Governs::Control)
            .expect("a summary accounts for every part");
        assert_eq!(
            control_claim.stated,
            crate::checkpoint::Stated::Key(control.verifying_key().bytes().to_vec())
        );

        let devices = standing
            .claims
            .iter()
            .find(|claim| claim.about == crate::checkpoint::Governs::Devices)
            .expect("a summary accounts for every part");
        assert_eq!(
            devices.stated,
            crate::checkpoint::Stated::Keys(BTreeSet::from([device
                .verifying_key()
                .bytes()
                .to_vec()]))
        );
        assert_ne!(
            control_claim.set_by, devices.set_by,
            "the creation set the control key and a later act set the devices"
        );
    }

    #[test]
    fn what_a_summary_says_is_what_the_record_would_have_to_carry() {
        // The summary a node offers must be one that stands up against that node's own record.
        // Building it from anything but the chain would be building one that falls over.
        let (objects, object, _control, _device) = an_account();
        let standing = objects.standing(object.name(), now()).expect("it resolves");
        let read_back = crate::checkpoint::declared(&{
            let mut carrier = following(
                &object,
                objects.head(object.name()).expect("a head"),
                Kind::HOLDER_CHECKPOINT,
                &[],
            );
            carrier.payload = BTreeMap::from([(
                crate::checkpoint::FIELD,
                crate::checkpoint::declaration(&standing.claims),
            )]);
            carrier
        });
        assert_eq!(read_back, Ok(Some(standing.claims)));
    }

    #[test]
    fn the_count_runs_from_the_creation_and_a_summary_puts_it_back_to_nothing() {
        // What a summary saves is acts to replay, and the act that created the object is one of
        // them. An act that carries a summary describes the state as of itself, so nothing is left
        // to replay behind it.
        let (mut objects, object, control, _device) = an_account();
        assert_eq!(
            objects
                .standing(object.name(), now())
                .expect("it resolves")
                .since,
            2,
            "the creation and the device that was added"
        );

        // Summarised once the fixture's own asking has come due: a summary has nowhere to say what
        // is waiting, so it waits for the queue to empty.
        let summary = summarising(&objects, &object, &control, once_due());
        assert_eq!(objects.admit(&summary, once_due()), Ok(Admitted::Extended));
        assert_eq!(
            objects
                .standing(object.name(), now())
                .expect("it resolves")
                .since,
            0
        );
    }

    #[test]
    fn a_summary_nobody_could_read_does_not_discharge_anything() {
        // Otherwise an object would put anything at all in that field and never owe one again.
        let (mut objects, object, control, _device) = an_account();
        let before = objects
            .standing(object.name(), now())
            .expect("it resolves")
            .since;

        let head = objects.head(object.name()).expect("a head").clone();
        let mut nonsense = following_at(&object, &head, Kind::HOLDER_CHECKPOINT, &[], once_due());
        nonsense.payload = BTreeMap::from([(
            crate::checkpoint::FIELD,
            Value::Text("not a summary at all".to_owned()),
        )]);
        let nonsense = signed_by_control(nonsense, &control);

        assert_eq!(objects.admit(&nonsense, once_due()), Ok(Admitted::Extended));
        assert_eq!(
            objects
                .standing(object.name(), now())
                .expect("it resolves")
                .since,
            before + 1,
            "it is one more act to replay and not one fewer"
        );
    }

    #[test]
    fn an_object_owes_a_summary_once_it_has_written_enough_and_not_before() {
        // An object that reaches the number and goes quiet is safe by construction: it is left with
        // exactly the number of acts that was chosen as reproducible. What the rule stops is
        // writing a great deal and never summarising.
        let (mut objects, object, _control, device) = an_account();
        let every = crate::parameter::SUMMARISE_EVERY.now();

        while objects
            .standing(object.name(), once_due())
            .expect("it resolves")
            .since
            < every
        {
            let head = objects.head(object.name()).expect("a head").clone();
            let standing = objects
                .standing(object.name(), once_due())
                .expect("it resolves");
            assert!(!standing.owed, "at {} acts", standing.since);

            // A spare key the resident device turns over — immediately, as devices do.
            let key = device_key(21).verifying_key().bytes();
            let act = if standing.since.is_multiple_of(2) {
                signed_by_device(
                    following_at(&object, &head, Kind::HOLDER_ADD_DEVICE, &key, once_due()),
                    &device,
                )
            } else {
                signed_by_device(
                    following_at(&object, &head, Kind::HOLDER_REMOVE_DEVICE, &key, once_due()),
                    &device,
                )
            };
            assert_eq!(objects.admit(&act, once_due()), Ok(Admitted::Extended));
        }

        let standing = objects
            .standing(object.name(), once_due())
            .expect("it resolves");
        assert_eq!(standing.since, every);
        assert!(standing.owed, "the next act it signs should carry one");
    }

    #[test]
    fn owing_a_summary_is_never_a_reason_to_refuse_an_act() {
        // **N is a number the protocol can change**, so a node that refused over it would refuse
        // what a node on another version keeps — and a refused act is not stored, which is the one
        // disagreement this design cannot have. The debt is said; it is not policed.
        let (mut objects, object, _control, device) = an_account();
        let key = device_key(21).verifying_key().bytes();

        // Signed by the resident device, which acts at once, so length piles up without anything
        // waiting — the debt is about acts-to-replay, not about what is in flight.
        for _ in 0..(crate::parameter::SUMMARISE_EVERY.now() * 2) {
            let head = objects.head(object.name()).expect("a head").clone();
            let act = signed_by_device(
                following_at(&object, &head, Kind::HOLDER_ADD_DEVICE, &key, once_due()),
                &device,
            );
            assert_eq!(
                objects.admit(&act, once_due()),
                Ok(Admitted::Extended),
                "still taken, however far behind it is"
            );
        }
        assert!(
            objects
                .standing(object.name(), once_due())
                .expect("it resolves")
                .owed
        );
    }

    #[test]
    fn nothing_summarises_over_an_asking_still_in_flight() {
        // **A summary has nowhere to say what is waiting.** It carries the control key and the
        // devices; an asking in flight leaves no trace in one. And a node serves the last summary
        // and what followed — so an asking made just before a summary is an asking no reader will
        // ever see, landing seventy-two epochs later on a state nobody was shown. That is the
        // notice `SPECS.md §11.12` promises, deleted by arithmetic, and the control key can write
        // both acts, so it is a thing to be done on purpose rather than an accident.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        // The words ask for a device of their own, which waits.
        let asking = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &device_key(66).verifying_key().bytes(),
                once_due(),
            ),
            &control,
        );
        objects
            .admit(&asking, once_due())
            .expect("the asking enters");

        // And then a summary, which would carry it away with it.
        let hiding = summarising(&objects, &object, &control, once_due());
        assert_eq!(
            objects.admit(&hiding, once_due()),
            Err(Refused::NotAuthorised),
            "not while anything is waiting"
        );

        // A device's is refused for the same reason, and by the same rule.
        let head = objects.head(object.name()).expect("a head").clone();
        let mut theirs = following_at(&object, &head, Kind::HOLDER_CHECKPOINT, &[], once_due());
        theirs.payload = BTreeMap::from([(
            crate::checkpoint::FIELD,
            crate::checkpoint::declaration(
                &objects
                    .standing(object.name(), once_due())
                    .expect("it resolves")
                    .claims,
            ),
        )]);
        assert_eq!(
            objects.admit(&signed_by_device(theirs, &device), once_due()),
            Err(Refused::NotAuthorised),
            "a device may summarise as freely as the words, and no more freely"
        );

        // Once the wait has run out there is nothing for a summary to hide, and it is taken.
        let later = once_due()
            .plus(Epochs(crate::parameter::CONTROL_WAITS.at(once_due())))
            .expect("no overflow");
        let honest = summarising(&objects, &object, &control, later);
        assert_eq!(objects.admit(&honest, later), Ok(Admitted::Extended));
    }

    #[test]
    fn a_summary_act_leaves_the_account_exactly_as_it_was() {
        // It restates what the chain already produces. A node that took the state from what a
        // summary declared would believe a claim instead of checking it — and would then resolve
        // differently from a node that replayed, with nobody having lied.
        let (mut objects, object, control, _device) = an_account();
        // Taken at the moment the summary is written, so that what is compared is like with like:
        // an act at a later epoch lands whatever was due by then, and that is the chain working
        // rather than the summary being believed.
        let before = holder_at(&objects, &object, once_due());

        let mut summary = summarising(&objects, &object, &control, once_due());
        // And it says the account is something else entirely.
        summary.payload = BTreeMap::from([(
            crate::checkpoint::FIELD,
            crate::checkpoint::declaration(&[crate::checkpoint::Claim {
                about: crate::checkpoint::Governs::Control,
                stated: crate::checkpoint::Stated::Key(vec![200; 32]),
                set_by: Name::of(b"nothing that ever happened"),
            }]),
        )]);
        let summary = signed_by_control(
            Operation {
                signatures: Vec::new(),
                ..summary
            },
            &control,
        );

        assert_eq!(objects.admit(&summary, once_due()), Ok(Admitted::Extended));
        assert_eq!(
            holder_at(&objects, &object, once_due()),
            before,
            "the account is what its chain says, not what a summary says"
        );
    }

    #[test]
    fn an_object_this_node_will_not_resolve_is_one_it_will_not_summarise_either() {
        // A summary drawn from a state that stopped being computable would be a statement about a
        // history nobody could follow.
        let (mut objects, object, control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let newer = signed_by_control(
            following(&object, &head, Kind::HOLDER_SET_GUARDIANS, &[3; 33]),
            &control,
        );

        assert_eq!(objects.admit(&newer, now()), Ok(Admitted::Extended));
        assert_eq!(objects.standing(object.name(), now()), None);
    }

    #[test]
    fn a_summary_act_still_has_to_be_signed_by_somebody_who_may_write() {
        let (mut objects, object, _control, _device) = an_account();
        let stranger = control_key(201);
        let summary = summarising(&objects, &object, &stranger, once_due());
        // Admitted at the moment it is dated: a summary waits for the queue to empty, so it is
        // written once the fixture's own asking has come due.
        assert_eq!(
            objects.admit(&summary, once_due()),
            Err(Refused::NotAuthorised)
        );
    }

    #[test]
    fn one_act_delivered_twice_is_one_act() {
        // **Not an attack: the ordinary path.** Two peers holding one record hand over overlapping
        // pages, and a page asked for again after a connection drops arrives in full. Reading the
        // second copy as two acts claiming one predecessor would let a node make its own objects
        // unresolvable simply by being told the truth twice.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let act = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_REMOVE_DEVICE,
                &device.verifying_key().bytes(),
                once_due(),
            ),
            &control,
        );

        assert_eq!(objects.admit(&act, once_due()), Ok(Admitted::Extended));
        assert_eq!(objects.admit(&act, once_due()), Ok(Admitted::AlreadyHere));
        assert_eq!(objects.admit(&act, once_due()), Ok(Admitted::AlreadyHere));
        assert!(
            matches!(objects.resolve(object.name()), Answer::Here(_)),
            "and the account still resolves"
        );
    }

    #[test]
    fn hearing_an_act_again_does_not_move_the_count_or_the_state() {
        // The chain holds exactly what it held. A second delivery that advanced the count would
        // make an object owe a summary for acts nobody wrote.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let act = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_REMOVE_DEVICE,
                &device.verifying_key().bytes(),
                once_due(),
            ),
            &control,
        );
        objects.admit(&act, once_due()).expect("taken");

        let before = (
            objects.resolve(object.name()),
            objects.standing(object.name(), once_due()),
            objects.head(object.name()).cloned(),
        );
        objects.admit(&act, once_due()).expect("already here");

        assert_eq!(
            (
                objects.resolve(object.name()),
                objects.standing(object.name(), once_due()),
                objects.head(object.name()).cloned()
            ),
            before
        );
    }

    #[test]
    fn two_different_acts_on_one_predecessor_are_still_a_fork() {
        // The rule the guard above must not have weakened.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();

        let one = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_REMOVE_DEVICE,
                &device.verifying_key().bytes(),
                once_due(),
            ),
            &control,
        );
        let other = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_ADD_DEVICE,
                &[11; 33],
                once_due(),
            ),
            &control,
        );

        assert_eq!(objects.admit(&one, once_due()), Ok(Admitted::Extended));
        assert_eq!(objects.admit(&other, once_due()), Ok(Admitted::Forked));
        assert_eq!(
            objects.resolve(object.name()),
            Answer::CannotResolve(Reason::Forked)
        );
    }

    #[test]
    fn an_act_cannot_be_split_off_by_stapling_a_signature_to_it() {
        // **The cheapest attack there was, and it needed no key at all.** What an act is called on a
        // chain covers its signatures; what a signature covers does not. So anybody who saw an act
        // go past could send it back with a few bytes added: a new name on the same chain, still
        // carrying the original signature, still checking out, and the object split for ever.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let act = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_REMOVE_DEVICE,
                &device.verifying_key().bytes(),
                once_due(),
            ),
            &control,
        );
        assert_eq!(objects.admit(&act, once_due()), Ok(Admitted::Extended));

        let mut padded = act.clone();
        padded.signatures.push(Signed {
            by: object.clone(),
            key: vec![0; 32],
            signature: [0; 64],
        });

        assert_eq!(
            padded.called(),
            act.called(),
            "one act, whatever was stapled to it — which is what closes this at the root"
        );
        assert_eq!(
            objects.admit(&padded, once_due()),
            Err(Refused::Malformed),
            "and a second line: an act carries the signatures its rule calls for and no others"
        );
        assert!(
            matches!(objects.resolve(object.name()), Answer::Here(_)),
            "and the account still resolves"
        );
    }

    #[test]
    fn a_creation_cannot_be_padded_either() {
        // Otherwise the padded one arriving first would take the name, the honest one would be
        // refused for already existing, and this node's head for that object would be a hash no
        // other node holds.
        let control = control_key(7);
        let mut padded = creation(&control);
        padded.signatures.push(Signed {
            by: padded.object.clone(),
            key: vec![0; 32],
            signature: [0; 64],
        });

        let mut objects = Objects::new();
        assert_eq!(objects.admit(&padded, now()), Err(Refused::Malformed));
    }

    #[test]
    fn an_object_this_node_cannot_read_is_not_one_anybody_may_extend() {
        // *Cannot read* is not *anything goes*. Taking unsigned acts on it would put things in this
        // node's record that no other node holds, which is the worse of the two divergences.
        let (mut objects, object, control, _device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let newer = signed_by_control(
            following(&object, &head, Kind::HOLDER_SET_GUARDIANS, &[3; 33]),
            &control,
        );
        assert_eq!(objects.admit(&newer, now()), Ok(Admitted::Extended));
        assert_eq!(
            objects.resolve(object.name()),
            Answer::CannotResolve(Reason::Unintelligible)
        );

        let opaque_head = objects.head(object.name()).expect("a head").clone();
        let stranger = control_key(202);
        let piling_on = signed_by_control(
            following(&object, &opaque_head, Kind::HOLDER_ADD_DEVICE, &[9; 33]),
            &stranger,
        );
        assert_eq!(
            objects.admit(&piling_on, now()),
            Err(Refused::NotAuthorised)
        );
    }

    #[test]
    fn an_object_this_node_cannot_read_cannot_be_split_by_a_stranger_either() {
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let newer = signed_by_control(
            following(&object, &head, Kind::HOLDER_SET_GUARDIANS, &[3; 33]),
            &control,
        );
        objects.admit(&newer, now()).expect("kept and passed on");

        // A second act claiming the predecessor the unreadable one already followed.
        let stranger = control_key(203);
        let splitting = signed_by_control(
            following(
                &object,
                &head,
                Kind::HOLDER_REMOVE_DEVICE,
                &device.verifying_key().bytes(),
            ),
            &stranger,
        );
        assert_eq!(
            objects.admit(&splitting, now()),
            Err(Refused::NotAuthorised)
        );
    }

    #[test]
    fn who_a_key_is_does_not_depend_on_what_arrived_first() {
        // **Announcing is meant to happen again, and only the first one names anything** — so a key
        // that announced twice must answer to one name. First by what the acts say, never by what
        // this node happened to hear first: two nodes do not receive things in the same order, and
        // a census that turned on that would have honest nodes disagreeing about who somebody is.
        use almena_time::Epochs;

        let key = control_key(11);
        let first = crate::announce::announce(crate::genesis::Which::Development, now(), &key);
        let later = crate::announce::announce(
            crate::genesis::Which::Development,
            now().plus(Epochs(1)).expect("no overflow"),
            &key,
        );
        assert_ne!(first.node, later.node, "two acts, so two names");

        let mut one = Objects::new();
        one.admit(&first.operation, now()).expect("taken");
        one.admit(&later.operation, now()).expect("taken");

        let mut other = Objects::new();
        other.admit(&later.operation, now()).expect("taken");
        other.admit(&first.operation, now()).expect("taken");

        let public = key.verifying_key().bytes();
        assert_eq!(one.node_called(&public), other.node_called(&public));
        assert_eq!(
            one.node_called(&public),
            Some(first.node.name()),
            "and it is the one the acts say came first"
        );
    }

    #[test]
    fn one_key_is_one_node_however_often_it_announces() {
        // The census is what the replication share is drawn across. A key that could enter it twice
        // would be one machine drawing two shares.
        let mut objects = Objects::new();
        for behind in 0..4 {
            let announced = crate::announce::announce(
                crate::genesis::Which::Development,
                Epoch::new(now().number() - behind),
                &control_key(12),
            );
            objects.admit(&announced.operation, now()).expect("taken");
        }
        assert_eq!(objects.nodes().count(), 1);
    }

    #[test]
    fn a_node_says_again_what_it_is_running_without_renaming_itself() {
        // **Announcing is meant to happen again.** What a node offers and what version it speaks
        // change over its life, and neither may rename it — which is why the act that named it
        // carries its key and nothing else.
        use crate::capability::Capability;

        let mut objects = Objects::new();
        let its_key = control_key(30);
        let announced =
            crate::announce::announce(crate::genesis::Which::Development, now(), &its_key);
        objects.admit(&announced.operation, now()).expect("taken");

        assert_eq!(
            objects.resolve(announced.node.name()),
            Answer::Here(State::Node {
                key: its_key.verifying_key().bytes(),
                offers: BTreeSet::new(),
                speaks: 0,
                claimed_by: None,
                reachable: BTreeSet::new(),
            }),
            "it has said nothing yet, which is a fact and not a gap"
        );

        let head = objects.head(announced.node.name()).expect("a head").clone();
        let offers = BTreeSet::from([Capability::Interface, Capability::Relay]);
        let saying = crate::announce::offering(
            &announced.node,
            &head,
            &offers,
            crate::announce::Speaking {
                version: 2,
                reachable: &BTreeSet::new(),
                issued: now(),
                key: &its_key,
            },
        );
        assert_eq!(objects.admit(&saying, now()), Ok(Admitted::Extended));

        assert_eq!(
            objects.resolve(announced.node.name()),
            Answer::Here(State::Node {
                key: its_key.verifying_key().bytes(),
                offers,
                speaks: 2,
                claimed_by: None,
                reachable: BTreeSet::new(),
            })
        );
        assert_eq!(
            objects.node_called(&its_key.verifying_key().bytes()),
            Some(announced.node.name()),
            "and it is still called what it was called"
        );
    }

    #[test]
    fn only_the_node_itself_may_say_what_it_offers() {
        let mut objects = Objects::new();
        let its_key = control_key(31);
        let announced =
            crate::announce::announce(crate::genesis::Which::Development, now(), &its_key);
        objects.admit(&announced.operation, now()).expect("taken");
        let head = objects.head(announced.node.name()).expect("a head").clone();

        let stranger = control_key(32);
        let lying = crate::announce::offering(
            &announced.node,
            &head,
            &BTreeSet::from([crate::capability::Capability::Interface]),
            crate::announce::Speaking {
                version: 9,
                reachable: &BTreeSet::new(),
                issued: now(),
                key: &stranger,
            },
        );
        assert_eq!(objects.admit(&lying, now()), Err(Refused::NotAuthorised));
    }

    #[test]
    fn a_capability_this_build_has_never_heard_of_stops_it_saying_what_that_node_offers() {
        // **Not passed over.** A reader that dropped it would count the node as offering less than
        // it does, and counting is the whole reason the field exists. Kept, passed on, and this
        // node declines to say — which is what it does with anything else it cannot read.
        let mut objects = Objects::new();
        let its_key = control_key(33);
        let announced =
            crate::announce::announce(crate::genesis::Which::Development, now(), &its_key);
        objects.admit(&announced.operation, now()).expect("taken");
        let head = objects.head(announced.node.name()).expect("a head").clone();

        let mut newer = crate::announce::offering(
            &announced.node,
            &head,
            &BTreeSet::from([crate::capability::Capability::Interface]),
            crate::announce::Speaking {
                version: 3,
                reachable: &BTreeSet::new(),
                issued: now(),
                key: &its_key,
            },
        );
        newer.payload.insert(
            crate::capability::OFFERS,
            Value::Array(vec![Value::Uint(1), Value::Uint(9_999)]),
        );
        let newer = signed_by_control(
            Operation {
                signatures: Vec::new(),
                ..newer
            },
            &its_key,
        );

        assert_eq!(objects.admit(&newer, now()), Ok(Admitted::Extended));
        assert_eq!(
            objects.resolve(announced.node.name()),
            Answer::CannotResolve(Reason::Unintelligible)
        );
    }

    #[test]
    fn what_the_network_is_running_is_counted_and_never_declared() {
        // The need is visible before it is a problem, and whoever wants to contribute can see what
        // to contribute. It counts what nodes **say**; what they do is measured by asking.
        use crate::capability::Capability;

        let mut objects = Objects::new();
        let offered = [
            vec![Capability::Interface],
            vec![Capability::Interface, Capability::Mailbox],
            vec![],
        ];
        for (which, offers) in offered.iter().enumerate() {
            let its_key = control_key(40 + which as u8);
            let announced =
                crate::announce::announce(crate::genesis::Which::Development, now(), &its_key);
            objects.admit(&announced.operation, now()).expect("taken");
            let head = objects.head(announced.node.name()).expect("a head").clone();
            let saying = crate::announce::offering(
                &announced.node,
                &head,
                &offers.iter().copied().collect(),
                crate::announce::Speaking {
                    version: if which == 2 { 1 } else { 2 },
                    reachable: &BTreeSet::new(),
                    issued: now(),
                    key: &its_key,
                },
            );
            objects.admit(&saying, now()).expect("taken");
        }

        let counted = objects.running();
        assert_eq!(counted.offering[&Capability::Interface], 2);
        assert_eq!(counted.offering[&Capability::Mailbox], 1);
        assert_eq!(
            counted.offering[&Capability::Relay],
            0,
            "nobody offers it, said as nought rather than left out"
        );
        assert_eq!(counted.speaking.get(&2), Some(&2));
        assert_eq!(counted.speaking.get(&1), Some(&1));
        assert_eq!(counted.unreadable, 0);
    }

    #[test]
    fn rewriting_whose_signature_it_says_it_is_does_not_split_the_chain() {
        // **The fourth door of the same shape, and the cheapest yet.** A signature covers everything
        // but the signature list, so the name inside it is not covered by it — while the act's name
        // on the chain is. Anybody who merely saw an act go past could rewrite that one name and
        // send it back: a new name on the same chain, carrying a signature that still verifies, with
        // no key and nothing forged.
        let (mut objects, object, control, device) = an_account();
        let head = objects.head(object.name()).expect("a head").clone();
        let act = signed_by_control(
            following_at(
                &object,
                &head,
                Kind::HOLDER_REMOVE_DEVICE,
                &device.verifying_key().bytes(),
                once_due(),
            ),
            &control,
        );
        assert_eq!(objects.admit(&act, once_due()), Ok(Admitted::Extended));

        let mut rewritten = act.clone();
        rewritten.signatures[0].by = Did::new(Network::Development, Name::of(b"anybody at all"));

        assert_eq!(
            rewritten.signing_bytes(),
            act.signing_bytes(),
            "the signature still covers exactly the same bytes"
        );
        assert_eq!(
            rewritten.called(),
            act.called(),
            "one act, whoever the signature now claims to be by"
        );

        assert_eq!(
            objects.admit(&rewritten, once_due()),
            Err(Refused::NotAuthorised),
            "and a second line: a signature has to say it is the object's own"
        );
        assert!(
            matches!(objects.resolve(object.name()), Answer::Here(_)),
            "and the account still resolves"
        );
    }

    #[test]
    fn a_creation_cannot_be_split_off_that_way_either() {
        // The same one field, against the act that brings an object into existence. It names itself
        // without its signatures, so the rewritten one takes the same name — and whichever arrived
        // first would decide what every honest node held.
        let control = control_key(7);
        let mut rewritten = creation(&control);
        rewritten.signatures[0].by = Did::new(Network::Development, Name::of(b"somebody else"));

        let mut objects = Objects::new();
        assert_eq!(
            objects.admit(&rewritten, now()),
            Err(Refused::NotAuthorised)
        );
    }

    #[test]
    fn a_node_this_record_cannot_read_is_said_and_not_dropped_from_the_figures() {
        // **Dropping it would make both figures look tidier than the network is.** A node whose
        // chain carries an act this build has no meaning for exists and offers something unknown;
        // leaving it out of the denominator would report a cleaner network than anybody measured.
        let mut objects = Objects::new();
        let its_key = control_key(50);
        let announced =
            crate::announce::announce(crate::genesis::Which::Development, now(), &its_key);
        objects.admit(&announced.operation, now()).expect("taken");
        let head = objects.head(announced.node.name()).expect("a head").clone();

        let mut newer = crate::announce::offering(
            &announced.node,
            &head,
            &BTreeSet::from([crate::capability::Capability::Interface]),
            crate::announce::Speaking {
                version: 3,
                reachable: &BTreeSet::new(),
                issued: now(),
                key: &its_key,
            },
        );
        newer.payload.insert(
            crate::capability::OFFERS,
            Value::Array(vec![Value::Uint(9_999)]),
        );
        let newer = signed_by_control(
            Operation {
                signatures: Vec::new(),
                ..newer
            },
            &its_key,
        );
        objects.admit(&newer, now()).expect("kept and passed on");

        let counted = objects.running();
        assert_eq!(counted.unreadable, 1, "one node nobody can read");
        assert_eq!(
            counted.offering[&crate::capability::Capability::Interface],
            0,
            "and it is counted as offering nothing rather than as offering what it last said"
        );
        assert!(
            counted.speaking.is_empty(),
            "and it is in no version's count, rather than in the wrong one"
        );
    }

    #[test]
    fn an_object_held_elsewhere_is_not_an_object_that_does_not_exist() {
        // **The plainest lie a node could tell**, and the one answer that had never had a producer.
        // Every node carries the line saying an act happened; only the nodes it was dealt to carry
        // what it said. Answering *it does not exist* about something visible in its own record
        // would be denying what it can see.
        let mut objects = Objects::new();
        let somewhere_else = Name::of(b"an object dealt to somebody else");

        assert_eq!(objects.resolve(&somewhere_else), Answer::DoesNotExist);
        objects.noted(&somewhere_else);
        assert_eq!(objects.resolve(&somewhere_else), Answer::NotHere);
    }

    #[test]
    fn having_the_history_is_more_than_knowing_it_exists() {
        // Taking note must never take anything away: a node that held an account and was then told
        // it exists would otherwise forget what it knew.
        let (mut objects, object, _control, _device) = an_account();
        let before = objects.resolve(object.name());

        objects.noted(object.name());
        assert_eq!(objects.resolve(object.name()), before);
    }

    #[test]
    fn getting_the_acts_stops_it_being_somewhere_else() {
        let control = control_key(7);
        let creation = creation(&control);
        let object = creation.object.clone();

        let mut objects = Objects::new();
        objects.noted(object.name());
        assert_eq!(objects.resolve(object.name()), Answer::NotHere);

        objects.admit(&creation, now()).expect("taken");
        assert!(
            matches!(objects.resolve(object.name()), Answer::Here(_)),
            "held here now, so no longer somewhere else"
        );
    }

    /// A node, and somebody with a chain of their own to claim it.
    fn a_node_and_a_claimant() -> (Objects, Did, ed25519::SigningKey, Did, ed25519::SigningKey) {
        let mut objects = Objects::new();
        let its_key = control_key(120);
        let announced =
            crate::announce::announce(crate::genesis::Which::Development, now(), &its_key);
        objects.admit(&announced.operation, now()).expect("taken");

        let theirs = control_key(121);
        let creation = creation(&theirs);
        let claimant = creation.object.clone();
        objects.admit(&creation, now()).expect("taken");

        (objects, announced.node, its_key, claimant, theirs)
    }

    /// What the two of them put their name to.
    fn a_claim(
        node: &Did,
        its_key: &ed25519::SigningKey,
        claimant: &Did,
        theirs: &ed25519::SigningKey,
        head: &Name,
    ) -> Operation {
        let challenge = crate::bind::Challenge {
            node: node.clone(),
            nonce: [7; 32],
            until: now().plus(Epochs(1)).expect("no overflow"),
        };
        let approval = crate::bind::Approval {
            claimant: claimant.clone(),
            signature: theirs.sign(&challenge.to_bytes()).bytes(),
        };
        crate::bind::bind(
            node,
            head,
            &crate::bind::Claiming {
                challenge: &challenge,
                approval: &approval,
                issued: now(),
            },
            its_key,
        )
    }

    #[test]
    fn a_node_and_whoever_contributed_it_say_so_together() {
        // **Both sides, or it binds nothing.** Approving a challenge proves somebody holds their own
        // key and not that they hold the node; the node saying it alone proves nobody agreed.
        let (mut objects, node, its_key, claimant, theirs) = a_node_and_a_claimant();
        let head = objects.head(node.name()).expect("a head").clone();

        assert_eq!(objects.claimed_by(node.name()), None, "a machine, so far");
        let claim = a_claim(&node, &its_key, &claimant, &theirs, &head);
        assert_eq!(objects.admit(&claim, now()), Ok(Admitted::Extended));
        assert_eq!(objects.claimed_by(node.name()), Some(claimant));
    }

    #[test]
    fn a_node_cannot_claim_to_have_been_contributed_by_somebody_who_did_not_agree() {
        // The whole of what the two-sided rule refuses: the node's word about a stranger.
        let (mut objects, node, its_key, claimant, _theirs) = a_node_and_a_claimant();
        let head = objects.head(node.name()).expect("a head").clone();

        let somebody_else = control_key(122);
        let claim = a_claim(&node, &its_key, &claimant, &somebody_else, &head);
        assert_eq!(objects.admit(&claim, now()), Err(Refused::NotAuthorised));
        assert_eq!(objects.claimed_by(node.name()), None);
    }

    #[test]
    fn an_approval_cannot_be_lifted_onto_a_different_machine() {
        // The challenge names the node it is for, so what somebody approved for one machine approves
        // nothing about another — which is what stops a code in a screenshot binding anybody's.
        let (mut objects, node, its_key, claimant, theirs) = a_node_and_a_claimant();
        let other_key = control_key(123);
        let other =
            crate::announce::announce(crate::genesis::Which::Development, now(), &other_key);
        objects.admit(&other.operation, now()).expect("taken");
        let head = objects.head(other.node.name()).expect("a head").clone();

        // Approved for the first node, offered as a claim on the second.
        let challenge = crate::bind::Challenge {
            node: node.clone(),
            nonce: [7; 32],
            until: now().plus(Epochs(1)).expect("no overflow"),
        };
        let approval = crate::bind::Approval {
            claimant: claimant.clone(),
            signature: theirs.sign(&challenge.to_bytes()).bytes(),
        };
        let lifted = crate::bind::bind(
            &other.node,
            &head,
            &crate::bind::Claiming {
                challenge: &challenge,
                approval: &approval,
                issued: now(),
            },
            &other_key,
        );

        assert_eq!(objects.admit(&lifted, now()), Err(Refused::NotAuthorised));
        let _ = its_key;
    }

    #[test]
    fn an_approval_that_has_run_out_binds_nothing() {
        // One that ends up in a screenshot, a support bundle or the node's own log must not bind
        // somebody's machine a year later.
        let (mut objects, node, its_key, claimant, theirs) = a_node_and_a_claimant();
        let head = objects.head(node.name()).expect("a head").clone();

        let challenge = crate::bind::Challenge {
            node: node.clone(),
            nonce: [7; 32],
            until: Epoch::new(now().number() - 1),
        };
        let approval = crate::bind::Approval {
            claimant: claimant.clone(),
            signature: theirs.sign(&challenge.to_bytes()).bytes(),
        };
        let stale = crate::bind::bind(
            &node,
            &head,
            &crate::bind::Claiming {
                challenge: &challenge,
                approval: &approval,
                issued: now(),
            },
            &its_key,
        );

        assert_eq!(objects.admit(&stale, now()), Err(Refused::NotAuthorised));
    }

    #[test]
    fn a_node_lets_go_of_whoever_contributed_it_on_its_own() {
        // Whoever claimed it agreed to be credited for what it served, and letting go of that costs
        // them nothing they can be held to — so nobody has to be asked.
        let (mut objects, node, its_key, claimant, theirs) = a_node_and_a_claimant();
        let head = objects.head(node.name()).expect("a head").clone();
        let claim = a_claim(&node, &its_key, &claimant, &theirs, &head);
        objects.admit(&claim, now()).expect("taken");

        let head = objects.head(node.name()).expect("a head").clone();
        let letting = crate::bind::unbind(&node, &head, now(), &its_key);
        assert_eq!(objects.admit(&letting, now()), Ok(Admitted::Extended));
        assert_eq!(objects.claimed_by(node.name()), None);
    }

    #[test]
    fn saying_again_what_it_runs_does_not_forget_who_contributed_it() {
        // What a node offers changes over its life; who put it there does not change with it.
        let (mut objects, node, its_key, claimant, theirs) = a_node_and_a_claimant();
        let head = objects.head(node.name()).expect("a head").clone();
        objects
            .admit(&a_claim(&node, &its_key, &claimant, &theirs, &head), now())
            .expect("taken");

        let head = objects.head(node.name()).expect("a head").clone();
        let saying = crate::announce::offering(
            &node,
            &head,
            &BTreeSet::from([crate::capability::Capability::Interface]),
            crate::announce::Speaking {
                version: 2,
                reachable: &BTreeSet::new(),
                issued: now(),
                key: &its_key,
            },
        );
        objects.admit(&saying, now()).expect("taken");

        assert_eq!(objects.claimed_by(node.name()), Some(claimant));
    }
}
