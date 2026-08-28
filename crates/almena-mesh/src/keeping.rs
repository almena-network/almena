//! Staying up to date with whoever else is on the network.
//!
//! Everything under this can pass the record between two nodes; nothing under it ever decides to.
//! This is the habit: dial whoever the zone named, ask them what came after where we had got to,
//! and answer the same question when it is asked back.
//!
//! # Where we got to is a fact about **them**
//!
//! A position is a position in one node's own record — nothing about validity is decided against
//! it, and two nodes that wrote the same acts in a different order have different positions for
//! them. So catching up means remembering, **per peer**, how much of *that peer's* record has been
//! read. One number for everybody would mean asking one node for a position in another's, and
//! getting an answer that means nothing.
//!
//! # It asks again rather than asking for everything
//!
//! An answer says how far the answering node has got. If that is further than we have read, there
//! is more, and the next question goes out immediately — so a node that is a long way behind walks
//! forward a page at a time instead of asking for a message nobody can hold.
//!
//! # Nothing here believes anything
//!
//! What arrives is handed to the node and admitted by the same rule as an act delivered by a
//! stranger over an interface, because that is what it is. An act that does not check out is
//! dropped and the rest carry on: one bad act in a page is not a reason to stop talking to
//! somebody, and it is not evidence about them either — they may be passing on what they were
//! given, exactly as this node does.

use std::collections::BTreeMap;
use std::sync::Arc;

use almena_format::identifier::Name;
use almena_node::{Epoch, Node};
use almena_store::root::{Published, Root, Witness};
use almena_store::summary::Seen;
use almena_time::Day;
use libp2p::{Multiaddr, PeerId};
use tokio::sync::RwLock;

use crate::sync::{Ask, Said};
use crate::{Happened, Listening};

/// How much of a record is handed over at a time.
///
/// A page rather than a record: big enough that catching up is not a thousand round trips, small
/// enough that a message is one a machine can hold while it reads it.
///
/// **The weight is the number that matters**, because the count is not a bound on anything a reader
/// cares about — one act may be as large as whatever the node that took it was willing to accept,
/// so a page of two hundred and fifty-six of them can be twice what the wire will carry. A node
/// whose answers cannot be read is one nobody can catch up with.
///
/// Half of what the wire carries, so that the answer's own wrapping fits beside the acts.
const PAGE: almena_node::Page = almena_node::Page {
    at_most: 256,
    weighing_at_most: 4 * 1024 * 1024,
};

/// How often a node looks at its own record to see whether it has anything to tell.
///
/// Short, because it is a read lock and a comparison — and because the whole point is that an act
/// does not sit waiting for somebody to happen to ask.
const NOTICING: std::time::Duration = std::time::Duration::from_millis(500);

/// What this node last noticed about itself, so that it can tell when there is something to say.
///
/// Both are cheap to take — one read lock and two numbers — which is what makes looking often
/// affordable, and looking often is what stops an act waiting to be asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Noticed {
    /// How much it has written down.
    written: u64,
    /// The last epoch it closed, if it has closed one.
    closed: Option<Epoch>,
}

impl Noticed {
    /// What this node looks like right now.
    async fn of(node: &Arc<RwLock<Node>>) -> Self {
        let node = node.read().await;
        Self {
            written: node.written() as u64,
            closed: node.last_closed(),
        }
    }
}

/// What this node has read of everybody else's record.
///
/// One number per peer, because a position belongs to the record it is a position in.
#[derive(Debug, Default)]
struct ReadSoFar(BTreeMap<PeerId, Reading>);

/// Where this node has got to in one peer's record, and whether it is waiting on an answer.
#[derive(Debug, Default, Clone, Copy)]
struct Reading {
    /// How much of that peer's record has been read.
    at: u64,
    /// Whether a question is outstanding.
    ///
    /// **Without this the cursor moves on anything that arrives.** An answer nobody asked for — a
    /// duplicate, or one to a different question — would push the position past records that are
    /// then never asked for again, and a node would sit quietly missing them for ever.
    waiting: bool,
    /// What this node has seen of that one, to be summarised at the end of the day.
    ///
    /// **Nobody says anything about themselves**, and this is the other half of that: a node's
    /// availability is what the nodes that kept asking it wrote down.
    seen: Seen,
}

