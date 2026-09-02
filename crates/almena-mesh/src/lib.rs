//! How nodes reach each other, and what they are called when they do.
//!
//! **One key, one name.** A node's key signs its roots, names it in the record, and is who it is on
//! the mesh. That is the whole reason this is here rather than somewhere simpler: two identities
//! would be two censuses of nodes, and the day they disagreed one of them would be the weak one
//! everybody believed anyway.
//!
//! # Two networks cannot talk to each other, and it is not a check anybody can forget
//!
//! Which network a node is on rides **inside the name of the protocol** it offers:
//!
//! ```text
//! /almena/<the hash of the act that opened the network>/sync/1.0.0
//! ```
//!
//! A node on another network offers a protocol with a different name, so there is nothing to
//! negotiate and the connection produces nothing. Nobody has to remember to compare a field, and no
//! version of this can ship with the comparison missing — the two are separated by the thing that
//! decides whether they can speak at all.
//!
//! # What this is not
//!
//! It carries bytes between nodes and decides nothing about them. Whether an act is valid, whose
//! root is whose, and what is owed to whom are all settled where they were settled before, by
//! something that has no idea there is a network. Reaching a node is not the same as believing one.

pub mod keeping;
pub mod sync;
pub mod whose;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, PoisonError, RwLock};

use almena_node::SigningKey;
use libp2p::core::ConnectedPoint;
use libp2p::futures::StreamExt as _;
use libp2p::identity::Keypair;
/// The parts a multiaddress is made of, so a face can read one without depending on libp2p.
pub use libp2p::multiaddr::Protocol;
use libp2p::swarm::dial_opts::DialOpts;
use libp2p::swarm::{ConnectionId, SwarmEvent};
// Re-exported so that a face can hold an address without reaching past this crate for the type.
// A door somebody has to go around is a door.
pub use libp2p::Multiaddr;

use libp2p::{PeerId, Swarm, identify, request_response};

/// What every protocol name here starts with.
const PREFIX: &str = "/almena";

/// The protocol nodes replicate the record over.
const SYNC: &str = "/sync/1.0.0";

/// A node's identity on the mesh, built from the key it already has.
///
/// # Errors
///
/// [`NotListening::NoIdentity`] — which cannot happen for a key this node is already using, and is
/// returned rather than asserted because a node that stopped on a claim about its own key would be
/// worse than one that said what went wrong.
pub fn identity(key: &SigningKey) -> Result<Keypair, NotListening> {
    let mut secret = key.secret();
    let built = Keypair::ed25519_from_bytes(&mut secret).map_err(|_| NotListening::NoIdentity);
    // The bytes go as soon as they are used. It costs nothing, and means one fewer copy of a node's
    // key sitting in memory for as long as the process runs.
    secret.fill(0);
    built
}

/// What nodes on `network` call the protocol they replicate the record over.
///
/// The network's own name is inside it, which is what keeps two networks from talking rather than a
/// comparison somebody has to remember to write.
#[must_use]
pub fn syncing(network: &str) -> String {
    format!("{PREFIX}/{network}{SYNC}")
}

/// What a node does on the mesh: say who it is, and pass the record around.
///
/// Two and no more. Saying who it is comes first because a node that could not be recognised would
/// have nothing to offer; passing the record around is the whole point of there being a mesh.
/// What a node does on the mesh, and the events that come out of doing it.
///
/// A module of its own for one reason: the derive writes an event type beside the behaviour, whose
/// variants it names after the fields and which cannot carry doc comments. Rather than let the
/// whole crate go undocumented to accommodate that, the exception is kept to the four lines that
/// need it.
mod doing {
    #![allow(
        missing_docs,
        reason = "the derived event type is named from the fields below"
    )]

    use libp2p::{identify, request_response};

    #[derive(libp2p::swarm::NetworkBehaviour)]
    pub struct Doing {
        /// Telling whoever connects what this node is and what it offers.
        pub(crate) identify: identify::Behaviour,
        /// Asking for what came after a position, and for one act by name.
        pub(crate) sync: request_response::Behaviour<crate::sync::Talking>,
        /// Carrying other nodes' traffic, for the ones that cannot be dialled.
        ///
        /// **Switched on only where the node says it offers it.** Doing it is somebody else's
        /// bandwidth being spent, so it is a thing a node volunteers rather than a thing it is
        /// signed up to by being on the network.
        pub(crate) relaying: libp2p::swarm::behaviour::toggle::Toggle<libp2p::relay::Behaviour>,
        /// Being carried, for when this node is the one that cannot be dialled.
        pub(crate) carried: libp2p::relay::client::Behaviour,
        /// How far away each peer is, asked over and over for as long as it is connected.
        ///
        /// **The only measurement this node takes of a connection.** Everything else it knows
        /// about a peer it was told or observed; a round trip is a thing it goes and finds out,
        /// and it is what makes a list of peers something a person can read rather than count.
        pub(crate) far: libp2p::ping::Behaviour,
    }
}

pub use doing::{Doing, DoingEvent};

/// A node listening on the mesh.
///
/// Holding one means it is reachable. Dropping it stops it, and nothing else does.
pub struct Listening {
    /// What the operating system actually gave it, which is not always what was asked for.
    addresses: Addresses,
    /// The swarm, kept because dropping it is what closes the listener.
    swarm: Swarm<Doing>,
    /// What has crossed, counted by the codec every byte goes through.
    crossed: crate::sync::Crossed,
    /// The questions put and not yet answered, and who each went to.
    ///
    /// Emptied as answers arrive and when a peer goes, so it holds what is genuinely outstanding.
    outstanding: BTreeMap<request_response::OutboundRequestId, (PeerId, Asked)>,
    /// How many questions this node has put, which is what gives each one its number.
    put: u64,
    /// Which relay each circuit was asked of, in the order they were asked.
    ///
    /// **Kept because a refusal does not name one.** A slot that is refused, or granted and then
    /// withdrawn, arrives as the circuit ending — and a node told only that something ended cannot
    /// say which relay stopped carrying it, or go and ask another.
    asked_of: Vec<(libp2p::core::transport::ListenerId, PeerId)>,
    /// Which listener each lent address came through.
    ///
    /// **So that losing one relay loses one relay.** A node carried by two that dropped every
    /// circuit when either ended would withdraw an address that still answers — and would then be
    /// wrong in the honest-looking direction, which is still wrong.
    lent: Vec<(libp2p::core::transport::ListenerId, Multiaddr)>,
    /// Where each dial this node made was going, until it is known whether it got there.
    ///
    /// **Kept because a failed dial does not name its address.** What comes back names the attempt,
    /// and a node told only that an attempt failed could not say where not to bother again — or
    /// where to try again later, which is what whoever drives this wants to know.
    dialled: BTreeMap<ConnectionId, Multiaddr>,
    /// Who is connected right now, shared with whoever wants to read it without stopping the mesh.
    peers: Peers,
}

