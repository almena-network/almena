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
pub mod identity;
pub mod peer;
pub mod record;
pub mod zone;

// What a face needs in order to call any of this, re-exported so that a face never reaches past
// the core to get it. A door somebody has to go around is a door.
pub use almena_store::chain::{Admitted, Answer, Reason, State};
pub use almena_store::genesis::{Opening, Which};
pub use almena_suite::ed25519::SigningKey;
pub use almena_time::Epoch;

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
    /// The instant this network's epoch zero began, in seconds since the Unix epoch.
    ///
    /// **The only wall-clock reading this platform ever writes down**, fixed by the act that opened
    /// the network and carried here so that nothing else has to read one. A face that worked it out
    /// for itself would be a face deciding what time it is, and a node that came back to a record
    /// could not work it out at all.
    began: u64,
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
    /// # Errors
    ///
    /// [`genesis::Refused`], saying whether somebody else is already here or this node is.
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
            began: opening.began,
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

        // Written down before it is answered for, and this is the order that matters: an act that
        // reached memory and not the disk would be one this node said it had taken and would not
        // have the morning after.
        if let Some(record) = self.record.as_mut()
            && record.wrote(&operation.to_bytes()).is_err()
        {
            return Err(Refused::NotKept);
        }

        self.log.append(operation, subject_of(operation));
        Ok(self.stamped(admitted, now))
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

        // Everything replayed goes to the record as it is admitted, so the acts that came over the
        // wire are written down here rather than pulled again on the next start.
        for act in acts {
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
            began: genesis::began(&opening).ok_or(record::NotReadable::Unreadable)?,
            record: None,
        };

        // Admitted under the same rules that took them the first time, and each act against the
        // epoch it declared rather than against now — acts read at a later hour must not be
        // rejected for being old.
        for act in acts {
            let operation = operation_from(act).ok_or(record::NotReadable::Unreadable)?;
            let at = operation.issued;
            node.objects
                .admit(&operation, at)
                .map_err(|_| record::NotReadable::Refused)?;
            node.log.append(&operation, subject_of(&operation));
        }
        Ok(node)
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
    pub fn summarise(
        &mut self,
        day: almena_time::Day,
        seen: &std::collections::BTreeMap<Did, almena_store::summary::Seen>,
        now: Epoch,
    ) -> bool {
        if !almena_store::summary::worth_writing(&self.did, day, seen, now) {
            return false;
        }
        // The hash of what it was drawn from. The observations themselves stay off the record and
        // are served by whoever made them, so that this can be checked by anybody who cares to ask.
        let drawn_from = Digest::of(&summarised(seen));
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
            seen,
            drawn_from,
        );
        self.submit(&written.operation, now).is_ok()
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

/// The observations a summary was drawn from, in bytes anybody can hash the same way.
///
/// **Not the summary itself.** Hashing what is already in the act would be a claim vouching for
/// itself; this is the working, so that whoever holds it can check the summary against it.
fn summarised(seen: &std::collections::BTreeMap<Did, almena_store::summary::Seen>) -> Vec<u8> {
    let watched = seen
        .iter()
        .map(|(node, seen)| {
            almena_format::cbor::Value::Array(vec![
                almena_format::cbor::Value::Text(node.to_string()),
                almena_format::cbor::Value::Uint(seen.asked),
                almena_format::cbor::Value::Uint(seen.answered),
                almena_format::cbor::Value::Uint(seen.behind),
            ])
        })
        .collect();
    almena_format::cbor::Value::Array(watched).to_bytes()
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
const fn subject_of(_operation: &Operation) -> Option<Did> {
    None
}

/// Whether any act this build knows how to write down can be about somebody else.
///
/// It is `false`, and it is a constant rather than a comment so that the day somebody teaches
/// [`subject_of`] to read one, this fails to be true and the answers stop saying *not askable*.
const ANYTHING_CARRIES_A_SUBJECT: bool = false;

#[cfg(test)]
mod tests {
    use super::{Joining, Node, Page, record};
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
        let name = Name::of(&account.to_bytes());
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
        let name = Name::of(&account.to_bytes());
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
        let name = Name::of(&account.to_bytes());
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
        let name = Name::of(&account.to_bytes());
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
        use almena_store::checkpoint::{Claim, Governs, falls_over};

        let mut node = opened();
        let control = key(9);
        let account = an_account(&control, Epoch::GENESIS);
        let object = account.object.clone();
        node.submit(&account, Epoch::GENESIS).expect("taken");

        let created = Name::of(&account.to_bytes());
        let added = a_device(&object, &created, &control);
        node.submit(&added, Epoch::GENESIS).expect("taken");

        let entries = node.chain_of(&object, Epoch::GENESIS).answer;
        let held: Vec<&almena_format::entry::Entry> = entries.iter().collect();

        // A summary that says the devices were last set when the account was created — which was
        // true, and stopped being true one act later.
        let hiding = Claim {
            about: Governs::Devices,
            set_by: created,
        };
        assert_eq!(
            falls_over(&[hiding], &held).len(),
            1,
            "the record says otherwise, and says which act it left out"
        );

        // And the honest one, citing the act that really did set them last.
        let honest = Claim {
            about: Governs::Devices,
            set_by: Name::of(&added.to_bytes()),
        };
        assert!(falls_over(&[honest], &held).is_empty());
    }

    #[test]
    fn a_node_writes_down_what_it_saw_of_others_and_never_of_itself() {
        // **The whole reason cross-observation exists.** What a node claims about its own uptime is
        // worth nothing; what is worth something is that the nodes which kept asking it wrote down
        // whether it answered.
        use almena_store::summary::Seen;
        use std::collections::BTreeMap;

        let mut node = opened();
        let somebody = Did::new(Network::Development, Name::of(b"another node"));
        let seen = BTreeMap::from([
            (
                somebody.clone(),
                Seen {
                    asked: 100,
                    answered: 97,
                    behind: 3,
                },
            ),
            (
                node.did().clone(),
                Seen {
                    asked: 100,
                    answered: 100,
                    behind: 0,
                },
            ),
        ]);

        // A day that is over. One still happening compares with nothing.
        let after = Epoch::new(almena_time::EPOCHS_PER_DAY * 2);
        assert!(node.summarise(almena_time::Day::new(1), &seen, after));

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
        let (day, _, watched) = read.expect("a summary");

        assert_eq!(day, almena_time::Day::new(1));
        assert_eq!(watched.len(), 1);
        assert!(watched.contains_key(&somebody));
        assert!(
            !watched.contains_key(node.did()),
            "and it said nothing about itself"
        );
    }

    #[test]
    fn a_day_still_happening_is_not_summarised() {
        use almena_store::summary::Seen;
        use std::collections::BTreeMap;

        let mut node = opened();
        let seen = BTreeMap::from([(
            Did::new(Network::Development, Name::of(b"another node")),
            Seen::default(),
        )]);

        assert!(!node.summarise(almena_time::Day::new(0), &seen, Epoch::new(23)));
        assert!(node.summarise(almena_time::Day::new(0), &seen, Epoch::new(24)));
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
    fn nobody_has_looked_is_not_the_same_as_there_is_nothing() {
        // No act can carry a subject yet, so an empty list here would read as *nobody has
        // certified this entity* — a claim, and a false one, because nobody has been able to.
        let node = opened();
        assert_eq!(
            node.about(node.government(), Epoch::GENESIS).answer,
            None,
            "the question is not askable yet, which is not the same as its answer being empty"
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

        let hash = Name::of(&account.to_bytes());
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

        let hash = Name::of(&account.to_bytes());
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

        let head = Name::of(&account.to_bytes());
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
}