impl ReadSoFar {
    /// How far this node has read of that peer's record.
    fn of(&self, peer: &PeerId) -> u64 {
        self.0.get(peer).map_or(0, |reading| reading.at)
    }

    /// Take note of having asked that peer for what comes next.
    fn asked(&mut self, peer: PeerId) {
        let reading = self.0.entry(peer).or_default();
        reading.waiting = true;
        reading.seen.asked += 1;
    }

    /// Take note of an answer, moving the cursor only if it was the one outstanding.
    ///
    /// Returns whether it counted.
    fn answered(&mut self, peer: PeerId, count: u64) -> bool {
        let reading = self.0.entry(peer).or_default();
        reading.seen.answered += 1;
        if !reading.waiting {
            return false;
        }
        reading.waiting = false;
        reading.at += count;
        true
    }

    /// Take note of how far behind that peer was seen to be.
    ///
    /// The furthest is kept, not the latest: **a node that is up and behind is worse than one that
    /// is down**, and a figure that forgot the worst of it would say the opposite.
    fn behind(&mut self, peer: PeerId, by: u64) {
        let seen = &mut self.0.entry(peer).or_default().seen;
        seen.behind = seen.behind.max(by);
    }

    /// What has been seen of everybody, ready to be written down.
    fn seen(&self) -> Vec<(PeerId, Seen)> {
        self.0
            .iter()
            .map(|(peer, reading)| (*peer, reading.seen))
            .collect()
    }

    /// Start the next day's counting, keeping where everybody has got to.
    fn a_new_day(&mut self) {
        for reading in self.0.values_mut() {
            reading.seen = Seen::default();
        }
    }

    /// Everybody this node has met.
    fn everybody(&self) -> Vec<PeerId> {
        self.0.keys().copied().collect()
    }
}

/// Keep up with the network, for as long as this is polled.
///
/// `seeds` is where to start: whoever the zone named. Nothing else is dialled, because nothing
/// else is known yet — the census that would name the rest lives in the record, and reading it is
/// what this is for.
///
/// `clock` says what epoch it is, asked each time rather than captured once, so that an act
/// arriving after an epoch boundary is admitted against the epoch it arrives in.
pub async fn keeping_up<C>(
    mut listening: Listening,
    node: Arc<RwLock<Node>>,
    seeds: Vec<Multiaddr>,
    clock: C,
    every: std::time::Duration,
) where
    C: Fn() -> Epoch + Send,
{
    let watched = Watched::default();
    watching(
        Present {
            listening: &mut listening,
            node: &node,
            watched: &watched,
        },
        seeds,
        clock,
        every,
    )
    .await;
}

/// What somebody outside can see of what this has witnessed.
///
/// Shared rather than returned, because the loop does not end: a caller that had to wait for it to
/// finish before learning anything would never learn anything.
#[derive(Debug, Clone, Default)]
pub struct Watched(Arc<RwLock<Witnessed>>);

impl Watched {
    /// What has been witnessed so far.
    pub async fn witnessed(&self) -> tokio::sync::RwLockReadGuard<'_, Witnessed> {
        self.0.read().await
    }
}

/// A node's presence on the mesh: what it is, how it is reached, and what it has learned of others.
///
/// The three travel together because they are one thing. A loop that had the socket without the
/// node could answer nothing, and one that had the node without somewhere to put what it witnessed
/// would learn things nobody could ever see.
pub struct Present<'a> {
    /// The socket.
    pub listening: &'a mut Listening,
    /// The node itself, shared with whatever else is drawing or serving it.
    pub node: &'a Arc<RwLock<Node>>,
    /// What other nodes have signed, as it is learned.
    pub watched: &'a Watched,
}

