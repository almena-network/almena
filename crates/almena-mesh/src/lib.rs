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
                SwarmEvent::NewListenAddr { address, .. } => {
                    if !self.addresses.contains(&address) {
                        self.addresses.push(address.clone());
                        return Happened::Reachable(address);
                    }
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    return Happened::Met(peer_id);
                }
                SwarmEvent::Behaviour(DoingEvent::Sync(request_response::Event::Message {
                    peer,
                    message,
                    ..
                })) => match message {
                    request_response::Message::Request {
                        request, channel, ..
                    } => return Happened::Asked(peer, request, Answering(channel)),
                    request_response::Message::Response { response, .. } => {
                        return Happened::Answered(peer, response);
                    }
                },
                SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    return Happened::Parted(peer_id);
                }
                // Everything else is the transport getting on with itself, and saying so would be
                // noise in the one place somebody looks when something is wrong.
                _ => {}
            }
        }
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
    /// Somebody connected, or this node connected to them.
    Met(PeerId),
    /// A connection ended. Ordinary, and not on its own a sign of anything.
    Parted(PeerId),
    /// Somebody asked something. It is not answered here — [`Listening::answer`] is.
    Asked(PeerId, sync::Ask, Answering),
    /// Something that was asked for came back.
    ///
    /// **Nothing about it is believed for having arrived.** The acts inside are somebody else's
    /// signed bytes and go through the same admission as any other; the peer that sent them
    /// vouches for nothing, including itself.
    Answered(PeerId, sync::Said),
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
    pub fn ask(&mut self, peer: &PeerId, question: sync::Ask) {
        self.swarm.behaviour_mut().sync.send_request(peer, question);
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

/// Why a node could not take its place on the mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotListening {
    /// The key would not make an identity, which a key already in use cannot fail to do.
    NoIdentity,
    /// The transport could not be built at all.
    NoTransport,
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
    let offering = syncing(network);

    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(identity(key)?)
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
        .with_behaviour(|keys| Doing {
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
        .build();

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
