//! Everything a node does, with nothing drawing it.
//!
//! **This is the whole of what a node is**, and the two ways of running one — the window and the
//! terminal — are two faces over it with no logic of their own. That is not tidiness: the moment a
//! face can do something the other cannot, one of them starts falling behind and nobody notices
//! until somebody tries. Anything a person can do to a node is a method here, and a face that
//! wants to offer it calls this.
//!
//! The same goes for the interface a client or a portal talks to over the network: it is one more
//! caller of this, not a second implementation of it.
//!
//! # Nothing here reads a clock
//!
//! Every answer that depends on when it is asked takes the epoch as an argument. Whatever drives
//! the node supplies it, which makes every rule about time testable at any moment in the network's
//! life rather than only at the one this machine happens to be having.
//!
//! # A node is a directory with a key in it
//!
//! [`directory`] is what makes that *one* node: while a process is the node in a directory, no
//! second one can be, because both would write one record and close one set of epochs, and one
//! identity with two histories in it is what a node caught contradicting itself looks like.
//! [`record`] is what it keeps there, so that stopping is not forgetting.
//!
//! [`identity`] is that key, and it stays where it was: the same directory is the same node
//! whoever starts it and however many times. A key made afresh on every start would be a different
//! node every time — a new identity in the mesh, and anything published about it stale without
//! anybody being told.
//!
//! # The two faces are held to the same list
//!
//! [`facade`] is what a person can do to a node and which of the two ways of running one can do
//! it. It lives here because neither face may see the other — a terminal node links no webview and
//! a windowed one links no terminal renderer — so a check comparing them has nowhere else to be.
//!
//! # Where a node finds its first neighbours
//!
//! [`zone`] is what the DNS zone publishes and how much of it is worth believing. The lookup
//! itself is not here: what a node does with an answer is a rule, and where the answer came from
//! is somebody else's business.
//!
//! # Every answer says what it was computed against
//!
//! An answer is not a value: it is a value **and the epoch and root it was true at**. Without
//! that, two questions asked in a row are not comparable, a long listing is not even consistent
//! with itself, and nothing anybody was told can be checked afterwards.

pub mod directory;
pub mod facade;
pub mod found;
pub mod identity;
pub mod peer;
pub mod record;
pub mod zone;

// What a face needs in order to call any of this, re-exported so that a face never reaches past
// the core to get it. A door somebody has to go around is a door.
pub use almena_store::chain::{Admitted, Answer, Reason, State};
pub use almena_store::genesis::{Opening, Which};
pub use almena_store::parameter::Parameter;
pub use almena_store::share::{COPIES_OF_A_STATUS_LIST, COPIES_OF_HISTORY};
pub use almena_suite::ed25519::SigningKey;
pub use almena_time::{Epoch, Epochs};

use std::collections::BTreeMap;

use almena_format::identifier::{Did, Name};
use almena_format::operation::Operation;
use almena_store::announce;
use almena_store::chain::{Objects, Refused};
use almena_store::genesis;
use almena_store::log::Log;
use almena_store::root::{Root, Roots};
use almena_store::tree::Path;
use almena_suite::digest::Digest;
use almena_suite::ed25519;

/// An answer, and what it was true at.
///
/// Everything this node says comes wrapped in one. Two consultations that do not say what they
/// were computed against cannot be compared, and a listing that does not say it is not consistent
/// even with itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered<T> {
    /// The epoch it was answered in.
    pub epoch: Epoch,
    /// The root over everything this node had written down at that moment.
    pub root: Digest,
    /// What was asked for.
    pub answer: T,
}

/// Why a node could not open a network.
///
/// Named apart from [`NotTaken`] because the two are different questions with the same shape: one
/// is about an act somebody handed over, this is about a node's own first moments.
pub type NotOpened = genesis::Refused;

/// Why a node would not take an operation.
///
/// It is [`Refused`] and nothing else: **whether an act may be written down is decided by the act
/// itself**, never by who delivered it or over what connection. There is no session to be
/// unauthorised in, because the signature is the authorisation.
pub type NotTaken = Refused;

/// Which epochs a node still owes a root for.
///
/// **A node publishes one every epoch, whether anything happened or not.** So a node that was off
/// for three epochs owes three roots when it comes back, not one: if it published only the epoch
/// it woke up in, the gap it left would mean either *nothing happened* or *I was not here*, and a
/// gap that means both means neither.
///
/// It holds no clock. Whatever is running the node says what time it is, and this says what that
/// implies — which is what makes catching up testable at any point in a network's life rather than
/// only at the one this machine is having.
#[derive(Debug, Clone, Default)]
pub struct Keeping {
    /// The last epoch closed, if any has been.
    closed: Option<Epoch>,
}

impl Keeping {
    /// Nothing closed yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every epoch that still owes a root, up to and including `now`.
    ///
    /// The first call closes only `now`: a node that has just started was not absent for the
    /// epochs before it existed, and claiming them would be publishing a history it did not
    /// observe.
    pub fn due(&mut self, now: Epoch) -> Vec<Epoch> {
        let owed = match self.closed {
            None => vec![now],
            Some(last) if now.number() > last.number() => ((last.number() + 1)..=now.number())
                .map(Epoch::new)
                .collect(),
            // Already closed, or a clock that went backwards. Neither is a reason to say something
            // different about an epoch already spoken for.
            Some(_) => Vec::new(),
        };

        if let Some(latest) = owed.last() {
            self.closed = Some(*latest);
        }
        owed
    }

    /// The last epoch this node closed.
    #[must_use]
    pub fn closed(&self) -> Option<Epoch> {
        self.closed
    }
}

/// What a node was handed, and what it was promised it would be.
///
/// The two travel together because one is only worth anything against the other: acts from a
/// stranger are a claim about a network, and the promise is what makes it checkable.
#[derive(Debug, Clone, Copy)]
pub struct Joining<'a> {
    /// The acts, in the order they were handed over.
    pub acts: &'a [Vec<u8>],
    /// The network they were said to be. The name of the act that opened it.
    pub network: &'a str,
}

/// What a node says about itself when it announces again.
///
/// **Not the key.** A node signs its own announcements with its own key, so there is nothing here
/// for a caller to supply and nothing to get wrong: what it may say is what it is running, what it
/// speaks and where it can be found.
#[derive(Debug, Clone, Copy)]
pub struct Saying<'a> {
    /// What it is running.
    pub offers: &'a std::collections::BTreeSet<almena_store::capability::Capability>,
    /// Which version of the protocol it speaks.
    pub version: u64,
    /// Where it says it can be reached.
    pub reachable: &'a std::collections::BTreeSet<String>,
}

/// What an observer has to say about a day, beyond what it wrote down asking by asking.
///
/// **Only the half that names nobody.** What was seen of other nodes is not passed in any more: it
/// is worked out from the askings this node recorded as they happened, so that the figures a
/// summary publishes and the observations its hash pins are the same list read two ways. Passing
/// the figures in separately is what let them differ, and a hash over figures commits to nothing.
///
/// What stays here is what this node went looking for **itself**, which names nobody. Which things
/// fall to which node comes from a census, and an observer behind on the record has a smaller one —
/// so a miss filed against a node would be a figure about the observer's own position wearing
/// somebody else's name.
#[derive(Debug, Clone, Copy)]
pub struct Watched {
    /// How much of what this node went looking for it found.
    pub looked: almena_store::summary::Looked,
}

/// How many days of raw observations a node keeps.
///
/// **Three, because what they are for is checking a summary that was published recently.** They
/// never enter the record and nothing else ages them out, so this is the whole of what bounds them.
/// A day whose summary is older than this is one whose observations this node no longer offers —
/// which is what `SPECS.md §5.1` means by *served by whoever made them, for as long as they keep
/// them*, said as a number instead of left to be discovered.
const DAYS_WATCHED: usize = 3;

/// How much of a record to hand over at a time.
///
/// Two numbers and both are needed. The count bounds the work; the weight bounds the message — and
/// they are not the same bound, because one act may be as large as whatever the node that took it
/// was willing to accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    /// How many acts at most.
    pub at_most: usize,
    /// How many bytes of acts at most, except that one act always fits.
    pub weighing_at_most: usize,
}

/// The acts of one object, as far as a page reaches, and where to carry on from.
///
/// **The cursor is the point.** A page with a bound and no cursor is one a caller cannot tell from
/// the whole of a chain: they fold what arrived, land on a state from earlier, and nothing anywhere
/// says so. Every other paged answer here says where it stopped, and now this one does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composing {
    /// The acts, oldest first, in the bytes their authors signed.
    pub acts: Vec<Vec<u8>>,
    /// The last act handed over, when there are more after it on this branch.
    ///
    /// [`None`] is the whole of what was owed. Asking again with this as `after` continues.
    pub more: Option<Name>,
}

/// What a node reports about itself, for whoever is drawing it.
///
/// **Defined once, here, and read by both faces.** If each face gathered its own facts, the two
/// would answer the same question differently the first time one of them was changed — which is
/// the drift the whole two-face arrangement exists to prevent, arriving through the door nobody
/// watches.
///
/// Every field is optional and **not one of them is ever a default**. A node that has not opened
/// or joined a network has no network, and that is *nobody has looked* rather than *there is none*:
/// an empty string here would be a fact nobody established.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    /// The network this node is on, if it is on one.
    pub network: Option<String>,
    /// The key this node is, if it has one.
    pub identity: Option<String>,
    /// How many acts it has written down.
    pub written: Option<u64>,
    /// The root over them.
    pub root: Option<String>,
    /// What it answers to on the mesh, worked out from its own key.
    ///
    /// **The one thing a node knows that has to go into DNS.** Everything else the zone carries is
    /// the operator's — a hostname, an address, a port — but this is the node's, and it is what
    /// turns a record saying *where to call* into one that also says *who answers*.
    pub peer: Option<String>,
}

/// What came of trying to write down who contributed a node.
///
/// Three outcomes and not two, because a typo and a claim that does not check out call for opposite
/// things: one is somebody to ask to paste it again, and the other is somebody who never agreed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claimed {
    /// It is in the record, and both of them said it.
    Written,
    /// What was handed over is not a challenge and an approval of it.
    NotAClaim,
    /// It read, and it does not bind: not signed by the key that claimant's chain authorises, or
    /// the challenge had stopped being good by the time it came back.
    NotTheirs,
}

/// Why a node did not keep a status list's bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotKept {
    /// No list in the record names those bytes as its current version.
    ///
    /// Which is also what an **old** version comes back as: the record names one version per list,
    /// and anything else is bytes somebody wanted stored.
    NotNamed,
    /// The window it covers has passed, so nothing it says is about a credential still alive.
    WindowPast,
}

/// What one node holds, and everything it can do.
///
/// It prints without its key, because a signing key has no `Debug` and should not get one: a
/// secret that can be formatted is a secret that ends up in a log somebody forgot they were
/// writing.
pub struct Node {
    /// What network this is: the hash of the act that opened it.
    network: Name,
    /// Almena Government, which is that same hash as an identity.
    government: Did,
    /// What this node itself is called: the hash of the act that introduced it.
    ///
    /// Not the network's name and not Almena Government's. Those are one value shared by everybody
    /// here, and a root stamped with a shared name says nothing about who published it — which
    /// matters most exactly where roots are compared, because two nodes are *supposed* to have
    /// different trees and only the name tells that apart from one node saying two things.
    did: Did,
    /// The record of what has been accepted, in order.
    log: Log,
    /// The chain of every object, and what each one says right now.
    objects: Objects,
    /// What this node has said about its own tree, epoch by epoch.
    roots: Roots,
    /// The key this node signs its roots with.
    key: ed25519::SigningKey,
    /// Where this node has actually reached everybody it has reached.
    ///
    /// **This node's own observation, and it never goes into the record.** Where a node says it is,
    /// is its own word in a place everybody holds; where this one found it is one node's experience,
    /// and putting the second into the first would make somebody's experience everybody's truth.
    /// **Bounded**, because it is the one thing here that grows without anybody signing anything:
    /// a connection is all it takes to be remembered and a key costs nothing to make. What it drops
    /// is what has gone longest unseen, which is what this is least use for.
    found_at: crate::found::Found,
    /// What this node saw of the others, asking by asking, by the day it saw it in.
    ///
    /// **What a summary is really drawn from.** The record carries a day's figures and the hash of
    /// what they came from; this is what they came from, and it never goes into the record — fifty
    /// nodes watching each other would make the record almost entirely telemetry. It is served to
    /// whoever wants to check the hash, for as long as this node keeps it.
    watching: BTreeMap<almena_time::Day, almena_store::watching::Watching>,
    /// The instant this network's epoch zero began, in seconds since the Unix epoch.
    ///
    /// **The only wall-clock reading this platform ever writes down**, fixed by the act that opened
    /// the network and carried here so that nothing else has to read one. A face that worked it out
    /// for itself would be a face deciding what time it is, and a node that came back to a record
    /// could not work it out at all.
    began: u64,
    /// The post it is holding for other people, when it runs a mailbox at all.
    ///
    /// **Outside the record, and that is the point.** Nothing here is replicated, nothing here is
    /// signed by this node, and nothing here survives it — which is what makes a mediator a service
    /// somebody chooses rather than a place where a person's correspondence becomes everybody's.
    /// A node holding post is holding it the way a locker holds a parcel: it knows whose and it
    /// knows how big, and moving to another mediator costs the sender an address and nobody a
    /// history (`SPECS.md §6.2`).
    post: almena_mailbox::mediator::Mediator,
    /// The status lists this node is serving, by the hash of the version it holds.
    ///
    /// **Opaque bytes, addressed by hash, and never understood** (`SPECS.md §10.2`, `§4.8`). A node
    /// replicating one does not read a bitstring: it holds what it was handed and answers whether
    /// it matches the version the record names. That is what makes *any source will do* true, and
    /// it is what keeps a format change out of every node in the network.
    ///
    /// Outside the record, like the post, and for a plainer reason: `SPECS.md §10.2` keeps no
    /// history of contents. Only the version in force is held, and a list whose window has passed
    /// is dropped whole — which needs no operation, because the credentials it covered carry their
    /// expiry signed inside them.
    lists: BTreeMap<Vec<u8>, Vec<u8>>,
    /// Where what it accepts is kept, when it is kept anywhere.
    ///
    /// [`None`] is a node that will not survive its own process. It is a real state and not a
    /// mistake — a test builds one, and so does a machine with nowhere to write — but it is never
    /// what a node run by somebody is.
    record: Option<record::Record>,
}

impl core::fmt::Debug for Node {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Node")
            .field("network", &self.network)
            .field("written", &self.log.len())
            .finish_non_exhaustive()
    }
}

impl Node {
    /// Open a network, becoming its first node.
    ///
    /// `began` is the instant epoch zero begins, in seconds since the Unix epoch — the one wall
    /// clock reading this platform ever writes down, written once so that everybody afterwards
    /// counts hours from the same place rather than from their own clock.
    ///
    /// `seeds` is what the zone published when this node looked for somebody to join. **A node
    /// opens a network only when there is nobody**, which is the one defence against creating a
    /// second production network by carelessness — one nobody can tell from the first, because
    /// both would say exactly the same word about themselves.
    ///
    /// **Production is opened once**, so before it is, the format this build writes is held to the
    /// checklist of [`almena_frozen`]: a record is append-only, and whatever is missing on the day
    /// a network opens is missing for as long as that network exists. Development is not asked,
    /// because development is re-opened whenever the format moves — which is the whole of why the
    /// two are different networks rather than one with a flag.
    ///
    /// # Errors
    ///
    /// [`genesis::Refused`], saying whether somebody else is already here, this node is, or the
    /// format is not one a network may be opened on for good.
    pub fn open(
        opening: &Opening,
        seeds: &[String],
        government: &ed25519::SigningKey,
        key: ed25519::SigningKey,
    ) -> Result<Self, genesis::Refused> {
        Self::opening(opening, seeds, government, key, None)
    }

    /// Open a network in `directory`, and keep it there.
    ///
    /// The difference from [`Node::open`] is the whole of what makes a node last: what it accepts
    /// is written down as it accepts it, so the same directory comes back as the same node on the
    /// same network rather than as a stranger who has opened a second one.
    ///
    /// # Errors
    ///
    /// [`genesis::Refused`], and [`genesis::Refused::TheRecordWouldNotStart`] when the directory
    /// will not hold a record — which is a refusal to come up rather than a node that runs and
    /// forgets.
    pub fn open_in(
        directory: &std::path::Path,
        opening: &Opening,
        seeds: &[String],
        government: &ed25519::SigningKey,
        key: ed25519::SigningKey,
    ) -> Result<Self, genesis::Refused> {
        let record = record::Record::open(directory)
            .map_err(|_| genesis::Refused::TheRecordWouldNotStart)?;
        Self::opening(opening, seeds, government, key, Some(record))
    }

    /// Open a network, keeping it or not.
    fn opening(
        opening: &Opening,
        seeds: &[String],
        government: &ed25519::SigningKey,
        key: ed25519::SigningKey,
        record: Option<record::Record>,
    ) -> Result<Self, genesis::Refused> {
        // **Asked before anything is built, and only of production** (`SPECS.md §4.3`, `§18`). The
        // one moment this question is worth putting is the moment before a record exists: after it,
        // there is nothing to do with the answer.
        if opening.which == Which::Production {
            let wanting: Vec<String> = almena_frozen::wanting()
                .into_iter()
                .map(|item| item.called)
                .collect();
            if !wanting.is_empty() {
                return Err(genesis::Refused::TheFormatIsNotFrozen(wanting));
            }
        }
        let opened = genesis::open(opening, seeds, false, government)?;
        let announced = announce::announce(opening.which, opening.beginning, &key);

        let mut node = Self {
            network: opened.network.clone(),
            government: opened.government.clone(),
            did: announced.node.clone(),
            log: Log::new(),
            objects: Objects::new(),
            roots: Roots::new(),
            key,
            found_at: crate::found::Found::new(),
            watching: BTreeMap::new(),
            began: opening.began,
            post: almena_mailbox::mediator::Mediator::new(),
            lists: BTreeMap::new(),
            record,
        };

        // Neither can fail: both acts were just built to be exactly what is accepted here. If one
        // ever did, the network would be open with a hole in its record, which is a state that
        // must not be reachable — so neither is silently ignored, and neither is reported as
        // something else that would send somebody looking in the wrong place.
        //
        // The genesis goes first because it is the record's first entry, and the node introduces
        // itself second: until it does, nothing it publishes can be attributed to it.
        for act in [&opened.operation, &announced.operation] {
            match node.submit(act, opening.beginning) {
                Ok(_) => {}
                Err(_) => return Err(genesis::Refused::TheRecordWouldNotStart),
            }
        }
        node.close(opening.beginning);
        Ok(node)
    }

    /// What this network is called.
    #[must_use]
    pub fn network(&self) -> &Name {
        &self.network
    }

    /// The trust anchor everything on this network is checked against.
    #[must_use]
    pub fn government(&self) -> &Did {
        &self.government
    }

    /// How many acts this node has written down.
    #[must_use]
    pub fn written(&self) -> usize {
        self.log.len()
    }