/// The same, with somebody watching what it witnesses.
pub async fn watching<C>(
    present: Present<'_>,
    seeds: Vec<Multiaddr>,
    clock: C,
    every: std::time::Duration,
) where
    C: Fn() -> Epoch + Send,
{
    let Present {
        listening,
        node,
        watched,
    } = present;
    for seed in seeds {
        // A seed that cannot be dialled is one node not reached, not a reason to stop before
        // trying the others. Which of them answers is not this node's to decide.
        let _ = listening.dial(seed);
    }

    let mut read = ReadSoFar::default();
    let mut asking = tokio::time::interval(every);

    // **Telling, so that an act does not wait for somebody to happen to ask.** A node notices its
    // own record growing and says so; everything that actually moves still moves by being asked
    // for. Looking is one read lock and a comparison, which is what makes looking often affordable.
    let mut looking = tokio::time::interval(NOTICING);
    let mut noticed = Noticed::of(node).await;
    let mut summarised: Option<Day> = None;

    loop {
        tokio::select! {
            _ = looking.tick() => {
                summarising(node, &mut read, &mut summarised, clock()).await;
                writing_down(node, watched, clock()).await;
                noticed = saying_what_changed(listening, node, &read, noticed).await;
            }
            _ = asking.tick() => {
                asking_everybody(listening, &mut read, noticed);
            }
            happened = listening.next() => match happened {
                Happened::Met(peer) => asking_one(listening, &mut read, peer, noticed),
                Happened::Asked(peer, question, back) => {
                    take_note(node, peer, &question).await;
                    if told_they_grew(&question, &mut read, peer) {
                        listening.ask(&peer, Ask::Since(read.of(&peer)));
                    }

                    let said = answering(node, &question, clock()).await;
                    let _ = listening.answer(back, said);
                }
                Happened::Answered(peer, said) => {
                    if let Some(saw) = witness_for(node, watched, peer, said.root.as_deref()).await
                    {
                        listening.ask(&peer, saw);
                    }
                    take_in(node, &said, clock()).await;
                    if taking_it_in(&mut read, peer, &said) {
                        // More where that came from, so the next page goes out now rather than at
                        // the next tick. A long way behind should not take an hour to walk forward.
                        listening.ask(&peer, Ask::Since(read.of(&peer)));
                        read.asked(peer);
                    }
                }
                Happened::Reachable(_) | Happened::Parted(_) => {}
            },
        }
    }
}

/// What other nodes have signed about their own epochs, and what they proved by signing it.
///
/// **A root is only worth anything against the key of the node that signed it**, and that key comes
/// out of the name the peer answers to — which the connection already proved they hold. Nothing is
/// resolved and nobody is asked.
#[derive(Debug, Default)]
pub struct Witnessed {
    /// One root per node per epoch, and the second different one is the interesting case.
    ///
    /// The whole signed artefact and not only what it says: **the signatures are the evidence**,
    /// and a pair with them stripped off proves nothing to anybody who was not there.
    seen: BTreeMap<(PeerId, u64), Published>,
    /// Every pair this node holds that cannot both be true, with their signatures on them.
    contradictions: Vec<(Published, Published)>,
}

impl Witnessed {
    /// Nothing seen yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Take in what a peer signed, if it really signed it.
    ///
    /// Returns whether it was kept. A root that does not check out is dropped without ceremony: it
    /// proves nothing about the peer either, since anybody can send anybody bytes.
    ///
    /// **A second, different root for one epoch from one node is the one thing that is provable
    /// against it**, and it is kept rather than acted on — what is done about misconduct is not
    /// this crate's to decide.
    pub fn take_in(&mut self, peer: PeerId, network: &Name, bytes: &[u8]) -> bool {
        let Some(published) = Published::read(bytes) else {
            return false;
        };
        let Some(key) = crate::whose::key_of(&peer) else {
            return false;
        };
        if published.accept(network, &key).is_err() {
            return false;
        }

        let at = (peer, published.root.epoch.number());
        match self.seen.get(&at) {
            Some(already) if almena_store::root::contradict(&already.root, &published.root) => {
                self.contradictions.push((already.clone(), published));
            }
            Some(_) => {}
            None => {
                self.seen.insert(at, published);
            }
        }
        true
    }

