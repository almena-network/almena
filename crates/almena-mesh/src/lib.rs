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

use std::collections::BTreeMap;

use almena_node::SigningKey;
use libp2p::futures::StreamExt as _;
use libp2p::identity::Keypair;
use libp2p::multiaddr::Protocol;
use libp2p::swarm::SwarmEvent;
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
    }
}

pub use doing::{Doing, DoingEvent};

/// A node listening on the mesh.
///
/// Holding one means it is reachable. Dropping it stops it, and nothing else does.
pub struct Listening {
    /// What the operating system actually gave it, which is not always what was asked for.
    addresses: Vec<Multiaddr>,
    /// The swarm, kept because dropping it is what closes the listener.
    swarm: Swarm<Doing>,
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
}

impl Listening {
    /// Where this node can be reached, as the addresses it really got.
    ///
    /// **Asked for rather than assumed.** A node told to listen on port zero is given one by the
    /// operating system, and a node on a machine with several addresses is reachable at more than
    /// one — publishing what was requested instead of what was granted is how a zone ends up
    /// pointing somewhere nothing is listening.
    #[must_use]
    pub fn addresses(&self) -> &[Multiaddr] {
        &self.addresses
    }

    /// The port it is actually listening on, if it got one.
    ///
    /// This is the value a `_seed` record cannot be written without.
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        self.addresses.iter().find_map(|address| {
            address.iter().find_map(|part| match part {
                Protocol::Tcp(port) => Some(port),
                _ => None,
            })
        })
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
            match self.swarm.select_next_some().await {
                SwarmEvent::NewListenAddr {
                    listener_id,
                    address,
                } => {
                    if let Some(happened) = self.now_reachable(listener_id, address) {
                        return happened;
                    }
                }
                // A slot refused, or granted and later withdrawn, arrives as the circuit ending
                // and never as an answer of its own — so what a relay would not do is read from
                // the listener going away, and the addresses through it stop being published.
                SwarmEvent::ListenerClosed { listener_id, .. }
                | SwarmEvent::ListenerError { listener_id, .. } => {
                    if let Some(relay) = self.stopped_carrying(listener_id) {
                        return Happened::NotCarried(relay);
                    }
                }
                SwarmEvent::ConnectionEstablished {
                    peer_id, endpoint, ..
                } => {
                    return Happened::Met(peer_id, met(&endpoint));
                }
                SwarmEvent::Behaviour(DoingEvent::Sync(request_response::Event::Message {
                    peer,
                    message,
                    ..
                })) => match message {
                    request_response::Message::Request {
                        request, channel, ..
                    } => return Happened::Asked(peer, request, Answering(channel)),
                    request_response::Message::Response {
                        request_id,
                        response,
                    } => {
                        // A question nobody here put would be an answer to nothing, which is what
                        // the zero says: it matches no outstanding question and so moves nothing.
                        let asked = self
                            .outstanding
                            .remove(&request_id)
                            .map_or(Asked(0), |(_, asked)| asked);
                        return Happened::Answered(peer, asked, response);
                    }
                },
                SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    // Whatever was asked of them will not be answered now, and holding on to it
                    // would be holding on to it for as long as the node ran.
                    self.outstanding.retain(|_, (peer, _)| *peer != peer_id);
                    return Happened::Parted(peer_id);
                }
                // Everything else is the transport getting on with itself, and saying so would be
                // noise in the one place somebody looks when something is wrong.
                _ => {}
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
        if self.addresses.contains(&address) {
            return None;
        }
        // **A node that carries has to say where it is, or it has nothing to lend.** What a relay
        // hands back for a slot is the way in through itself, so one that had claimed no address of
        // its own would grant a reservation to nowhere. Only addresses somebody else could use are
        // offered: lending loopback would be handing somebody a way in that leads to themselves.
        if self.carries() && !borrowed(&address) && worth_publishing(&address) {
            self.swarm.add_external_address(address.clone());
        }
        self.addresses.push(address.clone());
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
        self.addresses.retain(|address| {
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
    Met(PeerId, Meeting),
    /// A connection ended. Ordinary, and not on its own a sign of anything.
    Parted(PeerId),
    /// Somebody asked something. It is not answered here — [`Listening::answer`] is.
    Asked(PeerId, sync::Ask, Answering),
    /// Something that was asked for came back.
    ///
    /// **Nothing about it is believed for having arrived.** The acts inside are somebody else's
    /// signed bytes and go through the same admission as any other; the peer that sent them
    /// vouches for nothing, including itself.
    Answered(PeerId, Asked, sync::Said),
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

/// What a connection says about where the other end can be reached.
fn met(endpoint: &libp2p::core::ConnectedPoint) -> Meeting {
    match endpoint {
        libp2p::core::ConnectedPoint::Dialer { address, .. } => Meeting::Dialled(address.clone()),
        libp2p::core::ConnectedPoint::Listener { .. } => Meeting::Answered,
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
    /// # Errors
    ///
    /// [`NotListening::AddressUnavailable`] when that address cannot be dialled at all. Somebody
    /// not answering is not an error here — it arrives later, or does not.
    pub fn dial(&mut self, address: Multiaddr) -> Result<(), NotListening> {
        self.swarm
            .dial(address)
            .map_err(|_| NotListening::AddressUnavailable)
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
        self.addresses.retain(|address| address != through);
        self.swarm.remove_external_address(through);
    }
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
            sync: request_response::Behaviour::new(
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
    let mut swarm = swarm_of(key, network, carrying)?;

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
        addresses: Vec::new(),
        swarm,
        outstanding: BTreeMap::new(),
        put: 0,
        asked_of: Vec::new(),
        lent: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
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