/// Where this node is listening, readable from anywhere.
///
/// **The addresses the operating system granted, not the ones asked for.** A node told to listen on
/// port zero is given one, and a machine with several addresses is reachable at each — so what a
/// zone should carry is what was granted, and publishing what was requested is how a record ends up
/// pointing somewhere nothing is listening.
///
/// It is a handle for the same reason [`Peers`] is: the socket is handed to whatever keeps the mesh
/// up, and afterwards nothing else holds it. **Reported and never written down** — what a node says
/// about where it is stays its operator's decision (`SPECS.md §17.18`), and this is what lets a face
/// show them the answer without the node having taken it.
///
/// Cloning it clones the handle, not the list.
#[derive(Debug, Clone, Default)]
pub struct Addresses(Arc<RwLock<Vec<Multiaddr>>>);

impl Addresses {
    /// Every address it is listening on, as a copy taken at this moment.
    #[must_use]
    pub fn all(&self) -> Vec<Multiaddr> {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The port it is actually listening on, if it got one.
    ///
    /// This is the value a `_seed` record cannot be written without.
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        self.all().iter().find_map(|address| {
            address.iter().find_map(|part| match part {
                Protocol::Tcp(port) => Some(port),
                _ => None,
            })
        })
    }

    /// Whether it already holds this one.
    fn has(&self, address: &Multiaddr) -> bool {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(address)
    }

    /// One more.
    fn add(&self, address: Multiaddr) {
        self.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .push(address);
    }

    /// Keep only the ones this says to keep.
    fn keep(&self, wanted: impl FnMut(&Multiaddr) -> bool) {
        self.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(wanted);
    }
}

/// What is known about one connected peer.
#[derive(Debug, Clone)]
pub struct Reached {
    /// The address this connection is on.
    pub address: Multiaddr,
    /// The last round trip measured to it, or nothing where none has come back yet.
    ///
    /// **Absent for a while after connecting, and that is not a fault.** The first ping goes out
    /// after the connection settles, so a peer that has just arrived has no round trip and a face
    /// drawing a nought would be inventing the fastest connection on the list.
    pub far: Option<std::time::Duration>,
}

/// Who a node is connected to right now, readable from anywhere.
///
/// **Cheap on purpose, and never behind the mesh.** A face that draws a peer count has to read it
/// on every frame, from a thread that is not running the mesh and must not wait for it — so this
/// is a shared map behind a plain lock rather than a question put to the socket. It says who is
/// connected, which is a fact about sockets; who is a node the record knows is a different
/// question, and is answered in the record.
///
/// Cloning it clones the handle, not the map: every copy sees the same peers.
#[derive(Debug, Clone, Default)]
pub struct Peers(Arc<RwLock<BTreeMap<PeerId, Reached>>>);

impl Peers {
    /// How many are connected right now.
    #[must_use]
    pub fn count(&self) -> usize {
        self.0.read().unwrap_or_else(PoisonError::into_inner).len()
    }

    /// Who is connected right now, as a copy taken at this moment.
    #[must_use]
    pub fn connected(&self) -> BTreeSet<PeerId> {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .copied()
            .collect()
    }

    /// Who is connected and at which address, as a copy taken at this moment.
    ///
    /// **The address the connection is actually on**, which is a different fact from any address
    /// the record or the zone carries: those say where a node said it could be reached, and this
    /// says where this node is talking to it — dialled, or answered and observed. It is what lets
    /// a face say *ip4/tcp* beside a peer instead of only counting it.
    ///
    /// It is a fact about a socket and it is never written down anywhere (§17.18): where a peer
    /// was reached in fact stays with the node that reached it.
    #[must_use]
    pub fn reached(&self) -> BTreeMap<PeerId, Reached> {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Somebody connected, at the address the connection is on.
    fn met(&self, peer: PeerId, address: Multiaddr) {
        self.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(peer, Reached { address, far: None });
    }

    /// A round trip to somebody came back.
    ///
    /// **The last one, not an average.** What a person reads a latency for is *how far away is it
    /// now*, and a mean over a connection that has been up for an hour hides the minute it went
    /// bad. A measurement for a peer that has since gone is dropped rather than kept.
    fn far(&self, peer: &PeerId, took: std::time::Duration) {
        if let Some(reached) = self
            .0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .get_mut(peer)
        {
            reached.far = Some(took);
        }
    }

    /// Somebody's last connection ended.
    fn lost(&self, peer: &PeerId) {
        self.0
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(peer);
    }
}

impl Listening {
    /// A handle on who this node is connected to, for reading from anywhere.
    ///
    /// Taken before the socket is handed to whatever keeps it up, because afterwards nothing else
    /// holds it — and a face that wanted a peer count then would have nobody to ask.
    #[must_use]
    pub fn peers(&self) -> Peers {
        self.peers.clone()
    }

    /// A handle on what has crossed this node's mesh, for reading from anywhere.
    ///
    /// Taken the same way and for the same reason as [`Listening::peers`]: afterwards nothing else
    /// holds it, and a face that wanted the figure then would have nobody to ask.
    #[must_use]
    pub fn crossed(&self) -> crate::sync::Crossed {
        self.crossed.clone()
    }

    /// Where this node can be reached, as the addresses it really got.
    ///
    /// **Asked for rather than assumed.** A node told to listen on port zero is given one by the
    /// operating system, and a node on a machine with several addresses is reachable at more than
    /// one — publishing what was requested instead of what was granted is how a zone ends up
    /// pointing somewhere nothing is listening.
    #[must_use]
    pub fn addresses(&self) -> Vec<Multiaddr> {
        self.addresses.all()
    }

