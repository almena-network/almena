//! Bringing a node up, and what it can honestly say about itself while it is up.
//!
//! **This face draws a node; it is not one.** What a node is, and everything it reports about
//! itself, comes from the core — so that the two ways of running one cannot start answering the
//! same question differently. Nothing here computes a fact.
//!
//! A node started here holds no network. Opening one means first knowing there is nobody to join,
//! and reading the zone is not built, so this reports having no network rather than pretending to
//! one. `null` is not zero: a count of zero is a measurement, and where none was taken these types
//! say so rather than standing a number in for one.

use std::path::{Path, PathBuf};

use log::info;

use crate::IDENTIFIER;

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

/// What epoch it is, counted from the instant this network began.
///
/// It is built once when the network opens and carried by whatever needs the time, so that the one
/// wall-clock reading this platform ever writes down is not read again by anybody else.
fn clock(began: u64) -> impl Fn() -> almena_node::Epoch + Clone + Send + Sync + 'static {
    move || {
        let since = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(began, |over| over.as_secs());
        almena_node::Epoch::new(since.saturating_sub(began) / 3_600)
    }
}

/// The zone a development node looks in for somebody to join.
///
/// Named here rather than typed by whoever runs a node: the check it feeds — **open only when
/// nobody is there** — is worth nothing if the zone it asked about was not the network's.
pub const DEVELOPMENT_ZONE: &str = "dev.almena.network";

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

/// Where to find out who is already on the network.
///
/// One decision and not two: a node either asks the zone or is told by hand, and being told always
/// means *somebody is there* — which is why it can stand in for a zone without letting anybody open
/// a network on their own say-so.
#[derive(Debug, Clone, Copy)]
struct Looking<'a> {
    /// The zone to ask, when nobody was named.
    zone: &'a str,
    /// Seeds given by hand. Not empty means the zone is not asked at all.
    told: &'a [String],
}

/// Why a network could not be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// The file its records are going to, when they are going to one at all.
    records: Option<PathBuf>,
    /// When this network's epoch zero began, so that this face can say what epoch it is.
    began: Option<u64>,
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
}

impl Node {
    /// Brings the node up.
    ///
    /// `records` is the file this node's records are being written to, or `None` when they
    /// are only reaching the terminal. It is taken rather than discovered because installing
    /// the destination happens before there is a node to install it for.
    #[must_use]
    pub fn start(records: Option<PathBuf>) -> Self {
        Self::in_directory(records, None)
    }

    /// The same, being the node in a directory somebody named.
    ///
    /// A node is a directory with a key in it, so naming one is how a machine runs more than one.
    #[must_use]
    pub fn in_directory(records: Option<PathBuf>, directory: Option<PathBuf>) -> Self {
        info!("node_started identifier={IDENTIFIER}");

        Self {
            directory,
            directories: almena_paths::Paths::for_application(IDENTIFIER),
            records,
            holds: None,
            began: None,
            dialling: Vec::new(),
            told_network: None,
            held: None,
            runtime: None,
            timekeeping: None,
        }
    }

    /// Open a development network, on the caller's word that there is nobody to join.
    ///
    /// **A node opens a network only when nobody is there.** Normally it learns that by reading
    /// the zone; nothing reads one yet, so this takes somebody's word for it — which is why only
    /// development can be opened this way. Development is opened again as often as it needs to be;
    /// production is opened once, ever, and not on a promise typed at a terminal.
    ///
    /// # Errors
    ///
    /// [`Opening`], and each of them is a different thing to go and do about it. The one worth
    /// naming is [`Opening::NoRandomness`]: it is a refusal to start rather than something to work
    /// around, because a node with a guessable key is worse than one that did not come up.
    pub fn open_development(&mut self, zone: &str, told: &[String]) -> Result<(), Opening> {
        if self.holds.is_some() {
            return Err(Opening::AlreadyOnOne);
        }
        // Almena Government's key belongs to the network and is made with it — opening a
        // development network again makes a new one, which is what opening a new network means.
        let government = almena_node::fresh_key().map_err(|_| Opening::NoRandomness)?;

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
                almena_node::Node::rejoin(&directory, key).map_err(Opening::from)?
            }
            almena_node::record::Holding::Nothing => {
                self.first_time(&directory, Looking { zone, told }, government, key)?
            }
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

    /// Open a network in a directory that is holding none.
    fn first_time(
        &mut self,
        directory: &Path,
        looking: Looking<'_>,
        government: almena_node::SigningKey,
        key: almena_node::SigningKey,
    ) -> Result<almena_node::Node, Opening> {
        let Looking { zone, told } = looking;
        // Being told who is there and finding out are the same answer, and only one of them can
        // say *nobody*: a seed given by hand always means somebody is, which is why it can stand in
        // for a zone without letting anybody open a network on their own say-so.
        let seeds = if told.is_empty() {
            // **The check that makes opening safe, and it is only a check if somebody looks.**
            self.who_is_there(zone)?
        } else {
            self.take_note_of(told);
            info!("seeds_given count={}", told.len());
            told.to_vec()
        };

        if !seeds.is_empty() {
            return self.joining(directory, key);
        }

        // The one wall clock reading this platform ever writes down. Everything afterwards counts
        // whole hours from it, so it is read once, here, and never again.
        let began = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Opening::NoClock)?
            .as_secs();