    /// What a node signed about an epoch, if this node has it.
    #[must_use]
    pub fn of(&self, peer: &PeerId, epoch: u64) -> Option<&Root> {
        self.seen
            .get(&(*peer, epoch))
            .map(|published| &published.root)
    }

    /// Everybody whose signed root this node holds.
    #[must_use]
    pub fn everybody(&self) -> Vec<PeerId> {
        let mut who: Vec<PeerId> = self.seen.keys().map(|(peer, _)| *peer).collect();
        who.dedup();
        who
    }

    /// Every pair one node signed that cannot both be true, signatures and all.
    #[must_use]
    pub fn contradictions(&self) -> &[(Published, Published)] {
        &self.contradictions
    }

    /// Forget a pair, once somebody has written it down.
    ///
    /// It stays in the record from then on, which is the point of putting it there — holding it
    /// here as well would mean writing it down again on every round.
    pub fn written_down(&mut self, pair: &(Published, Published)) {
        self.contradictions.retain(|held| held != pair);
    }
}

/// Take note of an answer, and say whether there is more to ask for.
///
/// **Only what was asked for moves the cursor.** An answer that is not the one outstanding — a
/// duplicate, or one to a question about something else — would otherwise push the position past
/// records nobody ever asks for again, and a node would sit quietly missing them for ever.
fn taking_it_in(read: &mut ReadSoFar, peer: PeerId, said: &Said) -> bool {
    // How far behind it is, from the one number it always tells: how much it has. A node that is up
    // and behind is worse than one that is down, because whoever asks it gets an answer and cannot
    // tell it is stale.
    read.behind(peer, said.written.saturating_sub(read.of(&peer)));

    read.answered(peer, said.acts.len() as u64) && read.of(&peer) < said.written
}

/// Write down what this node saw of the others, once a day is over.
///
/// **Nobody says anything about themselves**, so a node's availability is what the nodes that kept
/// asking it wrote down. Raw observations stay off the record — fifty nodes watching each other
/// would make the record almost entirely telemetry — and what goes in is the day's aggregate with
/// the hash of what it was drawn from.
///
/// A day still happening is not summarised: a summary drawn over half a window compares with
/// nothing, which is the one thing summaries are for.
async fn summarising(
    node: &Arc<RwLock<Node>>,
    read: &mut ReadSoFar,
    already: &mut Option<Day>,
    now: Epoch,
) {
    let yesterday = Day::of(now);
    let Some(yesterday) = yesterday.number().checked_sub(1).map(Day::new) else {
        return;
    };
    if !yesterday.over(now) || *already == Some(yesterday) {
        return;
    }

    let mut seen = BTreeMap::new();
    {
        let node = node.read().await;
        for (peer, watched) in read.seen() {
            // Only somebody the record names. A key nobody announced themselves with is somebody
            // speaking the protocol without being anybody, and a figure filed against no name is a
            // figure about nobody.
            if let Some(key) = crate::whose::key_of(&peer)
                && let Some(named) = node.node_called(&key, now).answer
            {
                seen.insert(named, watched);
            }
        }
    }

    let written = node.write().await.summarise(yesterday, &seen, now);
    if written {
        read.a_new_day();
    }
    *already = Some(yesterday);
}

/// Put anything caught into the record, so that what one node found everybody can check.
///
/// **Until it is written down it is one node's private observation**, and travels no further than
/// people willing to take its word. In the record it travels like everything else and carries its
/// own proof, so nobody has to take anybody's word at all.
///
/// A pair that is already there is refused for existing, which is the right answer: one
/// contradiction is one object however many people noticed it.
async fn writing_down(node: &Arc<RwLock<Node>>, watched: &Watched, now: Epoch) {
    let caught = watched.0.read().await.contradictions().to_vec();
    if caught.is_empty() {
        return;
    }

    let mut node = node.write().await;
    for pair in &caught {
        let written = node.write_down(&pair.0, &pair.1, now);
        let _ = written;
    }
    drop(node);

    let mut witnessed = watched.0.write().await;
    for pair in &caught {
        witnessed.written_down(pair);
    }
}

