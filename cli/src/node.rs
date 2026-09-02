//! Bringing a node up, and what it can honestly say about itself while it is up.
//!
//! **This face draws a node; it is not one.** What a node is, and everything it reports about
//! itself, comes from the core — so that the two ways of running one cannot start answering the
//! same question differently. Nothing here computes a fact.
//!
//! A node started here holds no network until it opens one, joins one or comes back to the one its
//! directory holds — and until then it reports having no network rather than pretending to one.
//! `null` is not zero: a count of zero is a measurement, and where none was taken these types say so
//! rather than standing a number in for one. The peer count is the one figure that is not the
//! record's: it is read off the mesh socket, which is a fact about connections and not about acts.

use std::path::{Path, PathBuf};

use log::{error, info};

use crate::IDENTIFIER;
use crate::clock::Offset;

/// How often the clock looks at itself.
///
/// Far more often than an epoch lasts, which costs nothing and means a node that comes back in the
/// middle of one catches up promptly instead of leaving a gap for the rest of the hour.
const LOOK: std::time::Duration = std::time::Duration::from_secs(30);

/// How long a node waits to be told where it can be reached before carrying on without knowing.
///
/// It carries on either way: being reachable is the operating system's business and a node that
/// refused to work until it had heard would be refusing over something it does not control.
const REACHABLE_WITHIN: std::time::Duration = std::time::Duration::from_secs(5);

/// How often a node asks whoever it knows what came after where it had got to.
///
/// Meeting somebody asks immediately and a page that is not the last asks again at once, so this is
/// only the floor: how long a node that is up to date waits before checking it still is.
const ASK_EVERY: std::time::Duration = std::time::Duration::from_secs(20);

/// How long whoever is already on the network is given to hand it over.
///
/// Generous, because it is a whole record travelling and it happens once in a node's life. Running
/// out is *somebody is there and would not answer*, which is not the same as nobody being there
/// and must never be treated as it.
const FETCH_WITHIN: std::time::Duration = std::time::Duration::from_secs(60);

/// What epoch it is, counted from the instant this network began, plus whatever the clock offset
/// file says where a development run named one.
///
/// It is built once when the network opens and carried by whatever needs the time, so that the one
/// wall-clock reading this platform ever writes down is not read again by anybody else. The offset
/// is looked at on every call, because the file is what a test moves while the node runs.
fn clock(
    began: u64,
    offset: std::sync::Arc<Offset>,
) -> impl Fn() -> almena_node::Epoch + Clone + Send + Sync + 'static {
    move || {
        let since = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(began, |over| over.as_secs());
        almena_node::Epoch::new(offset.applied(since.saturating_sub(began) / 3_600))
    }
}

/// The zone a development node looks in for somebody to join.
///
/// Named here rather than typed by whoever runs a node: the check it feeds — **open only when
/// nobody is there** — is worth nothing if the zone it asked about was not the network's.
pub const DEVELOPMENT_ZONE: &str = "dev.almena.network";

/// How long one look at a zone is given before it is called a silence.
///
/// **Ten seconds is a long time to answer a question `dig` answers in ten milliseconds**, and it is
/// what the resolver in use here needs on a bad minute. Whoever is watching a node start is waiting
/// on this, so it is the shortest span that does not turn an answer into a silence.
const ASKING_FOR: std::time::Duration = std::time::Duration::from_secs(10);

/// How many times a zone is asked before a node concludes nobody answered.
///
/// **Three, and the reason is which way the mistake falls.** A zone read as silent when it would
/// have answered is a node refusing to open a network nobody is on — recoverable by running it
/// again, and infuriating. A zone read as empty when it is not would be a second network, which is
/// the thing that cannot be undone. Asking again only ever costs time: the answer that opens
/// anything is *nobody is here*, and no number of attempts can invent a seed.
const ASK_AT_MOST: u32 = 3;

/// The one word a network is called by, in a path and in a record line.
///
/// **Short, lower case and not translated.** It names a directory and it is read by whoever is
/// looking at one, so it is the same word on every machine and in every language — the same reason
/// the log carries a stable code rather than a translated sentence.
const fn worded(which: almena_node::Which) -> &'static str {
    match which {
        almena_node::Which::Development => "dev",
        almena_node::Which::Production => "pro",
    }
}

/// The zone a node looks in for somebody to join on the production network.
///
/// **The other one, and there are only two** (`SPECS.md §4.5`). Which zone was read is the weak
/// proof of which network a node is on — the strong one is inside the act that opened it and inside
/// the name of the protocol nodes speak — but it is where a node that has nothing yet starts.
pub const PRODUCTION_ZONE: &str = "almena.network";

/// What this node will and will not do for whoever asks.
///
/// Announced as an answer like any other, so that what it said and what it did are two facts a
/// third party can hold up against each other.
#[must_use]
pub fn limits() -> almena_api::Limits {
    almena_api::Limits {
        per_connection: 600,
        window: 60,
        largest_act: 65_536,
        connections: 256,
    }
}

/// What a run asked for, when its directory turns out to hold no record.
///
/// **Three words and one meaning each.** Opening is the once-ever act and refuses when somebody is
/// there; joining is what every node but the first does and refuses when nobody is; and a run that
/// said neither joins if it can and otherwise comes up on no network, which is what the window does
/// and what a first start with no flags should do rather than silently staying off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Intent {
    /// Open the network, and refuse if the zone names somebody.
    Open,
    /// Join the network, and refuse if the zone names nobody.
    Join,
    /// Join if somebody is there, and otherwise say there is no network yet.
    Whichever,
}

/// Where to find out who is already on the network, and what to do about the answer.
///
/// One decision and not two: a node either asks the zone or is told by hand, and being told always
/// means *somebody is there* — which is why it can stand in for a zone without letting anybody open
/// a network on their own say-so. The one exception is said out loud in `nobody_is_there`, and it
/// reaches development alone.
#[derive(Debug, Clone, Copy)]
struct Looking<'a> {
    /// Which network is being opened, if it turns out nobody is there.
    which: almena_node::Which,
    /// The zone to ask, when nobody was named.
    zone: &'a str,
    /// Seeds given by hand. Not empty means the zone is not asked at all.
    told: &'a [String],
    /// What to do once it is known whether anybody is there.
    intent: Intent,
    /// Do not ask the zone: whoever is running this said nobody is there.
    ///
    /// **Development only, and the command line refuses it for production before this is
    /// reached.** It exists for a machine with no resolver and a network being tried out with
    /// nothing published anywhere; it is never the reason a production network opens.
    nobody_is_there: bool,
}