    /// Hand over a signed act.
    ///
    /// **The signature is the authorisation.** There is nothing to log in to and no permission to
    /// ask for: this node checks that the act is well formed and that it follows what it says it
    /// follows, and if it does, it is written down. Whoever delivered it, and over what, does not
    /// enter into it.
    ///
    /// # Errors
    ///
    /// [`NotTaken`], naming which rule the act broke.
    pub fn submit(
        &mut self,
        operation: &Operation,
        now: Epoch,
    ) -> Result<Answered<Admitted>, NotTaken> {
        let admitted = self.objects.admit(operation, now)?;

        // **An act this node already held is written down once and no more.** Acts arrive twice by
        // design, and a second copy in the record would be a second position in the tree, a second
        // line in the log, and — because the record is what a restart replays — a duplicate that
        // came back every morning.
        //
        // **Unless there is no line for it**, which is the one case where *already here* is not
        // true of the whole node. Admission moves memory and the writing comes after it, so a disk
        // that refused left the act applied and unwritten — and a plain short-circuit here would
        // make the obvious repair, handing it over again, the one thing that could not fix it. So
        // the question asked is whether **this node has a line saying that act happened**, not
        // whether the chain has seen it.
        if admitted == Admitted::AlreadyHere && self.log.knows(&operation.called()) {
            return Ok(self.stamped(admitted, now));
        }

        // **A fork is settled by replaying, from this node's own record, the branch the resolution
        // named** (`SPECS.md §4.9`). Nothing about it is believed: every act on that branch is
        // validated again in order, so what authorised the resolution is the state its own branch
        // produced. Done before the act is written down, because a resolution this node cannot
        // carry out is one it has not taken.
        if admitted == Admitted::Resolves {
            self.settling(operation, now)?;
        }

        // Written down before it is answered for, and this is the order that matters: an act that
        // reached memory and not the disk would be one this node said it had taken and would not
        // have the morning after.
        if let Some(record) = self.record.as_mut()
            && record.wrote(&operation.to_bytes()).is_err()
        {
            return Err(Refused::NotKept);
        }

        let entry = self.log.append(operation, subject_of(operation));
        if let Some(keeping) = self.record.as_mut()
            && keeping.noted(&entry).is_err()
        {
            return Err(Refused::NotKept);
        }
        Ok(self.stamped(admitted, now))
    }

    /// Carry out a resolution: gather the branch it named and replay it.
    ///
    /// The branch is walked backwards from the act the resolution chains from, following each act's
    /// own `prev` — which is the same walk anybody reading this object would do, and needs nothing
    /// of this node's own ordering.
    ///
    /// **A node that does not hold one of those acts cannot do this**, and says so. Knowing an act
    /// happened is universal and holding its bytes is not (`SPECS.md §4.6`), so this is one node's
    /// limit rather than a fact about the object: another node holding the branch settles it, and
    /// this one learns the outcome when the acts reach it.
    fn settling(&mut self, resolution: &Operation, now: Epoch) -> Result<(), NotTaken> {
        let whose = resolution.object.name().clone();
        let mut along = vec![resolution.clone()];
        let mut following = resolution.previous.clone();

        while let Some(name) = following {
            let bytes = self.log.act(&name).ok_or(Refused::NoSuchPredecessor)?;
            let act = operation_from(bytes).ok_or(Refused::Malformed)?;
            following = act.previous.clone();
            along.push(act);
            // The chain is bounded by what this node holds, and every act on it is one it took —
            // so this ends at the creation, which names itself and follows nothing.
        }
        along.reverse();
        self.objects.resolved(&whose, &along, now)?;
        Ok(())
    }

    /// What this node wrote down at a position in its own record.
    ///
    /// **Its own position, meaning nothing anywhere else** — every node has its own record and its
    /// own order. What it is for is walking this node's record forward to find something to go and
    /// ask somebody else about.
    #[must_use]
    pub fn at_sequence(&self, sequence: u64) -> Option<Name> {
        self.log
            .at_sequence(sequence)
            .map(|entry| entry.hash.clone())
    }

    /// Take note that an act happened without holding what it said.
    ///
    /// **This is what a shared-out history looks like from the inside.** The line saying an act
    /// happened is universal — it is what lets anybody check a chain's shape, find what was said
    /// about somebody, and prove where something sits — and what the act *said* is carried by the
    /// nodes it was dealt to. A node that could not tell the two apart would have to keep
    /// everything for ever.
    ///
    /// The entry goes into the tree exactly as one whose act is held: **an entry is never skipped**,
    /// because the tree over them is what this node has put its name to.
    pub fn note(&mut self, entry: &almena_format::entry::Entry) {
        let written = self.log.noted(entry);
        self.objects.noted(entry.object.name());
        if let Some(keeping) = self.record.as_mut() {
            // Whether it reached the disk is worth knowing and not worth stopping for: an entry
            // that did not is one this node will be told about again, and refusing to hold it in
            // the meantime would help nobody.
            let _ = keeping.noted(&written);
        }
    }

    /// Let go of everything the share-out no longer deals to this node.
    ///
    /// **What replaces every node keeping everything.** The record only grows, and a network whose
    /// only plan was that has no plan; what a node keeps is the share it was dealt, which it does
    /// not choose and which moves every month. It keeps the line saying each act happened either
    /// way — that is universal, and the tree over those lines is what it has signed.
    ///
    /// It never lets go of its own chain, nor of what the share-out is itself drawn from: the act
    /// that opened the network, and every act that says who a node is. A node that let those go
    /// could no longer work out what it was supposed to keep — it would have let go of the answer to
    /// the question it needed the answer to.
    ///
    /// Returns how many it let go of.
    pub fn let_go_of_what_is_not_mine(&mut self, now: Epoch) -> usize {
        let (network, census) = self.share_out(now);
        let drawn = almena_store::share::Drawn::at(&network, now, &census);
        let mine = self.did.name().clone();

        let letting: Vec<Name> = self
            .log
            .everything_held()
            .into_iter()
            .filter(|(_, object)| *object != mine && !self.objects.everybody_keeps(object))
            .filter(|(thing, _)| !drawn.falls_to(thing, &mine, COPIES_OF_HISTORY))
            .map(|(thing, _)| thing)
            .collect();

        letting
            .iter()
            .filter(|thing| self.log.let_go(thing))
            .count()
    }

    /// The acts this node knows happened, has not got, and is supposed to hold.
    ///
    /// **The share-out moves every month, so what falls here is not what fell here.** A node that
    /// only ever let go would be quietly short of everything that moved *towards* it — nothing
    /// would be wrong with any single answer it gave, and only somebody asking would find out.
    ///
    /// It is the same three tests letting go uses, read the other way round: this node's own chain
    /// and what everybody keeps are always owed, and everything else is owed exactly where the
    /// share-out deals it here. Asking for what is *not* owed would be worse than not asking —
    /// letting go runs on the same tick, so the thing would be fetched and dropped for ever.
    #[must_use]
    pub fn owed(&self, now: Epoch) -> Vec<Name> {
        let (network, census) = self.share_out(now);
        let drawn = almena_store::share::Drawn::at(&network, now, &census);
        let mine = self.did.name().clone();

        self.log
            .missing_on()
            .into_iter()
            .filter(|(thing, object)| {
                *object == mine
                    || self.objects.everybody_keeps(object)
                    || drawn.falls_to(thing, &mine, COPIES_OF_HISTORY)
            })
            .map(|(thing, _)| thing)
            .collect()
    }

    /// The acts this node knows happened and has not got.
    ///
    /// **What it has to go and ask somebody for.** An object it can only say is held elsewhere is
    /// one nobody can use through this node, and the way that stops being a dead end is that the
    /// node asks whoever the share-out dealt it to.
    #[must_use]
    pub fn not_got(&self) -> Vec<Name> {
        self.log.missing()
    }

    /// Take in what an act said, at the position its entry already holds.
    ///
    /// **For an act this node knew happened and had not got.** It goes through the same admission
    /// as anything a stranger hands over — the entry having been there says an act happened, not
    /// that this is it — and its place in the tree does not move, because that tree is signed.
    ///
    /// # Errors
    ///
    /// [`Refused`], naming which rule it broke, exactly as when an act arrives any other way.
    pub fn fill_in(&mut self, operation: &Operation, now: Epoch) -> Result<(), Refused> {
        if self.log.holds(&operation.called()) {
            return Ok(());
        }
        // An act this node has no line for is not something to fill in: a gap is a place its own
        // record says something belongs, and this is not one.
        if !self.log.knows(&operation.called()) {
            return self.submit(operation, now).map(|_| ());
        }
        // The name covers everything but the signatures, so matching it means the content matches
        // and only the signature is still open. Whoever handed it over could otherwise sign with a
        // key they made that morning and have this node serve it under a name it vouches for.
        if !self.objects.vouches_for(operation) {
            return Err(Refused::NotAuthorised);
        }
        if let Some(record) = self.record.as_mut()
            && record.wrote(&operation.to_bytes()).is_err()
        {
            return Err(Refused::NotKept);
        }
        self.log.keep(operation);
        Ok(())
    }

    /// Whether this node holds what an act said, as against knowing that it happened.
    #[must_use]
    pub fn holds(&self, act: &Name) -> bool {
        self.log.holds(act)
    }

    /// What this node says about an object.
    #[must_use]
    pub fn resolve(&self, name: &Name, now: Epoch) -> Answered<Answer> {
        self.stamped(self.objects.resolve(name), now)
    }

    /// One act, in the bytes it arrived in.
    ///
    /// Never re-encoded and never signed by this node. What comes back is the author's own act,
    /// which is the only thing worth having: a node is a messenger, and an answer somebody has to
    /// take this node's word for is not an answer.
    #[must_use]
    pub fn act(&self, hash: &Name, now: Epoch) -> Answered<Option<Vec<u8>>> {
        self.stamped(self.log.act(hash).map(<[u8]>::to_vec), now)
    }

    /// Everything written down about somebody other than its author.
    ///
    /// **[`None`] is not the same as an empty list, and this returns [`None`] today.** No act
    /// carries a subject yet, because the three that will — a certification, a vote, a
    /// contradiction — are not built. An empty list here would read as *nobody has certified this
    /// entity*, which is a claim, and a false one: nobody has been able to.
    ///
    /// *Nobody has looked yet* and *somebody looked and there is nothing* are different facts, and
    /// a caller that cannot tell them apart will publish the wrong one.
    #[must_use]
    pub fn about(&self, subject: &Did, now: Epoch) -> Answered<Option<Vec<Name>>> {
        self.stamped(self.about_hashes(subject), now)
    }

    /// Where an act sits in this node's tree, and the path that proves it.
    ///
    /// The position is **this node's** and another node will give a different one for the same
    /// act. Both are right, and each proves its own against its own root — which is exactly what
    /// makes counting independent trees mean something.
    #[must_use]
    pub fn inclusion(&self, hash: &Name, now: Epoch) -> Answered<Option<(u64, Path)>> {
        self.stamped(self.log.inclusion(hash), now)
    }

    /// Close an epoch: say what the tree looked like, and sign it.
    ///
    /// **Called every epoch, whether anything happened or not.** A node that only published when
    /// there was something to publish would leave gaps meaning either *nothing happened* or *I was
    /// not here*, and a gap that means both means neither.
    pub fn close(&mut self, epoch: Epoch) -> Root {
        self.close_over(epoch, self.log.len() as u64)
    }

    /// Close every epoch owed, saying about the ones it missed only what it can know.
    ///
    /// **A root is over the tree as it was when that epoch ended, and a node only knows the tree at
    /// the moments it looks.** So an epoch it is closing late gets the tree it last put its name
    /// to: whatever arrived since, it observed *now*, and putting it in an earlier epoch's root
    /// would make an act appear to have been written down before it existed — which is precisely
    /// what a tree is for stopping.
    ///
    /// Only the epoch it is in gets the tree it has. Being under-attributed costs an act nothing;
    /// it turns up in the next root. Being over-attributed costs everybody the property.
    ///
    /// `owed` is expected in order, oldest first, which is how they are counted out.
    pub fn close_owed(&mut self, owed: &[Epoch]) -> usize {
        let carried = self
            .roots
            .last()
            .and_then(|last| self.roots.at(last))
            .map_or(0, |root| root.size);

        for (which, epoch) in owed.iter().enumerate() {
            let size = if which + 1 == owed.len() {
                self.log.len() as u64
            } else {
                carried
            };
            self.close_over(*epoch, size);
        }
        owed.len()
    }

    /// Close an epoch over a tree of exactly that size.
    fn close_over(&mut self, epoch: Epoch, size: u64) -> Root {
        let over = self
            .log
            .root_at(size)
            .unwrap_or_else(|| unreachable!("a size this record reached"));
        let root = Root {
            network: self.network.clone(),
            node: self.did.clone(),
            epoch,
            size,
            root: over,
        };
        // It only refuses if this node were about to say something different about an epoch it has
        // already closed, and a node does not contradict itself. Saying the same thing twice is
        // fine and is what happens when an epoch is closed again with nothing new in it.
        let fresh = self.roots.publish(root).is_ok();

        // **What comes back is what this node stands by**, not what it has just worked out. An
        // epoch closed a second time after more arrived would otherwise hand back a root the node
        // will not honour — and anybody who published it would be holding out two roots for one
        // epoch in this node's name, which is the one thing that is provable against it.
        let standing = self
            .roots
            .at(epoch)
            .cloned()
            .unwrap_or_else(|| unreachable!("an epoch just closed has a root"));

        // Kept because it cannot be worked out again: a root says where the tree stood when that
        // epoch closed, and the finished record no longer shows that. A node that recomputed one
        // and got a different answer would be signing two roots for one epoch, against itself.
        if fresh && let Some(record) = self.record.as_mut() {
            let _ = record.published(&standing);
        }
        standing
    }

    /// Where this node wrote an act down, proved against the root it signed for `epoch`.
    ///
    /// **A proof nobody can check is not a proof.** What comes back is the position, the path, and
    /// the node's own signed root — which carries the size the path has to be counted against and
    /// the signature that makes it this node's word rather than anybody's bytes.
    ///
    /// [`None`] when this node never closed that epoch, or the act was not yet written down when it
    /// did. Both are *there is no such proof* rather than a failure: an act that arrived after an
    /// epoch closed is genuinely not in that epoch's tree, and saying otherwise would be inventing
    /// a proof.
    #[must_use]
    pub fn inclusion_in(
        &self,
        hash: &Name,
        epoch: Epoch,
        now: Epoch,
    ) -> Answered<Option<(u64, Path, almena_store::root::Published)>> {
        let found = self.roots.at(epoch).and_then(|root| {
            let (at, path) = self.log.inclusion_at(hash, root.size)?;
            Some((at, path, root.publish(&self.key)))
        });
        self.stamped(found, now)
    }

    /// The acts that advance an object's own chain, in the order this node wrote them.
    ///
    /// **What lets anybody check a summary without asking anybody.** A checkpoint says which act
    /// last set each part of an object; whether it left something out is answered by looking at
    /// that object's acts, and every node holds them.
    #[must_use]
    pub fn chain_of(&self, object: &Did, now: Epoch) -> Answered<Vec<almena_format::entry::Entry>> {
        let chain = self.log.chain_of(object).into_iter().cloned().collect();
        self.stamped(chain, now)
    }

    /// The post this node is holding, if holding post is something it does.
    ///
    /// **The gate is the node's own announcement and nothing else** — the same act the rest of the
    /// network reads to decide whether to send anybody here. A second switch, in a configuration
    /// file or a flag, would be a way for a node to say one thing and do another; there is only
    /// the one, and it is public.
    ///
    /// [`None`] is not an error and not a refusal to say. It is a node that does not run a mailbox,
    /// which most nodes do not, and the question simply is not one it answers.
    pub fn post(&mut self) -> Option<&mut almena_mailbox::mediator::Mediator> {
        self.carries_post().then_some(&mut self.post)
    }

    /// Whether this node announced that it holds post.
    #[must_use]
    pub fn carries_post(&self) -> bool {
        matches!(
            self.objects.resolve(self.did.name()),
            Answer::Here(State::Node { ref offers, .. })
                if offers.contains(&almena_store::capability::Capability::Mailbox)
        )
    }

    /// The keys the account itself says operate it, as of that moment.
    ///
    /// **What a mailbox checks a collection against**, and the reason there is no second register
    /// of who may collect: taking a device off an account takes it off the mailbox in the same act,
    /// with nothing to remember to do afterwards. It reads `come_due`, so a removal the control key
    /// asked for counts from the epoch it lands and not from the epoch it was written.
    ///
    /// [`None`] where this node cannot say what the account is — it never saw it, it is forked, or
    /// its history has an act this build cannot read. A mailbox that guessed *no devices* there
    /// would turn a node's own ignorance into a lockout.
    #[must_use]
    pub fn devices_on(&self, whose: &Did, now: Epoch) -> Option<Vec<Vec<u8>>> {
        match self.objects.resolve(whose.name()) {
            Answer::Here(State::Holder(holder)) => {
                let settled = holder.come_due(now);
                // A frozen account may not act, and collecting post is acting. Saying *no* is the
                // one thing left, and taking delivery is not saying no (`SPECS.md §11.12`).
                (!settled.frozen).then(|| settled.devices.into_iter().collect())
            }
            _ => None,
        }
    }

    /// The acts somebody needs to work out what an object is now.
    ///
    /// **The last summary this node could read, and everything on that branch after it** — which is
    /// the whole point of a summary: whoever arrives takes those and stops, instead of replaying a
    /// history that grows for ever. Where there is no summary the whole chain comes back, and that
    /// is not a failure: an object that never wrote much has a handful of acts, and one that wrote
    /// a great deal owes a summary and is behind by at most the number that owing starts at.
    ///
    /// Oldest first, in the bytes their authors signed. **The state itself is not served**: a node
    /// that handed over a finished answer would be a source somebody had to believe, and what comes
    /// back instead is materials, each of which carries the signature that makes it check out.
    ///
    /// Where a chain has split there is no branch to follow, so everything that object ever wrote
    /// comes back and the two acts claiming one predecessor are there to be seen. Picking a side
    /// would be the one thing no node may do.
    #[must_use]
    pub fn state_of(
        &self,
        object: &Did,
        after: Option<&Name>,
        page: Page,
        now: Epoch,
    ) -> Answered<Option<Composing>> {
        let held = self.log.chain_of(object);
        let split = matches!(
            self.objects.resolve(object.name()),
            Answer::CannotResolve(almena_store::chain::Reason::Forked)
        );
        let walked = match (split, self.objects.head(object.name())) {
            (false, Some(head)) => almena_store::checkpoint::branch(&held, head),
            _ => held,
        };

        // The last summary and what followed it. Where the count reaches the whole chain there is
        // no summary to include, and the arithmetic lands on the creation on its own.
        //
        // **Except where the chain has split**, and then everything comes back. The count is of one
        // branch, so measuring the other from it would cut acts off the far end of a list that is
        // not in chain order at all — and whoever is looking at a split needs to see both sides.
        let since = if split {
            walked.len()
        } else {
            self.objects
                .since_summarising(object.name())
                .and_then(|since| usize::try_from(since).ok())
                .unwrap_or(walked.len())
        };
        let wanted = since.saturating_add(1).min(walked.len());
        let from = match after {
            // **Continuing where the last page stopped.** The cursor names the last act handed
            // over, so the next one starts after it — and the summary arithmetic above no longer
            // applies, because whoever is asking already holds the summary and everything up to
            // here.
            Some(cursor) => match walked.iter().position(|entry| &entry.hash == cursor) {
                Some(at) => at + 1,
                // A cursor this node cannot place on this branch. **Answered as nothing rather
                // than as the start**: handing back the first page to somebody who asked for the
                // fourth would look like an answer and be a different one.
                None => return self.stamped(None, now),
            },
            None => walked.len().saturating_sub(wanted),
        };

        let mut acts = Vec::new();
        let mut weight = 0;
        let mut more = None;
        for entry in &walked[from..] {
            let Some(act) = self.log.act(&entry.hash) else {
                continue;
            };
            // One act always fits, so that something too large for a page is still reachable rather
            // than being a hole nobody can get past.
            if !acts.is_empty()
                && (acts.len() >= page.at_most || weight + act.len() > page.weighing_at_most)
            {
                // **Where it stopped, so that stopping is something the caller is told.** Without
                // it a page is indistinguishable from the whole of it, and whoever folded the
                // answer would land on a state from earlier and have no way to know.
                more = acts
                    .last()
                    .map(|_| walked[from + acts.len() - 1].hash.clone());
                break;
            }
            weight += act.len();
            acts.push(act.to_vec());
        }
        self.stamped(Some(Composing { acts, more }), now)
    }

    /// Where an object stands on summarising itself, and the summary it would sign.
    ///
    /// **What the app warns on.** A summary does not benefit whoever writes it — it benefits
    /// whoever arrives later — so the thing that depends on goodwill when somebody else pays gets
    /// done seldom and late. Saying how far behind an object is turns it into something visible.
    #[must_use]
    pub fn standing(
        &self,
        object: &Did,
        now: Epoch,
    ) -> Answered<Option<almena_store::chain::Standing>> {
        let standing = self.objects.standing(object.name(), now);
        self.stamped(standing, now)
    }