/// Say what has changed about this node since it last looked.
///
/// **Telling, so that nothing waits to be asked for.** What actually moves still moves by being
/// asked for and admitted like anything else; this only starts the asking. Returns what it saw, to
/// be compared against next time.
async fn saying_what_changed(
    listening: &mut Listening,
    node: &Arc<RwLock<Node>>,
    read: &ReadSoFar,
    before: Noticed,
) -> Noticed {
    let now = Noticed::of(node).await;
    let grown = now.written > before.written;
    let closed = now.closed.filter(|_| now.closed != before.closed);

    for peer in read.everybody() {
        if grown {
            // Everybody, including whoever this node just learned it from. That is how an act gets
            // past two nodes: each one that takes it tells the ones it knows.
            listening.ask(&peer, Ask::Grown(now.written));
        }
        // A closed epoch is a new thing this node has to say about itself, and the others have one
        // too — so it is also the moment to ask them for theirs.
        if let Some(closed) = closed {
            listening.ask(&peer, Ask::Root(closed.number()));
        }
    }
    now
}

/// Whether that was somebody saying they have grown past where this node had read.
///
/// **The number in it is a hint and nothing else.** This node asks from where *it* got to, and what
/// comes back is admitted like anything else — so the worst a liar buys is one question asked.
///
/// Notes the question as outstanding when it says yes, because the caller is about to ask it.
fn told_they_grew(question: &Ask, read: &mut ReadSoFar, peer: PeerId) -> bool {
    let Ask::Grown(written) = question else {
        return false;
    };
    if read.of(&peer) >= *written {
        return false;
    }
    read.asked(peer);
    true
}

/// Ask one node for what came after where this node got to, and for what it signed.
///
/// Everything, straight away rather than at the next tick: the point of meeting somebody is that
/// they may have what this node has not — and that goes for what they have signed as much as for
/// what they have written down.
fn asking_one(listening: &mut Listening, read: &mut ReadSoFar, peer: PeerId, noticed: Noticed) {
    listening.ask(&peer, Ask::Since(read.of(&peer)));
    read.asked(peer);
    if let Some(closed) = noticed.closed {
        listening.ask(&peer, Ask::Root(closed.number()));
    }
}

/// Ask everybody for what came after where this node got to, and for what they signed.
///
/// **Everybody, every time.** A node that only asked whoever last had something would stop asking
/// the quiet ones, and quiet is what a node looks like just before it has something. This is the
/// floor rather than the usual way: meeting somebody asks, and so does anything changing.
fn asking_everybody(listening: &mut Listening, read: &mut ReadSoFar, noticed: Noticed) {
    for peer in read.everybody() {
        asking_one(listening, read, peer, noticed);
    }
}

/// Keep what a peer said it saw, when that is what it said.
///
/// One of the things that arrive changes this node rather than only reading it: somebody's word
/// that they saw a root is evidence to keep, not a question to answer.
async fn take_note(node: &Arc<RwLock<Node>>, peer: PeerId, question: &Ask) {
    let Ask::Saw(epoch, signature) = question else {
        return;
    };
    let Some(key) = crate::whose::key_of(&peer) else {
        return;
    };
    // Dropped if it is not that key's word about a root this node published. It proves nothing
    // about the peer either — anybody can send anybody bytes.
    let _ = node.write().await.saw(
        Epoch::new(*epoch),
        Witness {
            key,
            signature: *signature,
        },
    );
}