/// Why a network could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opening {
    /// This node is already on one. A node is a directory with a key in it, and a second network
    /// over the same directory would be a second history for one identity.
    AlreadyOnOne,
    /// The operating system would not produce randomness, so there is no key to be.
    NoRandomness,
    /// The platform would not say where this node keeps things, so it has nowhere to be a node.
    NoDirectory,
    /// There is a key in the directory and it cannot be read.
    ///
    /// Told apart from having none, because they call for opposite things: one is a first start,
    /// and the other is somebody's identity that this must not write over.
    UnreadableIdentity,
    /// This machine puts itself before 1970, so there is no instant to count epochs from.
    NoClock,
    /// The record would not take the acts that start it.
    ///
    /// It must not be reachable. It is here so that if it ever happens it is said, rather than
    /// reported as one of the reasons above and sending somebody to look at their directory.
    RecordWouldNotStart,
    /// The format this build writes is not one a network may be opened on for good.
    ///
    /// Only production is asked, and what is opened from here is development — so this too must not
    /// be reachable, and is named rather than folded into another so that the day it does arise it
    /// is said.
    FormatIsNotFrozen,
    /// There is nowhere for the node's own work to run, so it would be on a network it could not
    /// keep the clock on.
    NoRuntime,
    /// There is a record in the directory and it cannot be read.
    ///
    /// Told apart from having none, because they call for opposite things: one is a first start,
    /// and the other is a node's history that this must not open a second network on top of.
    UnreadableRecord,
    /// The record no longer holds everything this node has already signed for.
    ///
    /// It comes up as nothing rather than as a node serving a history that contradicts the
    /// inclusion proofs it has already handed out.
    RecordDoesNotAddUp,
    /// Somebody else is already the node in this directory.
    DirectoryHeld,
    /// The zone did not answer, so whether anybody is there is unknown.
    ///
    /// **Not the same as nobody being there**, and this is the one place the difference costs the
    /// most: a node that read silence as an empty zone would open a second network beside the one
    /// that was already running, and the two would say the same things about themselves.
    ZoneSilent,
    /// There is no network to do this on.
    NoNetwork,
    /// What was handed over is not a challenge and an approval of it.
    ///
    /// A typo, a half-copied line, or the two the wrong way round. Nothing was written down and
    /// nothing is wrong with the node — it is worth telling apart from an approval that read
    /// perfectly and turned out not to be theirs.
    NotAClaim,
    /// It read, and it does not bind.
    ///
    /// The approval is not signed by the key that claimant's own chain authorises, or the challenge
    /// had stopped being good by the time it came back. **A binding that cannot be checked is not a
    /// weaker binding**: it would be this node's word about somebody who never agreed.
    NotTheirs,
    /// The record would not take it.
    ///
    /// It must not be reachable — there is a network, so there is a chain to add to. It is here so
    /// that if it ever happens it is said, rather than reported as one of the reasons above and
    /// sending somebody to look at whoever they were claiming for.
    NotWrittenDown,
    /// The mesh port could not be listened on — usually because somebody else has it.
    ///
    /// Not worked around by taking another: a node whose port moved is a node whose published
    /// record is now wrong, and nobody reading that record would be told.
    MeshAddressUnavailable,
    /// What was handed over is not the network the zone promised.
    ///
    /// Somebody listed as being on this network is on another one, or is passing off one it made
    /// up. Nothing was written down: the question is asked before anything is.
    NotThePromisedNetwork,
    /// Nobody named as being there would hand over the network.
    ///
    /// Told apart from a zone that said nothing: this is *somebody is there and would not answer*,
    /// which is a thing to go and look at rather than a reason to open a second network.
    NobodyAnswered,
    /// The zone says somebody is already there, so there is a network to join rather than open.
    ///
    /// The core's refusal, not this face's: whether anybody being there means opening or joining is
    /// a rule, and it lives below the interface.
    ThereIsANetwork,
    /// This filesystem will not say whether anybody else is in the directory.
    ///
    /// Coming up anyway would be deciding, on a filesystem that will not answer, that nobody else
    /// is there — and being wrong about that is two histories under one identity.
    DirectoryCannotBeHeld,
    /// Joining was asked for and the zone names nobody, so there is nothing to join.
    ///
    /// Told apart from silence: the zone answered, and what it said is that this network has no
    /// node yet. Opening one is a different act with its own flag, and a join that opened would be
    /// the accident that flag exists to prevent.
    NobodyIsThere,
    /// The network opened and Almena Government's key could not be kept beside the record.
    ///
    /// Refused before the record exists, so nothing was opened: a network whose government key
    /// went with the process would be one nobody could ever publish the core on or answer for.
    GovernmentKeyNotKept,
    /// This node did not open the network, so it holds no government key to act with.
    NoGovernmentKey,
    /// The record would not take the government's act, for the reason named.
    ///
    /// Carried as the store's own word, because a ceremony's refusal is worth reading exactly:
    /// a key that is not the government's, a reason short of a language, a grade the vocabulary
    /// does not number, an act nobody asked with.
    NotTaken(almena_node::NotTaken),
}

impl From<almena_node::directory::NotHeld> for Opening {
    fn from(why: almena_node::directory::NotHeld) -> Self {
        match why {
            almena_node::directory::NotHeld::AlreadyHeld => Self::DirectoryHeld,
            almena_node::directory::NotHeld::NotWritable => Self::NoDirectory,
            almena_node::directory::NotHeld::CannotTell => Self::DirectoryCannotBeHeld,
        }
    }
}

impl From<almena_node::record::NotReadable> for Opening {
    fn from(why: almena_node::record::NotReadable) -> Self {
        match why {
            almena_node::record::NotReadable::NotWritable => Self::NoDirectory,
            almena_node::record::NotReadable::DoesNotAddUp => Self::RecordDoesNotAddUp,
            almena_node::record::NotReadable::AnotherNetwork => Self::NotThePromisedNetwork,
            almena_node::record::NotReadable::Unreadable
            | almena_node::record::NotReadable::Refused => Self::UnreadableRecord,
        }
    }
}

impl From<almena_node::identity::NoIdentity> for Opening {
    fn from(why: almena_node::identity::NoIdentity) -> Self {
        match why {
            almena_node::identity::NoIdentity::NoRandomness => Self::NoRandomness,
            almena_node::identity::NoIdentity::Unreadable => Self::UnreadableIdentity,
            almena_node::identity::NoIdentity::NotWritable => Self::NoDirectory,
        }
    }
}

/// What a face was told about taking a place on the mesh.
///
/// Grouped because the three are one decision: where this node listens, whether it carries other
/// nodes, and who it asks to carry it. The two faces take the same one, which is what keeps them
/// two faces rather than two programs.
#[derive(Debug, Clone, Copy)]
pub struct Joining<'a> {
    /// The port to listen on, which is the one somebody publishes.
    pub port: u16,
    /// Whether this node carries other nodes' traffic.
    pub carrying: almena_mesh::Carrying,
    /// Relays to ask to carry this one, for a node that cannot be dialled.
    pub carried_by: &'a [String],
    /// Whether this node holds post for other people, and says so in the record.
    pub mediator: bool,
}

/// Ask each of those relays to carry this node.
///
/// **One that will not is one relay and not a reason to stop.** Which of them answers is not this
/// node's to decide, and the answer arrives later either way — asking is not being carried, and
/// what a slot makes reachable is published when one is granted.
fn asking_to_be_carried(listening: &mut almena_mesh::Listening, relays: &[String]) {
    for relay in relays {
        match listening.ask_to_be_carried_at(relay) {
            Ok(address) => info!("mesh_asked_to_be_carried relay={address}"),
            Err(why) => error!("mesh_relay_not_asked relay={relay} reason={why:?}"),
        }
    }
}

/// Drive the mesh until the operating system has said where this node can be reached.
///
/// That is a fact the node has to report, and afterwards the mesh belongs to whatever is keeping
/// up — a face that went on reading it would be a face deciding what to do about it. Not hearing
/// within [`REACHABLE_WITHIN`] is said rather than waited on: the port is what somebody publishes,
/// and being reachable is the operating system's business.
fn waiting_to_be_reachable(
    runtime: &tokio::runtime::Runtime,
    listening: &mut almena_mesh::Listening,
) {
    runtime.block_on(async {
        let waiting = tokio::time::timeout(REACHABLE_WITHIN, async {
            loop {
                if let almena_mesh::Happened::Reachable(address) = listening.next().await {
                    info!("mesh_reachable address={address}");
                    if listening.port().is_some() {
                        return;
                    }
                }
            }
        });
        let _ = waiting.await;
    });
    match listening.port() {
        Some(port) => info!("mesh_port port={port}"),
        None => info!("mesh_port port=unknown"),
    }
}