    /// How the record is shared out right now, as anybody holding it would work it out.
    ///
    /// **The share is not this node's opinion of what it should keep.** It is a rule everybody
    /// computes the same way from what everybody has, which is what makes a node that has not got
    /// what falls to it visibly short rather than merely suspected.
    ///
    /// The census is every node the record names **that has not closed**, including ones that
    /// announced once and were never heard from again. Telling those apart needs measurement this
    /// does not have, and counting them is the safe way round: it assigns work to nodes that may
    /// not do it, and the shortfall then shows up as a shortfall rather than being hidden by
    /// shrinking what was expected.
    ///
    /// **A node that has closed is a different case and is left out** (`SPECS.md §4.1`): it said it
    /// was going, so assigning it work would be assigning work to somebody who is not there, and
    /// counting it as absent would be a figure about the same. Everything it ever said stays in the
    /// record and is still read.
    ///
    /// Asked at a moment, because *closed* is a moment: a share-out drawn for an epoch before a node
    /// closed has to be the same share-out afterwards, or the past would move under whoever is
    /// checking it.
    #[must_use]
    pub fn share_out(&self, at: Epoch) -> (Name, Vec<&Name>) {
        (self.network().clone(), self.objects.nodes_at(at).collect())
    }

    /// Whether a thing falls to this node in the share-out.
    #[must_use]
    pub fn falls_to_me(&self, thing: &Name, copies: Parameter, at: Epoch) -> bool {
        let (network, census) = self.share_out(at);
        almena_store::share::Drawn::at(&network, at, &census).falls_to(
            thing,
            self.did.name(),
            copies,
        )
    }

    /// Which nodes are expected to hold a thing.
    #[must_use]
    pub fn holders_of(&self, thing: &Name, copies: Parameter, at: Epoch) -> Answered<Vec<Did>> {
        let (network, census) = self.share_out(at);
        let holders = almena_store::share::Drawn::at(&network, at, &census)
            .holders(thing, copies)
            .into_iter()
            .map(|name| Did::new(self.which_marking(), name.clone()))
            .collect();
        self.stamped(holders, at)
    }

    /// What the whole network went looking for on a day, out of this node's own record.
    ///
    /// **The figure §5.2 asks for, and anybody with the record arrives at the same one** — it is a
    /// sum over signed acts everybody holds, not an assertion by whoever runs anything. A node that
    /// is behind on the record has fewer summaries and says so in the count of observers, which is
    /// why that count is part of the figure and not a footnote to it.
    #[must_use]
    pub fn kept(&self, day: almena_time::Day, now: Epoch) -> Answered<almena_store::summary::Kept> {
        let mut summaries = Vec::new();
        for named in self.objects.nodes() {
            let observer = Did::new(self.which_marking(), named.clone());
            for entry in self.log.chain_of(&observer) {
                if entry.kind != almena_store::kind::Kind::NODE_SUMMARY.number() {
                    continue;
                }
                if let Some(act) = self.log.act(&entry.hash).and_then(operation_from) {
                    summaries.push(act);
                }
            }
        }
        let held: Vec<&Operation> = summaries.iter().collect();
        self.stamped(almena_store::summary::kept(day, &held), now)
    }

    /// Whether this node's record proves that key signed two things that cannot both be true.
    ///
    /// **It is what the record says, not what this node has decided.** A network without permission
    /// can impose one consequence — that whoever contradicts themselves stops earning the right to
    /// write — and everything else belongs to whoever is relying on the answer. This is how they
    /// come to know it.
    #[must_use]
    pub fn contradicted(&self, key: &[u8; ed25519::PUBLIC_KEY_WIDTH]) -> bool {
        self.objects.contradicted(key)
    }

    /// Say what this node is running, on its own chain.
    ///
    /// **The first announcement named it and carried nothing else**, because what a node offers and
    /// what version it speaks change over its life and its name must not. This is how those get
    /// into the record, where anybody can count them.
    pub fn offering(&mut self, saying: Saying<'_>, now: Epoch) -> bool {
        let Saying {
            offers,
            version,
            reachable,
        } = saying;
        let Some(head) = self.objects.head(self.did.name()).cloned() else {
            // A node with no chain has not announced itself, so it has nothing to add to.
            return false;
        };
        // Signed with this node's own key, which is the only one that may say any of it — and the
        // reason the key is not something a caller hands in.
        let said = almena_store::announce::offering(
            &self.did,
            &head,
            offers,
            almena_store::announce::Speaking {
                version,
                reachable,
                issued: now,
                key: &self.key,
            },
        );
        self.submit(&said, now).is_ok()
    }

    /// What the network says it is running, counted across every node the record names.
    ///
    /// **Counted and never declared**, so that what is missing is visible before it is a problem
    /// and whoever wants to contribute can see what to contribute. It counts what nodes *say*; what
    /// they do is measured by asking, and a figure that mixed the two would be neither.
    #[must_use]
    pub fn running(&self, now: Epoch) -> Answered<almena_store::chain::Running> {
        self.stamped(self.objects.running(), now)
    }

    /// How the network's trust anchor stands against what `SPECS.md §7.1` asks of it.
    ///
    /// **Published rather than judged, and this is the whole of what holds the line.** The
    /// composition is configuration and not protocol: how many owners Almena has is its own to
    /// change by governance, exactly as for every other organisation, so nothing here refuses
    /// anything. What is not optional is *saying it* — while Almena is a set of keys in one pair of
    /// hands, the trust anchor of the network is a person, and `SPECS.md §7.1` puts that beside
    /// `SPECS.md §7.7`'s single-owner warning as its close relative.
    ///
    /// Three of the four criteria are here because they are owners and thresholds. The fourth — no
    /// more than half of them in one organisation or jurisdiction — cannot be: owners are root
    /// identifiers and root identifiers are anonymous (`SPECS.md §8.1`), so it is a declaration
    /// Almena makes and not something a node reads.
    ///
    /// [`None`] where this node cannot resolve its own government, which is its own ignorance and
    /// not an answer about the anchor.
    #[must_use]
    pub fn anchor(&self, now: Epoch) -> Answered<Option<Anchor>> {
        let held = match self.objects.resolve(self.government.name()) {
            almena_store::chain::Answer::Here(almena_store::chain::State::Government {
                body,
                ..
            }) => {
                let body = body.come_due(now);
                Some(Anchor {
                    owners: body.owners.len(),
                    thresholds: body.thresholds,
                    one_pair_of_hands: almena_store::government::one_pair_of_hands(&body),
                    wanting: almena_store::government::counted(&body),
                })
            }
            _ => None,
        };
        self.stamped(held, now)
    }

    /// Everything in the catalogue this node holds, by what each object is.
    ///
    /// **Names and not entries**, so that whoever asked composes each one from its own acts and
    /// checks the signatures on the way — the same rule the rest of the interface follows, and for
    /// the same reason: a catalogue handed over finished is one somebody has to believe.
    ///
    /// Unlike `about`, an empty answer here is a fact and not a gap: this node holds the acts it
    /// holds, and *nothing in the catalogue yet* is what a network before its first source looks
    /// like.
    #[must_use]
    pub fn catalogue(&self, now: Epoch) -> Answered<almena_store::chain::Catalogue> {
        self.stamped(self.objects.catalogue(), now)
    }

    /// The bytes of a status list version, where this node is holding them.
    ///
    /// **By hash, because the hash is the whole of the question** (`SPECS.md §10.2`). Whoever asks
    /// already knows which version the record names; what a node adds is a copy of the bytes, and
    /// either they match or they do not.
    #[must_use]
    pub fn list(&self, version: &[u8]) -> Option<Vec<u8>> {
        self.lists.get(version).cloned()
    }

    /// Take a status list's bytes, if the record names them.
    ///
    /// **Checked against the record and never against the sender.** A node keeps what the record
    /// says is the current version of some list; anything else is bytes somebody wanted stored, and
    /// a node that took those would be a node anybody can fill.
    ///
    /// It does **not** check that the share-out deals this list to this node: the assignment says
    /// what a node is short of if it does not have it (`SPECS.md §4.6`), never what it is forbidden
    /// to hold. A node that serves more than its share serves more copies of a public list, which
    /// is the direction that costs nobody anything.
    ///
    /// # Errors
    ///
    /// [`NotKept`], telling apart bytes no list in the record names from a version that is no
    /// longer the one in force and from a window that has already passed.
    pub fn keep_list(&mut self, bytes: Vec<u8>, now: Epoch) -> Result<Name, NotKept> {
        let version = almena_suite::digest::Digest::of(&bytes).bytes().to_vec();
        let clock = self.clock().ok_or(NotKept::NotNamed)?;
        let held = self
            .objects
            .status_lists()
            .into_iter()
            .find(|kept| kept.version == version)
            .ok_or(NotKept::NotNamed)?;
        // **Only the version in force** (`SPECS.md §10.2`): no history of contents is kept, so an
        // older version is not something to store, it is something nobody may use.
        if held.cohort.past(&clock, now) {
            return Err(NotKept::WindowPast);
        }
        self.lists.insert(version, bytes);
        Ok(held.list)
    }

    /// Let go of every list whose window has passed.
    ///
    /// **No operation, and nothing to consult** (`SPECS.md §10.2`). Every credential the list
    /// covered carries its expiry signed inside it and cannot move, so all of them are dead and the
    /// list can be thrown away whole rather than pruned — the same shape a closed entity already
    /// has in `SPECS.md §12.1`.
    pub fn forget_past_lists(&mut self, now: Epoch) {
        let Some(clock) = self.clock() else {
            return;
        };
        let living: std::collections::BTreeSet<Vec<u8>> = self
            .objects
            .status_lists()
            .into_iter()
            .filter(|kept| !kept.cohort.past(&clock, now))
            .map(|kept| kept.version)
            .collect();
        // **And a version the record no longer names goes too.** Holding it would be holding a copy
        // of something no verifier may use, and serving it would be serving a stale answer to
        // somebody who has to compare it against the record anyway.
        self.lists.retain(|version, _| living.contains(version));
    }

    /// This network's clock, which is its genesis instant and the arithmetic hanging from it.
    ///
    /// **[`None`] where the figure the record carries is no instant at all**, which refuses to
    /// answer rather than counting from the Unix epoch: a clock from a moment nobody fixed would
    /// put every window in the wrong place and say nothing about having done so.
    fn clock(&self) -> Option<almena_time::Clock> {
        almena_time::Clock::from_unix(self.began)
    }

    /// Take note of having actually reached somebody somewhere.
    ///
    /// **Kept apart from the record, and never written into it.** Where a node says it is, is its
    /// own word in a place everybody holds; where this node reached it is this node's observation
    /// and nobody else's. Folding the second into the first would put one node's experience into
    /// everybody's copy of the truth.
    ///
    /// By key, because that is what a connection proves somebody holds — the name it answers to is
    /// a separate question, and one the record may not be able to answer yet.
    pub fn reached(&mut self, key: [u8; ed25519::PUBLIC_KEY_WIDTH], at: String) {
        self.found_at.reached(&key, at);
    }

    /// Where this node has really reached whoever holds that key.
    ///
    /// Empty is *this node has not reached them*, which is not *they are nowhere*.
    #[must_use]
    pub fn found_at(&self, key: &[u8; ed25519::PUBLIC_KEY_WIDTH]) -> Vec<String> {
        self.found_at.at(key)
    }

    /// Where the record says a node can be reached.
    ///
    /// **What it said about itself**, which is what anybody else reading the record would also see —
    /// never where this node happened to find it. Whether it really answers there is a different
    /// question, and one that is measured by asking.
    #[must_use]
    pub fn reachable_at(&self, node: &Name) -> std::collections::BTreeSet<String> {
        match self.objects.resolve(node) {
            almena_store::chain::Answer::Here(almena_store::chain::State::Node {
                reachable,
                ..
            }) => reachable,
            _ => std::collections::BTreeSet::new(),
        }
    }

    /// Show a challenge for whoever is being asked to say they contributed this node.
    ///
    /// **The node shows, and does not decide.** Approving it is somebody else putting their name to
    /// a machine in a record that does not forget, so the node can ask and nothing more.
    ///
    /// The nonce is new every time, which is what makes an approval of one challenge an approval of
    /// nothing else. It is good until `until` and no longer: an approval that ended up in a
    /// screenshot, a support bundle or this node's own log must not bind somebody's machine a year
    /// later. **Nothing but this node remembers it was shown** — the record never saw it, and could
    /// not tell one shown twice from one shown once.
    ///
    /// # Errors
    ///
    /// [`getrandom::Error`] when the operating system will not produce randomness. A challenge
    /// somebody could have guessed is one an approval could be collected for in advance, so there
    /// is nothing to fall back on and nothing worth showing.
    pub fn asking_who_contributed_me(
        &self,
        until: Epoch,
    ) -> Result<almena_store::bind::Challenge, getrandom::Error> {
        let mut nonce = [0u8; 32];
        getrandom::fill(&mut nonce)?;
        Ok(almena_store::bind::Challenge {
            node: self.did.clone(),
            nonce,
            until,
        })
    }

    /// Write down that somebody contributed this node, with what they both put their name to.
    ///
    /// **Both sides or nothing.** The node signs it because it is the node's chain; the approval
    /// travels inside the act, and is checked against the key that whoever is claiming it has
    /// authorised in **their own** chain — never against anything the act itself carries, which is
    /// the thing under suspicion.
    ///
    /// [`false`] when the approval is not theirs, when the challenge had stopped being good, or
    /// when this node has no chain to add to. A binding that cannot be checked is not a weaker
    /// binding: it is this node's word about somebody who never agreed.
    pub fn contributed_by(
        &mut self,
        challenge: &almena_store::bind::Challenge,
        approval: &almena_store::bind::Approval,
        now: Epoch,
    ) -> bool {
        let Some(head) = self.objects.head(self.did.name()).cloned() else {
            return false;
        };
        let said = almena_store::bind::bind(
            &self.did,
            &head,
            &almena_store::bind::Claiming {
                challenge,
                approval,
                issued: now,
            },
            &self.key,
        );
        self.submit(&said, now).is_ok()
    }

    /// The same, from the text a person handed back rather than from types.
    ///
    /// **The reading belongs here and not in a face.** What a challenge and an approval look like
    /// written down is part of what they are, and two faces each reading them their own way would
    /// be two nodes that accept different things.
    pub fn contributed_by_text(&mut self, challenge: &str, approval: &str, now: Epoch) -> Claimed {
        let (Some(challenge), Some(approval)) = (
            almena_store::bind::Challenge::read(challenge),
            almena_store::bind::Approval::read(approval),
        ) else {
            return Claimed::NotAClaim;
        };
        if self.contributed_by(&challenge, &approval, now) {
            Claimed::Written
        } else {
            Claimed::NotTheirs
        }
    }

    /// Say this node is no longer contributed by anybody.
    ///
    /// **The node alone.** Whoever claimed it agreed to be credited for what it served, and letting
    /// go of that costs them nothing they can be held to — so nobody has to be asked. Credit stops
    /// from here and never in arrears: what was served was served.
    pub fn contributed_by_nobody(&mut self, now: Epoch) -> bool {
        let Some(head) = self.objects.head(self.did.name()).cloned() else {
            return false;
        };
        let said = almena_store::bind::unbind(&self.did, &head, now, &self.key);
        self.submit(&said, now).is_ok()
    }

    /// Close this node: say it stops counting from here.
    ///
    /// **The one way out of a node whose key is somebody else's** (`SPECS.md §4.1`). A node does not
    /// rotate — what a rotation preserves is an identity with something behind it, and a node has
    /// none: its roots stay where they are, signed by the key that signed them, and a new node
    /// starts with no history and has lost none. It could not rotate either, because the only thing
    /// that governs a node is the very key that was lost.
    ///
    /// What it costs is nothing that was said: closing is a state and never a deletion, and this
    /// node's roots, summaries and everything it replicated stay in the record and go on being
    /// read. What changes is the census the share-out is drawn from — no work is assigned to it and
    /// it is not counted as absent either, because it is not there.
    ///
    /// **It does not come back.** Coming back means announcing a new node, with a new key and a new
    /// name; one that returned would bring whoever took its key back with it.
    pub fn close_itself(&mut self, now: Epoch) -> bool {
        let Some(head) = self.objects.head(self.did.name()).cloned() else {
            return false;
        };
        let said = almena_store::announce::close(&self.did, &head, now, &self.key);
        self.submit(&said, now).is_ok()
    }

    /// Who the record says contributed this node, if anybody has and it checked out.
    ///
    /// [`None`] is a node nobody has claimed, which is a machine — and a machine cannot be credited
    /// for what it serves. Not a fault and not rare: a node runs perfectly well unclaimed.
    #[must_use]
    pub fn contributor_of(&self, node: &Name) -> Option<Did> {
        self.objects.claimed_by(node)
    }

    /// Say this node now also offers this, keeping everything else it has said.
    ///
    /// **For something switched on rather than chosen at the start**: carrying other nodes' traffic
    /// is a thing an operator turns on, and a node that could only say what it offered at the
    /// moment it was named would never say it at all.
    ///
    /// [`false`] when the record already says it, so that starting twice is not two acts saying one
    /// thing.
    pub fn also_offering(
        &mut self,
        capability: almena_store::capability::Capability,
        now: Epoch,
    ) -> bool {
        let name = self.did.name().clone();
        let almena_store::chain::Answer::Here(almena_store::chain::State::Node {
            offers,
            speaks,
            reachable,
            ..
        }) = self.objects.resolve(&name)
        else {
            return false;
        };
        if offers.contains(&capability) {
            return false;
        }
        let mut now_offers = offers;
        now_offers.insert(capability);
        self.offering(
            Saying {
                offers: &now_offers,
                version: speaks,
                reachable: &reachable,
            },
            now,
        )
    }

    /// Say this node can now also be reached here, keeping everything else it has said.
    ///
    /// **For an address that arrives rather than one that was chosen**: a port the operating system
    /// granted, or a relay agreeing to carry this node. Neither is known when it starts, and a node
    /// that could only say where it was at the moment it started would be a node behind a household
    /// router saying it is nowhere.
    ///
    /// What it offers and which version it speaks are read back from its own record rather than
    /// asked for again, because restating them from whatever a caller happened to pass would let a
    /// change of address quietly become a change of what the node is.
    ///
    /// [`false`] when nothing changed, so that a caller does not write an act saying what the
    /// record already says.
    pub fn also_reachable_at(
        &mut self,
        addresses: &std::collections::BTreeSet<String>,
        now: Epoch,
    ) -> bool {
        let name = self.did.name().clone();
        let almena_store::chain::Answer::Here(almena_store::chain::State::Node {
            offers,
            speaks,
            reachable,
            ..
        }) = self.objects.resolve(&name)
        else {
            // A node that has not announced itself has no announcement to add to.
            return false;
        };
        let now_reachable: std::collections::BTreeSet<String> =
            reachable.union(addresses).cloned().collect();
        if now_reachable == reachable {
            return false;
        }
        self.offering(
            Saying {
                offers: &offers,
                version: speaks,
                reachable: &now_reachable,
            },
            now,
        )
    }

    /// Say this node can no longer be reached at those addresses.
    ///
    /// **The other half, and not an optional one.** A relay that stops carrying it leaves an
    /// address in the record that answers nothing, and a record that only ever adds addresses is a
    /// record that fills with doors nobody is behind.
    pub fn no_longer_reachable_at(
        &mut self,
        addresses: &std::collections::BTreeSet<String>,
        now: Epoch,
    ) -> bool {
        let name = self.did.name().clone();
        let almena_store::chain::Answer::Here(almena_store::chain::State::Node {
            offers,
            speaks,
            reachable,
            ..
        }) = self.objects.resolve(&name)
        else {
            return false;
        };
        let left: std::collections::BTreeSet<String> =
            reachable.difference(addresses).cloned().collect();
        if left == reachable {
            return false;
        }
        self.offering(
            Saying {
                offers: &offers,
                version: speaks,
                reachable: &left,
            },
            now,
        )
    }