/// Take in what a peer signed, and say back that it was seen.
///
/// [`None`] when there was nothing to see or it did not check out. **A witness nobody hears is
/// nobody's word**, which is why the answer goes back rather than being kept here: what it buys is
/// that a node cannot quietly show one root to one person and another to somebody else — the two
/// would carry different witnesses, and the pair is the proof.
async fn witness_for(
    node: &Arc<RwLock<Node>>,
    watched: &Watched,
    peer: PeerId,
    root: Option<&[u8]>,
) -> Option<Ask> {
    let root = root?;
    // Held to the key the peer's own name carries, which the connection already proved they hold.
    let network = node.read().await.network().clone();
    if !watched.0.write().await.take_in(peer, &network, root) {
        return None;
    }

    let published = Published::read(root)?;
    let seen = node.read().await.countersign(&published.root);
    Some(Ask::Saw(published.root.epoch.number(), seen.signature))
}

/// What this node has to say to that question.
///
/// It says only what it holds. A question about an epoch it has not closed comes back with nothing
/// in it rather than with something made up, and *nothing to say* is an answer.
async fn answering(node: &Arc<RwLock<Node>>, question: &Ask, now: Epoch) -> Said {
    let node = node.read().await;
    let written = node.written() as u64;

    match question {
        Ask::Since(from) => Said {
            acts: node.since(*from, PAGE, now).answer,
            written,
            root: None,
        },
        Ask::Act(name) => Said {
            acts: node.act(name, now).answer.into_iter().collect(),
            written,
            root: None,
        },
        // Nothing is asked by either of these, so nothing is answered beyond the fact that it
        // arrived. What to do about them is the caller's, because they change the node or what it
        // asks next, and this only reads it.
        Ask::Saw(_, _) | Ask::Grown(_) => Said {
            acts: Vec::new(),
            written,
            root: None,
        },
        Ask::Root(epoch) => Said {
            acts: Vec::new(),
            written,
            // Signed as it goes out. The signature is over bytes that do not change, so making it
            // here costs one signature and saves keeping a second copy that could drift.
            root: node
                .root_at(Epoch::new(*epoch))
                .map(|root| node.publish(root).to_bytes()),
        },
    }
}

/// Put what arrived through the node's own admission.
///
/// **The one place this could have gone wrong and did not.** Nothing here checks anything: it hands
/// each act to the node exactly as a stranger's act is handed to it over an interface, and what
/// decides whether it is kept is the act's own signature.
async fn take_in(node: &Arc<RwLock<Node>>, said: &Said, now: Epoch) {
    let mut node = node.write().await;
    for act in &said.acts {
        let Ok(value) = almena_format::cbor::read(act) else {
            // Not canonical bytes, so not an act. Dropped, and the rest carry on: one bad act in a
            // page says nothing about the peer, who may be passing on what it was given.
            continue;
        };
        let Some(operation) = almena_format::operation::read(&value) else {
            continue;
        };
        // Refused acts are the ordinary case and not a fault: most of a page is usually already
        // held, and *already here* is one of the reasons an act is refused.
        let _ = node.submit(&operation, now);
    }
}

/// Get a network's record from whoever will hand it over.
///
/// **What a node does before it is one.** It has no record, so it cannot answer anything and has
/// nothing to admit acts against — it dials whoever it was told about, asks from the beginning, and
/// keeps asking until the answer stops growing.
///
/// `network` has to be known in advance, because it is inside the name of the protocol two nodes
/// negotiate. That is why the zone publishes it: somebody arriving for the first time has no record
/// to take it from.
///
/// Nothing here checks the acts. They are handed back exactly as they arrived, and whoever builds a
/// node out of them admits them by the same rule as anything else — including that the first of
/// them has to be the act whose hash is this network's name.
///
/// [`None`] when nobody answered in time, which is *nobody would say* and never *there is nothing*.
pub async fn fetch(
    listening: &mut Listening,
    seeds: Vec<Multiaddr>,
    patience: std::time::Duration,
) -> Option<Vec<Vec<u8>>> {
    for seed in seeds {
        let _ = listening.dial(seed);
    }

    let mut got: Vec<Vec<u8>> = Vec::new();
    let mut from: Option<PeerId> = None;

    let asking = async {
        loop {
            match listening.next().await {
                // Everybody who answers is dialled, because which of them is reachable is not
                // this node's to guess — but only the first to answer is followed.
                Happened::Met(peer) if from.is_none() => {
                    listening.ask(&peer, Ask::Since(0));
                }
                // **One record from one node.** Positions belong to the record they are positions
                // in, so pages from two nodes spliced together are not a record at all: they
                // overlap, they interleave, and what comes out is refused by the first node that
                // tries to replay it.
                Happened::Answered(peer, said)
                    if from.is_none_or(|following| following == peer) =>
                {
                    from = Some(peer);
                    got.extend(said.acts);
                    if (got.len() as u64) >= said.written {
                        return got;
                    }
                    // More where that came from. Asked at once rather than on any schedule: a node
                    // with no record yet cannot do anything else until it has one.
                    listening.ask(&peer, Ask::Since(got.len() as u64));
                }
                // Somebody else answering, or asking this node something. It has nothing, so it
                // says nothing — and they find out by not being answered.
                _ => {}
            }
        }
    };

    tokio::time::timeout(patience, asking).await.ok()
}