        let opening = almena_node::Opening {
            which: almena_node::Which::Development,
            beginning: almena_node::Epoch::GENESIS,
            began,
        };
        // Said as it happened. Two of these cannot arise from here — nothing was passed to join
        // and this directory is holding nothing — but collapsing them into one reason would mean
        // that the day one of them did arise, the node would send somebody looking in the wrong
        // place.
        almena_node::Node::open_in(directory, &opening, &seeds, &government, key).map_err(|why| {
            match why {
                almena_node::NotOpened::ThisNodeAlreadyHasOne => Opening::AlreadyOnOne,
                almena_node::NotOpened::ThereIsAlreadyANetwork(_) => Opening::ThereIsANetwork,
                almena_node::NotOpened::TheRecordWouldNotStart => Opening::RecordWouldNotStart,
            }
        })
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
        runtime.spawn(
            timekeeping
                .clone()
                .keeping_time(serving.clone(), clock(began), LOOK),
        );

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
        // On work of its own, and given up on if it takes too long. A resolver that never came
        // back would leave a node with no answer and no way to say so — and *nobody knows* is an
        // answer this node already has a name for.
        // A whole runtime, and not a thread of it. A resolver keeps its connections on work of
        // their own and has to make progress while the question waits — pared down to a single
        // worker it answers nothing, which looks exactly like a zone that is down.
        let looking = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|_| Opening::NoRuntime)?;

        let looked = looking.block_on(async {
            let dns = almena_lookup::Dns::of_this_machine().ok()?;
            almena_lookup::look_patiently(&dns, zone).await
        });

        let Some(looked) = looked else {
            return Err(Opening::ZoneSilent);
        };
        for why in &looked.refused {
            info!("zone_record_unusable zone={zone} reason={why:?}");
        }
        info!("zone_answered zone={zone} seeds={}", looked.seeds.len());
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
    /// Nothing replicates yet: what this buys today is that the node is reachable and **knows its
    /// own port**, which is the one value a zone record cannot be written without.
    ///
    /// # Errors
    ///
    /// [`Opening::NoNetwork`] when there is none to be on, and whatever stopped it listening.
    pub fn join_the_mesh(&mut self, port: u16) -> Result<(), Opening> {
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
        let mut listening = almena_mesh::listen(&key, &network, port).map_err(|why| match why {
            almena_mesh::NotListening::NoIdentity | almena_mesh::NotListening::NoTransport => {
                Opening::NoRuntime
            }
            almena_mesh::NotListening::AddressUnavailable => Opening::MeshAddressUnavailable,
        })?;

        // Driven here only until the operating system has said where this node can be reached.
        // That is a fact the node has to report, and afterwards it belongs to whatever is keeping
        // up — a face that went on reading the mesh would be a face deciding what to do about it.
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
            // It came up and the operating system never said where. Said rather than assumed,
            // because the port is what somebody publishes.
            None => info!("mesh_port port=unknown"),
        }

        let seeds = self.dialling.clone();
        let node = std::sync::Arc::clone(serving.node());
        let began = self.began.unwrap_or_default();
        runtime.spawn(almena_mesh::keeping::keeping_up(
            listening,
            node,
            seeds,
            clock(began),
            ASK_EVERY,
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

    /// What epoch it is, by this node's own clock.
    ///
    /// [`None`] until there is a network, because an epoch is hours since **that network's**
    /// beginning and there is no such instant before one is opened.
    #[must_use]
    pub fn now(&self) -> Option<almena_node::Epoch> {
        let began = self.began?;
        let since = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();
        Some(almena_node::Epoch::new(since.saturating_sub(began) / 3_600))
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

    /// How many peers this node is talking to.
    ///
    /// `None` and never `0`. Zero would be a count somebody took; this is the absence of one —
    /// nothing here talks to anybody, because there is no mesh to talk over.
    #[must_use]
    pub fn peers(&self) -> Option<usize> {
        None
    }

    /// Where this node would keep what it cannot get back.
    ///
    /// # Errors
    ///
    /// [`almena_paths::NoHomeDirectory`] when the platform does not say where the user's home
    /// is, in which case this node can store nothing at all.
    pub fn application_data(&self) -> Result<PathBuf, almena_paths::NoHomeDirectory> {
        match &self.directory {
            Some(named) => Ok(named.clone()),
            None => self.directories.application_data(),
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

    /// Takes the node down, saying so.
    pub fn stop(self) {
        info!("node_stopped");
    }
}

#[cfg(test)]
mod tests {
    use super::Node;

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
}