    /// What this node's record calls the node that holds `key`.
    ///
    /// **How a key becomes somebody.** A connection proves who holds a key; everything written down
    /// about a node is written about its name, so between the two there has to be the record — and
    /// this is it. [`None`] for a key nobody ever announced themselves with.
    #[must_use]
    pub fn node_called(
        &self,
        key: &[u8; ed25519::PUBLIC_KEY_WIDTH],
        now: Epoch,
    ) -> Answered<Option<Did>> {
        let named = self
            .objects
            .node_called(key)
            .map(|name| Did::new(self.which_marking(), name.clone()));
        self.stamped(named, now)
    }

    /// How identifiers on this network are written.
    fn which_marking(&self) -> almena_format::identifier::Network {
        self.government.network()
    }

    /// The last epoch this node has closed.
    ///
    /// [`None`] before it has closed one. It is what a node offers when somebody asks what it has
    /// signed, because asking about an epoch nobody has reached is asking about nothing.
    #[must_use]
    pub fn last_closed(&self) -> Option<Epoch> {
        self.roots.last()
    }

    /// What this node said about an epoch, if it said anything.
    #[must_use]
    pub fn root_at(&self, epoch: Epoch) -> Option<&Root> {
        self.roots.at(epoch)
    }

    /// Epochs between the first this node closed and `through` that it never closed.
    #[must_use]
    pub fn missing(&self, through: Epoch) -> Vec<u64> {
        self.roots.missing(through)
    }

    /// The root over everything written down right now.
    ///
    /// Separate from [`Answered`] because two callers already hold their own stamp and would
    /// otherwise be building one to throw it away.
    #[must_use]
    pub fn root_now(&self) -> Digest {
        self.log.root()
    }

    /// The latest act on an object's chain, which the next one has to follow.
    ///
    /// Whoever is about to sign needs it: an act that follows anything but the head is either a
    /// fork or a refusal, and a client that asks first turns the fork into the rare accident of
    /// two people acting at once rather than an everyday mistake.
    #[must_use]
    pub fn head(&self, name: &Name) -> Option<&Name> {
        self.objects.head(name)
    }

    /// What this node reports about itself.
    #[must_use]
    pub fn facts(&self) -> Facts {
        Facts {
            network: Some(self.network.as_str().to_owned()),
            identity: Some(written_out(&self.key())),
            written: Some(self.log.len() as u64),
            root: Some(written_out(self.log.root().bytes())),
            peer: Some(self.peer()),
        }
    }

    /// Come back to the node that is already in `directory`.
    ///
    /// **Not the same act as opening one.** Opening makes a network; this returns to the one that
    /// is here. A node that opened every time it started would make a new network every morning
    /// and be a stranger on each of them.
    ///
    /// The whole record is replayed, so everything but the acts and the roots is worked out again
    /// rather than trusted from disk: the tree, the chains, what each object resolves to, which
    /// network this is, and what this node is called.
    ///
    /// # Errors
    ///
    /// [`record::NotReadable`]. The one worth knowing about is
    /// [`record::NotReadable::DoesNotAddUp`]: the record no longer produces the tree this node has
    /// already signed, so it has lost acts it vouched for and refuses to serve a history that
    /// contradicts what it has said.
    pub fn rejoin(
        directory: &std::path::Path,
        key: ed25519::SigningKey,
    ) -> Result<Self, record::NotReadable> {
        let mut node = Self::replaying(&record::Record::acts(directory)?, key)?;
        // **The tree comes back from every entry, not only from the acts still held.** What this
        // node signed was the tree over all of them; one rebuilt from a subset would be a different
        // shape, and the node would contradict a root it had already published.
        node.take_back_entries(directory)?;
        node.take_back_roots(directory)?;
        node.record =
            Some(record::Record::open(directory).map_err(|_| record::NotReadable::NotWritable)?);
        Ok(node)
    }

    /// Become a node on the network these acts describe.
    ///
    /// **How a node joins one rather than opening one.** The acts came from somebody else, and
    /// nothing about them is believed for that: they go through the same admission as anything
    /// handed over by a stranger, and the first of them has to be an act that opens a network or
    /// there is no network here to be on.
    ///
    /// The joining node then says who it is, because until it does the record has no name for it —
    /// so `now` is what epoch it is, which the acts themselves say how to work out.
    ///
    /// # Errors
    ///
    /// [`record::NotReadable`].
    pub fn join(
        directory: &std::path::Path,
        key: ed25519::SigningKey,
        joining: Joining<'_>,
        now: Epoch,
    ) -> Result<Self, record::NotReadable> {
        let Joining { acts, network } = joining;

        // **Checked before anything is replayed or written down.** Whoever named the seed also
        // named the network, and a node that took whatever it was handed would be calling that the
        // network it joined — with somebody else's key as the anchor everything is trusted from.
        let first = acts.first().ok_or(record::NotReadable::Unreadable)?;
        let opening = operation_from(first).ok_or(record::NotReadable::Unreadable)?;
        if opening.object.name().as_str() != network {
            return Err(record::NotReadable::AnotherNetwork);
        }

        let mut node = Self::replaying(acts, key)?;
        node.record =
            Some(record::Record::open(directory).map_err(|_| record::NotReadable::NotWritable)?);

        // **Only what was taken**, so that a restart replays what this node holds rather than what
        // it was handed. A frame that was already held is not one more act, and writing it down
        // would make it one every morning.
        let taken: Vec<Vec<u8>> = node.log.everything();
        for act in &taken {
            if let Some(keeping) = node.record.as_mut() {
                keeping
                    .wrote(act)
                    .map_err(|_| record::NotReadable::NotWritable)?;
            }
        }

        // **Its own announcement, which nobody else's record can hold for it.** Until this, the
        // network knows the acts this node pulled and does not know the node.
        let announced = announce::announce(node.which()?, now, &node.key);
        node.submit(&announced.operation, now)
            .map_err(|_| record::NotReadable::Refused)?;
        Ok(node)
    }

    /// A node built by replaying acts, with nothing written down and no roots taken back.
    fn replaying(acts: &[Vec<u8>], key: ed25519::SigningKey) -> Result<Self, record::NotReadable> {
        let first = acts.first().ok_or(record::NotReadable::Unreadable)?;
        let opening = operation_from(first).ok_or(record::NotReadable::Unreadable)?;
        let which = genesis::declares(&opening).ok_or(record::NotReadable::Unreadable)?;

        let mut node = Self {
            network: opening.object.name().clone(),
            government: opening.object.clone(),
            did: announce::announce(which, opening.issued, &key).node,
            log: Log::new(),
            objects: Objects::new(),
            roots: Roots::new(),
            key,
            found_at: crate::found::Found::new(),
            watching: BTreeMap::new(),
            began: genesis::began(&opening).ok_or(record::NotReadable::Unreadable)?,
            post: almena_mailbox::mediator::Mediator::new(),
            lists: BTreeMap::new(),
            record: None,
        };

        // Admitted under the same rules that took them the first time, and each act against the
        // epoch it declared rather than against now — acts read at a later hour must not be
        // rejected for being old.
        for act in acts {
            let operation = operation_from(act).ok_or(record::NotReadable::Unreadable)?;
            let at = operation.issued;
            let admitted = match node.objects.admit(&operation, at) {
                Ok(admitted) => admitted,
                // **An act this build will not take that an earlier one did.** Rules tighten, and a
                // record outlives the build that wrote it — so this is a real state and refusing to
                // come up over it is the worst answer available. `SPECS.md §4.8` rule 1 is that a
                // node stores and propagates every act whether it understands it or not, and a node
                // that will not start has stopped replicating altogether: the fork between versions
                // that rule exists to prevent, arriving as an outage rather than as a disagreement.
                //
                // So the act is kept, its line is kept — the tree over those lines is what this
                // node signed, and one rebuilt without them would contradict a root it published —
                // and the object stops resolving, which is rule 2. Nothing is claimed about it and
                // nothing is served from before the act.
                Err(_) => {
                    node.objects.beyond(operation.object.name());
                    node.log.append(&operation, subject_of(&operation));
                    continue;
                }
            };
            // **Written down only if it was taken**, exactly as when it arrived the first time. An
            // act already held is not written twice: a second copy would be a second leaf in the
            // tree for one act, so two nodes handed the same acts in different numbers of copies
            // would sign different roots — and, worse, the copy would take over the name it shares.
            // What an act is called leaves out how it was signed, so *already held* is answered
            // before any signature is looked at: writing the copy down would put bytes nobody
            // checked under a name this node vouches for.
            if admitted != Admitted::AlreadyHere {
                node.log.append(&operation, subject_of(&operation));
            }
        }
        Ok(node)
    }

    /// Put back every entry this node had, including those whose acts it no longer holds.
    fn take_back_entries(
        &mut self,
        directory: &std::path::Path,
    ) -> Result<(), record::NotReadable> {
        for written in record::Record::entries(directory)? {
            let value =
                almena_format::cbor::read(&written).map_err(|_| record::NotReadable::Unreadable)?;
            let entry =
                almena_format::entry::read(&value).ok_or(record::NotReadable::Unreadable)?;
            self.note(&entry);
        }
        Ok(())
    }

    /// Which network this is, read back from the act that opened it.
    fn which(&self) -> Result<Which, record::NotReadable> {
        let first = self
            .log
            .at_sequence(0)
            .and_then(|entry| self.log.act(&entry.hash))
            .ok_or(record::NotReadable::Unreadable)?;
        let opening = operation_from(first).ok_or(record::NotReadable::Unreadable)?;
        genesis::declares(&opening).ok_or(record::NotReadable::Unreadable)
    }

    /// Take back the roots this node has already signed, and check the record against them.
    fn take_back_roots(&mut self, directory: &std::path::Path) -> Result<(), record::NotReadable> {
        let mut last: Option<Root> = None;
        for bytes in record::Record::roots(directory)? {
            let root = Root::read(&bytes).ok_or(record::NotReadable::Unreadable)?;
            // Two different roots for one epoch inside a node's own record is the misconduct the
            // whole cross-signing mechanism exists to catch, found here against itself.
            self.roots
                .publish(root.clone())
                .map_err(|_| record::NotReadable::DoesNotAddUp)?;
            last = Some(root);
        }

        // **What makes replay worth trusting, and it has to be the root and not the length.** A
        // record that lost an act and gained a later one is as long as it was and is not the same
        // record — and every inclusion proof this node ever gave against that root would now check
        // out against nothing.
        if let Some(root) = last
            && self.log.root_at(root.size) != Some(root.root)
        {
            return Err(record::NotReadable::DoesNotAddUp);
        }
        Ok(())
    }

    /// What this node answers to on the mesh.
    ///
    /// A pure function of its key, so it is the same every time the same directory starts — which
    /// is what makes it worth publishing somewhere other people will read it.
    #[must_use]
    pub fn peer(&self) -> String {
        peer::of(&self.key.verifying_key())
    }

    /// When this network's epoch zero began, in seconds since the Unix epoch.
    ///
    /// A fact of the network, fixed by the act that opened it. It is asked for rather than worked
    /// out, because a face that worked it out would be a face deciding what time it is.
    #[must_use]
    pub fn began(&self) -> u64 {
        self.began
    }

    /// The acts this node wrote down from `sequence` onward, in the bytes they arrived in.
    ///
    /// **How a node that has fallen behind catches up**, and the reason it is by position rather
    /// than by time: position is this node's own, so asking *what have you written since I last
    /// looked* needs no clock and no agreement about one.
    ///
    /// At most `most` of them. A node with a long record asked for everything would be composing a
    /// message nobody can hold, so the answer is a page and the asker comes back for the next —
    /// which is also why the answer says how far this node has got.
    ///
    /// **What comes back is not a claim about validity.** These are somebody else's signed acts,
    /// handed on untouched; whoever receives them puts them through exactly the same admission
    /// they would have gone through arriving any other way. Receiving is not believing.
    #[must_use]
    pub fn since(&self, sequence: u64, page: Page, now: Epoch) -> Answered<Vec<Vec<u8>>> {
        let mut acts = Vec::new();
        let mut weighs = 0usize;
        let mut at = sequence;

        while acts.len() < page.at_most {
            let Some(entry) = self.log.at_sequence(at) else {
                break;
            };
            let Some(act) = self.log.act(&entry.hash) else {
                break;
            };
            // **Stopped by weight and not only by count**, because how much a page weighs is the
            // number that matters to whoever has to read it — and one act may be as large as
            // whatever the node that took it was willing to accept. A page counted only in acts
            // would be one a reader can refuse, and a node whose answers cannot be read is one
            // nobody can catch up with.
            //
            // The first act goes in whatever it weighs. A page that could be empty because one act
            // is too heavy would be a node that could never hand that act over at all.
            if !acts.is_empty() && weighs + act.len() > page.weighing_at_most {
                break;
            }
            weighs += act.len();
            acts.push(act.to_vec());
            at += 1;
        }
        self.stamped(acts, now)
    }

    /// What this node is called, which is the hash of the act that introduced it.
    #[must_use]
    pub fn did(&self) -> &Did {
        &self.did
    }

    /// The key it signs with, which is what its name resolves to.
    #[must_use]
    pub fn key(&self) -> [u8; ed25519::PUBLIC_KEY_WIDTH] {
        self.key.verifying_key().bytes()
    }

    /// Sign an epoch's root, so it can be published and served.
    #[must_use]
    pub fn publish(&self, root: &Root) -> almena_store::root::Published {
        let mut published = root.publish(&self.key);
        // Whoever has said they saw it goes with it. They are not part of what this node signed —
        // it cannot sign a list that grows after it signed — and each one stands on its own.
        published.witnesses = self.roots.witnesses(root.epoch).to_vec();
        published
    }

    /// Take in somebody's word that they saw one of this node's own roots.
    ///
    /// **What a node cannot do to itself.** A node that showed one root to one person and another
    /// to somebody else would be caught by the two carrying different witnesses — so collecting
    /// them is how a node makes its own honesty checkable by people who cannot see its tree.
    ///
    /// Returns whether it was kept.
    pub fn saw(&mut self, epoch: Epoch, witness: almena_store::root::Witness) -> bool {
        self.roots.saw(epoch, witness)
    }

    /// Write down that somebody said two things about one epoch that cannot both be true.
    ///
    /// **The one thing that can be proved against a node**, put where everybody can check it. This
    /// node is not vouching for anything: what convinces is the two signatures inside, and its own
    /// only says who bothered to write it down.
    ///
    /// Returns whether it was taken. *Already here* is the ordinary answer once somebody else has
    /// found the same pair, and is not a failure.
    pub fn write_down(
        &mut self,
        one: &almena_store::root::Published,
        other: &almena_store::root::Published,
        now: Epoch,
    ) -> bool {
        let written = almena_store::contradiction::publish(one, other, now, &self.key);
        self.submit(&written.operation, now).is_ok()
    }

    /// Write down what this node saw of the others over a day.
    ///
    /// **Nobody says anything about themselves.** What this node claims about its own availability
    /// is worth nothing; what is worth something is that the nodes which kept asking it questions
    /// wrote down whether it answered — so this is always about somebody else, and this node leaves
    /// itself out.
    ///
    /// Returns whether it was written. A day still happening, or one with nobody but this node in
    /// it, is not summarised: either would compare with nothing.
    pub fn summarise(&mut self, day: almena_time::Day, watched: Watched, now: Epoch) -> bool {
        let Watched { looked } = watched;
        let Some(watching) = self.watching.get(&day) else {
            // Nothing was watched that day, so there is nothing to summarise and nothing a hash
            // could pin. An observer with no observations has no account to be held to.
            return false;
        };
        // **The figures and the hash come from one list, read two ways.** The other way round — a
        // hash over the figures being published — commits to nothing: it checks out against the act
        // carrying them whatever they say, and an observer that watched nobody passes exactly as
        // well as one that watched everybody. What this pins is the observations, so that having
        // published it the observer cannot later produce a different account of what it saw.
        let drawn_from = watching.digest();
        let seen = watching.seen(&|of| self.node_of(of, now));
        if !almena_store::summary::worth_writing(&self.did, day, &seen, now) {
            return false;
        }
        let Some(head) = self.objects.head(self.did.name()).cloned() else {
            // A node with no chain has not announced itself, so it has nothing to add to.
            return false;
        };
        let written = almena_store::summary::publish(
            almena_store::summary::Observer {
                observer: &self.did,
                head: &head,
                by: &self.key,
            },
            day,
            &seen,
            looked,
            drawn_from,
        );
        self.submit(&written.operation, now).is_ok()
    }

    /// Take note of one question put to one peer, and what came of it.
    ///
    /// **Written down as it happens, and it is what the day's summary is drawn from.** Kept by the
    /// day it fell in, so that a summary is over a window and not over whatever had accumulated
    /// when somebody asked for one.
    pub fn watched(&mut self, day: almena_time::Day, noted: almena_store::watching::Noted) {
        self.watching.entry(day).or_default().wrote(noted);
        // **Only a few days are kept**, because these never enter the record and nothing ages them
        // out but this. What they are for is checking a summary that was published recently; a day
        // whose summary is older than that is one whose observations this node no longer offers,
        // which is what `SPECS.md §5.1` means by *for as long as they keep them*.
        while self.watching.len() > DAYS_WATCHED {
            let Some(oldest) = self.watching.keys().next().copied() else {
                break;
            };
            self.watching.remove(&oldest);
        }
    }

    /// What this node saw on that day, asking by asking, or nothing where it kept none.
    ///
    /// **The thing a summary's hash is a promise about.** Without it the hash is a promise that
    /// could be kept rather than one that is.
    #[must_use]
    pub fn watching(&self, day: almena_time::Day) -> Option<&almena_store::watching::Watching> {
        self.watching.get(&day)
    }

    /// The name the record knows a peer by, from the key a connection proved they hold.
    ///
    /// A peer it cannot name is nobody here: a figure filed against no name is a figure about
    /// nobody, and one filed against a guess is worse. The observation itself is kept either way —
    /// it is what this node really saw.
    fn node_of(&self, of: &[u8], now: Epoch) -> Option<Did> {
        let key: [u8; ed25519::PUBLIC_KEY_WIDTH] = of.try_into().ok()?;
        self.node_called(&key, now).answer
    }

    /// This node's word that it saw somebody else's root.
    #[must_use]
    pub fn countersign(&self, root: &Root) -> almena_store::root::Witness {
        root.countersign(&self.key)
    }

    /// The hashes of everything said about somebody, or nothing while nothing can be said.
    ///
    /// It becomes a list on the day the first act that carries a subject exists. Until then the
    /// answer is that the question cannot be asked here yet — which is a different thing from
    /// asking it and finding nothing.
    fn about_hashes(&self, subject: &Did) -> Option<Vec<Name>> {
        let said: Vec<Name> = self
            .log
            .about(subject)
            .into_iter()
            .map(|entry| entry.hash.clone())
            .collect();

        if said.is_empty() && !ANYTHING_CARRIES_A_SUBJECT {
            return None;
        }
        Some(said)
    }

    /// Wrap an answer in what it was true at.
    fn stamped<T>(&self, answer: T, epoch: Epoch) -> Answered<T> {
        Answered {
            epoch,
            root: self.log.root(),
            answer,
        }
    }
}

/// One act, out of the bytes it was written down in.
fn operation_from(bytes: &[u8]) -> Option<Operation> {
    let value = almena_format::cbor::read(bytes).ok()?;
    almena_format::operation::read(&value)
}

/// A key nobody can guess, from the operating system's own source.
///
/// **Not derived from anything.** A node's key is what its roots are signed with and what its
/// identity on the mesh will be, so anything reproducible would be a network somebody else could
/// stand up a copy of.
///
/// # Errors
///
/// [`getrandom::Error`] when the operating system will not produce randomness. That is not a
/// condition to work around: a node that generated a key from something predictable would be worse
/// than one that refused to start, because it would start.
pub fn fresh_key() -> Result<ed25519::SigningKey, getrandom::Error> {
    let mut secret = [0u8; ed25519::PUBLIC_KEY_WIDTH];
    getrandom::fill(&mut secret)?;
    Ok(ed25519::SigningKey::from_secret(secret))
}