#[cfg(test)]
mod tests {
    use super::ReadSoFar;
    use almena_format::identifier::Name;
    use libp2p::PeerId;

    /// A root somebody signed, and the name they answer to on the mesh.
    fn signed(seed: u8, epoch: u64, over: &[u8]) -> (libp2p::PeerId, Vec<u8>, Name) {
        use almena_format::identifier::{Did, Network};
        use almena_suite::digest::Digest;

        let key = almena_suite::ed25519::SigningKey::from_secret([seed; 32]);
        let network = Name::of(b"one network");
        let root = almena_store::root::Root {
            network: network.clone(),
            node: Did::new(Network::Development, Name::of(&[seed])),
            epoch: almena_node::Epoch::new(epoch),
            size: 4,
            root: Digest::of(over),
        };
        let peer = crate::identity(&key).expect("a key").public().to_peer_id();
        (peer, root.publish(&key).to_bytes(), network)
    }

    #[test]
    fn a_root_is_kept_when_the_peer_really_signed_it() {
        // Held to the key the peer's own name carries — nothing is resolved and nobody is asked.
        let (peer, bytes, network) = signed(3, 7, b"what they saw");
        let mut witnessed = super::Witnessed::new();

        assert!(witnessed.take_in(peer, &network, &bytes));
        assert!(witnessed.of(&peer, 7).is_some());
    }

    #[test]
    fn a_root_from_somebody_who_did_not_sign_it_is_dropped() {
        // **The check that makes any of this worth doing.** Anybody can send anybody bytes, so a
        // root that arrives is worth exactly what its signature is worth against the sender's key.
        let (_, bytes, network) = signed(3, 7, b"what they saw");
        let (somebody_else, _, _) = signed(4, 7, b"anything");
        let mut witnessed = super::Witnessed::new();

        assert!(!witnessed.take_in(somebody_else, &network, &bytes));
        assert!(witnessed.of(&somebody_else, 7).is_none());
    }

    #[test]
    fn a_root_for_another_network_is_dropped() {
        let (peer, bytes, _) = signed(3, 7, b"what they saw");
        let mut witnessed = super::Witnessed::new();
        assert!(!witnessed.take_in(peer, &Name::of(b"a different network"), &bytes));
    }

    #[test]
    fn one_node_saying_two_things_about_one_epoch_is_caught_and_kept() {
        // The one thing that is provable against a node. It is kept rather than acted on: what is
        // done about misconduct is not this crate's to decide.
        let (peer, first, network) = signed(3, 7, b"one history");
        let (_, second, _) = signed(3, 7, b"another history");
        let mut witnessed = super::Witnessed::new();

        assert!(witnessed.take_in(peer, &network, &first));
        assert!(witnessed.take_in(peer, &network, &second));
        assert_eq!(witnessed.contradictions().len(), 1);

        let caught = &witnessed.contradictions()[0];
        assert!(
            almena_store::contradiction::against(
                &almena_store::contradiction::publish(
                    &caught.0,
                    &caught.1,
                    almena_node::Epoch::GENESIS,
                    &almena_suite::ed25519::SigningKey::from_secret([1; 32]),
                )
                .operation
            )
            .is_some(),
            "and what is kept is enough to write down evidence anybody can check"
        );
    }