    /// A handle on where this node is listening, for reading from anywhere.
    ///
    /// Taken the same way and for the same reason as [`Listening::peers`]: afterwards nothing else
    /// holds it, and a face that wanted to show an operator where their node can be reached would
    /// have nobody to ask.
    #[must_use]
    pub fn where_it_listens(&self) -> Addresses {
        self.addresses.clone()
    }

    /// The port it is actually listening on, if it got one.
    ///
    /// This is the value a `_seed` record cannot be written without.
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        self.addresses.port()
    }

    /// Wait for the next thing worth telling somebody about.
    ///
    /// **It returns**, which is the point: a node that ran the mesh in a loop of its own would
    /// leave whoever started it with nothing to say and nowhere to say it. What each event means
    /// is decided here; what is done about it is not.
    ///
    /// Nothing is replicated yet. What this buys is that the node is reachable and knows where.
    pub async fn next(&mut self) -> Happened {
        loop {
            let event = self.swarm.select_next_some().await;
            if let Some(happened) = self.meaning(event) {
                return happened;
            }
        }
    }

    /// What one thing the transport reported means, if it means anything to anybody outside.
    ///
    /// [`None`] is the transport getting on with itself, and saying so would be noise in the one
    /// place somebody looks when something is wrong.
    fn meaning(&mut self, event: SwarmEvent<DoingEvent>) -> Option<Happened> {
        match event {
            SwarmEvent::NewListenAddr {
                listener_id,
                address,
            } => self.now_reachable(listener_id, address),
            // A slot refused, or granted and later withdrawn, arrives as the circuit ending and
            // never as an answer of its own — so what a relay would not do is read from the
            // listener going away, and the addresses through it stop being published.
            SwarmEvent::ListenerClosed { listener_id, .. }
            | SwarmEvent::ListenerError { listener_id, .. } => {
                self.stopped_carrying(listener_id).map(Happened::NotCarried)
            }
            SwarmEvent::ConnectionEstablished {
                peer_id,
                connection_id,
                endpoint,
                ..
            } => Some(self.met(peer_id, connection_id, &endpoint)),
            SwarmEvent::OutgoingConnectionError { connection_id, .. } => {
                self.not_reached(connection_id)
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                num_established,
                ..
            } => self.closed(peer_id, num_established),
            // A round trip came back, or did not. Nothing is said to whoever drives this: it is
            // not something that happened to the node, it is a number about a peer that already
            // exists, and it is read off `peers()` beside everything else about that peer.
            SwarmEvent::Behaviour(DoingEvent::Far(libp2p::ping::Event {
                peer,
                result: Ok(took),
                ..
            })) => {
                self.peers.far(&peer, took);
                None
            }
            SwarmEvent::Behaviour(DoingEvent::Far(_)) => None,
            SwarmEvent::Behaviour(DoingEvent::Sync(event)) => self.spoken(event),
            _ => None,
        }
    }

    /// What one thing said or not said over the record protocol means.
    fn spoken(
        &mut self,
        event: request_response::Event<sync::Ask, sync::Said>,
    ) -> Option<Happened> {
        match event {
            request_response::Event::Message { peer, message, .. } => {
                Some(self.said(peer, message))
            }
            // A question that will not be answered, said as one. Cleared from the outstanding list
            // here as well as when a connection ends, so that what is held is what is genuinely
            // still open.
            request_response::Event::OutboundFailure {
                peer,
                request_id,
                error,
                ..
            } => {
                let asked = self
                    .outstanding
                    .remove(&request_id)
                    .map_or(Asked(0), |(_, asked)| asked);
                Some(Happened::Unanswered(peer, asked, why(&error)))
            }
            _ => None,
        }
    }

    /// A connection came about, whichever end opened it.
    fn met(
        &mut self,
        peer: PeerId,
        connection: ConnectionId,
        endpoint: &ConnectedPoint,
    ) -> Happened {
        self.dialled.remove(&connection);
        self.peers.met(peer, on(endpoint));
        Happened::Met(peer, met(endpoint))
    }

    /// A dial this node made did not give a connection.
    ///
    /// **The address, and not the attempt.** What failed is named by its attempt, which means
    /// nothing to anybody outside; what whoever drives this wants to know is where not to bother
    /// again just yet — and where to try again later. [`None`] for an attempt this node did not
    /// make by name, which is the transport's own business.
    fn not_reached(&mut self, connection: ConnectionId) -> Option<Happened> {
        self.dialled.remove(&connection).map(Happened::NotReached)
    }

    /// A connection to somebody ended, and this is whether that means they have gone.
    ///
    /// **Two nodes that dialled each other hold two connections**, one each way, and one of them
    /// ending is not the two parting: the other still carries questions and answers. Said only
    /// when the last one goes, so that whoever hears it can act on it — forgetting what was asked,
    /// or dialling again — without acting on a peer that is still there.
    fn closed(&mut self, peer: PeerId, remaining: u32) -> Option<Happened> {
        if remaining > 0 {
            return None;
        }
        // Whatever was asked of them will not be answered now, and holding on to it would be
        // holding on to it for as long as the node ran.
        self.outstanding
            .retain(|_, (asked_of, _)| *asked_of != peer);
        self.peers.lost(&peer);
        Some(Happened::Parted(peer))
    }

    /// One message off the wire: a question somebody put, or an answer to one this node put.
    fn said(
        &mut self,
        peer: PeerId,
        message: request_response::Message<sync::Ask, sync::Said>,
    ) -> Happened {
        match message {
            request_response::Message::Request {
                request, channel, ..
            } => Happened::Asked(peer, request, Answering(channel)),
            request_response::Message::Response {
                request_id,
                response,
            } => {
                // A question nobody here put would be an answer to nothing, which is what the zero
                // says: it matches no outstanding question and so moves nothing.
                let asked = self
                    .outstanding
                    .remove(&request_id)
                    .map_or(Asked(0), |(_, asked)| asked);
                Happened::Answered(peer, asked, response)
            }
        }
    }