/// Bytes as a person would paste them into a support channel.
fn written_out(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// What an act is about, when that is not its author.
///
/// **Nothing carries one yet, and that is the truth rather than a gap left open.** The acts that
/// have a subject are a certification, a vote and a contradiction — each of them a claim about
/// somebody who neither signs it nor could stop it — and none of the three is built. When the
/// first one is, this reads it out of that act rather than guessing.
///
/// A daily summary will never carry one even then: it speaks about many nodes at once, and one
/// entry per observed node per day is the arithmetic that choosing an aggregate avoided.
fn subject_of(operation: &Operation) -> Option<Did> {
    // **From the one place that says it**, which is the same place anybody checking an inclusion
    // proof rebuilds the entry from. Deciding it here as well would be two answers to one question,
    // and the day they differed an honest proof for an honest act would be refused.
    almena_store::subject_of(operation)
}

/// How the network's trust anchor stands, as the record has it.
///
/// **Facts and no verdict**, which is the same discipline every other figure this node publishes
/// follows: it says how many and what it costs, and whoever is relying on the seal decides what
/// that is worth to them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    /// How many owners it has. Nought is the state a network opens in.
    pub owners: usize,
    /// What each class of act costs them.
    pub thresholds: almena_store::entity::Thresholds,
    /// Whether the key the genesis gave it is still what signs.
    pub one_pair_of_hands: bool,
    /// What `SPECS.md §7.1` asks that the record does not yet show.
    pub wanting: Vec<almena_store::government::Wanting>,
}

/// Whether any act this build knows how to write down can be about somebody else.
///
/// True since a contradiction says who it is against, which is what makes *what has been said about
/// this node* a question with an answer rather than one the interface has to decline.
const ANYTHING_CARRIES_A_SUBJECT: bool = true;

#[cfg(test)]
mod tests {
    use super::{COPIES_OF_HISTORY, Joining, Node, Page, Watched, record};
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::{Signed, create};
    use almena_store::chain::{Answer, Reason, State};
    use almena_store::genesis::{Refused, Which};
    use almena_store::kind::Kind;
    use almena_suite::ed25519;
    use almena_time::{Epoch, Epochs};
    use std::collections::BTreeMap;

    /// A fixed instant, so that a test is never about what time it is here.
    const WHEN: u64 = 1_800_000_000;

    fn at(which: super::Which) -> super::Opening {
        super::Opening {
            which,
            beginning: Epoch::GENESIS,
            began: WHEN,
        }
    }

    /// A page of at most this many acts, with room enough that nothing here is trimmed by weight.
    /// Taking a device off the account, so a chain can be made to grow with real acts.
    fn a_removal(
        object: &Did,
        head: &Name,
        control: &ed25519::SigningKey,
        device: &[u8],
    ) -> almena_format::operation::Operation {
        signed(
            almena_format::operation::Operation {
                object: object.clone(),
                previous: Some(head.clone()),
                kind: Kind::HOLDER_REMOVE_DEVICE.number(),
                version: 1,
                issued: Epoch::GENESIS,
                payload: BTreeMap::from([(1, Value::Bytes(device.to_vec()))]),
                signatures: Vec::new(),
            },
            control,
        )
    }

    /// An act that carries a summary of the state it leaves behind.
    /// A summary of that account, written at that moment.
    ///
    /// **Dated, because a summary waits for the queue to empty.** A summary has nowhere to say what
    /// is in flight, so nothing summarises over an asking — and every fixture here starts with
    /// askings the control key made, which come due one window after they were written.
    fn a_summary(
        object: &Did,
        head: &Name,
        control: &ed25519::SigningKey,
        claims: &[almena_store::checkpoint::Claim],
        at: Epoch,
    ) -> almena_format::operation::Operation {
        signed(
            almena_format::operation::Operation {
                object: object.clone(),
                previous: Some(head.clone()),
                kind: Kind::HOLDER_CHECKPOINT.number(),
                version: 1,
                issued: at,
                payload: BTreeMap::from([(
                    almena_store::checkpoint::FIELD,
                    almena_store::checkpoint::declaration(claims),
                )]),
                signatures: Vec::new(),
            },
            control,
        )
    }

    /// The same act, with the control key's name on it.
    fn signed(
        mut operation: almena_format::operation::Operation,
        control: &ed25519::SigningKey,
    ) -> almena_format::operation::Operation {
        let signature = control.sign(&operation.signing_bytes());
        operation.signatures.push(Signed {
            by: operation.object.clone(),
            key: control.verifying_key().bytes().to_vec(),
            signature: signature.bytes(),
        });
        operation
    }

    /// An account made busy with real acts, and where its chain ends up.
    fn a_busy_account(
        node: &mut Node,
        object: &Did,
        control: &ed25519::SigningKey,
        rounds: usize,
    ) -> Name {
        let mut head = node.head(object.name()).expect("a head").clone();
        for _ in 0..rounds {
            // Askings, twice a round: what a busy account needs here is length, and the words
            // asking for the same key again is one more act however often it is said.
            let on = a_device(object, &head, control);
            node.submit(&on, Epoch::GENESIS).expect("taken");
            head = on.called();
            let again = a_device(object, &head, control);
            node.submit(&again, Epoch::GENESIS).expect("taken");
            head = again.called();
        }
        head
    }

    fn a_page(at_most: usize) -> Page {
        Page {
            at_most,
            weighing_at_most: usize::MAX,
        }
    }

    fn key(seed: u8) -> ed25519::SigningKey {
        ed25519::SigningKey::from_secret([seed; 32])
    }

    fn opened() -> Node {
        opened_by(6)
    }

    /// The same network, opened by a node with a key of its own.
    ///
    /// The government key is held fixed on purpose: change it and the genesis changes, and two
    /// nodes that were meant to be on one network end up on two, where nothing they say about each
    /// other means anything.
    fn opened_by(node: u8) -> Node {
        Node::open(&at(Which::Development), &[], &key(5), key(node)).expect("nobody to join")
    }

    /// A holder creation, signed by the control key it establishes.
    /// A device added to an account, signed by the key that controls it.
    fn a_device(
        object: &Did,
        head: &Name,
        control: &ed25519::SigningKey,
    ) -> almena_format::operation::Operation {
        let device = almena_suite::p256::SigningKey::from_secret([4; 32])
            .expect("a key")
            .verifying_key()
            .bytes();

        let mut operation = almena_format::operation::Operation {
            object: object.clone(),
            previous: Some(head.clone()),
            kind: Kind::HOLDER_ADD_DEVICE.number(),
            version: 1,
            issued: Epoch::GENESIS,
            payload: BTreeMap::from([(1, Value::Bytes(device.to_vec()))]),
            signatures: Vec::new(),
        };
        let signature = control.sign(&operation.signing_bytes());
        operation.signatures.push(Signed {
            by: object.clone(),
            key: control.verifying_key().bytes().to_vec(),
            signature: signature.bytes(),
        });
        operation
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

    #[test]
    fn a_node_and_whoever_contributed_it_both_have_to_say_so() {
        // **The rule the whole thing rests on.** A node saying alone who contributed it is a node
        // writing somebody's name into a record that does not forget; whoever is claiming it saying
        // alone is somebody claiming a machine they may not hold. It takes both, or it binds
        // nothing.
        let mut node = opened_by(6);
        let mine = node.did().name().clone();
        assert_eq!(
            node.contributor_of(&mine),
            None,
            "a node nobody has claimed is a machine, which is not a fault"
        );

        // Whoever is going to claim it has an account of their own, and a key it authorises.
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let claimant = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let challenge = node
            .asking_who_contributed_me(Epoch::GENESIS)
            .expect("the operating system has randomness");
        let approval = almena_store::bind::Approval {
            claimant: claimant.clone(),
            signature: control.sign(&challenge.to_bytes()).bytes(),
        };
        assert!(node.contributed_by(&challenge, &approval, Epoch::GENESIS));
        assert_eq!(node.contributor_of(&mine), Some(claimant));

        // And letting go is the node alone: whoever claimed it agreed to be credited, and giving
        // that up costs them nothing anybody could hold them to.
        assert!(node.contributed_by_nobody(Epoch::GENESIS));
        assert_eq!(node.contributor_of(&mine), None);
    }

    #[test]
    fn an_approval_somebody_else_signed_binds_nobody() {
        // The whole reason the approval is checked against the claimant's own chain rather than
        // against anything the act carries: what the act carries is the thing under suspicion.
        let mut node = opened_by(7);
        let account = an_account(&key(9), Epoch::GENESIS);
        let claimant = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let challenge = node
            .asking_who_contributed_me(Epoch::GENESIS)
            .expect("the operating system has randomness");
        // Signed by a key that speaks for nobody, and put forward under a name that is somebody.
        let approval = almena_store::bind::Approval {
            claimant,
            signature: key(200).sign(&challenge.to_bytes()).bytes(),
        };
        assert!(!node.contributed_by(&challenge, &approval, Epoch::GENESIS));
        assert_eq!(node.contributor_of(node.did().name()), None);
    }

    #[test]
    fn an_approval_of_one_challenge_approves_nothing_else() {
        // A nonce nobody could have guessed is what makes an approval an approval of **this**, and
        // what stops one collected for something else being lifted onto a machine.
        let mut node = opened_by(8);
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let claimant = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let shown = node
            .asking_who_contributed_me(Epoch::GENESIS)
            .expect("the operating system has randomness");
        let other = node
            .asking_who_contributed_me(Epoch::GENESIS)
            .expect("the operating system has randomness");
        assert_ne!(shown.nonce, other.nonce, "each one is its own");

        // Approving the one that was shown, put forward beside the one that was not.
        let approval = almena_store::bind::Approval {
            claimant,
            signature: control.sign(&shown.to_bytes()).bytes(),
        };
        assert!(!node.contributed_by(&other, &approval, Epoch::GENESIS));
        assert_eq!(node.contributor_of(node.did().name()), None);
    }

    #[test]
    fn an_approval_that_has_stopped_being_good_binds_nothing() {
        // What a short life is for: one that ended up in a screenshot, a support bundle or the
        // node's own log must not bind somebody's machine a year later.
        let mut node = opened_by(4);
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let claimant = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let challenge = node
            .asking_who_contributed_me(Epoch::GENESIS)
            .expect("the operating system has randomness");
        let approval = almena_store::bind::Approval {
            claimant,
            signature: control.sign(&challenge.to_bytes()).bytes(),
        };
        let later = Epoch::GENESIS
            .plus(almena_time::Epochs(1))
            .expect("no overflow");
        assert!(!node.contributed_by(&challenge, &approval, later));
        assert_eq!(node.contributor_of(node.did().name()), None);
    }

    #[test]
    fn two_fresh_keys_are_not_the_same_key() {
        // The property the whole thing rests on. A key derived from anything reproducible would be
        // a network somebody else could stand up a copy of.
        let one = super::fresh_key().expect("the operating system has randomness");
        let other = super::fresh_key().expect("the operating system has randomness");
        assert_ne!(one.verifying_key().bytes(), other.verifying_key().bytes());
    }

    #[test]
    fn a_node_that_has_just_started_owes_only_the_epoch_it_started_in() {
        // It was not absent for the epochs before it existed, and publishing them would be
        // claiming a history it never observed.
        let mut keeping = super::Keeping::new();
        let started = Epoch::GENESIS.plus(Epochs(500)).expect("no overflow");
        assert_eq!(keeping.due(started), vec![started]);
    }

    #[test]
    fn a_node_that_was_away_owes_every_epoch_it_missed() {
        // Not just the one it woke up in. A gap that could mean *nothing happened* or *I was not
        // here* means neither, and this is what stops one being left.
        let mut keeping = super::Keeping::new();
        keeping.due(Epoch::GENESIS.plus(Epochs(10)).expect("no overflow"));

        let awake = Epoch::GENESIS.plus(Epochs(14)).expect("no overflow");
        let owed: Vec<u64> = keeping.due(awake).into_iter().map(Epoch::number).collect();
        assert_eq!(owed, vec![11, 12, 13, 14]);
    }

    #[test]
    fn nothing_is_owed_twice_and_a_clock_going_backwards_owes_nothing() {
        let mut keeping = super::Keeping::new();
        let ten = Epoch::GENESIS.plus(Epochs(10)).expect("no overflow");
        keeping.due(ten);

        assert!(keeping.due(ten).is_empty(), "already spoken for");
        assert!(
            keeping
                .due(Epoch::GENESIS.plus(Epochs(4)).expect("no overflow"))
                .is_empty(),
            "a clock that went backwards is not a reason to say something else about an epoch"
        );
        assert_eq!(keeping.closed(), Some(ten));
    }

    #[test]
    fn catching_up_leaves_no_hole_in_what_the_node_published() {
        // The two halves meeting: what is owed gets closed, and the node's own record of holes
        // comes back empty.
        let mut node = opened();
        let mut keeping = super::Keeping::new();
        keeping.due(Epoch::GENESIS);

        let awake = Epoch::GENESIS.plus(Epochs(5)).expect("no overflow");
        for epoch in keeping.due(awake) {
            node.close(epoch);
        }
        assert!(node.missing(awake).is_empty());
    }

    #[test]
    fn a_node_reports_what_it_is_and_never_a_default() {
        // Both faces read this, so it is the one place the two could start disagreeing — and the
        // one thing that must never happen here is a friendly default standing in for a fact.
        let node = opened();
        let facts = node.facts();

        assert_eq!(facts.network.as_deref(), Some(node.network().as_str()));
        assert_eq!(
            facts.written,
            Some(2),
            "the act that opened the network, and the one this node introduced itself with"
        );
        assert!(
            facts.identity.is_some_and(|key| key.len() == 64),
            "the key, written out"
        );
        assert!(facts.root.is_some_and(|root| root.len() == 64));
    }

    #[test]
    fn a_node_with_no_network_has_looked_at_nothing() {
        // *Nobody has looked* and *there is none* are different facts, and a face that could not
        // tell them apart would draw a zero where nothing had been counted.
        let nothing = super::Facts::default();
        assert_eq!(nothing.network, None);
        assert_eq!(nothing.identity, None);
        assert_eq!(nothing.written, None);
        assert_eq!(nothing.root, None);
    }

    #[test]
    fn a_node_that_opens_a_network_has_written_the_act_that_opened_it() {
        let node = opened();
        assert_eq!(node.written(), 2, "opening it, and joining it");
        assert_eq!(node.network(), node.government().name());
    }

    #[test]
    fn a_production_network_is_opened_against_the_frozen_format_and_development_is_not() {
        // **Both halves matter.** A record is append-only, so what is missing on the day production
        // opens is missing for as long as that network exists — and development is re-opened every
        // time the format moves, which is why it is a second network rather than a flag.
        //
        // What this cannot show is the refusal, because there is nothing here that could make the
        // checklist fail on purpose without lying to it. What it shows is the link: the same
        // question the gate puts is put here, and a production network opens only while the answer
        // is empty. The items themselves are held to in `almena_frozen`.
        assert!(
            almena_frozen::wanting().is_empty(),
            "the format this build writes: {:#?}",
            almena_frozen::wanting()
        );
        assert!(
            Node::open(&at(Which::Production), &[], &key(5), key(6)).is_ok(),
            "and so a production network may be opened on it"
        );
        assert!(Node::open(&at(Which::Development), &[], &key(5), key(6)).is_ok());
    }

    #[test]
    fn an_act_the_disk_refused_is_mended_by_being_handed_over_again() {
        // **A real state, and the repair for it has to be the obvious one.** Admission moves memory
        // and the writing comes after, so a disk that refuses leaves the act applied and in neither
        // the log nor the record. What must not happen is what used to: handing it over again
        // answered *already here* and wrote nothing, so the only thing that mended it was a
        // restart — the one repair nobody thinks to try and the one that costs the most.
        let scratch = Scratch::new("refused-write");
        let mut node = Node::open_in(&scratch.0, &at(Which::Development), &[], &key(5), key(6))
            .expect("nobody to join");
        let written = node.written();

        node.record = Some(record::will_not_write(&scratch.0).expect("a record to refuse with"));
        let account = an_account(&key(9), Epoch::GENESIS);
        assert_eq!(
            node.submit(&account, Epoch::GENESIS),
            Err(almena_store::chain::Refused::NotKept),
            "the disk refused it"
        );
        assert_eq!(node.written(), written, "and nothing was written down");
        assert!(
            matches!(
                node.resolve(account.object.name(), Epoch::GENESIS).answer,
                Answer::Here(_)
            ),
            "while memory is ahead of the record, which is the state being mended"
        );

        node.record = Some(record::Record::open(&scratch.0).expect("a record that writes"));
        node.submit(&account, Epoch::GENESIS)
            .expect("handing it over again is what mends it");
        assert_eq!(
            node.written(),
            written + 1,
            "and now there is a line for it"
        );

        // Once, and no more: a second copy in the record would be a second position in the tree.
        node.submit(&account, Epoch::GENESIS).expect("taken");
        assert_eq!(node.written(), written + 1);
    }

    /// The acts a node hands over to compose an object from, from the beginning.
    fn composed(node: &Node, object: &Did, page: Page, now: Epoch) -> Vec<Vec<u8>> {
        node.state_of(object, None, page, now)
            .answer
            .expect("no cursor was given, so there is nothing it could fail to place")
            .acts
    }

    #[test]
    fn a_page_that_stopped_short_says_where_it_stopped_and_the_next_carries_on() {
        // **The defect this closes.** A page with a bound and no cursor is indistinguishable from
        // the whole of a chain: a caller folds what arrived, lands on a state from earlier, and
        // nothing anywhere says so. Every other paged answer here names where it stopped.
        let mut node = opened();
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");
        a_busy_account(&mut node, &object, &control, 3);

        let whole = composed(&node, &object, a_page(100), Epoch::GENESIS);
        assert!(whole.len() > 2, "a chain worth paging over");

        let mut gathered = Vec::new();
        let mut after = None;
        loop {
            let page = node
                .state_of(&object, after.as_ref(), a_page(2), Epoch::GENESIS)
                .answer
                .expect("every cursor here came from the page before it");
            gathered.extend(page.acts);
            match page.more {
                Some(more) => after = Some(more),
                None => break,
            }
        }
        assert_eq!(
            gathered, whole,
            "and following the cursor to the end gives exactly what one page would have"
        );
    }

    #[test]
    fn a_cursor_this_node_cannot_place_is_answered_as_nothing_and_never_as_the_beginning() {
        // Handing back the first page to somebody who asked for the fourth would look like an
        // answer and be a different one — and they would fold it and be wrong.
        let mut node = opened();
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        assert_eq!(
            node.state_of(
                &object,
                Some(&Name::of(b"an act nobody wrote")),
                a_page(100),
                Epoch::GENESIS
            )
            .answer,
            None
        );
    }

    #[test]
    fn what_a_node_cannot_read_is_worked_out_again_from_the_record_and_never_carried() {
        // **Why a build that learns to read an act heals the object it refused.** Opacity is not a
        // thing the record holds and not a thing a restart inherits: it is computed by admission
        // from the acts themselves, so a node coming back up puts every act through the same
        // question again and answers it with what *this* build knows. A build that could read the
        // act would answer differently, which is the whole of what upgrading is.
        let scratch = Scratch::new("opaque-is-recomputed");
        let mut node = Node::open_in(&scratch.0, &at(Which::Development), &[], &key(5), key(6))
            .expect("nobody to join");

        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        // An act on that account carrying a critical field this build has no meaning for. It is
        // kept and passed on — replication does not require understanding — and the object stops
        // resolving, which is the other half of the same rule.
        let mut beyond = almena_format::operation::Operation {
            object: object.clone(),
            previous: Some(account.called()),
            kind: almena_store::kind::Kind::HOLDER_ADD_DEVICE.number(),
            version: 1,
            issued: Epoch::GENESIS,
            payload: BTreeMap::from([(1, Value::Bytes(vec![2; 33])), (9_999, Value::Uint(1))]),
            signatures: Vec::new(),
        };
        let signature = control.sign(&beyond.signing_bytes());
        beyond.signatures.push(Signed {
            by: object.clone(),
            key: control.verifying_key().bytes().to_vec(),
            signature: signature.bytes(),
        });
        node.submit(&beyond, Epoch::GENESIS)
            .expect("kept and passed on");
        assert!(matches!(
            node.resolve(object.name(), Epoch::GENESIS).answer,
            Answer::CannotResolve(Reason::Unintelligible)
        ));

        // Coming back up, every act goes through admission again from nothing. This build still
        // cannot read that field, so it answers the same — and a build that could would not, with
        // no state anywhere to stop it.
        drop(node);
        let again = Node::rejoin(&scratch.0, key(6)).expect("the record comes back");
        assert_eq!(
            again.written(),
            4,
            "the genesis, this node's announcement, the account and the act it could not read"
        );
        assert!(matches!(
            again.resolve(object.name(), Epoch::GENESIS).answer,
            Answer::CannotResolve(Reason::Unintelligible)
        ));
    }

    #[test]
    fn a_node_comes_up_over_an_act_it_will_not_take_and_stops_resolving_what_it_is_on() {
        // **A record outlives the build that wrote it, and rules tighten.** So this is a real
        // state: an act an earlier build accepted that this one will not. Refusing to come up over
        // it is the worst answer available — `SPECS.md §4.8` rule 1 is that a node stores and
        // propagates every act whether it understands it or not, and a node that will not start has
        // stopped replicating altogether, which is the fork between versions that rule prevents,
        // arriving as an outage instead.
        let scratch = Scratch::new("will-not-take");
        let mut node = Node::open_in(&scratch.0, &at(Which::Development), &[], &key(5), key(6))
            .expect("nobody to join");
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");
        let written = node.written();

        // An act this build refuses, put straight into the record — standing in for one an earlier
        // build let through and this one will not. Unsigned is the plainest such act there is, and
        // what it stands in for is any rule that has since tightened.
        let refused = almena_format::operation::Operation {
            object: object.clone(),
            previous: Some(account.called()),
            kind: almena_store::kind::Kind::HOLDER_ADD_DEVICE.number(),
            version: 1,
            issued: Epoch::GENESIS,
            payload: BTreeMap::from([(1, Value::Bytes(vec![3; 33]))]),
            signatures: Vec::new(),
        };
        assert!(
            node.submit(&refused, Epoch::GENESIS).is_err(),
            "handed over the ordinary way it is refused, which is what makes it the right stand-in"
        );
        drop(node);

        record::Record::open(&scratch.0)
            .expect("a record")
            .wrote(&refused.to_bytes())
            .expect("straight into it, as an earlier build would have left it");

        // And the node comes up.
        let again = Node::rejoin(&scratch.0, key(6)).expect("a node comes up over it");
        assert_eq!(
            again.written(),
            written + 1,
            "the line saying it happened is kept — the tree over those lines is what this node signed"
        );
        assert!(
            matches!(
                again.resolve(object.name(), Epoch::GENESIS).answer,
                Answer::CannotResolve(Reason::Unintelligible)
            ),
            "and the object stops resolving rather than being served from before the act"
        );
    }

    /// A directory of this test's own, removed when it is done with it.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("almena-node-{name}"));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_node_that_stops_comes_back_as_the_same_node_on_the_same_network() {
        // The whole reason a record exists. Before it, every start opened a new network and the
        // node was a stranger on each one.
        let scratch = Scratch::new("survives");
        let opened = Node::open_in(&scratch.0, &at(Which::Development), &[], &key(5), key(6))
            .expect("nobody to join");
        let (network, did, written) = (
            opened.network().clone(),
            opened.did().clone(),
            opened.written(),
        );
        drop(opened);

        let back = Node::rejoin(&scratch.0, key(6)).expect("its own record");
        assert_eq!(back.network(), &network, "the same network");
        assert_eq!(back.did(), &did, "and the same node on it");
        assert_eq!(
            back.written(),
            written,
            "with everything it had written down"
        );
    }