/// Say in the record that this node holds post, once.
///
/// **Where it is counted, and before anybody is told to come here.** A client picks a mediator
/// from what the record says a node offers, and a mailbox that answered without having said so
/// would be a service the network could not see. Saying it twice is one act: the core writes
/// nothing when the record already says it.
fn offering_post(
    runtime: &tokio::runtime::Runtime,
    serving: &almena_serve::Serving,
    now: almena_node::Epoch,
) {
    let said = runtime.block_on(async {
        serving
            .node()
            .write()
            .await
            .also_offering(almena_node::Capability::Mailbox, now)
    });
    info!(
        "mediator_offered {}",
        if said {
            "written=now"
        } else {
            "written=before"
        }
    );
}

/// A node, running.
///
/// Holding one of these means the node is up. Dropping it is not how it is stopped — see
/// [`Node::stop`], which says so in the record.
#[derive(Debug)]
pub struct Node {
    /// Where this node keeps things, when somebody named it rather than letting the platform say.
    ///
    /// A node is a directory with a key in it, so this is what makes one machine able to run more
    /// than one — two directories are two nodes, as separate as nodes on two machines.
    directory: Option<PathBuf>,
    /// Where this node keeps things, resolved once at start.
    directories: almena_paths::Paths,
    /// The DNS servers an operator named, or nothing to use the machine's own.
    resolvers: Vec<std::net::SocketAddr>,
    /// Which network this node is for, chosen once and never mixed with the other.
    ///
    /// **It decides where the node lives**, so a node for one network cannot read the other's key,
    /// record or roots — they are not in the same directory. What already made the two networks
    /// separate is the record itself: the act that opened a network is inside it, its hash is the
    /// network's name, and the mesh protocol carries that name so two networks have nothing to
    /// negotiate. What this adds is the one thing those do not cover — **the key**, which is
    /// thirty-two bytes with no network in them and would otherwise be one node's identity on both.
    ///
    /// That matters in one direction in particular. Development is where directories get copied,
    /// machines get shared and nobody is careful; production is opened once. One key across both
    /// would mean a careless afternoon in development costing a node in production.
    which: almena_node::Which,
    /// The file its records are going to, when they are going to one at all.
    records: Option<PathBuf>,
    /// When this network's epoch zero began, so that this face can say what epoch it is.
    began: Option<u64>,
    /// Epochs added to the wall clock's, from a file a development run named — or nothing.
    ///
    /// Shared with every clock this node hands out, so that the interface, the mesh and the
    /// timekeeping all read the same file and move together.
    offset: std::sync::Arc<Offset>,
    /// The node itself, once there is a network to be on.
    ///
    /// `None` until one is opened or joined. It is held ready to serve from the moment it exists,
    /// rather than being wrapped for serving later: a node that had to be given away in order to
    /// answer questions would be one this face could no longer draw, and drawing it is the other
    /// half of what a face is for.
    holds: Option<almena_serve::Serving>,
    /// Where this node's own work runs, once it is on a network.
    ///
    /// One for the node, not one per thing it does. Keeping the clock and answering questions are
    /// both work a node on a network has, and giving each its own would let a node be half up.
    runtime: Option<std::sync::Arc<tokio::runtime::Runtime>>,
    /// Which network the seeds say they are on, kept from the records that named them.
    ///
    /// It cannot be worked out here: what separates two networks is the name of the protocol they
    /// speak, and that name has this inside it — so a node with no record has to be told.
    told_network: Option<String>,
    /// Where the zone said to start, kept from when it was read.
    ///
    /// Empty is a network nobody else was on when this node came up, which is the ordinary state
    /// for the first node and says nothing about whether anybody is on it now.
    dialling: Vec<almena_mesh::Multiaddr>,
    /// This directory, held for as long as this node is the node in it.
    ///
    /// Kept rather than used and dropped: the lock lasts exactly as long as the open file, so
    /// letting go of it here would let a second process become this same node while it ran.
    held: Option<almena_node::directory::Held>,
    /// What this node has closed, shared with whatever is closing it.
    ///
    /// It starts when the network does, **not** when the interface does: an epoch is owed whether
    /// or not anybody is asking, and a node whose clock only ran while it was answering would
    /// leave gaps meaning *nothing happened* and *I was not here* at once.
    timekeeping: Option<almena_serve::Timekeeping>,
    /// Who this node is connected to on the mesh, readable on every frame.
    ///
    /// Taken off the socket before it is handed to whatever keeps the mesh up, because afterwards
    /// nothing else holds it. [`None`] until there is a mesh to count over — and that is *nobody
    /// counted*, which a zero would misreport.
    peers: Option<almena_mesh::Peers>,
    /// The address the interface was asked to serve on, once it was.
    ///
    /// Kept because nothing else knows it: it is the caller's, and the one half of the link a
    /// client reads that the node itself cannot work out.
    interface: Option<String>,
    /// The challenge shown this run, for the view to draw.
    ///
    /// It is a thing shown to a person and gone; it never reaches the record. The view draws it
    /// rather than the run printing it, because printing happens before the alternate screen opens
    /// and whoever is watching would see it only after leaving.
    challenge: Option<String>,
}

impl Node {
    /// Brings the node up.
    ///
    /// `records` is the file this node's records are being written to, or `None` when they
    /// are only reaching the terminal. It is taken rather than discovered because installing
    /// the destination happens before there is a node to install it for.
    #[must_use]
    pub fn start(records: Option<PathBuf>) -> Self {
        Self::in_directory(records, None, Vec::new(), almena_node::Which::Development)
    }

    /// The same, being the node in a directory somebody named.
    ///
    /// A node is a directory with a key in it, so naming one is how a machine runs more than one.
    #[must_use]
    pub fn in_directory(
        records: Option<PathBuf>,
        directory: Option<PathBuf>,
        resolvers: Vec<std::net::SocketAddr>,
        which: almena_node::Which,
    ) -> Self {
        info!(
            "node_started identifier={IDENTIFIER} network={}",
            worded(which)
        );

        Self {
            directory,
            directories: almena_paths::Paths::for_application(IDENTIFIER),
            resolvers,
            which,
            records,
            holds: None,
            began: None,
            offset: std::sync::Arc::new(Offset::none()),
            dialling: Vec::new(),
            told_network: None,
            held: None,
            runtime: None,
            timekeeping: None,
            peers: None,
            interface: None,
            challenge: None,
        }
    }

    /// The same node, with the epochs written in `file` added to its clock on every look.
    ///
    /// **Before the node is on a network**, because the clock is handed out the moment it is,
    /// and one handed out without the file would keep the wall's time while the rest moved. The
    /// command line has refused this for production already; nothing here asks again.
    #[must_use]
    pub fn reading_the_clock_offset_from(mut self, file: PathBuf) -> Self {
        info!("clock_offset_file path={}", file.display());
        self.offset = std::sync::Arc::new(Offset::reading(file));
        self
    }