    /// Take in an address this node has just been given, if it is one it did not have.
    ///
    /// A circuit arrives the same way and is not the same fact: it says a relay has agreed to carry
    /// this node, which is somebody else's agreement and is reported as one.
    fn now_reachable(
        &mut self,
        listener: libp2p::core::transport::ListenerId,
        address: Multiaddr,
    ) -> Option<Happened> {
        if self.addresses.has(&address) {
            return None;
        }
        // **A node that carries has to say where it is, or it has nothing to lend.** What a relay
        // hands back for a slot is the way in through itself, so one that had claimed no address of
        // its own would grant a reservation to nowhere. Only addresses somebody else could use are
        // offered: lending loopback would be handing somebody a way in that leads to themselves.
        if self.carries() && !borrowed(&address) && worth_publishing(&address) {
            self.swarm.add_external_address(address.clone());
        }
        self.addresses.add(address.clone());
        Some(if borrowed(&address) {
            self.lent.push((listener, address.clone()));
            Happened::Carried(address)
        } else {
            Happened::Reachable(address)
        })
    }

    /// Which relay stopped carrying this node, if that is what ended.
    ///
    /// A listener ending is also how an ordinary listener ends, so the ones this node asked a relay
    /// for are the only ones that mean anything here.
    fn stopped_carrying(
        &mut self,
        listener: libp2p::core::transport::ListenerId,
    ) -> Option<PeerId> {
        let at = self
            .asked_of
            .iter()
            .position(|(asked, _)| *asked == listener)?;
        let (_, relay) = self.asked_of.remove(at);
        // Only what came through **that** listener. A node carried by two relays that lost one has
        // lost one, and withdrawing the other's address would be withdrawing a door somebody is
        // still behind.
        // Only what came through **that** listener. A node carried by two relays that lost one has
        // lost one, and withdrawing the other's address would be withdrawing a door somebody is
        // still behind.
        self.addresses.keep(|address| {
            !self
                .lent
                .iter()
                .any(|(lender, lent)| *lender == listener && lent == address)
        });
        self.lent.retain(|(lender, _)| *lender != listener);
        self.lent.retain(|(lender, _)| *lender != listener);
        Some(relay)
    }
}

/// The way back to whoever asked.
///
/// Held rather than answered immediately because **what the answer is, is not this crate's to
/// decide**: it hands the question up and puts back whatever comes down. A channel that is dropped
/// without an answer leaves the asker with a failed request, which is the truthful outcome when a
/// node has nothing to say.
pub struct Answering(request_response::ResponseChannel<sync::Said>);

impl core::fmt::Debug for Answering {
    /// It prints as what it is. There is nothing inside worth showing and nothing that would mean
    /// anything to whoever read it.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("Answering")
    }
}

/// Something a node on the mesh may want said out loud.
///
/// It does not clone and it does not compare: one of these carries the way back to whoever asked,
/// and a copy of that would be two answers to one question.
#[derive(Debug)]
pub enum Happened {
    /// It can now be reached here.
    ///
    /// **Where a publishable address comes from**: what the operating system granted, rather than
    /// what was asked for.
    Reachable(Multiaddr),
    /// A relay agreed to carry this node, and this is where that makes it reachable.
    ///
    /// **Worth publishing and worth marking as lent.** It answers for as long as that relay keeps
    /// carrying it, which is not a promise anybody made and not something this node is told when it
    /// ends.
    Carried(Multiaddr),
    /// A relay would not carry it. Full, unwilling, or gone.
    ///
    /// Not on its own a reason to stop: another relay is a thing to ask. It is a reason not to
    /// publish an address through this one.
    NotCarried(PeerId),
    /// Somebody connected, or this node connected to them.
    ///
    /// Once per connection, and two nodes that dialled each other hold two — so it may be heard
    /// twice of one peer, and the second time says how the other connection came about.
    Met(PeerId, Meeting),
    /// The last connection to somebody ended. Ordinary, and not on its own a sign of anything.
    ///
    /// **The last, and not any.** A node holding two connections to one peer that lost one still
    /// has the peer, and saying otherwise would have whoever listens dial somebody they are
    /// already talking to.
    Parted(PeerId),
    /// This node dialled there and nobody answered — or the wrong somebody did.
    ///
    /// **Where, rather than what went wrong.** Nobody listening, a name that would not resolve, and
    /// a machine at that address holding a different key all come to the same thing for whoever
    /// drives this: that address did not give a connection just now, and may later.
    NotReached(Multiaddr),
    /// Somebody asked something. It is not answered here — [`Listening::answer`] is.
    Asked(PeerId, sync::Ask, Answering),
    /// Something that was asked for came back.
    ///
    /// **Nothing about it is believed for having arrived.** The acts inside are somebody else's
    /// signed bytes and go through the same admission as any other; the peer that sent them
    /// vouches for nothing, including itself.
    Answered(PeerId, Asked, sync::Said),
    /// A question this node put will not be answered, and why.
    ///
    /// **Reported rather than left to a timeout somebody wrote themselves.** A node that only ever
    /// heard about answers would hold a question open for as long as it ran, and would have no way
    /// to tell *nobody has replied yet* from *this peer cannot reply at all*.
    ///
    /// [`Unanswerable::NotOnThisNetwork`] is the one that matters most, and it is `SPECS.md §4.12`
    /// happening: two networks do not talk because the network's own name is inside the name of the
    /// protocol, so there is nothing to negotiate. It arrives here as an answer that will not come
    /// rather than as a comparison somebody remembered to write.
    Unanswered(PeerId, Asked, Unanswerable),
}

/// Why a question will not be answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unanswerable {
    /// They offer no protocol this node speaks.
    ///
    /// **Which is what being on another network looks like from here** (`SPECS.md §4.12`): the
    /// protocol is named `/almena/<the hash of the act that opened the network>/sync/1.0.0`, so a
    /// node on another network offers a name this one never asks for. It is not a check that can be
    /// forgotten, because it is what decides whether the two can speak at all.
    NotOnThisNetwork,
    /// The connection went before an answer came back.
    TheyWentAway,
    /// Nothing came back in time.
    NothingCameBack,
    /// Something went wrong on the wire.
    TheWireFailed,
}

/// How a connection came about, and what that says about where somebody can be reached.
///
/// **The two are not the same fact and must not be stored as one.** Dialling somebody and being
/// answered proves they can be reached there — the address had to work, which is what makes it
/// worth having beside what they merely published. Being dialled proves only where they came
/// *from*: an outbound connection leaves from whatever port the machine had spare, so recording it
/// as an address that peer can be reached at would be recording something that was never true and
/// will not be true twice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Meeting {
    /// This node dialled them and they answered, there.
    Dialled(Multiaddr),
    /// They dialled this node. Where they came from is not where they can be found.
    Answered,
}