    #[test]
    fn a_pair_written_down_is_not_written_down_again() {
        // It stays in the record from then on, which is the point of putting it there.
        let (peer, first, network) = signed(3, 7, b"one history");
        let (_, second, _) = signed(3, 7, b"another history");
        let mut witnessed = super::Witnessed::new();
        witnessed.take_in(peer, &network, &first);
        witnessed.take_in(peer, &network, &second);

        let caught = witnessed.contradictions()[0].clone();
        witnessed.written_down(&caught);
        assert!(witnessed.contradictions().is_empty());
    }

    #[test]
    fn the_same_root_arriving_twice_is_not_a_contradiction() {
        // It arrives twice all the time — every node is asked on every round.
        let (peer, bytes, network) = signed(3, 7, b"one history");
        let mut witnessed = super::Witnessed::new();

        assert!(witnessed.take_in(peer, &network, &bytes));
        assert!(witnessed.take_in(peer, &network, &bytes));
        assert!(witnessed.contradictions().is_empty());
    }

    #[test]
    fn two_nodes_saying_different_things_prove_nothing() {
        // They have different trees by design; that is what having more than one node is for.
        let (one, first, network) = signed(3, 7, b"what one saw");
        let (other, second, _) = signed(4, 7, b"what the other saw");
        let mut witnessed = super::Witnessed::new();

        assert!(witnessed.take_in(one, &network, &first));
        assert!(witnessed.take_in(other, &network, &second));
        assert!(witnessed.contradictions().is_empty());
    }

    #[test]
    fn where_this_node_got_to_is_remembered_for_each_peer_separately() {
        // A position belongs to the record it is a position in. One number for everybody would
        // mean asking one node for a position in another's, and getting an answer about nothing.
        let one = PeerId::random();
        let other = PeerId::random();
        let mut read = ReadSoFar::default();

        read.asked(one);
        read.answered(one, 7);
        assert_eq!(read.of(&one), 7);
        assert_eq!(read.of(&other), 0, "and nothing has been read of theirs");
    }

    #[test]
    fn an_answer_nobody_asked_for_does_not_move_the_cursor() {
        // **The bug this exists to stop.** A duplicate, or an answer to a different question, would
        // otherwise push the position past records that are then never asked for again — and the
        // node would sit quietly missing them for ever.
        let peer = PeerId::random();
        let mut read = ReadSoFar::default();

        assert!(!read.answered(peer, 5), "nothing was outstanding");
        assert_eq!(read.of(&peer), 0);

        read.asked(peer);
        assert!(read.answered(peer, 5));
        assert_eq!(read.of(&peer), 5);

        assert!(
            !read.answered(peer, 5),
            "and the same answer again counts once"
        );
        assert_eq!(read.of(&peer), 5);
    }

    #[test]
    fn somebody_never_met_has_been_read_of_from_the_beginning() {
        // Which is what makes the first question to a new peer ask for everything.
        assert_eq!(ReadSoFar::default().of(&PeerId::random()), 0);
    }

    #[test]
    fn reading_more_adds_to_what_was_read_before() {
        let peer = PeerId::random();
        let mut read = ReadSoFar::default();
        read.asked(peer);
        read.answered(peer, 3);
        read.asked(peer);
        read.answered(peer, 4);
        assert_eq!(read.of(&peer), 7, "a page at a time, walking forward");
    }

    #[test]
    fn everybody_met_is_asked_again() {
        // A node that only asked whoever last had something would stop asking the quiet ones, and
        // quiet is what a node looks like just before it has something.
        let mut read = ReadSoFar::default();
        let met: Vec<PeerId> = (0..3).map(|_| PeerId::random()).collect();
        for peer in &met {
            read.asked(*peer);
            read.answered(*peer, 1);
        }
        assert_eq!(read.everybody().len(), 3);
    }
}