    #[test]
    fn what_was_accepted_before_it_stopped_is_still_there_after() {
        let scratch = Scratch::new("acts");
        let mut node = Node::open_in(&scratch.0, &at(Which::Development), &[], &key(5), key(6))
            .expect("nobody to join");
        let account = an_account(&key(9), Epoch::GENESIS);
        let name = account.object.name().clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");
        drop(node);

        let back = Node::rejoin(&scratch.0, key(6)).expect("its own record");
        assert!(
            matches!(back.resolve(&name, Epoch::GENESIS).answer, Answer::Here(_)),
            "the account it was told about is still an account it knows"
        );
    }

    #[test]
    fn the_root_it_signed_before_it_stopped_is_the_root_it_still_stands_by() {
        // The reason the roots are kept rather than worked out again: a node that recomputed a
        // different root for an epoch it had already signed would be signing two roots for one
        // epoch, against itself.
        let scratch = Scratch::new("roots");
        let mut node = Node::open_in(&scratch.0, &at(Which::Development), &[], &key(5), key(6))
            .expect("nobody to join");
        node.submit(&an_account(&key(9), Epoch::GENESIS), Epoch::GENESIS)
            .expect("taken");
        let signed = node.close(Epoch::GENESIS);
        drop(node);

        let back = Node::rejoin(&scratch.0, key(6)).expect("its own record");
        assert_eq!(
            back.root_at(Epoch::GENESIS),
            Some(&signed),
            "what it said about that epoch has not moved"
        );
    }

    #[test]
    fn a_record_that_lost_acts_it_vouched_for_stops_the_node() {
        // Coming up anyway would mean serving a history that contradicts inclusion proofs this
        // node has already handed out.
        let scratch = Scratch::new("lost");
        let mut node = Node::open_in(&scratch.0, &at(Which::Development), &[], &key(5), key(6))
            .expect("nobody to join");
        node.submit(&an_account(&key(9), Epoch::GENESIS), Epoch::GENESIS)
            .expect("taken");
        node.close(Epoch::GENESIS);
        drop(node);

        // The acts go; what the node signed about them stays.
        std::fs::remove_file(record::acts_at(&scratch.0)).expect("removed");
        Node::open_in(&scratch.0, &at(Which::Development), &[], &key(5), key(7))
            .expect("a fresh record");

        assert_eq!(
            Node::rejoin(&scratch.0, key(6)).err(),
            Some(record::NotReadable::DoesNotAddUp)
        );
    }

    #[test]
    fn an_epoch_closed_late_is_over_the_tree_this_node_last_put_its_name_to() {
        // **What a tree is for stopping.** An act observed now must not appear in the root of an
        // epoch that ended before it existed — that would make a position in time forgeable by
        // simply being behind, with no forgery required.
        let mut node = opened();
        let signed = node
            .root_at(Epoch::GENESIS)
            .expect("opening closed it")
            .clone();

        node.submit(&an_account(&key(9), Epoch::GENESIS), Epoch::GENESIS)
            .expect("taken");

        let owed: Vec<Epoch> = (1..=3)
            .map(|by| Epoch::GENESIS.plus(Epochs(by)).expect("no overflow"))
            .collect();
        node.close_owed(&owed);

        for missed in &owed[..2] {
            let root = node.root_at(*missed).expect("closed");
            assert_eq!(
                root.size, signed.size,
                "an epoch it was not looking at says only that nothing it saw had changed"
            );
            assert_eq!(root.root, signed.root);
        }

        let now = node.root_at(owed[2]).expect("closed");
        assert_eq!(
            now.size,
            node.written() as u64,
            "and the epoch it is in gets the tree it has"
        );
    }

    #[test]
    fn nothing_is_included_in_an_epoch_that_ended_before_it_arrived() {
        // The same thing said as a proof, which is where it would have done the damage.
        let mut node = opened();
        let account = an_account(&key(9), Epoch::GENESIS);
        let name = account.called();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let owed: Vec<Epoch> = (1..=3)
            .map(|by| Epoch::GENESIS.plus(Epochs(by)).expect("no overflow"))
            .collect();
        node.close_owed(&owed);

        assert!(
            node.inclusion_in(&name, owed[0], owed[2]).answer.is_none(),
            "it was not in that epoch's tree, so there is no proof that it was"
        );
        assert!(
            node.inclusion_in(&name, owed[2], owed[2]).answer.is_some(),
            "and it is in the one it was actually observed in"
        );
    }

    #[test]
    fn closing_an_epoch_twice_hands_back_the_same_root_both_times() {
        // What a node stands by, not what it has just worked out. Handing back a fresh root for an
        // epoch already spoken for would give whoever asked two roots for one epoch in this node's
        // name — the one thing that is provable against it.
        let mut node = opened();
        let first = node.close(Epoch::GENESIS);

        node.submit(&an_account(&key(9), Epoch::GENESIS), Epoch::GENESIS)
            .expect("taken");
        let again = node.close(Epoch::GENESIS);

        assert_eq!(first, again);
        assert_eq!(node.root_at(Epoch::GENESIS), Some(&first));
    }

    #[test]
    fn a_node_hands_on_what_it_wrote_down_from_wherever_somebody_got_to() {
        let mut node = opened();
        node.submit(&an_account(&key(9), Epoch::GENESIS), Epoch::GENESIS)
            .expect("taken");

        let all = node.since(0, a_page(100), Epoch::GENESIS).answer;
        assert_eq!(all.len(), node.written(), "everything, from the beginning");

        let rest = node.since(2, a_page(100), Epoch::GENESIS).answer;
        assert_eq!(rest.len(), 1, "and only what came after where they got to");
        assert_eq!(rest[0], all[2], "the same act, in the same bytes");
    }

    #[test]
    fn a_page_stops_at_a_weight_as_well_as_at_a_count() {
        // The number that matters to whoever has to read it. A page counted only in acts is one a
        // reader can refuse, and a node whose answers cannot be read is one nobody can catch up
        // with.
        let node = opened();
        let handed = node
            .since(
                0,
                Page {
                    at_most: 100,
                    weighing_at_most: 1,
                },
                Epoch::GENESIS,
            )
            .answer;

        assert_eq!(
            handed.len(),
            1,
            "the first act goes in whatever it weighs, or it could never be handed over at all"
        );
    }

    #[test]
    fn asking_for_everything_gets_a_page_and_not_a_message_nobody_can_hold() {
        let node = opened();
        assert_eq!(node.since(0, a_page(1), Epoch::GENESIS).answer.len(), 1);
    }

    #[test]
    fn asking_past_the_end_is_an_empty_answer_and_not_a_failure() {
        // What a node that is already up to date gets, which is the common case and must not look
        // like anything going wrong.
        let node = opened();
        assert!(
            node.since(u64::from(u32::MAX), a_page(100), Epoch::GENESIS)
                .answer
                .is_empty()
        );
    }

    #[test]
    fn a_node_joins_a_network_it_was_handed_and_becomes_one_of_its_nodes() {
        // The other half of opening. A node that could only open would be a network of one.
        let scratch = Scratch::new("joins");
        let mut first = opened_by(6);
        first
            .submit(&an_account(&key(9), Epoch::GENESIS), Epoch::GENESIS)
            .expect("taken");

        let handed = first.since(0, a_page(100), Epoch::GENESIS).answer;
        let joined = Node::join(
            &scratch.0,
            key(7),
            Joining {
                acts: &handed,
                network: first.network().as_str(),
            },
            Epoch::GENESIS,
        )
        .expect("joined");

        assert_eq!(joined.network(), first.network(), "the same network");
        assert_ne!(joined.did(), first.did(), "and a different node on it");
        assert_eq!(
            joined.written(),
            first.written() + 1,
            "everything it was handed, and its own announcement on top"
        );
    }

    #[test]
    fn a_node_that_joined_holds_what_it_was_handed() {
        let scratch = Scratch::new("holds");
        let mut first = opened_by(6);
        let account = an_account(&key(9), Epoch::GENESIS);
        let named = account.object.name().clone();
        first.submit(&account, Epoch::GENESIS).expect("taken");

        let joined = Node::join(
            &scratch.0,
            key(7),
            Joining {
                acts: &first.since(0, a_page(100), Epoch::GENESIS).answer,
                network: first.network().as_str(),
            },
            Epoch::GENESIS,
        )
        .expect("joined");

        assert!(matches!(
            joined.resolve(&named, Epoch::GENESIS).answer,
            Answer::Here(_)
        ));
    }

    #[test]
    fn a_node_that_joined_comes_back_to_the_same_network_without_asking_again() {
        // What being written down as it arrives is for: the second start is a rejoin from disk,
        // not a second trip to somebody else's node.
        let scratch = Scratch::new("joined-again");
        let first = opened_by(6);
        let joined = Node::join(
            &scratch.0,
            key(7),
            Joining {
                acts: &first.since(0, a_page(100), Epoch::GENESIS).answer,
                network: first.network().as_str(),
            },
            Epoch::GENESIS,
        )
        .expect("joined");
        let (network, did, written) = (
            joined.network().clone(),
            joined.did().clone(),
            joined.written(),
        );
        drop(joined);

        let back = Node::rejoin(&scratch.0, key(7)).expect("its own record");
        assert_eq!(back.network(), &network);
        assert_eq!(back.did(), &did);
        assert_eq!(back.written(), written);
    }

    #[test]
    fn nothing_that_does_not_open_a_network_can_be_joined() {
        // A node handed acts that begin with anything else is a node with no network to be on, and
        // guessing one from the rest would be inventing a trust anchor.
        let scratch = Scratch::new("no-genesis");
        let account = an_account(&key(9), Epoch::GENESIS);
        assert!(
            Node::join(
                &scratch.0,
                key(7),
                Joining {
                    acts: &[account.to_bytes()],
                    network: account.object.name().as_str(),
                },
                Epoch::GENESIS
            )
            .is_err()
        );
    }

    #[test]
    fn a_proof_comes_back_with_the_signed_root_it_is_against() {
        // **The whole point.** A path proves an entry against a root of a stated size — so without
        // the root, and without the node's name on it, it proves nothing to whoever received it.
        let mut node = opened();
        let account = an_account(&key(9), Epoch::GENESIS);
        let name = account.called();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        // A later epoch, because opening a network closes the first one — and closing an epoch
        // twice hands back what this node already stands by rather than a fresh root.
        let next = Epoch::GENESIS.plus(Epochs(1)).expect("no overflow");
        node.close(next);

        let answered = node.inclusion_in(&name, next, Epoch::GENESIS);
        let (at, path, published) = answered.answer.expect("a proof");

        assert_eq!(
            published.accept(node.network(), &node.key()),
            Ok(()),
            "the root it is against is this node's own word"
        );

        let entry = almena_format::entry::Entry::of(&account, at, None);
        assert!(
            almena_store::tree::included(
                &entry.to_bytes(),
                at as usize,
                published.root.size as usize,
                &path,
                &published.root.root
            ),
            "and the path carries it to exactly that root, at exactly that size"
        );
    }

    #[test]
    fn there_is_no_proof_against_an_epoch_this_node_never_closed() {
        // Inventing one would be putting this node's name on a root it never published.
        let mut node = opened();
        let account = an_account(&key(9), Epoch::GENESIS);
        let name = account.called();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let later = Epoch::GENESIS.plus(Epochs(9)).expect("no overflow");
        assert!(
            node.inclusion_in(&name, later, Epoch::GENESIS)
                .answer
                .is_none()
        );
    }

    #[test]
    fn an_act_that_arrived_after_an_epoch_closed_is_not_in_that_epoch() {
        // It genuinely is not in that tree, and a proof saying otherwise would be a lie that
        // checked out against nothing.
        let mut node = opened();
        node.close(Epoch::GENESIS);

        let account = an_account(&key(9), Epoch::GENESIS);
        let name = account.called();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        assert!(
            node.inclusion_in(&name, Epoch::GENESIS, Epoch::GENESIS)
                .answer
                .is_none()
        );
    }

    #[test]
    fn nothing_is_joined_that_is_not_the_network_it_was_promised_to_be() {
        // **The one check that has to happen before anything else.** A node that replayed first
        // and asked afterwards would already have written somebody else's network to disk and
        // announced itself on it, with their key as the anchor everything is trusted from.
        let scratch = Scratch::new("promised");
        let elsewhere = opened_by(6);

        assert_eq!(
            Node::join(
                &scratch.0,
                key(7),
                Joining {
                    acts: &elsewhere.since(0, a_page(100), Epoch::GENESIS).answer,
                    network: "zQmSomeOtherNetworkEntirely",
                },
                Epoch::GENESIS,
            )
            .err(),
            Some(record::NotReadable::AnotherNetwork)
        );
        assert!(
            !record::acts_at(&scratch.0).exists(),
            "and nothing was written down before the question was asked"
        );
    }

    #[test]
    fn a_summary_that_leaves_out_a_governing_act_falls_over_against_the_record() {
        // **What makes a summary safe to sign with a routine key.** Signing it stops anybody else
        // forging one; it does not stop the object leaving something out — and that is caught by
        // looking at the record, which every node holds and nobody has to be asked for.
        use almena_store::checkpoint::{Claim, Governs, Placed, Stated, Verdict, branch, left_out};

        let mut node = opened();
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let created = account.called();
        let added = a_device(&object, &created, &control);
        node.submit(&added, Epoch::GENESIS).expect("taken");

        let entries = node.chain_of(&object, Epoch::GENESIS).answer;
        let held: Vec<&almena_format::entry::Entry> = entries.iter().collect();
        let carrier = added.called();
        let walked = branch(&held, &carrier);
        let placed = Placed {
            carrier: &carrier,
            branch: &walked,
        };
        let device = Stated::Keys(std::collections::BTreeSet::from([
            almena_suite::p256::SigningKey::from_secret([4; 32])
                .expect("a key")
                .verifying_key()
                .bytes()
                .to_vec(),
        ]));

        // A summary that says the devices were last set when the account was created — which was
        // true, and stopped being true one act later.
        let hiding = Claim {
            about: Governs::Devices,
            stated: device.clone(),
            set_by: created,
        };
        assert_eq!(
            left_out(&hiding, placed),
            Verdict::LeftOut(carrier.clone()),
            "the record says otherwise, and says which act it left out"
        );

        // And the honest one, citing the act that really did set them last.
        let honest = Claim {
            about: Governs::Devices,
            stated: device,
            set_by: carrier.clone(),
        };
        assert_eq!(left_out(&honest, placed), Verdict::Stands);
    }

    #[test]
    fn a_summary_that_makes_a_value_up_falls_over_against_the_record_too() {
        // **The half that citing an act cannot cover.** The summary names the right last act, the
        // record has nothing later, and the value is one nothing ever produced. What settles it is
        // the acts themselves — which the record hands over, signed by their author.
        use almena_store::checkpoint::{Claim, Governs, Placed, Stated, Verdict, branch, holds_up};

        let mut node = opened();
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let created = account.called();
        let added = a_device(&object, &created, &control);
        node.submit(&added, Epoch::GENESIS).expect("taken");

        let entries = node.chain_of(&object, Epoch::GENESIS).answer;
        let held: Vec<&almena_format::entry::Entry> = entries.iter().collect();
        let carrier = added.called();
        let walked = branch(&held, &carrier);
        let acts = [&account, &added];

        let lying = Claim {
            about: Governs::Devices,
            stated: Stated::Keys(std::collections::BTreeSet::from([vec![99; 33]])),
            set_by: carrier.clone(),
        };
        assert_eq!(
            holds_up(
                &lying,
                Placed {
                    carrier: &carrier,
                    branch: &walked
                },
                &acts,
                Epoch::GENESIS
            ),
            Verdict::Fabricated,
            "a device nobody ever added is not one the record will carry"
        );
    }

    /// Another node, announced into this one's record, so that it is somebody the record can name.
    fn somebody_else(node: &mut Node, seed: u8, now: Epoch) -> (Did, [u8; 32]) {
        let their_key = key(seed);
        let announced = almena_store::announce::announce(Which::Development, now, &their_key);
        node.submit(&announced.operation, now).expect("announced");
        (announced.node, their_key.verifying_key().bytes())
    }