/// What a failed question says about why it failed.
fn why(error: &request_response::OutboundFailure) -> Unanswerable {
    match error {
        // **The one that is not a fault.** Dialling somebody on another network reaches them, and
        // then there is no protocol in common: the name of this one has the network inside it.
        request_response::OutboundFailure::UnsupportedProtocols => Unanswerable::NotOnThisNetwork,
        request_response::OutboundFailure::ConnectionClosed
        | request_response::OutboundFailure::DialFailure => Unanswerable::TheyWentAway,
        request_response::OutboundFailure::Timeout => Unanswerable::NothingCameBack,
        request_response::OutboundFailure::Io(_) => Unanswerable::TheWireFailed,
    }
}

/// What a connection says about where the other end can be reached.
/// The address a connection is on, whichever end opened it.
///
/// A dial knows where it went. A connection this node answered knows where the other end appeared
/// to come from — `send_back_addr` — which is what the transport observed and not a claim anybody
/// made about themselves. Neither is written down; both are what a face draws beside a peer.
fn on(endpoint: &ConnectedPoint) -> Multiaddr {
    match endpoint {
        ConnectedPoint::Dialer { address, .. } => address.clone(),
        ConnectedPoint::Listener { send_back_addr, .. } => send_back_addr.clone(),
    }
}

fn met(endpoint: &ConnectedPoint) -> Meeting {
    match endpoint {
        ConnectedPoint::Dialer { address, .. } => Meeting::Dialled(address.clone()),
        ConnectedPoint::Listener { .. } => Meeting::Answered,
    }
}

/// One question this node put, so that the answer says which.
///
/// **Without it an answer is only *something arrived from that peer*.** Two questions in flight to
/// one node come back as two answers of the same shape, and a node reading them by arrival would
/// take the answer to one as the answer to the other — moving a cursor past records it then never
/// asks for again, or counting a reply it never asked for as availability.
///
/// It is the transport's own number and never travels: what the two ends agree on is the exchange,
/// not a name for it, so there is nothing here anybody could claim to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Asked(u64);

impl Asked {
    /// A question by its number, for tests that need two that are not each other.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn numbered(number: u64) -> Self {
        Self(number)
    }
}

impl Listening {
    /// Put an answer back to whoever asked.
    ///
    /// # Errors
    ///
    /// The answer, back, when the asker has already gone. Common and not a fault: the way to find
    /// out somebody stopped waiting is that they stopped waiting.
    pub fn answer(&mut self, back: Answering, said: sync::Said) -> Result<(), sync::Said> {
        self.swarm.behaviour_mut().sync.send_response(back.0, said)
    }

    /// Ask somebody something.
    ///
    /// The answer arrives later, as [`Happened::Answered`], because a node that waited for one
    /// would be a node that stopped listening to everybody else while it did.
    pub fn ask(&mut self, peer: &PeerId, question: sync::Ask) -> Asked {
        self.put += 1;
        let asked = Asked(self.put);
        let sent = self.swarm.behaviour_mut().sync.send_request(peer, question);
        self.outstanding.insert(sent, (*peer, asked));
        asked
    }

    /// Dial somebody, so that there is a connection to ask over.
    ///
    /// **From a port of its own, never from the one it listens on.** Dialling from the listening
    /// port is the transport's default, and it is what makes two nodes that dial each other at
    /// the same moment fail to connect at all: each leaves from its own listening port towards
    /// the other's, the operating system sees one connection opened from both ends at once, and
    /// both ends then try to speak first — a handshake in which nobody answers. Two seeds coming
    /// up together is exactly that moment, and it must not be the moment the network does not
    /// form. A fresh port makes the two dials two connections, which is what they were.
    ///
    /// The identity an address ends in is still held to: whoever answers has to prove they hold
    /// that key, and an impostor at the right host and port arrives as [`Happened::NotReached`].
    ///
    /// # Errors
    ///
    /// [`NotListening::AddressUnavailable`] when that address cannot be dialled at all. Somebody
    /// not answering is not an error here — it arrives later, as [`Happened::NotReached`], or a
    /// connection does.
    pub fn dial(&mut self, address: Multiaddr) -> Result<(), NotListening> {
        let dialling = DialOpts::unknown_peer_id()
            .address(address.clone())
            .allocate_new_port()
            .build();
        let attempt = dialling.connection_id();
        self.swarm
            .dial(dialling)
            .map_err(|_| NotListening::AddressUnavailable)?;
        self.dialled.insert(attempt, address);
        Ok(())
    }

    /// Whether this node carries other nodes' traffic.
    ///
    /// **What the socket does, not what somebody meant to switch on.** It is what the record ends
    /// up saying this node offers, so it is read from the thing that would actually carry it.
    #[must_use]
    pub fn carries(&self) -> bool {
        self.swarm.behaviour().relaying.is_enabled()
    }

    /// The same as [`Self::ask_to_be_carried`], from an address as somebody wrote it.
    ///
    /// # Errors
    ///
    /// [`NotListening::AddressUnavailable`] when that is not an address at all, and otherwise
    /// whatever [`Self::ask_to_be_carried`] gives.
    pub fn ask_to_be_carried_at(&mut self, relay: &str) -> Result<Multiaddr, NotListening> {
        let address: Multiaddr = relay
            .parse()
            .map_err(|_| NotListening::AddressUnavailable)?;
        self.ask_to_be_carried(&address).map(|()| address)
    }

    /// Ask a node that carries traffic to carry this one's.
    ///
    /// **For a node that cannot be dialled**, which is most of the machines a person owns: behind a
    /// household router there is no address anybody outside can knock on, and without this such a
    /// machine could hold the record and answer nothing. A node that has an address of its own has
    /// no business asking.
    ///
    /// It is a request and not a setting. Whether a slot is granted is the relay's to decide, and
    /// the answer arrives later as [`Happened::Carried`] or [`Happened::NotCarried`] — asking is
    /// not being carried, and a node that published an address on the strength of having asked
    /// would be publishing somewhere nothing answers.
    ///
    /// # Errors
    ///
    /// [`NotListening::Anonymous`] when the address does not say **which** node the relay is. A
    /// circuit runs through somebody, and being carried by whoever happens to answer at a host and
    /// port is being carried by whoever took that host and port.
    ///
    /// [`NotListening::AddressUnavailable`] when the circuit cannot be listened on at all.
    pub fn ask_to_be_carried(&mut self, relay: &Multiaddr) -> Result<(), NotListening> {
        let Some(Protocol::P2p(who)) = relay.iter().find(|part| matches!(part, Protocol::P2p(_)))
        else {
            return Err(NotListening::Anonymous);
        };
        let listener = self
            .swarm
            .listen_on(relay.clone().with(Protocol::P2pCircuit))
            .map_err(|_| NotListening::AddressUnavailable)?;
        self.asked_of.push((listener, who));
        Ok(())
    }