    /// Open the network this node was told it is for, if there is nobody to join.
    ///
    /// **One flow, and which network it is for is the only thing that differs.** A node opens a
    /// network only when nobody is there, and it finds that out by reading that network's zone —
    /// the same question, asked of a different zone. Nothing else about opening changes.
    ///
    /// What the two do not share is what happens afterwards. Development is opened again as often
    /// as it needs to be, so a network there lives as long as it is useful; **production is opened
    /// once, ever**, and the node holds the format to the checklist of `almena_frozen` before it
    /// will do it. That is a difference in what is at stake and not in the steps.
    ///
    /// # Errors
    ///
    /// [`Opening`], and each of them is a different thing to go and do about it. The one worth
    /// naming is [`Opening::NoRandomness`]: it is a refusal to start rather than something to work
    /// around, because a node with a guessable key is worse than one that did not come up.
    /// [`Opening::ThereIsANetwork`] is the zone naming somebody: a network to join, not to open.
    ///
    /// `nobody_is_there` opens without asking the zone. It reaches development alone, and the
    /// command line is what keeps it from production.
    pub fn open(
        &mut self,
        zone: &str,
        told: &[String],
        nobody_is_there: bool,
    ) -> Result<(), Opening> {
        self.taking_part(Looking {
            which: self.which,
            zone,
            told,
            intent: Intent::Open,
            nobody_is_there,
        })
    }

    /// Join the network the zone names, or the seeds given by hand.
    ///
    /// **What every node but the first does.** Somebody already there hands the record over, it is
    /// checked against the network the zone promised, and this node announces itself on it. When
    /// nobody is there it refuses: opening is a different act, asked for with its own word.
    ///
    /// # Errors
    ///
    /// [`Opening`], and [`Opening::NobodyIsThere`] when the zone answered and named nobody.
    pub fn join(&mut self, zone: &str, told: &[String]) -> Result<(), Opening> {
        self.taking_part(Looking {
            which: self.which,
            zone,
            told,
            intent: Intent::Join,
            nobody_is_there: false,
        })
    }

    /// Take part in whatever network there is: come back to the one held, or join the one named.
    ///
    /// **For a run that said neither open nor join, which is every start after the first.** A
    /// directory holding a record comes back to its network; one holding nothing joins if the zone
    /// names somebody, as the window does, and otherwise says there is no network yet. It never
    /// opens one: a start that opened whenever it found nobody would be how a restart on a moved
    /// directory becomes a second network.
    ///
    /// # Errors
    ///
    /// [`Opening`], and [`Opening::NoNetwork`] where there is nothing to come back to and nobody
    /// to join — which a run carries on past, drawing a node on no network.
    pub fn take_part(&mut self, zone: &str, told: &[String]) -> Result<(), Opening> {
        self.taking_part(Looking {
            which: self.which,
            zone,
            told,
            intent: Intent::Whichever,
            nobody_is_there: false,
        })
    }

    /// Take this node's place on a network: the one this directory holds, or the one it is told of.
    ///
    /// **Every start looks for neighbours, and only a first start does anything with the answer.**
    /// A directory holding a record comes back to its network and dials whoever the zone and the
    /// seeds name — a node that only dialled on its first day would, after every restart, wait to
    /// be dialled by nodes that were themselves waiting. A directory holding nothing does what
    /// `looking` says, which is the whole of what keeps a restart from becoming a second network.
    fn taking_part(&mut self, looking: Looking<'_>) -> Result<(), Opening> {
        if self.holds.is_some() {
            return Err(Opening::AlreadyOnOne);
        }
        // This node's own key belongs to the directory and outlives every run. Making one afresh
        // each time would be a different node every time, and anything published about it stale
        // without anybody being told.
        let directory = self.application_data().map_err(|_| Opening::NoDirectory)?;
        // Taken before anything in the directory is read or written, including the key: two
        // processes racing to make one would each think they had made it.
        let held = almena_node::directory::hold(&directory).map_err(Opening::from)?;

        let key = almena_node::identity::load_or_make(&directory).map_err(Opening::from)?;
        info!(
            "identity_at path={}",
            almena_node::identity::at(&directory).display()
        );

        // Opening a network and coming back to the one already here are different acts, and doing
        // the first where the second belonged is how a directory ends up on a second network.
        let opened = match almena_node::record::holding(&directory) {
            almena_node::record::Holding::Unreadable(why) => return Err(Opening::from(why)),
            almena_node::record::Holding::ARecord { network, written } => {
                info!(
                    "record_found network={} written={written}",
                    network.as_str()
                );
                // **Best effort, and silence is not fatal here.** The record is what this node is
                // on; the zone only says who else to dial, and a node that refused to come back
                // because DNS was slow would be a node whose uptime depended on somebody else's.
                self.looking_for_neighbours(looking);
                almena_node::Node::rejoin(&directory, key).map_err(Opening::from)?
            }
            // **Nothing here, so this node is on no network.** What happens now is what the run
            // asked for, and never something a start falls into.
            almena_node::record::Holding::Nothing => self.first_time(&directory, looking, key)?,
        };

        let began = opened.began();
        info!(
            "on_network network={} node={} peer={} written={}",
            opened.network().as_str(),
            opened.did(),
            opened.peer(),
            opened.written()
        );
        self.began = Some(began);
        self.held = Some(held);
        self.hold(almena_serve::Serving::new(opened, limits()), began)
    }

    /// Where to dial once back on a network: the seeds given by hand, or what the zone says.
    ///
    /// **One look, and whatever it says.** A first start asks three times because reading a zone
    /// as silent costs it the chance to open; a restart is on its network already and loses only
    /// somebody to dial, which the record's own addresses cover too. So one budget, and silence is
    /// a line in the records rather than a reason.
    fn looking_for_neighbours(&mut self, looking: Looking<'_>) {
        if !looking.told.is_empty() {
            self.take_note_of(looking.told);
            info!("seeds_given count={}", looking.told.len());
            return;
        }
        if looking.nobody_is_there {
            return;
        }
        match self.asking_once(looking.zone) {
            Ok(seeds) => info!("zone_read_on_rejoin seeds={}", seeds.len()),
            Err(_) => info!("zone_silent_on_rejoin zone={}", looking.zone),
        }
    }

    /// Open a network, join one, or say there is none, in a directory that is holding no record.
    fn first_time(
        &mut self,
        directory: &Path,
        looking: Looking<'_>,
        key: almena_node::SigningKey,
    ) -> Result<almena_node::Node, Opening> {
        let Looking {
            which,
            zone,
            told,
            intent,
            nobody_is_there,
        } = looking;
        // Being told who is there and finding out are the same answer, and only one of them can
        // say *nobody*: a seed given by hand always means somebody is, which is why it can stand in
        // for a zone without letting anybody open a network on their own say-so.
        let seeds = if !told.is_empty() {
            self.take_note_of(told);
            info!("seeds_given count={}", told.len());
            told.to_vec()
        } else if nobody_is_there {
            // **Somebody's word instead of the zone's**, which the command line lets through for
            // development alone. Said in the records, because a network opened this way beside one
            // that was already there is a thing whoever reads them later has to be able to see.
            info!("zone_not_asked reason=nobody_is_there");
            Vec::new()
        } else {
            // **The check that makes opening safe, and it is only a check if somebody looks.**
            self.who_is_there(zone)?
        };

        if !seeds.is_empty() {
            if intent == Intent::Open {
                // The core would refuse too; refusing here keeps the mesh from being dialled for
                // a record this run has already said it does not want.
                return Err(Opening::ThereIsANetwork);
            }
            return self.joining(directory, key);
        }
        match intent {
            Intent::Open => self.opening(directory, which, key),
            Intent::Join => Err(Opening::NobodyIsThere),
            Intent::Whichever => Err(Opening::NoNetwork),
        }
    }