    /// One thing seen, written down as it happened.
    fn saw(
        of: [u8; 32],
        at: Epoch,
        saw: almena_store::watching::Saw,
    ) -> almena_store::watching::Noted {
        almena_store::watching::Noted {
            of: of.to_vec(),
            at,
            saw,
        }
    }

    /// A day's summary with nothing said about the looking, which names nobody either way.
    const fn watched() -> Watched {
        Watched {
            looked: almena_store::summary::Looked {
                asked_for: 0,
                found: 0,
            },
        }
    }

    #[test]
    fn a_node_writes_down_what_it_saw_of_others_and_never_of_itself() {
        // **The whole reason cross-observation exists.** What a node claims about its own uptime is
        // worth nothing; what is worth something is that the nodes which kept asking it wrote down
        // whether it answered.
        let mut node = opened();
        let day = almena_time::Day::new(1);
        let during = Epoch::new(almena_time::EPOCHS_PER_DAY);
        let (somebody, theirs) = somebody_else(&mut node, 7, during);
        let mine = key(6).verifying_key().bytes();

        use almena_store::watching::Saw;
        for what in [
            Saw::Asked,
            Saw::Answered,
            Saw::Asked,
            Saw::Answered,
            Saw::Asked,
            Saw::Behind(3),
        ] {
            node.watched(day, saw(theirs, during, what));
        }
        // And about itself, which must not reach the summary however it got written down.
        node.watched(day, saw(mine, during, Saw::Asked));

        // A day that is over. One still happening compares with nothing.
        let after = Epoch::new(almena_time::EPOCHS_PER_DAY * 2);
        assert!(node.summarise(day, watched(), after));

        let chain = node.chain_of(node.did(), after).answer;
        let summary = chain
            .iter()
            .find(|entry| entry.kind == Kind::NODE_SUMMARY.number())
            .expect("it is in this node's own chain and nobody else's");

        let act = node.act(&summary.hash, after).answer.expect("the act");
        let read = almena_format::cbor::read(&act).ok().and_then(|value| {
            almena_format::operation::read(&value)
                .as_ref()
                .and_then(almena_store::summary::read)
        });
        let (said, behind, seen, _) = read.expect("a summary");

        assert_eq!(said, day);
        assert_eq!(seen.len(), 1);
        let of_them = seen.get(&somebody).expect("what it saw of them");
        assert_eq!(
            (of_them.asked, of_them.answered, of_them.behind),
            (3, 2, 3),
            "worked out from the askings themselves"
        );
        assert!(
            !seen.contains_key(node.did()),
            "and it said nothing about itself"
        );

        // **The hash is over the observations and not over the figures.** A hash of what is being
        // published would check out against the act carrying it whatever it said.
        assert_eq!(
            behind,
            node.watching(day).expect("it kept them").digest(),
            "and what it pins is what this node can still hand over"
        );
    }

    #[test]
    fn a_summary_pins_the_observations_and_never_the_figures_it_publishes() {
        // The failure the old mechanism could not catch: an observer that watched nobody passed
        // exactly as well as one that watched everybody, because the hash was over the aggregate
        // the act carried and checking it against that act always succeeded.
        let mut node = opened();
        let day = almena_time::Day::new(1);
        let during = Epoch::new(almena_time::EPOCHS_PER_DAY);
        let (_, theirs) = somebody_else(&mut node, 7, during);

        // Nothing watched: nothing to summarise, and nothing a hash could pin.
        assert!(!node.summarise(day, watched(), Epoch::new(almena_time::EPOCHS_PER_DAY * 2)));

        node.watched(day, saw(theirs, during, almena_store::watching::Saw::Asked));
        let held = node.watching(day).expect("it kept them");
        assert_eq!(held.len(), 1);
        assert_ne!(
            held.digest(),
            almena_store::watching::Watching::new().digest(),
            "an empty day and a watched one do not hash alike"
        );
    }

    #[test]
    fn only_a_few_days_of_observations_are_kept() {
        // They never enter the record and nothing else ages them out. What they are for is checking
        // a summary published recently, and a day older than that is one this node stops offering.
        let mut node = opened();
        let during = Epoch::new(almena_time::EPOCHS_PER_DAY);
        let (_, theirs) = somebody_else(&mut node, 7, during);

        for day in 0..(super::DAYS_WATCHED as u64 + 4) {
            node.watched(
                almena_time::Day::new(day),
                saw(theirs, during, almena_store::watching::Saw::Asked),
            );
        }
        assert!(node.watching(almena_time::Day::new(0)).is_none());
        assert!(
            node.watching(almena_time::Day::new(super::DAYS_WATCHED as u64 + 3))
                .is_some()
        );
    }

    #[test]
    fn a_day_still_happening_is_not_summarised() {
        let mut node = opened();
        let day = almena_time::Day::new(0);
        let (_, theirs) = somebody_else(&mut node, 7, Epoch::GENESIS);
        node.watched(
            day,
            saw(theirs, Epoch::GENESIS, almena_store::watching::Saw::Asked),
        );

        assert!(!node.summarise(day, watched(), Epoch::new(23)));
        assert!(node.summarise(day, watched(), Epoch::new(24)));
    }

    #[test]
    fn a_node_is_not_called_what_its_network_is_called() {
        // The one that would go unnoticed. Both are names this node holds, both are the hash of an
        // act, and the wrong one costs nothing until roots are compared — at which point every
        // honest pair of nodes on a network looks like one node saying two things about an epoch.
        let node = opened();
        assert_ne!(node.did(), node.government());
        assert_ne!(node.did().name(), node.network());
    }

    #[test]
    fn two_nodes_on_one_network_do_not_contradict_each_other() {
        // They have different trees by design; that is what having more than one node is for.
        let mut mine = opened_by(6);
        let mut theirs = opened_by(7);
        assert_eq!(mine.network(), theirs.network(), "one network");
        assert_ne!(mine.did(), theirs.did(), "two nodes");
        theirs
            .submit(&an_account(&key(9), Epoch::GENESIS), Epoch::GENESIS)
            .expect("taken");

        let one = mine.close(Epoch::GENESIS);
        let other = theirs.close(Epoch::GENESIS);
        assert_ne!(one.root, other.root, "they wrote down different things");
        assert!(
            !almena_store::root::contradict(&one, &other),
            "and neither of them is misconduct"
        );
    }

    #[test]
    fn one_node_saying_two_things_about_an_epoch_is_still_caught() {
        // The check above must not have been bought by making contradiction impossible.
        let mut node = opened();
        let honest = node.close(Epoch::GENESIS);

        node.submit(&an_account(&key(9), Epoch::GENESIS), Epoch::GENESIS)
            .expect("taken");
        let mut second = honest.clone();
        second.root = node.root_now();
        second.size = node.written() as u64;

        assert!(almena_store::root::contradict(&honest, &second));
    }

    #[test]
    fn a_node_will_not_open_a_network_when_there_is_one_to_join() {
        let seeds = vec!["/dns/madrid.example/tcp/443".to_owned()];
        let outcome = Node::open(&at(Which::Production), &seeds, &key(5), key(6));
        assert_eq!(outcome.err(), Some(Refused::ThereIsAlreadyANetwork(seeds)));
    }

    #[test]
    fn the_trust_anchor_resolves_from_the_moment_the_network_opens() {
        let node = opened();
        let answered = node.resolve(node.government().name(), Epoch::GENESIS);
        assert!(matches!(
            answered.answer,
            Answer::Here(State::Government { .. })
        ));
    }

    #[test]
    fn every_answer_says_what_it_was_true_at() {
        // Two consultations that do not say what they were computed against are not comparable,
        // and a listing that does not say it is not consistent even with itself. So the stamp is
        // on every kind of answer, not on the convenient ones.
        let node = opened();
        let asked = Epoch::GENESIS.plus(Epochs(42)).expect("no overflow");
        let anchor = node.government().name().clone();
        let missing = Name::of(b"never happened");

        let stamps = [
            (
                node.resolve(&anchor, asked).epoch,
                node.resolve(&anchor, asked).root,
            ),
            (
                node.act(&missing, asked).epoch,
                node.act(&missing, asked).root,
            ),
            (
                node.about(node.government(), asked).epoch,
                node.about(node.government(), asked).root,
            ),
            (
                node.inclusion(&missing, asked).epoch,
                node.inclusion(&missing, asked).root,
            ),
        ];

        for (epoch, root) in stamps {
            assert_eq!(epoch, asked, "the epoch it was asked at");
            assert_eq!(
                root,
                node.root_at(Epoch::GENESIS).expect("closed").root,
                "the root over everything written down"
            );
        }
    }

    #[test]
    fn writing_something_moves_the_root_that_answers_carry() {
        let mut node = opened();
        let anchor = node.government().name().clone();

        let first = node.resolve(&anchor, Epoch::GENESIS);
        let account = an_account(&key(9), Epoch::GENESIS);
        node.submit(&account, Epoch::GENESIS).expect("taken");
        let later = node.resolve(&anchor, Epoch::GENESIS);

        assert_ne!(first.root, later.root, "the record grew");
        assert_eq!(first.answer, later.answer, "and the anchor did not change");
    }