    /// Stop being carried by that relay, giving the slot back.
    ///
    /// **A slot held is a slot somebody else has not got.** A node that found an address of its own
    /// — the router opened, the machine moved — should say so rather than keep a place it no longer
    /// needs, and the addresses it publishes should stop including the circuit at the same time.
    pub fn carry_me_no_longer(&mut self, through: &Multiaddr) {
        self.addresses.keep(|address| address != through);
        self.swarm.remove_external_address(through);
    }
}

/// Whether an address is one anybody on the internet could reach this node at.
///
/// **A stricter question than [`worth_publishing`], and they are not the same one.** That one asks
/// whether an address is worth putting in the *record*, where a node on a household network is
/// genuinely reachable by the other nodes on it — a development network on one LAN works because
/// of exactly that. This asks whether an address is worth putting in a **public zone**, where the
/// people reading it are anywhere, and there a private address is two things at once: useless,
/// because nobody outside can dial it, and a small leak, because it describes somebody's LAN to
/// everybody who looks.
///
/// So the private ranges go, and so do the ones that look public and are not: carrier-grade NAT
/// (`100.64/10`), unique local addresses (`fc00::/7`) and the documentation ranges, which is what
/// an overlay network or a mistyped example leaves behind.
#[must_use]
pub fn reachable_from_anywhere(address: &Multiaddr) -> bool {
    // Written as *not any of these* rather than as a run of negations: the list is what is being
    // kept out, and reading it that way is reading what it is for.
    address.iter().all(|part| match part {
        Protocol::Ip4(at) => {
            let [first, second, ..] = at.octets();
            !(at.is_loopback()
                || at.is_unspecified()
                || at.is_link_local()
                || at.is_broadcast()
                || at.is_private()
                || at.is_documentation()
                // Carrier-grade NAT, and what an overlay hands out. Not private by the letter of
                // the word, and not dialable from outside either.
                || (first == 100 && (64..128).contains(&second))
                // Benchmarking, which is nobody's address and turns up in copied configuration.
                || (first == 198 && (18..20).contains(&second)))
        }
        Protocol::Ip6(at) => {
            let [first, second, ..] = at.segments();
            !(at.is_loopback()
                || at.is_unspecified()
                || at.is_multicast()
                // Unique local: the IPv6 answer to a private range, and what overlays hand out.
                || (first & 0xfe00) == 0xfc00
                // Link local.
                || (first & 0xffc0) == 0xfe80
                // Documentation.
                || (first == 0x2001 && second == 0x0db8))
        }
        _ => true,
    })
}

/// Where the host name goes, which is whoever keeps the zone's to choose and nobody else's.
pub const HOST: &str = "<name>";

/// What a zone would have to carry for this node to be a seed.
///
/// # It is composed here and not on a screen
///
/// The shape of these records is the platform's (`ZONES.md`), not a face's. Composed on each face
/// it would be composed twice and the two would drift — and for a record that **newcomers verify
/// against** that is worse than it sounds: a `_seed` carrying the wrong `net=` sends whoever reads
/// it to a network that is not this one. So the node says it and the faces only carry it.
///
/// # What goes in, and the one thing that cannot
///
/// The port actually bound, this node's own public key, and the name of its network are the parts
/// nobody else can produce. **The host name is not among them** — it is the zone keeper's choice,
/// so [`HOST`] stands in its place rather than being guessed at.
///
/// Only addresses **anybody could reach this node at** are listed — which is stricter than what the
/// record carries: a private address is genuinely reachable by the other nodes on that LAN and is
/// useless in a public zone, besides describing somebody's network to everybody who looks. See
/// [`reachable_from_anywhere`].
///
/// A relayed address is left out too, even though it is reachable: a circuit answers for as long as
/// another node agrees to carry this one and stops without it being told, so a zone pointing at one
/// points at a door nobody is behind.
///
/// # It says where this node thinks it is
///
/// A node knows what it bound, which behind a household router is not what anybody else can dial.
/// Whoever keeps the zone checks the record before publishing it — dial the address, see that the
/// handshake key is the `peer=`, see that the record handed over starts with the act `net=` names.
/// That is what they have to do anyway, and it is why this is a draft and not a publication.
#[must_use]
pub fn seed_record(
    peer: &str,
    network: &str,
    port: u16,
    serving_on: Option<u16>,
    addresses: &[Multiaddr],
) -> String {
    let mut lines = vec![format!(
        "_seed  v=1 host={HOST} port={port} peer={peer} net={network}"
    )];
    if let Some(at) = serving_on {
        lines.push(format!("_api   v=1 url=https://{HOST}:{at} peer={peer}"));
    }
    for address in addresses {
        if !reachable_from_anywhere(address) || borrowed(address) {
            continue;
        }
        match address.iter().next() {
            Some(Protocol::Ip4(at)) => lines.push(format!("{HOST}  A     {at}")),
            Some(Protocol::Ip6(at)) => lines.push(format!("{HOST}  AAAA  {at}")),
            _ => {}
        }
    }
    lines.join("\n")
}

/// Whether an address is one somebody else could reach this node at.
///
/// **Not every address the operating system grants is a way in.** Loopback is where every machine
/// on earth is, and a node that published it would be publishing an address that resolves, on the
/// reader's machine, to the reader. The unspecified address is *all of them*, which is a way of
/// listening and not a place. Link-local works only for whoever is already on the wire.
///
/// It matters more than it looks. Where nodes are is counted — how spread out the copies of the
/// record are is a figure anybody can read — and an address every node shares would make a network
/// on one machine look as spread out as one across three countries. The count would not be wrong by
/// a little; it would be wrong in the direction that hides the thing it exists to show.
///
/// A circuit is judged by the relay it runs through, which is the address anybody would dial.
#[must_use]
pub fn worth_publishing(address: &Multiaddr) -> bool {
    address.iter().all(|part| match part {
        Protocol::Ip4(at) => {
            !at.is_loopback() && !at.is_unspecified() && !at.is_link_local() && !at.is_broadcast()
        }
        Protocol::Ip6(at) => !at.is_loopback() && !at.is_unspecified(),
        _ => true,
    })
}