    /// Open a network here, and keep Almena Government's key beside the record.
    ///
    /// **The key is written before the record and taken away if the record does not start**, so
    /// that the two are never apart: a record without its government key is a network nobody can
    /// publish the core on, and a key without a record is a file that would refuse the next open.
    fn opening(
        &mut self,
        directory: &Path,
        which: almena_node::Which,
        key: almena_node::SigningKey,
    ) -> Result<almena_node::Node, Opening> {
        // Almena Government's key belongs to the network and is made with it — opening a
        // development network again makes a new one, which is what opening a new network means.
        let government = almena_node::fresh_key().map_err(|_| Opening::NoRandomness)?;
        let kept = almena_node::government::keep(directory, &government)
            .map_err(|_| Opening::GovernmentKeyNotKept)?;

        // The one wall clock reading this platform ever writes down. Everything afterwards counts
        // whole hours from it, so it is read once, here, and never again.
        let began = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Opening::NoClock)?
            .as_secs();

        let opening = almena_node::Opening {
            which,
            beginning: almena_node::Epoch::GENESIS,
            began,
        };
        // Said as it happened. Two of these cannot arise from here — nothing was passed to join
        // and this directory is holding nothing — but collapsing them into one reason would mean
        // that the day one of them did arise, the node would send somebody looking in the wrong
        // place.
        let opened = almena_node::Node::open_in(directory, &opening, &[], &government, key)
            .map_err(|why| match why {
                almena_node::NotOpened::ThisNodeAlreadyHasOne => Opening::AlreadyOnOne,
                almena_node::NotOpened::ThereIsAlreadyANetwork(_) => Opening::ThereIsANetwork,
                almena_node::NotOpened::TheRecordWouldNotStart => Opening::RecordWouldNotStart,
                // **Production only.** Development is re-opened whenever the format moves, so it is
                // never asked the question; production is opened once and is asked it before
                // anything is built.
                almena_node::NotOpened::TheFormatIsNotFrozen(_) => Opening::FormatIsNotFrozen,
            });
        match opened {
            Ok(node) => {
                info!("government_key_at path={}", kept.display());
                Ok(node)
            }
            Err(why) => {
                let _ = std::fs::remove_file(&kept);
                Err(why)
            }
        }
    }

    /// Take up a node, and start its clock.
    ///
    /// **The clock starts with the network, not with the interface.** An epoch is owed whether or
    /// not anybody is asking, so a node whose epochs only closed while it was answering would leave
    /// gaps that mean *nothing happened* and *I was not here* at the same time.
    fn hold(&mut self, serving: almena_serve::Serving, began: u64) -> Result<(), Opening> {
        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|_| Opening::NoRuntime)?,
        );
        let timekeeping = almena_serve::Timekeeping::new();
        runtime.spawn(timekeeping.clone().keeping_time(
            serving.clone(),
            clock(began, std::sync::Arc::clone(&self.offset)),
            LOOK,
        ));

        self.runtime = Some(runtime);
        self.timekeeping = Some(timekeeping);
        self.holds = Some(serving);
        Ok(())
    }

    /// What the zone published, handed on without a verdict.
    ///
    /// **Nothing is decided here.** Whether somebody being there means opening or joining is the
    /// core's rule, and a face that answered it would be a face with logic of its own — the drift
    /// the two-face arrangement exists to prevent, arriving through the door nobody watches.
    ///
    /// The one thing this does refuse is silence: a zone that did not answer has not said nobody is
    /// there, and passing an empty list on would be saying it on its behalf.
    fn who_is_there(&mut self, zone: &str) -> Result<Vec<String>, Opening> {
        // **Asked more than once before it is called a silence.** The resolver's own answers vary
        // from tens of milliseconds to several seconds against the very servers `dig` answers from
        // at once, so one attempt against one budget is a coin toss — and the side it lands on
        // matters: reading a zone that would have answered as one that did not is a node refusing
        // to open a network that nobody is on. It is the safe side of the mistake and it is still a
        // mistake. Nothing is risked by asking again, because the answer that opens anything is
        // *nobody is here*, and asking twice cannot invent a seed.
        for attempt in 1..=ASK_AT_MOST {
            match self.asking_once(zone) {
                Ok(seeds) => return Ok(seeds),
                Err(why) if attempt == ASK_AT_MOST => return Err(why),
                Err(_) => info!("zone_silent_asking_again zone={zone} attempt={attempt}"),
            }
        }
        Err(Opening::ZoneSilent)
    }

    /// One look at the zone, given up on after [`ASKING_FOR`].
    fn asking_once(&mut self, zone: &str) -> Result<Vec<String>, Opening> {
        // **On a thread of its own, and given up on from outside it.** A limit that lives inside
        // the work it is limiting is a limit the work can defeat: measured, a resolver wedged for
        // twenty-three seconds sailed past a ten-second `timeout` without it ever firing, because
        // nothing inside yielded and a timer nothing polls does not go off. Whatever the resolver
        // is doing in there, it is doing it to its own runtime on its own thread — and this side
        // stops waiting when it said it would.
        //
        // The thread is left to finish or not. It holds nothing this node needs and it goes when
        // the process does; joining it would be waiting for the very thing that was given up on.
        let asking = self.resolvers.clone();
        let zone = zone.to_owned();
        let (tell, hear) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("almena-zone".to_owned())
            .spawn(move || {
                // A whole runtime, and not a thread of it. A resolver keeps its connections on work
                // of their own and has to make progress while the question waits — pared down to a
                // single worker it answers nothing, which looks exactly like a zone that is down.
                let Ok(looking) = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                else {
                    let _ = tell.send(None);
                    return;
                };
                let found = looking.block_on(async move {
                    // Named servers where the operator named some, and the machine's own otherwise.
                    // A machine whose resolver is not usable is a real state, and without a way to
                    // say so it is a machine that cannot take part at all.
                    let dns = match asking.is_empty() {
                        true => almena_lookup::Dns::of_this_machine().ok()?,
                        false => almena_lookup::Dns::asking(&asking).ok()?,
                    };
                    almena_lookup::look(&dns, &zone).await
                });
                let _ = tell.send(found);
            })
            .map_err(|_| Opening::NoRuntime)?;

        let Ok(Some(looked)) = hear.recv_timeout(ASKING_FOR) else {
            return Err(Opening::ZoneSilent);
        };
        for why in &looked.refused {
            info!("zone_record_unusable reason={why:?}");
        }
        info!("zone_answered seeds={}", looked.seeds.len());
        self.take_note_of(&looked.seeds);
        Ok(looked.seeds)
    }

    /// Get the record from whoever is already on the network, and become a node on it.
    ///
    /// **A node joins when somebody is there.** It has no record of its own yet, so it cannot
    /// answer anything and has nothing to admit acts against until it has one — which is why this
    /// happens before there is a node at all, and why nothing else can happen until it does.
    fn joining(
        &mut self,
        directory: &Path,
        key: almena_node::SigningKey,
    ) -> Result<almena_node::Node, Opening> {
        let network = self
            .dialling
            .first()
            .and_then(|_| self.told_network.clone())
            .ok_or(Opening::ThereIsANetwork)?;

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|_| Opening::NoRuntime)?;
        let _inside = runtime.enter();

        let mut listening =
            almena_mesh::listen(&key, &network, 0).map_err(|_| Opening::MeshAddressUnavailable)?;
        let seeds = self.dialling.clone();
        let acts = runtime
            .block_on(almena_mesh::keeping::fetch(
                &mut listening,
                seeds,
                FETCH_WITHIN,
            ))
            .ok_or(Opening::NobodyAnswered)?;

        info!("record_fetched acts={}", acts.len());
        // The one wall clock this reads, and only to place the acts it was given in time. What
        // epoch it is comes from the network's own beginning, which is inside those acts.
        let now = almena_node::Epoch::GENESIS;
        almena_node::Node::join(
            directory,
            key,
            almena_node::Joining {
                acts: &acts,
                // What the zone promised. Checked against what arrived before anything is written
                // down: a node that took whatever it was handed would be calling that the network
                // it joined, with somebody else's key as its anchor.
                network: &network,
            },
            now,
        )
        .map_err(Opening::from)
    }

    /// Take note of some seed records: where to dial, and which network they speak for.
    ///
    /// A record that cannot be used is left out with its reason said, and the rest stand: one node
    /// publishing a bad line must not cost everybody else their way in.
    fn take_note_of(&mut self, records: &[String]) {
        self.dialling.clear();
        for record in records {
            let seed = match almena_node::zone::Seed::read(record) {
                Ok(seed) => seed,
                Err(why) => {
                    info!("seed_unusable reason={why:?}");
                    continue;
                }
            };
            match almena_mesh::dialling(&seed) {
                Ok(address) => {
                    self.told_network
                        .get_or_insert_with(|| seed.network().to_owned());
                    self.dialling.push(address);
                }
                Err(why) => info!("seed_undialable host={} reason={why:?}", seed.host()),
            }
        }
    }

    /// Take a place on the mesh, listening on `port`.
    ///
    /// It runs on the work this node already has, beside answering questions and keeping the clock.
    /// From here the node dials whoever the zone named and whoever the record says can be reached,
    /// keeps up with what they wrote down, and **knows its own port**, which is the one value a zone
    /// record cannot be written without.
    ///
    /// # Errors
    ///
    /// [`Opening::NoNetwork`] when there is none to be on, and whatever stopped it listening.
    pub fn join_the_mesh(&mut self, joining: &Joining<'_>) -> Result<(), Opening> {
        let Joining {
            port,
            carrying,
            carried_by,
            mediator,
        } = *joining;
        let serving = self.holds.as_ref().ok_or(Opening::NoNetwork)?;
        let runtime = self.runtime.as_ref().ok_or(Opening::NoNetwork)?;
        let network =
            runtime.block_on(async { serving.node().read().await.network().as_str().to_owned() });

        let key = almena_node::identity::load_or_make(
            &self.application_data().map_err(|_| Opening::NoDirectory)?,
        )
        .map_err(Opening::from)?;

        // Built inside the node's own work rather than beside it: a listener asks the runtime for a
        // socket the moment it is made, and one made outside would be a node that came up and then
        // fell over on its first connection.
        let _inside = runtime.enter();
        let mut listening =
            almena_mesh::listening(&key, &network, port, carrying).map_err(|why| match why {
                almena_mesh::NotListening::NoIdentity
                | almena_mesh::NotListening::NoTransport
                | almena_mesh::NotListening::Anonymous => Opening::NoRuntime,
                almena_mesh::NotListening::AddressUnavailable => Opening::MeshAddressUnavailable,
            })?;

        asking_to_be_carried(&mut listening, carried_by);
        waiting_to_be_reachable(runtime, &mut listening);

        let telling = clock(
            self.began.unwrap_or_default(),
            std::sync::Arc::clone(&self.offset),
        );
        if mediator {
            offering_post(runtime, serving, telling());
        }

        // Taken before the socket goes: afterwards nothing else holds it, and a frame that wanted a
        // peer count would have nobody to ask.
        self.peers = Some(listening.peers());

        let seeds = self.dialling.clone();
        let node = std::sync::Arc::clone(serving.node());
        runtime.spawn(almena_mesh::keeping::keeping_up(
            listening, node, seeds, telling, ASK_EVERY,
        ));
        Ok(())
    }

    /// Close whatever epochs are owed, now, and say how many that was.
    ///
    /// The clock does this on its own; asking is for the moment somebody does not want to wait for
    /// it. Both go through one record of what has been closed, so asking twice in a row is not two
    /// answers about one epoch.
    ///
    /// [`None`] when there is no network, which is *there is nothing to close* rather than *none
    /// were*.
    #[must_use]
    pub fn close_epoch(&self) -> Option<usize> {
        let serving = self.holds.as_ref()?;
        let timekeeping = self.timekeeping.as_ref()?;
        let runtime = self.runtime.as_ref()?;
        let now = self.now()?;

        let closed = runtime.block_on(timekeeping.catch_up(serving, now));
        info!("epochs_closed count={closed} through={}", now.number());
        Some(closed)
    }

    /// Where this node's own work runs, once it is on a network.
    #[must_use]
    pub fn runtime(&self) -> Option<&std::sync::Arc<tokio::runtime::Runtime>> {
        self.runtime.as_ref()
    }

    /// The node, ready to answer, once there is one.
    #[must_use]
    pub fn serving(&self) -> Option<&almena_serve::Serving> {
        self.holds.as_ref()
    }

    /// This node's clock, for whatever needs the time while it runs.
    ///
    /// **One clock, handed out and never rebuilt**: the interface, the mesh and the timekeeping
    /// all count from the same beginning and read the same offset file, so no two of them can
    /// disagree about what hour it is. [`None`] until there is a network, because an epoch is
    /// hours since **that network's** beginning and there is no such instant before one is opened.
    #[must_use]
    pub fn clock(&self) -> Option<impl Fn() -> almena_node::Epoch + Clone + Send + Sync + 'static> {
        let began = self.began?;
        Some(clock(began, std::sync::Arc::clone(&self.offset)))
    }

    /// What epoch it is, by this node's own clock.
    ///
    /// [`None`] until there is a network, for the reason [`Self::clock`] gives.
    #[must_use]
    pub fn now(&self) -> Option<almena_node::Epoch> {
        self.clock().map(|telling| telling())
    }

    /// What this node reports about itself.
    ///
    /// Read from the core and not assembled here, so that the windowed face and this one cannot
    /// answer the same question differently. A node with no network reports having looked at
    /// nothing, which is not the same as reporting nothing.
    #[must_use]
    pub fn facts(&self) -> almena_node::Facts {
        self.holds
            .as_ref()
            .map_or_else(almena_node::Facts::default, |serving| {
                serving.node().blocking_read().facts()
            })
    }

    /// How many peers this node is connected to on the mesh right now.
    ///
    /// `None` until the node has taken a place on the mesh, and never `0` for that: zero is a count
    /// somebody took, and before there is a socket nobody has. Read off the socket's own handle on
    /// every frame, which is what makes it a fact about connections and not about the record.
    #[must_use]
    pub fn peers(&self) -> Option<usize> {
        self.peers.as_ref().map(almena_mesh::Peers::count)
    }

    /// How many nodes the record's own observers have lately found answering nothing.
    ///
    /// A fact from the record, drawn beside the peer count: who this node reaches is one thing,
    /// and who everybody's daily summaries say has gone quiet is another. `None` where there is no
    /// record to read it from.
    #[must_use]
    pub fn silent(&self) -> Option<usize> {
        let serving = self.holds.as_ref()?;
        let now = self.now()?;
        Some(serving.node().blocking_read().departed(now))
    }

    /// Where the interface is being served, once it is.
    ///
    /// The address the run asked for, which is the one somebody publishes and the one a client is
    /// told. `None` is a state: nothing is served, and a plausible address standing in for one
    /// would send somebody to a door that is not open.
    #[must_use]
    pub fn interface_at(&self) -> Option<&str> {
        self.interface.as_deref()
    }

    /// Take note that the interface is being served on `address`.
    pub fn serving_at(&mut self, address: &str) {
        self.interface = Some(address.to_owned());
    }

    /// The link a client reads to choose this node: where its interface is, and who answers there.
    ///
    /// **The string the client reads, exactly.** `address` is `host:port` as the interface was
    /// asked to serve, and `peer` is what the node answers to on the mesh — the same identity the
    /// zone carries, and the key the client pins the interface's certificate against. `None` until
    /// both halves exist, because a link with one of them would be a door with no lock on it.
    #[must_use]
    pub fn link(&self) -> Option<String> {
        let address = self.interface.as_deref()?;
        let peer = self.facts().peer?;
        Some(format!("almena://node?address={address}&peer={peer}"))
    }

    /// The challenge shown this run, if one was asked for.
    #[must_use]
    pub fn challenge(&self) -> Option<&str> {
        self.challenge.as_deref()
    }

    /// Where this node would keep what it cannot get back.
    ///
    /// # Errors
    ///
    /// [`almena_paths::NoHomeDirectory`] when the platform does not say where the user's home
    /// is, in which case this node can store nothing at all.
    pub fn application_data(&self) -> Result<PathBuf, almena_paths::NoHomeDirectory> {
        match &self.directory {
            // **A directory somebody named is theirs, whole.** They said where this node lives, so
            // nothing is appended to it: two networks in one named directory is two directories
            // they can name, which is how a machine already runs more than one node.
            Some(named) => Ok(named.clone()),
            // **The network is part of the path**, so the two never share a key. Everything else
            // that separates them lives in the record; this is what separates what is beside it.
            None => Ok(self
                .directories
                .application_data()?
                .join(worded(self.which))),
        }
    }

    /// The file this node's records are going to, if any.
    ///
    /// `None` means they are reaching the terminal and nothing else, which is what a node with
    /// no writable directory gets.
    #[must_use]
    pub fn records(&self) -> Option<&Path> {
        self.records.as_deref()
    }

    /// Show a challenge for whoever contributed this node to approve.
    ///
    /// **The node asks and decides nothing.** Approving it is somebody putting their name beside a
    /// machine in a record that does not forget, and the only thing this node can do about that is
    /// ask. It is good for `for_epochs` and then it is not: one that ended up in a screenshot or a
    /// support bundle must not bind somebody's machine a year later.
    ///
    /// **Nothing but this node remembers it was shown.** The record never saw it and could not tell
    /// one shown twice from one shown once — but the same act arriving twice is one act, so a
    /// replay changes nothing either way.
    ///
    /// # Errors
    ///
    /// [`Opening::NoNetwork`] when there is no node to be claimed, and [`Opening::NoRandomness`]
    /// when the operating system will not produce any — a challenge somebody could guess is one an
    /// approval could be collected for in advance.
    pub fn asking_who_contributed_me(&mut self, for_epochs: u64) -> Result<String, Opening> {
        let serving = self.holds.as_ref().ok_or(Opening::NoNetwork)?;
        let now = self.now().ok_or(Opening::NoNetwork)?;
        let until = now
            .plus(almena_node::Epochs(for_epochs))
            .ok_or(Opening::NoNetwork)?;
        let challenge = serving
            .node()
            .blocking_read()
            .asking_who_contributed_me(until)
            .map_err(|_| Opening::NoRandomness)?
            .to_text();
        // Kept for the view. What was shown is what has to be approved, and the view is where
        // whoever is watching this node will read it.
        self.challenge = Some(challenge.clone());
        Ok(challenge)
    }

    /// The key this node signs with, read back from its directory.
    ///
    /// **For the certificate it serves under**: the interface is served under the node's own key
    /// unless an operator names a pair of files, and the key is the directory's rather than this
    /// process's memory, so it is read where it lives.
    ///
    /// # Errors
    ///
    /// [`Opening`], as when the node came up: the directory, or the key in it.
    pub fn identity(&self) -> Result<almena_node::SigningKey, Opening> {
        let directory = self.application_data().map_err(|_| Opening::NoDirectory)?;
        almena_node::identity::load_or_make(&directory).map_err(Opening::from)
    }

    /// Almena Government's key, if this node opened the network.
    fn government(&self) -> Result<almena_node::SigningKey, Opening> {
        let directory = self.application_data().map_err(|_| Opening::NoDirectory)?;
        almena_node::government::load(&directory).map_err(|why| match why {
            almena_node::government::NoKey::NotHere => Opening::NoGovernmentKey,
            almena_node::government::NoKey::Unreadable
            | almena_node::government::NoKey::NotWritable => Opening::UnreadableIdentity,
        })
    }

    /// Publish the core Almena maintains, as Almena Government.
    ///
    /// **Each act through this node's own admission, and nothing twice**: what the record already
    /// holds is skipped, so this is safe to run again. Only the node that opened the network holds
    /// the key this signs with.
    ///
    /// # Errors
    ///
    /// [`Opening::NoNetwork`] with no node, [`Opening::NoGovernmentKey`] where this node did not
    /// open the network, and [`Opening::NotTaken`] with the store's own refusal.
    pub fn publish_core(&mut self) -> Result<almena_node::CorePublished, Opening> {
        let government = self.government()?;
        let serving = self.holds.as_ref().ok_or(Opening::NoNetwork)?;
        let now = self.now().ok_or(Opening::NoNetwork)?;
        serving
            .node()
            .blocking_write()
            .publish_core(&government, now)
            .map_err(Opening::NotTaken)
    }

    /// Certify an entity, as Almena Government, with a reason in the languages given.
    ///
    /// # Errors
    ///
    /// As [`Self::publish_core`], and [`Opening::NotAClaim`] for text that is not an identifier.
    pub fn certify(
        &mut self,
        subject: &str,
        grade: almena_node::Grade,
        reason: &std::collections::BTreeMap<String, String>,
    ) -> Result<String, Opening> {
        let subject = almena_node::Did::parse(subject).map_err(|_| Opening::NotAClaim)?;
        let government = self.government()?;
        let serving = self.holds.as_ref().ok_or(Opening::NoNetwork)?;
        let now = self.now().ok_or(Opening::NoNetwork)?;
        serving
            .node()
            .blocking_write()
            .certify(
                &government,
                almena_node::Sealing {
                    subject: &subject,
                    grade,
                    reason,
                },
                now,
            )
            .map(|sealed| sealed.to_string())
            .map_err(Opening::NotTaken)
    }

    /// Answer an asking to be certified, as Almena Government, with what it says in the languages
    /// given.
    ///
    /// # Errors
    ///
    /// As [`Self::certify`].
    pub fn reply(
        &mut self,
        to: &str,
        said: &std::collections::BTreeMap<String, String>,
    ) -> Result<String, Opening> {
        let to = almena_node::Name::parse(to).map_err(|_| Opening::NotAClaim)?;
        let government = self.government()?;
        let serving = self.holds.as_ref().ok_or(Opening::NoNetwork)?;
        let now = self.now().ok_or(Opening::NoNetwork)?;
        serving
            .node()
            .blocking_write()
            .reply(&government, &to, said, now)
            .map(|answered| answered.to_string())
            .map_err(Opening::NotTaken)
    }

    /// Write down that somebody contributed this node, from what they handed back.
    ///
    /// Both halves go in: the challenge this node showed, and their approval of it. The approval is
    /// checked against the key **their own** chain authorises, so one that reads is not one that
    /// binds.
    ///
    /// # Errors
    ///
    /// [`Opening::NoNetwork`] when there is no node to claim, [`Opening::NotAClaim`] when the text
    /// is not a challenge and an approval, and [`Opening::NotTheirs`] when it read and does not
    /// bind.
    pub fn contributed_by(&mut self, challenge: &str, approval: &str) -> Result<(), Opening> {
        let serving = self.holds.as_ref().ok_or(Opening::NoNetwork)?;
        let now = self.now().ok_or(Opening::NoNetwork)?;
        match serving
            .node()
            .blocking_write()
            .contributed_by_text(challenge, approval, now)
        {
            almena_node::Claimed::Written => Ok(()),
            almena_node::Claimed::NotAClaim => Err(Opening::NotAClaim),
            almena_node::Claimed::NotTheirs => Err(Opening::NotTheirs),
        }
    }

    /// Say this node is no longer contributed by anybody.
    ///
    /// **The node alone**, because whoever claimed it gave up something they were owed and nobody
    /// has to agree to that. Credit stops from here and never in arrears: what was served was
    /// served.
    ///
    /// # Errors
    ///
    /// [`Opening::NoNetwork`] when there is no node, and [`Opening::NotWrittenDown`] when the
    /// record would not take it.
    pub fn contributed_by_nobody(&mut self) -> Result<(), Opening> {
        let serving = self.holds.as_ref().ok_or(Opening::NoNetwork)?;
        let now = self.now().ok_or(Opening::NoNetwork)?;
        serving
            .node()
            .blocking_write()
            .contributed_by_nobody(now)
            .then_some(())
            .ok_or(Opening::NotWrittenDown)
    }

    /// Close this node, so that it stops counting.
    ///
    /// **The one way out of a node whose key is somebody else's** (`SPECS.md §4.1`). It is not a
    /// way of taking a node down for the afternoon: a closed node does not come back, and coming
    /// back means announcing a new one with a new key and a new name.
    ///
    /// # Errors
    ///
    /// [`Opening::NoNetwork`] when there is no node, and [`Opening::NotWrittenDown`] when the
    /// record would not take it.
    pub fn close_this_node(&mut self) -> Result<(), Opening> {
        let serving = self.holds.as_ref().ok_or(Opening::NoNetwork)?;
        let now = self.now().ok_or(Opening::NoNetwork)?;
        serving
            .node()
            .blocking_write()
            .close_itself(now)
            .then_some(())
            .ok_or(Opening::NotWrittenDown)
    }

    /// Takes the node down, saying so.
    pub fn stop(self) {
        info!("node_stopped");
    }
}