    #[test]
    fn an_account_can_be_created_and_then_resolved() {
        // The two halves of what a node is for, meeting: somebody hands over a signed act, and can
        // then ask this node about what it created.
        let mut node = opened();
        let account = an_account(&key(9), Epoch::GENESIS);
        let name = account.object.name().clone();

        node.submit(&account, Epoch::GENESIS).expect("taken");
        match node.resolve(&name, Epoch::GENESIS).answer {
            Answer::Here(State::Holder(holder)) => {
                assert_eq!(holder.control, key(9).verifying_key().bytes());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn what_has_been_said_about_somebody_is_a_question_with_an_answer() {
        // It was not, and an empty list would then have read as *nobody has said anything about
        // this node* — a claim, and a false one, because nobody had been able to. Now a
        // contradiction says who it is against, so nothing said is a true answer and not a silence.
        let node = opened();
        assert_eq!(
            node.about(node.government(), Epoch::GENESIS).answer,
            Some(Vec::new()),
            "nothing has been said about it, which is a fact and not an absence of one"
        );
    }

    #[test]
    fn an_object_nobody_has_heard_of_does_not_exist() {
        let node = opened();
        assert_eq!(
            node.resolve(&Name::of(b"never happened"), Epoch::GENESIS)
                .answer,
            Answer::DoesNotExist
        );
    }

    #[test]
    fn an_act_comes_back_in_its_own_bytes_and_unsigned_by_this_node() {
        // A node is a messenger. An answer somebody has to take this node's word for is not an
        // answer at all.
        let mut node = opened();
        let account = an_account(&key(9), Epoch::GENESIS);
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let hash = account.called();
        assert_eq!(
            node.act(&hash, Epoch::GENESIS).answer,
            Some(account.to_bytes())
        );
    }

    #[test]
    fn an_act_can_prove_where_this_node_wrote_it_down() {
        let mut node = opened();
        let account = an_account(&key(9), Epoch::GENESIS);
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let hash = account.called();
        let answered = node.inclusion(&hash, Epoch::GENESIS);
        let (at, path) = answered.answer.expect("it is in there");
        assert_eq!(at, 2, "after opening the network and joining it");

        let entry = almena_format::entry::Entry::of(&account, at, None);
        assert!(almena_store::tree::included(
            &entry.to_bytes(),
            at as usize,
            node.written(),
            &path,
            &answered.root
        ));
    }

    #[test]
    fn an_epoch_is_closed_whether_anything_happened_or_not() {
        // A node that only published when there was something to publish would leave gaps meaning
        // either *nothing happened* or *I was not here*.
        let mut node = opened();
        for epoch in 1..=4 {
            node.close(Epoch::GENESIS.plus(Epochs(epoch)).expect("no overflow"));
        }
        let through = Epoch::GENESIS.plus(Epochs(4)).expect("no overflow");
        assert!(node.missing(through).is_empty(), "no gaps");

        let quiet = node.root_at(Epoch::GENESIS.plus(Epochs(3)).expect("no overflow"));
        assert_eq!(
            quiet.expect("closed").size,
            2,
            "nothing happened since it opened, and it said so"
        );
    }

    #[test]
    fn a_node_signs_the_roots_it_publishes() {
        let mut node = opened();
        let root = node.close(Epoch::GENESIS.plus(Epochs(1)).expect("no overflow"));
        let published = node.publish(&root);
        assert_eq!(published.accept(node.network(), &node.key()), Ok(()));
    }

    #[test]
    fn an_act_from_the_future_is_not_taken() {
        let mut node = opened();
        let ahead = Epoch::GENESIS.plus(Epochs(5)).expect("no overflow");
        let account = an_account(&key(9), ahead);
        assert_eq!(
            node.submit(&account, Epoch::GENESIS).err(),
            Some(almena_store::chain::Refused::FromTheFuture)
        );
    }

    #[test]
    fn an_object_whose_history_this_build_cannot_read_stops_resolving() {
        let mut node = opened();
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let name = account.object.name().clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let head = account.called();
        let mut newer = almena_format::operation::Operation {
            object: account.object.clone(),
            previous: Some(head),
            kind: 9_999,
            version: 1,
            issued: Epoch::GENESIS,
            payload: BTreeMap::new(),
            signatures: Vec::new(),
        };
        let signature = control.sign(&newer.signing_bytes());
        newer.signatures.push(Signed {
            by: account.object.clone(),
            key: control.verifying_key().bytes().to_vec(),
            signature: signature.bytes(),
        });

        node.submit(&newer, Epoch::GENESIS).expect("stored anyway");
        assert_eq!(
            node.resolve(&name, Epoch::GENESIS).answer,
            Answer::CannotResolve(Reason::Unintelligible)
        );
    }

    #[test]
    fn asking_what_an_object_is_hands_back_the_summary_and_what_followed() {
        // **The whole reason a summary exists.** Whoever arrives takes the summary and what came
        // after it and stops, instead of replaying a history that only ever grows.
        let mut node = opened();
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");
        let head = a_busy_account(&mut node, &object, &control, 8);

        let everything = composed(&node, &object, a_page(100), Epoch::GENESIS);
        assert_eq!(
            everything.len(),
            17,
            "with no summary, the whole chain is what it takes"
        );

        // Now it summarises, and the same question costs one act. Written once the askings above
        // have come due: nothing summarises over something in flight, because a summary has
        // nowhere to say it is there.
        let settled = Epoch::GENESIS.plus(Epochs(72)).expect("no overflow");
        let standing = node.standing(&object, settled).answer.expect("it resolves");
        let summary = a_summary(&object, &head, &control, &standing.claims, settled);
        node.submit(&summary, settled).expect("taken");

        let after = composed(&node, &object, a_page(100), settled);
        assert_eq!(after.len(), 1, "the summary, and nothing behind it");
        assert_eq!(after[0], summary.to_bytes());

        // And one more act puts exactly one more thing on the pile. Dated where it sits: an act
        // may not be dated before the act it follows, and the summary above is at `settled`.
        let mut another = a_device(&object, &summary.called(), &control);
        another.issued = settled;
        another.signatures.clear();
        let another = signed(another, &control);
        node.submit(&another, settled).expect("taken");
        assert_eq!(composed(&node, &object, a_page(100), settled).len(), 2);
    }

    #[test]
    fn the_summary_a_node_offers_stands_up_against_its_own_record() {
        // Anything else would be a node handing out something it knew would fall over. It does not,
        // because what it offers is built from the chain it replayed and from nothing else.
        use almena_store::checkpoint;

        let mut node = opened();
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let added = a_device(&object, &account.called(), &control);
        node.submit(&added, Epoch::GENESIS).expect("taken");

        // Once the asking above has come due: a summary has nowhere to say what is in flight, so
        // nothing summarises over one.
        let settled = Epoch::GENESIS.plus(Epochs(72)).expect("no overflow");
        let standing = node.standing(&object, settled).answer.expect("it resolves");
        let summary = a_summary(
            &object,
            &added.called(),
            &control,
            &standing.claims,
            settled,
        );
        node.submit(&summary, settled).expect("taken");

        let entries = node.chain_of(&object, settled).answer;
        let held: Vec<&almena_format::entry::Entry> = entries.iter().collect();
        let carrier = summary.called();
        let walked = checkpoint::branch(&held, &carrier);

        let fell = checkpoint::falls_over(
            &standing.claims,
            checkpoint::Placed {
                carrier: &carrier,
                branch: &walked,
            },
            &[&account, &added, &summary],
            // Checked at the moment the summary was written, which is what a summary is *of*.
            // Replaying at the genesis would judge a claim about a landed device against a chain
            // whose asking had not landed yet — the summary and the check disagreeing about the
            // time rather than about the account.
            settled,
        );
        assert!(fell.is_empty(), "{fell:?}");
    }

    #[test]
    fn a_node_says_how_far_behind_an_object_is_and_refuses_nothing_over_it() {
        // A summary does not benefit whoever writes it, so what depends on goodwill when somebody
        // else pays gets done seldom and late. The node makes the debt visible; it does not police
        // it — the number can change, and a node that refused over it would refuse what a node on
        // another version keeps.
        let mut node = opened();
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let every = almena_store::parameter::SUMMARISE_EVERY.now();
        a_busy_account(
            &mut node,
            &object,
            &control,
            usize::try_from(every).expect("a small number"),
        );

        let standing = node
            .standing(&object, Epoch::GENESIS)
            .answer
            .expect("it resolves");
        assert!(standing.owed);
        assert!(standing.since > every);
    }

    #[test]
    fn a_chain_that_split_hands_over_everything_it_wrote() {
        // There is no branch to follow, so there is nothing to trim to: whoever is looking at a
        // split needs to see both sides, and picking one would be the thing no node may do.
        let mut node = opened();
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let created = account.called();
        let one = a_device(&object, &created, &control);
        node.submit(&one, Epoch::GENESIS).expect("taken");

        // A second act claiming the same predecessor, which somebody with the right to sign made.
        let other = a_removal(&object, &created, &control, &[5; 33]);
        node.submit(&other, Epoch::GENESIS)
            .expect("kept, and it splits the chain");

        assert!(matches!(
            node.resolve(object.name(), Epoch::GENESIS).answer,
            almena_store::chain::Answer::CannotResolve(almena_store::chain::Reason::Forked)
        ));
        assert_eq!(
            composed(&node, &object, a_page(100), Epoch::GENESIS).len(),
            3,
            "the creation and both acts that claim it"
        );
    }

    #[test]
    fn a_node_that_will_not_say_what_an_object_is_still_hands_over_its_acts() {
        // Denying service is allowed and lying is not, and handing over signed materials is neither:
        // an object this node cannot read is one whoever asks may still be able to.
        let mut node = opened();
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let newer = signed(
            almena_format::operation::Operation {
                object: object.clone(),
                previous: Some(account.called()),
                // **A kind this build numbers and has no meaning for on this object.** An act
                // from another class of thing entirely, which is what an act from a newer version
                // looks like from here — and unlike a hole in this build's own vocabulary, it does
                // not stop being one as the holes get filled.
                kind: Kind::ENTITY_ADD_OWNER.number(),
                version: 1,
                issued: Epoch::GENESIS,
                payload: BTreeMap::from([(1, Value::Bytes(vec![3; 33]))]),
                signatures: Vec::new(),
            },
            &control,
        );
        node.submit(&newer, Epoch::GENESIS)
            .expect("kept and passed on");

        assert!(matches!(
            node.resolve(object.name(), Epoch::GENESIS).answer,
            almena_store::chain::Answer::CannotResolve(_)
        ));
        assert_eq!(
            composed(&node, &object, a_page(100), Epoch::GENESIS).len(),
            2,
            "and the acts are still there to be worked out from"
        );
        assert_eq!(
            node.standing(&object, Epoch::GENESIS).answer,
            None,
            "but it will not offer a summary of a history it cannot follow"
        );
    }

    #[test]
    fn an_act_heard_twice_is_written_down_once() {
        // A second copy in the record would be a second position in the tree, a second line in the
        // log, and — because the record is what a restart replays — a duplicate that came back
        // every morning.
        let mut node = opened();
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let added = a_device(&object, &account.called(), &control);
        node.submit(&added, Epoch::GENESIS).expect("taken");
        let written = node.written();
        let root = node.root_now();

        assert_eq!(
            node.submit(&added, Epoch::GENESIS)
                .expect("already here")
                .answer,
            almena_store::chain::Admitted::AlreadyHere
        );
        assert_eq!(node.written(), written, "nothing was written down again");
        assert_eq!(node.root_now(), root, "and the tree did not move");
        assert!(matches!(
            node.resolve(object.name(), Epoch::GENESIS).answer,
            almena_store::chain::Answer::Here(_)
        ));
    }

    #[test]
    fn a_node_works_out_its_own_share_from_the_record_and_nothing_else() {
        // **What makes a node that has not got its share visibly short.** The rule is computed the
        // same way by everybody from what everybody holds, so being behind is a fact somebody can
        // check rather than an accusation somebody has to be believed about.
        let mut node = opened();
        let mut announced = vec![node.did().name().clone()];
        for seed in 20u8..30 {
            let other =
                almena_store::announce::announce(Which::Development, Epoch::GENESIS, &key(seed));
            node.submit(&other.operation, Epoch::GENESIS)
                .expect("taken");
            announced.push(other.node.name().clone());
        }

        let (network, census) = node.share_out(Epoch::GENESIS);
        assert_eq!(census.len(), announced.len(), "every node it has heard of");

        // A thing falls to five of the eleven, and this node's own answer agrees with the rule.
        let thing = Name::of(b"a stretch of history");
        let drawn = almena_store::share::Drawn::at(&network, Epoch::GENESIS, &census);
        let holders = drawn.holders(&thing, COPIES_OF_HISTORY);
        assert_eq!(holders.len(), 5);

        assert_eq!(
            node.falls_to_me(&thing, COPIES_OF_HISTORY, Epoch::GENESIS),
            holders.contains(&node.did().name())
        );
        assert_eq!(
            node.holders_of(&thing, COPIES_OF_HISTORY, Epoch::GENESIS)
                .answer
                .len(),
            5
        );
    }

    #[test]
    fn a_node_on_its_own_is_the_whole_share() {
        // Day one. Asking for five when there is one would make the shortfall a number about how
        // small the network is rather than about anything anybody could fix.
        let node = opened();
        let thing = Name::of(b"a stretch of history");

        assert!(node.falls_to_me(&thing, COPIES_OF_HISTORY, Epoch::GENESIS));
        assert_eq!(
            node.holders_of(&thing, COPIES_OF_HISTORY, Epoch::GENESIS)
                .answer,
            vec![node.did().clone()]
        );
    }

    #[test]
    fn two_nodes_holding_the_same_record_share_it_out_the_same_way() {
        // The property everything else rests on. Two share-outs that disagreed would be two
        // opinions, and a node short of its share could always say somebody else had it.
        let mut one = opened();
        let mut other = opened();
        let announcements: Vec<_> = (40u8..48)
            .map(|seed| {
                almena_store::announce::announce(Which::Development, Epoch::GENESIS, &key(seed))
            })
            .collect();

        for announced in &announcements {
            one.submit(&announced.operation, Epoch::GENESIS)
                .expect("taken");
        }
        // The other hears about them in the opposite order.
        for announced in announcements.iter().rev() {
            other
                .submit(&announced.operation, Epoch::GENESIS)
                .expect("taken");
        }

        for which in 0..40 {
            let thing = Name::of(format!("thing {which}").as_bytes());
            let (mine, my_census) = one.share_out(Epoch::GENESIS);
            let (theirs, their_census) = other.share_out(Epoch::GENESIS);
            assert_eq!(
                almena_store::share::Drawn::at(&mine, Epoch::GENESIS, &my_census)
                    .holders(&thing, COPIES_OF_HISTORY),
                almena_store::share::Drawn::at(&theirs, Epoch::GENESIS, &their_census)
                    .holders(&thing, COPIES_OF_HISTORY),
                "{thing:?}"
            );
        }
    }

    #[test]
    fn a_node_adds_up_what_every_observer_said_about_a_day() {
        // **The figure anybody arrives at.** It is a sum over signed acts in a record everybody
        // holds, not an assertion by whoever runs anything — which is what makes a shortfall
        // something a third party can check rather than a number somebody publishes about itself.
        let mut node = opened();
        let day = almena_time::Day::new(0);
        let after = Epoch::new(almena_time::EPOCHS_PER_DAY);

        // Nobody has said anything yet, and that is an absence of evidence and not good health.
        let nothing = node.kept(day, after).answer;
        assert_eq!(nothing.observers, 0);
        assert_eq!(nothing.asked_for, 0);

        // This node writes down its own day, drawn from what it actually watched.
        let (_, theirs) = somebody_else(&mut node, 7, Epoch::GENESIS);
        node.watched(
            day,
            saw(theirs, Epoch::GENESIS, almena_store::watching::Saw::Asked),
        );
        assert!(node.summarise(
            day,
            Watched {
                looked: almena_store::summary::Looked {
                    asked_for: 12,
                    found: 11,
                },
            },
            after
        ));

        let kept = node.kept(day, after).answer;
        assert_eq!(kept.observers, 1);
        assert_eq!(kept.asked_for, 12);
        assert_eq!(kept.found, 11);

        // And a different day is a different figure, drawn from nobody.
        assert_eq!(
            node.kept(almena_time::Day::new(1), after).answer.observers,
            0
        );
    }

    #[test]
    fn a_contradiction_is_found_by_the_node_it_is_against() {
        // **Which is the whole reason it says who it is against.** It is looked for by the party
        // affected, not by whoever bothered to write it down — and until it could be, what one node
        // discovered nobody else could find.
        let mut node = opened();
        let against = key(3);
        let node_did = Did::new(Network::Development, Name::of(b"the node that did it"));

        let a_root = |over: &[u8]| {
            almena_store::root::Root {
                network: node.network().clone(),
                node: node_did.clone(),
                epoch: Epoch::GENESIS,
                size: 4,
                root: almena_suite::digest::Digest::of(over),
            }
            .publish(&against)
        };
        let (one, other) = (a_root(b"one history"), a_root(b"another history"));

        assert!(
            node.write_down(&one, &other, Epoch::GENESIS),
            "evidence anybody can check"
        );

        // Found by who it is against, and not by anything else.
        let about = node
            .about(&node_did, Epoch::GENESIS)
            .answer
            .expect("a question with an answer");
        assert_eq!(about.len(), 1, "the contradiction, indexed by the affected");
        assert!(
            node.about(node.government(), Epoch::GENESIS)
                .answer
                .is_some_and(|said| said.is_empty()),
            "and nothing has been said about anybody else"
        );
    }

    #[test]
    fn the_record_says_which_keys_have_signed_two_histories() {
        // Said by key and not by name, because that is what the evidence establishes: two
        // signatures. Whether the key belongs to a node anybody has heard of is a different
        // question, answered by resolving that node's own name.
        let mut node = opened();
        let against = key(4);
        let node_did = Did::new(
            Network::Development,
            Name::of(b"whoever it turns out to be"),
        );

        let a_root = |over: &[u8]| {
            almena_store::root::Root {
                network: node.network().clone(),
                node: node_did.clone(),
                epoch: Epoch::GENESIS,
                size: 9,
                root: almena_suite::digest::Digest::of(over),
            }
            .publish(&against)
        };

        let public = against.verifying_key().bytes();
        assert!(!node.contradicted(&public), "nothing is proved yet");

        assert!(node.write_down(&a_root(b"one"), &a_root(b"the other"), Epoch::GENESIS));
        assert!(node.contradicted(&public));
        assert!(
            !node.contradicted(&key(5).verifying_key().bytes()),
            "and it says nothing about anybody else"
        );
    }

    #[test]
    fn a_node_joining_writes_down_what_it_took_and_not_what_it_was_handed() {
        // **The worst thing a name that leaves out the signature could have done.** Already-held is
        // answered before any signature is looked at — that is what makes it cheap — so a copy of a
        // real act carrying a signature nobody made shares its name, comes back *already here*, and
        // used to be written down anyway: bytes nobody checked, under a name this node vouches for,
        // taking over the one it serves. And with no forgery at all, one act written twice is one
        // act with two leaves in the tree.
        let mut seed = opened();
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        seed.submit(&account, Epoch::GENESIS).expect("taken");
        let added = a_device(&account.object, &account.called(), &control);
        seed.submit(&added, Epoch::GENESIS).expect("taken");

        let handed = seed.since(0, a_page(100), Epoch::GENESIS).answer;
        let honest = handed.len();

        // A copy of the last act with its signature replaced by nothing anybody signed.
        let mut forged = added.clone();
        forged.signatures[0].signature = [0; 64];
        assert_eq!(
            forged.called(),
            added.called(),
            "one act, whatever was printed as its signature"
        );

        let mut offered = handed.clone();
        offered.push(forged.to_bytes());
        offered.push(added.to_bytes());

        let network = seed.network().as_str().to_owned();
        let scratch = Scratch::new("joined");
        let joined = Node::join(
            &scratch.0,
            key(11),
            Joining {
                acts: &offered,
                network: &network,
            },
            Epoch::GENESIS,
        )
        .expect("joined");

        assert_eq!(
            joined.written(),
            honest + 1,
            "the acts it took, plus its own announcement — and neither copy"
        );
        assert_eq!(
            joined.act(&added.called(), Epoch::GENESIS).answer,
            Some(added.to_bytes()),
            "and what it serves under that name is what was signed"
        );
    }

    #[test]
    fn a_node_can_know_an_act_happened_without_holding_what_it_said() {
        // **The arrangement that replaces everybody keeping everything.** The line saying an act
        // happened is universal; what it said belongs to the nodes it was dealt to. Until now a
        // node could not tell the two apart, so it had to keep all of it for ever.
        let mut node = opened();
        let account = an_account(&key(9), Epoch::GENESIS);
        let elsewhere = account.object.clone();
        let entry = almena_format::entry::Entry::of(&account, 0, None);

        assert_eq!(
            node.resolve(elsewhere.name(), Epoch::GENESIS).answer,
            almena_store::chain::Answer::DoesNotExist
        );

        node.note(&entry);
        assert_eq!(
            node.resolve(elsewhere.name(), Epoch::GENESIS).answer,
            almena_store::chain::Answer::NotHere,
            "it exists and is held elsewhere, which is one more question and not an absence"
        );
        assert!(
            !node.holds(&account.called()),
            "and this node has not got it"
        );
        assert_eq!(
            node.act(&account.called(), Epoch::GENESIS).answer,
            None,
            "so it cannot hand it over, and says so"
        );
        assert_eq!(
            node.chain_of(&elsewhere, Epoch::GENESIS).answer.len(),
            1,
            "and yet the chain's shape is still there for anybody checking a summary"
        );
    }

    #[test]
    fn what_a_node_signed_over_does_not_move_when_it_gets_the_act_after_all() {
        // **An entry is never skipped and never written twice.** The tree over them is what this
        // node has put its name to, so a node that took note and later received the act must not
        // end up with two leaves — it would stop being able to reproduce a root it had signed.
        let mut node = opened();
        let account = an_account(&key(9), Epoch::GENESIS);
        node.note(&almena_format::entry::Entry::of(&account, 0, None));

        let written = node.written();
        let root = node.root_now();

        node.submit(&account, Epoch::GENESIS).expect("taken");
        assert_eq!(node.written(), written, "one act, one leaf");
        assert_eq!(node.root_now(), root, "and the tree did not move");
        assert!(
            matches!(
                node.resolve(account.object.name(), Epoch::GENESIS).answer,
                almena_store::chain::Answer::Here(_)
            ),
            "and now it can say what the object is"
        );
    }

    #[test]
    fn a_node_comes_back_with_the_tree_it_signed_over_even_where_it_kept_no_act() {
        // **The constraint everything else about letting go has to obey.** What a node put its name
        // to is the tree over its entries, so it may let go of what an act said and never of the
        // line saying the act happened. A node that came back with fewer entries would build a
        // different tree and contradict a root it had already published — which is the one thing
        // that can be proved against a node.
        let scratch = Scratch::new("entries");
        let root_before;
        let written_before;
        let elsewhere = an_account(&key(9), Epoch::GENESIS);

        {
            let mut node = Node::open_in(&scratch.0, &at(Which::Development), &[], &key(5), key(6))
                .expect("a fresh directory");
            // Something it holds.
            let mine = an_account(&key(11), Epoch::GENESIS);
            node.submit(&mine, Epoch::GENESIS).expect("taken");
            // And something it only knows happened.
            node.note(&almena_format::entry::Entry::of(
                &elsewhere,
                node.written() as u64,
                None,
            ));

            written_before = node.written();
            root_before = node.root_now();
            assert!(!node.holds(&elsewhere.called()));
        }

        let back = Node::rejoin(&scratch.0, key(6)).expect("its own record");
        assert_eq!(back.written(), written_before, "every entry came back");
        assert_eq!(
            back.root_now(),
            root_before,
            "so the tree is the one it signed over"
        );
        assert_eq!(
            back.resolve(elsewhere.object.name(), Epoch::GENESIS).answer,
            almena_store::chain::Answer::NotHere,
            "and it still says that one is held elsewhere"
        );
        assert!(
            !back.holds(&elsewhere.called()),
            "having never held what it said"
        );
    }

    #[test]
    fn a_node_lets_go_of_what_it_was_not_dealt_and_keeps_the_line_saying_it_happened() {
        // **What replaces every node keeping everything.** The record only grows, and a network
        // whose only plan was that has no plan. What a node keeps is the share it was dealt — which
        // it does not choose and which moves every month — and it keeps the line saying each act
        // happened either way, because the tree over those lines is what it has signed.
        let mut node = opened();
        // Enough nodes that a thing falls to some of them and not to others.
        for seed in 60u8..75 {
            let other =
                almena_store::announce::announce(Which::Development, Epoch::GENESIS, &key(seed));
            node.submit(&other.operation, Epoch::GENESIS)
                .expect("taken");
        }
        // And things that are somebody's history, rather than the census the share is drawn from.
        for seed in 130u8..160 {
            let account = an_account(&key(seed), Epoch::GENESIS);
            node.submit(&account, Epoch::GENESIS).expect("taken");
        }

        let written = node.written();
        let root = node.root_now();
        let let_go = node.let_go_of_what_is_not_mine(Epoch::GENESIS);

        assert!(let_go > 0, "some of it was dealt to somebody else");
        assert_eq!(node.written(), written, "and every entry stayed");
        assert_eq!(node.root_now(), root, "so the tree it signed did not move");
        assert_eq!(
            node.not_got().len(),
            let_go,
            "and it knows exactly what it no longer has"
        );
    }

    #[test]
    fn what_a_node_goes_and_fetches_is_what_it_is_owed_and_not_everything_it_is_missing() {
        // **The share-out moves every month, so what falls here is not what fell here.** A node
        // that only ever let go would be quietly short of everything that moved *towards* it, and
        // one that went and fetched everything it was missing would fetch what it had just dropped
        // and drop it again on the next tick — the two halves of one question, and only one of
        // them was being asked.
        let mut node = opened();
        for seed in 60u8..75 {
            let other =
                almena_store::announce::announce(Which::Development, Epoch::GENESIS, &key(seed));
            node.submit(&other.operation, Epoch::GENESIS)
                .expect("taken");
        }
        for seed in 130u8..160 {
            let account = an_account(&key(seed), Epoch::GENESIS);
            node.submit(&account, Epoch::GENESIS).expect("taken");
        }

        let let_go = node.let_go_of_what_is_not_mine(Epoch::GENESIS);
        assert!(let_go > 0, "some of it was dealt to somebody else");
        assert_eq!(node.not_got().len(), let_go);
        assert!(
            node.owed(Epoch::GENESIS).is_empty(),
            "and none of it is owed here, so there is nothing to go and ask for"
        );

        // A later moment is a different share-out, and some of what it dropped now falls to it
        // again. That is the half nothing was asking for.
        let later = Epoch::new(almena_time::deadline::ASSIGNMENT_SEED_ROTATION.now() * 3);
        let owed = node.owed(later);
        assert!(
            !owed.is_empty(),
            "the share moved and some of what it let go of came back to it"
        );
        for thing in &owed {
            assert!(
                node.not_got().contains(thing),
                "and everything owed is something it knows happened and has not got"
            );
        }
    }

    #[test]
    fn a_node_never_lets_go_of_what_it_said_itself() {
        // One that could not say what it is would be one nobody could check anything it said
        // against — its key, what it offers, what it saw of everybody else.
        let mut node = opened();
        for seed in 80u8..92 {
            let other =
                almena_store::announce::announce(Which::Development, Epoch::GENESIS, &key(seed));
            node.submit(&other.operation, Epoch::GENESIS)
                .expect("taken");
        }
        for seed in 160u8..190 {
            let account = an_account(&key(seed), Epoch::GENESIS);
            node.submit(&account, Epoch::GENESIS).expect("taken");
        }
        node.let_go_of_what_is_not_mine(Epoch::GENESIS);

        let mine = node.did().clone();
        for entry in node.chain_of(&mine, Epoch::GENESIS).answer {
            assert!(
                node.holds(&entry.hash),
                "a node keeps its own chain whatever the share-out says"
            );
        }
    }

    #[test]
    fn what_it_let_go_of_it_can_ask_for_and_take_back() {
        // **Held elsewhere has to be one more question and not a dead end.** What comes back goes
        // through the same admission as anything a stranger hands over, and lands in the place its
        // entry already had.
        let mut node = opened();
        for seed in 100u8..112 {
            let other =
                almena_store::announce::announce(Which::Development, Epoch::GENESIS, &key(seed));
            node.submit(&other.operation, Epoch::GENESIS)
                .expect("taken");
        }
        let mut acts = Vec::new();
        for seed in 190u8..220 {
            let account = an_account(&key(seed), Epoch::GENESIS);
            node.submit(&account, Epoch::GENESIS).expect("taken");
            acts.push(account);
        }

        node.let_go_of_what_is_not_mine(Epoch::GENESIS);
        let Some(gone) = node.not_got().first().cloned() else {
            panic!("something was let go of")
        };
        let Some(back) = acts.iter().find(|act| act.called() == gone) else {
            panic!("and this node had it a moment ago")
        };

        let written = node.written();
        let root = node.root_now();
        node.fill_in(back, Epoch::GENESIS).expect("taken back");

        assert!(node.holds(&gone), "it has what the act said again");
        assert_eq!(
            node.written(),
            written,
            "at the place its entry already had"
        );
        assert_eq!(node.root_now(), root, "so the tree did not move");
    }

    #[test]
    fn what_is_handed_back_has_to_be_what_was_signed() {
        // **The name covers everything but the signatures**, so matching it means the content
        // matches and only the signature is still open. Left unchecked, whoever handed it over
        // could sign with a key they made that morning and have this node serve it under a name it
        // vouches for.
        let mut node = opened();
        for seed in 100u8..112 {
            let other =
                almena_store::announce::announce(Which::Development, Epoch::GENESIS, &key(seed));
            node.submit(&other.operation, Epoch::GENESIS)
                .expect("taken");
        }
        let account = an_account(&key(230), Epoch::GENESIS);
        node.submit(&account, Epoch::GENESIS).expect("taken");
        node.let_go_of_what_is_not_mine(Epoch::GENESIS);

        if !node.not_got().contains(&account.called()) {
            // It fell to this node, so there is nothing to hand back. Nothing to test here, and
            // saying so beats pretending the case was covered.
            return;
        }

        let mut theirs = account.clone();
        let stranger = key(231);
        theirs.signatures[0].key = stranger.verifying_key().bytes().to_vec();
        theirs.signatures[0].signature = stranger.sign(&theirs.signing_bytes()).bytes();
        assert_eq!(
            theirs.called(),
            account.called(),
            "it is offered under the name this node is missing"
        );

        assert_eq!(
            node.fill_in(&theirs, Epoch::GENESIS),
            Err(almena_store::chain::Refused::NotAuthorised)
        );
        assert!(!node.holds(&account.called()), "and it did not keep it");
        assert!(node.fill_in(&account, Epoch::GENESIS).is_ok());
        assert!(node.holds(&account.called()), "the real one it does keep");
    }
}