/// Whether an address is one this node holds or one it is lent.
///
/// **Two different facts and never one.** An address of its own answers for as long as the node
/// runs; a circuit answers for as long as somebody else agrees to carry it, and stops without this
/// node doing anything or being told. Publishing them as the same kind of address is how a record
/// comes to point at a door nobody is behind.
#[must_use]
pub fn borrowed(address: &Multiaddr) -> bool {
    address
        .iter()
        .any(|part| matches!(part, Protocol::P2pCircuit))
}

/// Where a seed from the zone can be dialled.
///
/// **The peer identity goes in the address**, which is what turns a redirected record into a failed
/// connection rather than a wrong node: whoever answers has to prove they hold that key before
/// anything is said to them, and an impostor at the right host and port cannot.
///
/// The host is left as a name rather than resolved here. A name may carry an address of either
/// kind, or both, and which one gets used is the dialler's business — a node that resolved it
/// itself would be choosing on somebody else's behalf.
///
/// # Errors
///
/// [`NotListening::AddressUnavailable`] when the record's own parts do not make an address, which
/// means the identity in it is not one.
pub fn dialling(seed: &almena_node::zone::Seed) -> Result<Multiaddr, NotListening> {
    format!(
        "/dns/{}/tcp/{}/p2p/{}",
        seed.host(),
        seed.port(),
        seed.peer()
    )
    .parse()
    .map_err(|_| NotListening::AddressUnavailable)
}

/// Everything a node does on the mesh, built over the transports it does it on.
///
/// **Being carried is set up whether or not this node ever needs it**, because whether it can be
/// dialled is not something it knows about itself: it finds out by nobody arriving. Carrying, by
/// contrast, is only there where it was volunteered.
fn swarm_of(
    key: &SigningKey,
    network: &str,
    carrying: Carrying,
    crossed: crate::sync::Crossed,
) -> Result<Swarm<Doing>, NotListening> {
    let offering = syncing(network);
    Ok(libp2p::SwarmBuilder::with_existing_identity(identity(key)?)
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )
        .map_err(|_| NotListening::NoTransport)?
        // **Without this a seed cannot be dialled at all.** A zone publishes a host name, not an
        // address — because a name carries whichever addresses that machine has and picking one
        // here would be choosing on its behalf — so the transport has to be able to resolve one.
        .with_dns()
        .map_err(|_| NotListening::NoTransport)?
        .with_relay_client(libp2p::noise::Config::new, libp2p::yamux::Config::default)
        .map_err(|_| NotListening::NoTransport)?
        .with_behaviour(|keys, carried| Doing {
            carried,
            relaying: libp2p::swarm::behaviour::toggle::Toggle::from(
                matches!(carrying, Carrying::ForOthers).then(|| {
                    libp2p::relay::Behaviour::new(
                        keys.public().to_peer_id(),
                        libp2p::relay::Config::default(),
                    )
                }),
            ),
            identify: identify::Behaviour::new(identify::Config::new(
                offering.clone(),
                keys.public(),
            )),
            far: libp2p::ping::Behaviour::default(),
            sync: request_response::Behaviour::with_codec(
                crate::sync::Talking { crossed },
                [(offering, request_response::ProtocolSupport::Full)],
                request_response::Config::default(),
            ),
        })
        .map_err(|_| NotListening::NoTransport)?
        .build())
}

/// Why a node could not take its place on the mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotListening {
    /// The key would not make an identity, which a key already in use cannot fail to do.
    NoIdentity,
    /// The transport could not be built at all.
    NoTransport,
    /// The address did not say which node it is, and a circuit has to run through somebody known.
    Anonymous,
    /// That address or port could not be listened on.
    ///
    /// Usually somebody else already has it, which is a thing to go and look at rather than to
    /// work around by quietly choosing another — a node whose port moved is a node whose published
    /// record is now wrong.
    AddressUnavailable,
}

/// Take a place on the mesh, listening on `port`.
///
/// `network` is the name of the act that opened this network, and it goes inside the protocol name
/// so that two networks have nothing to say to one another.
///
/// Port zero means *whatever is free*, which is right for a test and wrong for a node whose address
/// somebody has published.
///
/// # Errors
///
/// [`NotListening`].
pub fn listen(key: &SigningKey, network: &str, port: u16) -> Result<Listening, NotListening> {
    listening(key, network, port, Carrying::ForNobody)
}

/// Whether this node carries other nodes' traffic.
///
/// **Volunteered, never assumed.** Relaying spends this node's bandwidth on somebody else's
/// conversation, so it is a thing an operator turns on — and having turned it on is a thing the
/// node says in the record and is measured on, like anything else it offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carrying {
    /// It carries traffic for nodes that cannot be dialled.
    ForOthers,
    /// It does not. The ordinary case, and not a lesser one.
    ForNobody,
}