#[cfg(test)]
mod tests {
    use super::{Intent, Looking, Node, Opening};

    /// A directory of this test's own, removed when it is done with it.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("almena-cli-node-{name}"));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A node in a directory of its own, on no network yet.
    fn in_scratch(scratch: &Scratch) -> Node {
        Node::in_directory(
            None,
            Some(scratch.0.clone()),
            Vec::new(),
            almena_node::Which::Development,
        )
    }

    /// What a run asks for when the zone is not to be asked at all.
    fn on_somebody_s_word(intent: Intent) -> Looking<'static> {
        Looking {
            which: almena_node::Which::Development,
            zone: "dev.almena.network",
            told: &[],
            intent,
            nobody_is_there: true,
        }
    }

    #[test]
    fn a_new_node_has_measured_nothing() {
        let node = Node::start(None);
        let facts = node.facts();

        // Each of these is `None` rather than an empty string or a zero, and this test is what
        // would fail if somebody made one of them "friendlier" by giving it a default.
        assert!(facts.network.is_none());
        assert!(facts.identity.is_none());
        assert!(facts.written.is_none());
        assert!(facts.root.is_none());
        assert!(node.peers().is_none());
        assert!(node.silent().is_none());
        assert!(node.interface_at().is_none());
        assert!(node.link().is_none());
        assert!(node.challenge().is_none());
    }

    #[test]
    fn what_this_face_reports_comes_from_the_core() {
        // Not assembled here. If it were, this face and the windowed one would answer the same
        // question differently the first time either was changed.
        assert_eq!(Node::start(None).facts(), almena_node::Facts::default());
    }

    #[test]
    fn a_node_knows_where_it_would_keep_things() {
        let node = Node::start(None);
        let directory = node.application_data().expect("a home directory");
        assert!(
            directory.to_string_lossy().contains("network.almena.cli"),
            "{directory:?}"
        );
    }

    #[test]
    fn opening_on_somebody_s_word_keeps_the_government_key_beside_the_record() {
        // **What the ceremonies later run on.** The key is made when the network opens and
        // nowhere else, so it is kept the moment it exists — readable by the owner alone.
        let scratch = Scratch::new("opens");
        let mut node = in_scratch(&scratch);
        node.open("dev.almena.network", &[], true)
            .expect("development opens on somebody's word");

        let facts = node.facts();
        assert!(facts.network.is_some());
        assert!(
            node.silent() == Some(0),
            "a count, and nought: nobody has gone quiet"
        );
        assert!(node.peers().is_none(), "no mesh yet, so nobody counted");
        assert!(almena_node::government::at(&scratch.0).exists());
        assert!(
            node.publish_core().is_ok(),
            "and the key that opened the network is the one that publishes on it"
        );
        node.stop();
    }

    #[test]
    fn joining_refuses_when_nobody_is_there_and_neither_word_says_there_is_no_network() {
        // Three words, one meaning each. Only *open* makes a network; *join* refuses without one
        // and a run that said neither is told, and carries on drawing a node on no network.
        let scratch = Scratch::new("nobody");
        let mut node = in_scratch(&scratch);
        assert_eq!(
            node.taking_part(on_somebody_s_word(Intent::Join)),
            Err(Opening::NobodyIsThere)
        );
        assert_eq!(
            node.taking_part(on_somebody_s_word(Intent::Whichever)),
            Err(Opening::NoNetwork)
        );
        assert!(
            !almena_node::government::at(&scratch.0).exists(),
            "and no key was written for a network that was not opened"
        );
    }

    #[test]
    fn the_link_a_client_reads_needs_the_interface_and_the_peer() {
        // Both halves or nothing: where to call, and who answers there.
        let scratch = Scratch::new("link");
        let mut node = in_scratch(&scratch);
        assert!(node.link().is_none());
        node.open("dev.almena.network", &[], true).expect("opens");
        assert!(node.link().is_none(), "no interface yet");
        node.serving_at("127.0.0.1:8791");
        let link = node.link().expect("both halves");
        let peer = node.facts().peer.expect("a peer");
        assert_eq!(
            link,
            format!("almena://node?address=127.0.0.1:8791&peer={peer}")
        );
        node.stop();
    }

    #[test]
    fn a_node_that_joined_holds_no_government_key() {
        // Only the node that opened the network holds it; every other node is refused with a
        // reason that says so rather than one that sends somebody to look at their key.
        let scratch = Scratch::new("joined");
        let mut node = in_scratch(&scratch);
        node.open("dev.almena.network", &[], true).expect("opens");
        std::fs::remove_file(almena_node::government::at(&scratch.0)).expect("taken away");
        assert_eq!(node.publish_core(), Err(Opening::NoGovernmentKey));
        node.stop();
    }
}