/// The same, saying whether this node carries other nodes' traffic.
///
/// # Errors
///
/// [`NotListening`], exactly as [`listen`].
pub fn listening(
    key: &SigningKey,
    network: &str,
    port: u16,
    carrying: Carrying,
) -> Result<Listening, NotListening> {
    let crossed = crate::sync::Crossed::default();
    let mut swarm = swarm_of(key, network, carrying, crossed.clone())?;

    // Both, because a machine that has an address of each kind is reachable at each, and which one
    // a caller can use is the caller's business rather than this node's.
    for address in [
        format!("/ip6/::/tcp/{port}"),
        format!("/ip4/0.0.0.0/tcp/{port}"),
    ] {
        let parsed: Multiaddr = address.parse().map_err(|_| NotListening::NoTransport)?;
        swarm
            .listen_on(parsed)
            .map_err(|_| NotListening::AddressUnavailable)?;
    }

    Ok(Listening {
        addresses: Addresses::default(),
        swarm,
        outstanding: BTreeMap::new(),
        put: 0,
        asked_of: Vec::new(),
        lent: Vec::new(),
        dialled: BTreeMap::new(),
        peers: Peers::default(),
        crossed,
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_seed_record_carries_only_addresses_anybody_could_dial() {
        // **What this is protecting.** A node on a household network is reachable by the other
        // nodes on it, so the record may carry `192.168.…` — and a public zone may not: nobody
        // outside can dial it, and it describes somebody's network to everybody who looks. The
        // overlay ranges are here because they are the ones that look public and are not.
        let record = crate::seed_record(
            "12D3KooWtest",
            "zQmnetwork",
            4001,
            Some(8443),
            &[
                "/ip4/88.12.34.56/tcp/4001".parse().expect("an address"),
                "/ip4/192.168.1.220/tcp/4001".parse().expect("an address"),
                "/ip4/100.100.212.57/tcp/4001".parse().expect("an address"),
                "/ip6/2a0c:5a81::1/tcp/4001".parse().expect("an address"),
                "/ip6/fd7a:115c:a1e0::1/tcp/4001"
                    .parse()
                    .expect("an address"),
                "/ip6/::1/tcp/4001".parse().expect("an address"),
            ],
        );

        assert!(record.contains("88.12.34.56"), "the public one is in");
        assert!(record.contains("2a0c:5a81::1"), "and the public v6");
        for kept_out in ["192.168.1.220", "100.100.212.57", "fd7a:115c", "::1/"] {
            assert!(
                !record.contains(kept_out),
                "{kept_out} reached a public zone"
            );
        }
        // The three parts only this node can produce, and the one it cannot.
        assert!(record.contains("port=4001 peer=12D3KooWtest net=zQmnetwork"));
        assert!(record.contains("url=https://<name>:8443"));
        assert!(
            record.contains("host=<name>"),
            "the name is whoever keeps the zone's to choose"
        );
    }

    #[test]
    fn a_node_serving_nothing_says_no_api_line() {
        // Absent rather than invented: what the zone is told this node serves is what it serves.
        let record = crate::seed_record("12D3KooWtest", "zQmnetwork", 4001, None, &[]);
        assert!(!record.contains("_api"), "it is not serving one");
        assert!(record.contains("_seed"));
    }
    use super::{identity, syncing};
    use almena_node::SigningKey;

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_secret([seed; 32])
    }

    #[test]
    fn the_name_on_the_mesh_is_the_one_this_platform_works_out_for_itself() {
        // **The check this crate is worth having for.** A node publishes its mesh name in DNS long
        // before it ever connects to anything, and it works that name out itself so that it can be
        // published without a mesh running. If the two ever disagreed, every zone would point at a
        // node that does not answer to the name it was given.
        for seed in [0u8, 1, 9, 200, 255] {
            let key = key(seed);
            assert_eq!(
                identity(&key)
                    .expect("a key")
                    .public()
                    .to_peer_id()
                    .to_string(),
                almena_node::peer::of(&key.verifying_key()),
                "seed {seed}"
            );
        }
    }

    #[test]
    fn one_key_is_one_name_here_too() {
        assert_eq!(
            identity(&key(3)).expect("a key").public().to_peer_id(),
            identity(&key(3)).expect("a key").public().to_peer_id()
        );
    }

    #[tokio::test]
    async fn a_node_takes_a_port_and_says_which_one_it_got() {
        // The value a `_seed` record cannot be written without, and the reason it has to be asked
        // for rather than assumed: a node told to take whatever is free does not know its own port
        // until the operating system has answered.
        let mut listening = super::listen(&key(4), "zQmSomeNetwork", 0).expect("a place");

        // The address arrives as an event, so the node is run until it says one.
        let said = tokio::time::timeout(std::time::Duration::from_secs(5), listening.next())
            .await
            .expect("it should be listening well within that");
        assert!(matches!(said, super::Happened::Reachable(_)));

        let port = listening.port().expect("a port");
        assert_ne!(port, 0, "zero is what was asked for, not what was granted");
        assert!(
            !listening.addresses().is_empty(),
            "and it says where it can be reached"
        );
    }

    #[tokio::test]
    async fn a_node_told_to_take_a_port_takes_that_one() {
        // Why the port is chosen rather than discovered: it is the one somebody publishes, and a
        // node that quietly took another would make that record false without telling anybody.
        let mut listening = super::listen(&key(5), "zQmSomeNetwork", 47_811).expect("a place");
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), listening.next()).await;
        assert_eq!(listening.port(), Some(47_811));
    }

    #[test]
    fn a_seed_from_the_zone_becomes_somewhere_to_dial() {
        let seed = almena_node::zone::Seed::read(
            "v=1 host=madrid.dev.almena.network port=4001              peer=12D3KooWD3eckifWpRn9wQpMG9R9hX3sD158z7EqHWmweQAJU5SA net=zQmSomeGenesis",
        )
        .expect("a usable record");

        let address = super::dialling(&seed).expect("somewhere to dial");
        let written = address.to_string();
        assert!(written.contains("madrid.dev.almena.network"), "{written}");
        assert!(written.contains("/tcp/4001"), "{written}");
        assert!(
            written.contains("12D3KooWD3eckifWpRn9wQpMG9R9hX3sD158z7EqHWmweQAJU5SA"),
            "and who has to answer, which is what makes a redirected record a failed connection"
        );
    }

    #[test]
    fn a_record_whose_identity_is_not_one_gives_nowhere_to_dial() {
        // A record that says where to call without saying who answers is the thing the identity is
        // in the record to prevent, and a nonsense identity is that record with extra steps.
        let seed = almena_node::zone::Seed::read(
            "v=1 host=madrid.dev.almena.network port=4001 peer=not-an-identity net=zQmSomeGenesis",
        )
        .expect("a readable record");
        assert!(super::dialling(&seed).is_err());
    }

    #[test]
    fn two_networks_do_not_offer_the_same_protocol() {
        // What actually keeps a development network and a production one apart. Not a field either
        // of them compares — a name they cannot both answer to.
        assert_ne!(syncing("zQmDevelopment"), syncing("zQmProduction"));
    }

    #[test]
    fn the_protocol_name_carries_the_network_and_says_what_it_is_for() {
        let name = syncing("zQmSomeGenesisHash");
        assert!(name.starts_with("/almena/"));
        assert!(name.contains("zQmSomeGenesisHash"));
        assert!(name.ends_with("/sync/1.0.0"), "{name}");
    }
}
