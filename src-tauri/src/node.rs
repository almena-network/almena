//! What this face draws when it draws a node.
//!
//! **It draws one; it is not one.** Everything a node reports about itself comes from the core, so
//! that the window and the terminal cannot start answering the same question differently. Nothing
//! here works a fact out.
//!
//! A node started here holds no network until it joins one, opens one, or comes back to the one
//! its directory holds — and until then it reports having looked at nothing, which is a different
//! thing from reporting that there is nothing.
//!
//! # One directory per network
//!
//! The node for each network lives under the platform's application data in a directory of its
//! own, `dev` or `pro`, exactly as the terminal keeps them: a node for one network must not be able
//! to read the other's key, because a key is thirty-two bytes with no network in them and one key
//! across both would mean a careless afternoon in development costing a node in production. Which
//! of the two a launch comes back to is remembered in the preferences.
//!
//! # Why the shape is repeated instead of shared
//!
//! What crosses to the webview has to be serialisable, and the core does not serialise: it is
//! replicated into the holder's application, where every dependency is paid for twice. So the
//! shape is written out here and filled **only** from the core's own answer — no field is computed
//! on this side, which is what keeps the repetition from becoming a second opinion.

use serde::Serialize;

use crate::clock::Offset;

/// What a node reports about itself, on its way to the webview.
///
/// Every field is optional and none of them ever gets a default. A node with no network has not
/// looked at one, which is not the same as there being none — and an empty string here would be a
/// fact nobody established.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    /// The network this node is on, if it is on one.
    pub network: Option<String>,
    /// The key this node is, if it has one.
    pub identity: Option<String>,
    /// How many acts it has written down.
    pub written: Option<u64>,
    /// The root over them.
    pub root: Option<String>,
    /// What it answers to on the mesh, which is the one thing a node knows that goes into DNS.
    pub peer: Option<String>,
    /// How many peers it is connected to on the mesh right now.
    ///
    /// **Not the core's figure and not computed here either**: it is read off the mesh socket's
    /// own handle, a fact about connections rather than about the record. `None` until the node
    /// has taken a place on the mesh, which is *nobody counted* and never nought.
    pub peers: Option<usize>,
    /// How many nodes the record's own observers have lately found answering nothing.
    ///
    /// The core's figure, read beside the peer count so that the two are never confused: who this
    /// node reaches is one thing, and who everybody's daily summaries say has gone quiet is another.
    pub silent: Option<usize>,
}

impl From<almena_node::Facts> for Facts {
    /// Straight across, field for field. Anything else would be this side deciding something.
    ///
    /// The two figures the core does not carry stay `None` here: whoever holds the mesh handle and
    /// the moment fills them in, and a conversion that guessed at them would be a guess.
    fn from(facts: almena_node::Facts) -> Self {
        Self {
            network: facts.network,
            identity: facts.identity,
            written: facts.written,
            root: facts.root,
            peer: facts.peer,
            peers: None,
            silent: None,
        }
    }
}

/// How often the clock looks at itself.
///
/// Far more often than an epoch lasts, which costs nothing and means a node that comes back in the
/// middle of one catches up promptly instead of leaving a gap for the rest of the hour.
const LOOK: std::time::Duration = std::time::Duration::from_secs(30);

/// How long a node waits to be told where it can be reached before carrying on without knowing.
///
/// It carries on either way: being reachable is the operating system's business, and refusing to
/// work over something it does not control would be refusing for the wrong reason.
const REACHABLE_WITHIN: std::time::Duration = std::time::Duration::from_secs(5);

/// How often a node asks whoever it knows what came after where it had got to.
///
/// Only the floor: meeting somebody asks immediately, and a page that is not the last asks again at
/// once. This is how long a node that is up to date waits before checking it still is.
const ASK_EVERY: std::time::Duration = std::time::Duration::from_secs(20);

/// What epoch it is, counted from the instant this network began, plus whatever the clock offset
/// file says where a development node reads one.
///
/// Built once when the network opens and carried by whatever needs the time, so that the one
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

/// The environment variable naming the clock offset file, read while developing.
///
/// **Nothing a deployment sets.** It is the window's spelling of the terminal's
/// `--clock-offset-file`, and it reaches a development node alone: production ignores it.
const CLOCK_OFFSET_FILE: &str = "ALMENA_CLOCK_OFFSET_FILE";

/// The clock offset a node on `which` reads: the file the environment names, or none.
///
/// A file named for a production node is ignored and said once in the records, rather than
/// honoured or refused: the window has no parser to refuse at, and a production node whose clock
/// somebody could move would be one signing roots for hours that have not happened.
fn offset_for(which: almena_node::Which) -> Offset {
    match std::env::var(CLOCK_OFFSET_FILE) {
        Ok(named) if !named.trim().is_empty() => match which {
            almena_node::Which::Development => {
                log::info!("clock_offset_file path={}", named.trim());
                Offset::reading(std::path::PathBuf::from(named.trim()))
            }
            almena_node::Which::Production => {
                log::warn!("clock_offset_ignored reason=production");
                Offset::none()
            }
        },
        _ => Offset::none(),
    }
}

/// The node this application is running, if it is running one.
///
/// It is held ready to serve from the moment it exists rather than being wrapped for serving
/// later: a node that had to be given away in order to answer questions would be one this face
/// could no longer draw, and drawing it is the other half of what a face is for.
#[derive(Default)]
pub struct Running {
    held: tokio::sync::RwLock<Option<almena_serve::Serving>>,
    /// When this network's epoch zero began, so that this face can say what epoch it is.
    began: std::sync::atomic::AtomicU64,
    /// Epochs added to the wall clock's, from the file the environment names for a development
    /// node — or nothing.
    ///
    /// Settled when the node comes up, once its network is known, and shared with every clock
    /// handed out from then on, so the interface, the mesh and the timekeeping read the same
    /// file and move together.
    offset: std::sync::Mutex<std::sync::Arc<Offset>>,
    /// This directory, held for as long as this node is the node in it.
    ///
    /// Kept rather than used and dropped: the lock lasts exactly as long as the open file, so
    /// letting go of it here would let a second process become this same node while it ran.
    held_directory: tokio::sync::Mutex<Option<almena_node::directory::Held>>,
    /// Where the zone said to start, kept from when it was read.
    ///
    /// Empty is a network nobody else was on when this node came up — the ordinary state for the
    /// first node, and no claim about whether anybody is on it now.
    dialling: tokio::sync::Mutex<Vec<almena_mesh::Multiaddr>>,
    /// Where this node is serving its interface, once it is.
    ///
    /// **Kept because nothing else knows it.** The address is the caller's — it is the one that gets
    /// published in the zone — and a node asked what it serves on would otherwise have to be told
    /// by whoever told it, which is two places for one fact.
    serving_at: tokio::sync::Mutex<Option<String>>,
    /// What this node has closed, shared with whatever is closing it.
    ///
    /// It starts with the network and **not** with the interface: an epoch is owed whether or not
    /// anybody is asking, and a node whose clock only ran while it was answering would leave gaps
    /// that mean *nothing happened* and *I was not here* at the same time.
    timekeeping: almena_serve::Timekeeping,
    /// Who this node is connected to on the mesh, once it has a place on it.
    ///
    /// Taken off the socket before it is handed to whatever keeps the mesh up, because afterwards
    /// nothing else holds it. Absent until then, which is *nobody counted*.
    peers: tokio::sync::Mutex<Option<almena_mesh::Peers>>,
    /// What has crossed this node's mesh, once it has a place on it.
    ///
    /// Taken off the socket beside the peers and for the same reason. Absent until then, which is
    /// *nothing has crossed because there is nothing for it to cross* — and not a total of nought.
    crossed: tokio::sync::Mutex<Option<almena_mesh::sync::Crossed>>,
    /// Where this node listens, once it has a place on the mesh.
    ///
    /// Taken off the socket beside the peers. **Reported and never written down**: what a node says
    /// about where it is stays its operator's decision (§17.18), and this is what lets the window
    /// show them the answer without the node having taken it.
    where_it_listens: tokio::sync::Mutex<Option<almena_mesh::Addresses>>,
    /// Which network the running node is for, and therefore which directory it lives in.
    ///
    /// Kept because every later command that reaches the directory — the mesh reading the key, the
    /// interface serving under it — has to reach the same one the node came up from.
    which: tokio::sync::Mutex<Option<almena_node::Which>>,
    /// How far a start got, and what stopped it where something did.
    ///
    /// **Decided here and read above.** Whether a node is up is not a thing an interface can work
    /// out from the facts it happens to have: a node holding a record, off the mesh and serving
    /// nothing looks exactly like one that has not finished starting, and the difference between
    /// those two is the whole of what somebody watching wants to know.
    phase: std::sync::Mutex<Phase>,
    /// The three tasks a running node is made of, so that ending the application can stop them.
    ///
    /// Kept rather than let go of, for the reason the directory is: a spawned task nothing holds
    /// is one nothing can ever ask to stop, and this application now ends by stopping its node
    /// rather than by having its process taken away mid-sentence.
    tasks: tokio::sync::Mutex<Tasks>,
}

/// How far a start got, in the vocabulary the strip, the tray and the Network screen all read.
///
/// Four and only four, because this state is drawn as a badge and the four tones are the only
/// four. **Failing carries what went wrong** — the same stable identifier every other refusal here
/// carries — because *something is wrong* is not something anybody can act on.
///
/// It does not serialise. What crosses is [`State`], where the word and the identifier are two
/// plain fields: an enum's wire shape is a decision serde would be making on this face's behalf,
/// and the shape the interface reads is worth writing out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Phase {
    /// No node: this directory holds no record yet, or the application has stopped the one it had.
    #[default]
    Stopped,
    /// A start is under way and has not finished.
    Starting,
    /// Up: on its network, and as far onto the mesh and the interface as it was asked to get.
    Running,
    /// Up, and something a start needed did not happen. The identifier says which.
    ///
    /// **It is not the opposite of running.** A node whose mesh port was taken still holds its
    /// record and still answers for it; what it does not do is what this names.
    Failing(&'static str),
}

impl Phase {
    /// The word for it, which is one of exactly four and is never translated here.
    ///
    /// The interface reads it as an identifier and looks the sentence up in its own catalogue, the
    /// way it does with every refusal this face reports.
    const fn worded(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Failing(_) => "failing",
        }
    }

    /// What went wrong, where something did.
    const fn failing(self) -> Option<&'static str> {
        match self {
            Self::Failing(why) => Some(why),
            _ => None,
        }
    }
}

/// The long-running work a node is made of, held so that it can be ended.
///
/// Three, in the order they are stopped: the door shuts, the node leaves the mesh, and only then
/// does it stop counting — an epoch is owed whether or not anybody is asking, so timekeeping is
/// the last thing to go rather than the first.
#[derive(Default)]
struct Tasks {
    /// The loop accepting interface connections.
    serving: Option<tokio::task::JoinHandle<()>>,
    /// Whatever is keeping this node's place on the mesh.
    mesh: Option<tokio::task::JoinHandle<()>>,
    /// The clock closing this node's epochs.
    timekeeping: Option<tokio::task::JoinHandle<()>>,
}

/// The word a network is called by in a path, the same as the terminal's.
///
/// Short, lower case and not translated: it names a directory, and the same word on every machine
/// and in every language is what lets a person find it.
const fn worded(which: almena_node::Which) -> &'static str {
    match which {
        almena_node::Which::Development => "dev",
        almena_node::Which::Production => "pro",
    }
}

/// The word the interface uses for a network, which is also what the preferences remember.
const fn named(which: almena_node::Which) -> &'static str {
    match which {
        almena_node::Which::Development => "development",
        almena_node::Which::Production => "production",
    }
}

/// Which network the interface named, and its zone.
fn which_of(which: &str) -> Result<(almena_node::Which, &'static str), &'static str> {
    match which {
        "production" => Ok((almena_node::Which::Production, PRODUCTION_ZONE)),
        "development" => Ok((almena_node::Which::Development, DEVELOPMENT_ZONE)),
        _ => Err("no_such_network"),
    }
}

/// Where the node for `which` keeps things: the application's data, and the network's own word.
///
/// **The network is part of the path**, so the two never share a key. Everything else that
/// separates them lives in the record; this is what separates what is beside it.
fn directory_of(
    app: &tauri::AppHandle,
    which: almena_node::Which,
) -> Result<std::path::PathBuf, &'static str> {
    Ok(tauri::Manager::path(app)
        .app_data_dir()
        .map_err(|_| "no_directory")?
        .join(worded(which)))
}

/// Which servers the zone is asked of: none for the machine's own, or the one `ALMENA_RESOLVER`
/// names.
///
/// **A development knob, read by nothing a deployment sets.** A zone emulated on this machine
/// answers on a port of its own, and this is how the window is pointed at it — the same reading
/// the terminal's `--resolver` does, so a value that works for one works for the other.
///
/// # Errors
///
/// `resolver_not_an_address` for something that is not one. A name is not an address here: finding
/// the resolver by name would take the very thing being named.
fn asking() -> Result<Vec<std::net::SocketAddr>, &'static str> {
    match std::env::var("ALMENA_RESOLVER") {
        Ok(named) if !named.trim().is_empty() => Ok(vec![
            almena_lookup::server(&named).map_err(|_| "resolver_not_an_address")?,
        ]),
        _ => Ok(Vec::new()),
    }
}

/// How long the zone is given to answer before this node calls it a silence.
///
/// The same ten seconds the terminal waits, and for the same reason: long enough for a resolver
/// that has to go and ask, short enough that a node which cannot come up says so while somebody is
/// still watching it try.
const ASKING_FOR: std::time::Duration = std::time::Duration::from_secs(10);

/// One look at the zone, on a thread and a runtime of its own.
///
/// # Why it is not simply awaited here
///
/// **A resolver keeps its connections on work of its own and has to make progress while the
/// question waits.** Handed to a runtime that is busy drawing an application, it answers nothing —
/// which looks exactly like a zone that is down, and a node that read that as *nobody is there*
/// would open a second network beside the one it should have joined. The terminal learned this and
/// gives the look a whole runtime; this face asked on the one the window runs on and got a silence
/// every time, against a resolver the terminal reads in milliseconds.
///
/// So the look gets a thread and a multi-thread runtime of its own, and the limit lives **outside**
/// the work it is limiting: a timer nothing polls does not go off, which is the other half of the
/// same lesson.
///
/// # Errors
///
/// `zone_silent` when nothing came back inside [`ASKING_FOR`], and whatever [`asking`] refused.
/// **A zone that answered with nothing is not this**: it comes back as a `Looked` holding nothing,
/// because *nobody is there* is an answer and only an answer may be acted on.
async fn looked_at(zone: &str) -> Result<almena_lookup::Looked, &'static str> {
    let servers = asking()?;
    let zone = zone.to_owned();
    let (tell, hear) = tokio::sync::oneshot::channel();

    std::thread::Builder::new()
        .name("almena-zone".to_owned())
        .spawn(move || {
            let Ok(looking) = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            else {
                let _ = tell.send(None);
                return;
            };
            let found = looking.block_on(async move {
                let dns = match servers.is_empty() {
                    true => almena_lookup::Dns::of_this_machine().ok()?,
                    false => almena_lookup::Dns::asking(&servers).ok()?,
                };
                almena_lookup::look(&dns, &zone).await
            });
            let _ = tell.send(found);
        })
        .map_err(|_| "zone_silent")?;

    match tokio::time::timeout(ASKING_FOR, hear).await {
        Ok(Ok(Some(looked))) => Ok(looked),
        _ => Err("zone_silent"),
    }
}

impl Running {
    /// Settle which clock offset this node reads, now that its network is known.
    ///
    /// **Before the first look at the clock and before any clock is handed out**, so that no
    /// reading and no clock keeps the wall's time while the rest moves.
    fn reading_the_clock_for(&self, which: almena_node::Which) {
        let offset = std::sync::Arc::new(offset_for(which));
        *self
            .offset
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = offset;
    }

    /// The offset every clock this node hands out reads.
    fn offset(&self) -> std::sync::Arc<Offset> {
        std::sync::Arc::clone(
            &self
                .offset
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    /// This node's clock, for whatever needs the time while it runs.
    ///
    /// **One clock, handed out and never rebuilt**: the interface, the mesh and the timekeeping
    /// all count from the same beginning and read the same offset file, so no two of them can
    /// disagree about what hour it is.
    fn clock(&self) -> impl Fn() -> almena_node::Epoch + Clone + Send + Sync + 'static {
        clock(
            self.began.load(std::sync::atomic::Ordering::Relaxed),
            self.offset(),
        )
    }

    /// What epoch it is, by this node's own clock.
    ///
    /// An epoch is whole hours since **this network's** beginning, so before one is opened there
    /// is no such instant and nothing to count from.
    fn now(&self) -> almena_node::Epoch {
        (self.clock())()
    }

    /// Say where a start has got to, so that whoever is watching sees it there.
    fn now_at(&self, phase: Phase) {
        *self
            .phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = phase;
    }

    /// Where a start has got to.
    fn phase(&self) -> Phase {
        *self
            .phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// What this node is doing, decided here and read above.
///
/// # Why this is one answer and not five the interface adds up
///
/// Everything on it can be asked separately — the facts say which network, the mesh handle says
/// whether there is a place on it, the origin says whether the door is open. Asked separately they
/// are five moments, and an interface drawing them together draws a node that never existed. Asked
/// as one they are one moment, decided where the node is run rather than where it is drawn, which
/// is the same rule [`Facts`] is written under.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    /// One of `stopped`, `starting`, `running`, `failing` — never anything else.
    pub state: &'static str,
    /// What went wrong, as the identifier the interface looks its sentence up by.
    ///
    /// `None` unless `state` is `failing`, and never a sentence: the node has no idea what
    /// language anybody reads in.
    pub failing: Option<&'static str>,
    /// Which network this node is on — `development` or `production` — or `None` where it is on
    /// none.
    ///
    /// **The word and not the network's own name.** The name is a fact about the network and is in
    /// [`Facts`]; this is which of the two, which is what a person reads and what the directory is
    /// named after.
    pub which: Option<&'static str>,
    /// Whether this node has a place on the mesh.
    pub mesh: bool,
    /// Whether this node is serving its interface.
    pub serving: bool,
    /// How many peers it is connected to, or `None` where nobody counted.
    ///
    /// The same figure [`Facts`] carries, on the same answer, so that the strip and the screen
    /// cannot draw two counts a second apart and disagree.
    pub peers: Option<usize>,
}

/// What this node is doing: where a start got to, and what is up.
///
/// Answered whether or not there is a node — a directory holding no record is `stopped`, which is a
/// state and not a failure — so the interface never has to tell a gap from an answer.
///
/// # Errors
///
/// None. A node that cannot be asked anything is one this reports as stopped.
#[tauri::command]
pub async fn node_state(running: tauri::State<'_, Running>) -> Result<State, ()> {
    Ok(state_of(&running).await)
}

/// What this node is doing, taken in one pass.
///
/// Apart from the command so that [`come_up`] can answer with it too: the press that brings a node
/// up and the poll that watches it must not be able to describe the same node differently.
async fn state_of(running: &Running) -> State {
    let phase = running.phase();
    // Each lock held once and read from the binding. Two reads of one lock inside one expression
    // is a deadlock, because the first guard lives until the expression ends.
    let which = *running.which.lock().await;
    let peers = running.peers.lock().await;
    let serving = running.serving_at.lock().await.is_some();
    State {
        state: phase.worded(),
        failing: phase.failing(),
        which: which.map(named),
        mesh: peers.is_some(),
        serving,
        peers: peers.as_ref().map(almena_mesh::Peers::count),
    }
}

/// What the node this application is running reports about itself.
///
/// A node with no network reports having looked at nothing, rather than standing zeroes in for
/// counts nobody took.
#[tauri::command]
pub async fn node_facts(running: tauri::State<'_, Running>) -> Result<Facts, ()> {
    let held = running.held.read().await;
    let Some(serving) = held.as_ref() else {
        return Ok(almena_node::Facts::default().into());
    };
    Ok(facts_of(serving, &running).await)
}

/// A node that has come up, the directory it holds, and which network it is for.
///
/// Three things that travel together because they are one state: a node without its directory
/// held would be one a second process could become, and one without its network would be one
/// nothing could find the key for.
struct Up {
    /// The node itself.
    node: almena_node::Node,
    /// Its directory, held for as long as it runs.
    holding: almena_node::directory::Held,
    /// Which network it is for, and therefore which directory it lives in.
    which: almena_node::Which,
}

/// Take up a node that has come up, and start its clock.
///
/// **One place, for the three ways a node comes up** — opened, joined, come back to — so that
/// what is remembered and what is started cannot differ between them. The clock starts with the
/// network and not with the interface: an epoch is owed whether or not anybody is asking.
async fn taking_up(
    app: &tauri::AppHandle,
    running: &Running,
    held: &mut Option<almena_serve::Serving>,
    up: Up,
) -> Facts {
    let Up {
        node,
        holding,
        which,
    } = up;
    let began = node.began();
    let facts = node.facts();
    running
        .began
        .store(began, std::sync::atomic::Ordering::Relaxed);
    // Settled before the first clock is handed out, and once more here for a node that joined
    // and already settled it: the same answer, because the network is the same.
    running.reading_the_clock_for(which);
    *running.held_directory.lock().await = Some(holding);
    *running.which.lock().await = Some(which);
    crate::preferences::remember_network(app, named(which));
    let serving = almena_serve::Serving::new(node, limits());
    running.tasks.lock().await.timekeeping = Some(tokio::spawn(
        running
            .timekeeping
            .clone()
            .keeping_time(serving.clone(), running.clock(), LOOK),
    ));
    *held = Some(serving);
    // On its network. Whether it is also on the mesh and serving is what `coming_up` settles next,
    // and until it has this is a node that has not finished starting rather than one that is up.
    running.now_at(Phase::Starting);
    facts.into()
}

/// The zone a production node looks in.
///
/// **Production is joined and never opened from here.** A network is opened once, ever, and this
/// application has no button for it: what an operator does with production is arrive at one that
/// already exists.
pub const PRODUCTION_ZONE: &str = "almena.network";

/// How long a seed is given to meet this node and hand over a record.
///
/// **A node that waited for ever would have no answer and no way to say so.** Long enough for a
/// record of some size over a connection that had to be made; short enough that a node which
/// cannot come up says so while somebody is still watching it try.
const JOINING_WITHIN: std::time::Duration = std::time::Duration::from_secs(120);

/// The zone a development node looks in for somebody to join.
///
/// Named here rather than typed into the window: the check it feeds — **open only when nobody is
/// there** — is worth nothing if the zone it asked about was not the network's.
pub const DEVELOPMENT_ZONE: &str = "dev.almena.network";

/// What the zone published, handed on without a verdict.
///
/// **Nothing is decided here.** Whether somebody being there means opening or joining is the core's
/// rule, and a face that answered it would be a face with logic of its own. The one thing this does
/// refuse is silence: a zone that did not answer has not said nobody is there.
///
/// # Errors
///
/// The reason it could not, as a stable identifier the interface translates.
async fn who_is_there(zone: &str, running: &Running) -> Result<Vec<String>, &'static str> {
    let looked = looked_at(zone).await?;

    // Kept as somewhere to dial, so that taking a place on the mesh does not ask the zone a second
    // time and get a different answer.
    *running.dialling.lock().await = looked
        .answer
        .seeds
        .iter()
        .filter_map(|seed| almena_mesh::dialling(seed).ok())
        .collect();
    Ok(looked.seeds)
}

/// The port a node takes on the mesh where nothing is remembered.
///
/// **Chosen and not discovered**, because it is the one that gets published in the zone: a node
/// that took whatever was free would be a node whose published record is wrong the next time it
/// starts. Not 4001, which is what a terminal node on the same computer takes while developing.
const MESH: u16 = 4002;

/// The address a node serves its interface on where nothing is remembered. Not the terminal's.
const INTERFACE: &str = "127.0.0.1:8791";

/// Bring a node that is on its network the rest of the way up: onto the mesh, and serving.
///
/// # Why a start does this and does not ask
///
/// A node holding a record, answering nobody and serving nothing is not a node anybody wanted. It
/// was the state every start after the first left this application in, because taking the mesh
/// place and opening the door happened once, in the walk, and never again. They are not decisions:
/// which network was the decision, and the port and the address were settled the first time and
/// **remembered**, because they are what somebody publishes.
///
/// # A failure here does not stop the application
///
/// A mesh port somebody else has taken, or an address that will not bind, leaves a node that still
/// holds its record and still answers for it. So neither is raised: the node stays up, the phase
/// says what did not happen by its identifier, and the interface draws it. The first thing that did
/// not happen is the one named — a second reason underneath the first is noise on a status strip.
///
/// Each half is skipped where it has already happened, so that this is safe to run over a node that
/// is partly up.
async fn coming_up(app: &tauri::AppHandle, running: &Running, serving: &almena_serve::Serving) {
    let (mesh, interface) = crate::preferences::remembered_place(app);
    let port = mesh.unwrap_or(MESH);
    let address = interface.unwrap_or_else(|| INTERFACE.to_owned());
    let mut wanting: Option<&'static str> = None;

    if running.peers.lock().await.is_none() {
        let asked = Place {
            port,
            carry: false,
            mediator: false,
            carried_by: Vec::new(),
        };
        match taking_a_place(app, running, serving, &asked).await {
            Ok(()) => log::info!("mesh_place_taken port={port}"),
            Err(why) => {
                log::error!("mesh_place_not_taken port={port} reason={why}");
                wanting = Some(why);
            }
        }
    }

    if running.serving_at.lock().await.is_none() {
        // The node's own key, always. Every node has one, so every node has a certificate; a pair
        // of files is for an operator who already has one, and asking a start to know about that
        // would be asking a start to be a decision.
        let under = Under {
            certificate: None,
            private_key: None,
        };
        match serving_on(app, running, serving.clone(), &address, under).await {
            Ok(()) => log::info!("interface_up address={address}"),
            Err(why) => {
                log::error!("interface_not_up address={address} reason={why}");
                wanting = wanting.or(Some(why));
            }
        }
    }

    running.now_at(wanting.map_or(Phase::Running, Phase::Failing));
}

/// Bring the node the rest of the way up, for the one press that puts a node on a network.
///
/// **The walk's press and a start run the same code.** Joining or opening leaves a node on its
/// network and nothing more; this is what makes it a node somebody can reach, and it is the very
/// call [`come_back`] makes — so the two cannot drift into doing different things to the same
/// directory.
///
/// # Errors
///
/// `no_network` where there is no node to bring up. Nothing else: what the mesh and the interface
/// did is in the state rather than in a refusal.
#[tauri::command]
pub async fn come_up(
    app: tauri::AppHandle,
    running: tauri::State<'_, Running>,
) -> Result<State, &'static str> {
    let held = running.held.read().await;
    let serving = held.as_ref().ok_or("no_network")?;
    coming_up(&app, &running, serving).await;
    Ok(state_of(&running).await)
}

/// Stop this node, before the process this application is goes.
///
/// # The order is the whole of it
///
/// The door shuts first, so that nothing arrives for a node that is on its way out. Then the mesh,
/// so that this node stops being somewhere other nodes are told to call. Then the clock, and the
/// node with it. The directory is last, because letting go of it is what lets the next process be
/// this node — and doing that while any of the three above were still running would be two
/// processes over one record, which is the one thing the lock exists to refuse.
///
/// It is safe to run over a node that never came up: every step is a `take`, and taking nothing is
/// nothing.
pub async fn stopping(running: &Running) {
    let mut tasks = running.tasks.lock().await;

    if let Some(serving) = tasks.serving.take() {
        serving.abort();
    }
    *running.serving_at.lock().await = None;

    if let Some(mesh) = tasks.mesh.take() {
        mesh.abort();
    }
    *running.peers.lock().await = None;
    *running.crossed.lock().await = None;
    *running.where_it_listens.lock().await = None;

    if let Some(timekeeping) = tasks.timekeeping.take() {
        timekeeping.abort();
    }
    let was_up = running.held.write().await.take().is_some();

    // The lock is the open file and nothing else, so dropping this is letting go of the directory.
    drop(running.held_directory.lock().await.take());
    *running.which.lock().await = None;
    running.now_at(Phase::Stopped);

    if was_up {
        log::info!("node_stopped");
    }
}

/// Stop the node this application is running, from where there is no `async` to be had.
///
/// The platform hands the exit back on the thread it was started on, so the wait for the node to
/// let go happens here rather than being spawned — a stop nobody waited for is a process that ends
/// with its directory still held.
pub fn stop(app: &tauri::AppHandle) {
    let running = tauri::Manager::state::<Running>(app);
    tauri::async_runtime::block_on(stopping(&running));
}

/// Come back to the network this directory already holds, if it holds one.
///
/// # Why a start is not a step somebody takes
///
/// A node is a directory with a key in it, and the same directory is the same node however many
/// times it is started. So coming back is not a decision and is never offered as one: it is what
/// every start after the first does, and asking about it would be asking somebody to confirm that
/// their node is still their node.
///
/// **Nothing is opened and nothing is joined here.** A directory holding no record comes back as
/// nothing, which is what sends somebody to the one decision that does have to be taken. Opening a
/// network from a start would be how a restart becomes a second network.
///
/// **A start also brings it the rest of the way up**, through [`coming_up`]: the mesh place and the
/// interface, on what the preferences remember. Neither is a decision, and a start that left them
/// undone left a node holding a record and answering nobody.
///
/// # Errors
///
/// The reason it could not, as a stable identifier the interface translates. **A directory that
/// holds nothing is not one of them**: it is `Ok(None)`, because having no network yet is a state
/// and not a failure. Neither is a mesh port that was taken or an address that would not bind: the
/// node comes back and the state says what is wrong.
#[tauri::command]
pub async fn come_back(
    app: tauri::AppHandle,
    running: tauri::State<'_, Running>,
) -> Result<Option<Facts>, &'static str> {
    let mut held = running.held.write().await;
    if held.is_some() {
        // Already back. Answered with what it is rather than refused: a webview that reloaded asks
        // this again, and in development that is every save.
        let serving = held.as_ref().ok_or("no_network")?;
        return Ok(Some(facts_of(serving, &running).await));
    }

    // Said before anything is read, so that a start somebody is watching says it is starting rather
    // than staying stopped until it has finished.
    running.now_at(Phase::Starting);

    let Some(which) = which_to_come_back_to(&app) else {
        // A directory holding nothing: not a failure, and the walk is what happens next.
        running.now_at(Phase::Stopped);
        return Ok(None);
    };
    let (directory, holding, key) = ready_to_come_back(&app, which).inspect_err(|why| {
        running.now_at(Phase::Failing(why));
    })?;
    let node = almena_node::Node::rejoin(&directory, key)
        .map_err(|why| match why {
            almena_node::record::NotReadable::NotWritable => "no_directory",
            almena_node::record::NotReadable::DoesNotAddUp => "record_does_not_add_up",
            almena_node::record::NotReadable::AnotherNetwork => "not_the_promised_network",
            almena_node::record::NotReadable::Unreadable
            | almena_node::record::NotReadable::Refused => "unreadable_record",
        })
        .inspect_err(|why| {
            // A start that could not read the record is a failure that stays on screen: there is no
            // node, and the identifier is the whole of what anybody can act on.
            running.now_at(Phase::Failing(why));
        })?;

    // **Best effort, and silence is not fatal here.** The record is what this node is on; the zone
    // only says who else to dial once it takes its place on the mesh, and a node that refused to
    // come back because DNS was slow would be a node whose uptime depended on somebody else's.
    let zone = which_of(named(which)).map_or(DEVELOPMENT_ZONE, |(_, zone)| zone);
    match who_is_there(zone, &running).await {
        Ok(seeds) => log::info!("zone_read_on_rejoin seeds={}", seeds.len()),
        Err(why) => log::info!("zone_silent_on_rejoin zone={zone} reason={why}"),
    }
    let facts = taking_up(
        &app,
        &running,
        &mut held,
        Up {
            node,
            holding,
            which,
        },
    )
    .await;

    // The rest of the way up, on what the preferences remember. The same call the walk's press
    // makes, so that a first start and every start after it leave the same node running.
    if let Some(serving) = held.as_ref() {
        coming_up(&app, &running, serving).await;
    }
    Ok(Some(facts))
}

/// Which network a launch comes back to: the remembered one, else whichever directory holds a
/// record.
///
/// A launch is not a decision, so nobody is asked; what was chosen once is what comes back, and a
/// machine that remembers nothing — an upgrade from before there were two directories — is looked
/// at rather than sent back through the walk over a record it already holds. `None` is a machine
/// with no record anywhere, which is what sends somebody to the one decision that has to be taken.
fn which_to_come_back_to(app: &tauri::AppHandle) -> Option<almena_node::Which> {
    let remembered =
        crate::preferences::remembered_network(app).and_then(|word| which_of(&word).ok());
    let mut candidates: Vec<almena_node::Which> = Vec::new();
    if let Some((which, _)) = remembered {
        candidates.push(which);
    }
    for which in [
        almena_node::Which::Development,
        almena_node::Which::Production,
    ] {
        if !candidates.contains(&which) {
            candidates.push(which);
        }
    }
    candidates.into_iter().find(|which| {
        directory_of(app, *which).is_ok_and(|directory| {
            !matches!(
                almena_node::record::holding(&directory),
                almena_node::record::Holding::Nothing
            )
        })
    })
}

/// What the node reports, with the two figures the core does not carry filled in.
async fn facts_of(serving: &almena_serve::Serving, running: &Running) -> Facts {
    let node = serving.node().read().await;
    let mut facts: Facts = node.facts().into();
    facts.silent = Some(node.departed(running.now()));
    facts.peers = running
        .peers
        .lock()
        .await
        .as_ref()
        .map(almena_mesh::Peers::count);
    facts
}

/// Join the network a zone names, by asking somebody already on it for the record.
///
/// # What joining is, and why it is not opening
///
/// Opening a network makes one. Joining takes the one that is there: this node asks a seed the zone
/// names for everything it has written down, replays it, and announces itself. **Nothing that
/// arrives is believed for having arrived** — the acts are somebody else's signed bytes and go
/// through the same admission as any other, and the network they opened is checked against the name
/// the zone published before a single one is replayed.
///
/// # It joins by itself, and the operator only chooses which
///
/// Which network is a decision — signing against the wrong one does not come undone — and it is the
/// only one asked of anybody. Finding somebody, pulling the record and announcing are this node's
/// own work, and a wizard that walked an operator through them would be asking them to press
/// buttons for steps they cannot judge.
///
/// # Errors
///
/// The reason it could not, as a stable identifier the interface translates — never a sentence.
#[tauri::command]
pub async fn join_a_network(
    app: tauri::AppHandle,
    running: tauri::State<'_, Running>,
    asked: Joining,
) -> Result<Facts, &'static str> {
    // **A join that failed is a fact about this node, not only an answer to whoever asked.**
    // Since the walk stopped choosing networks, the press that joins hands over to the frame
    // whatever happens — so the reason has to survive the call that produced it, and the state
    // is where every screen already reads one from. Written here rather than at each `?` above:
    // one place that cannot be forgotten by the next refusal added below.
    joining(app, &running, asked).await.inspect_err(|why| {
        running.now_at(Phase::Failing(why));
    })
}

/// Join a network, with every refusal on its way out reported by the caller.
///
/// It is the body of [`join_a_network`] and nothing else. Separated so that the phase is written
/// once, over every path that fails, instead of at each of the dozen places one can.
async fn joining(
    app: tauri::AppHandle,
    running: &tauri::State<'_, Running>,
    asked: Joining,
) -> Result<Facts, &'static str> {
    let mut held = running.held.write().await;
    if held.is_some() {
        return Err("already_on_a_network");
    }
    let (wanted, theirs) = which_of(&asked.which)?;
    let (seed, dialling) = somebody_to_join(&asked, theirs).await?;
    let network = seed.network().to_owned();
    let address = almena_mesh::dialling(&seed).map_err(|_| "no_transport")?;
    // Kept for afterwards, so that taking a place on the mesh does not ask the zone a second time
    // and get a different answer.
    *running.dialling.lock().await = dialling;

    let (directory, holding, key) = ready(&app, wanted)?;
    let port = asked
        .port
        .or_else(|| crate::preferences::remembered_place(&app).0)
        .unwrap_or(MESH);
    let acts = pulled(&key, &network, port, &address).await?;

    // The instant the network began, out of the act that opened it — which is the only place it is
    // written and the reason a newcomer counts epochs from where everybody else does.
    let began = almena_node::Node::began_in(&acts).ok_or("record_does_not_add_up")?;
    running
        .began
        .store(began, std::sync::atomic::Ordering::Relaxed);
    // The act that announces this node is placed at the hour the network's clock says, and on
    // the development network that clock may have been moved — so the offset is settled before
    // the first look at it and not only when the node is taken up.
    running.reading_the_clock_for(wanted);
    let now = running.now();

    let joined = almena_node::Node::join(
        &directory,
        key,
        almena_node::Joining {
            acts: &acts,
            network: &network,
        },
        now,
    )
    .map_err(|why| match why {
        almena_node::record::NotReadable::AnotherNetwork => "not_the_promised_network",
        almena_node::record::NotReadable::NotWritable => "no_directory",
        almena_node::record::NotReadable::DoesNotAddUp => "record_does_not_add_up",
        almena_node::record::NotReadable::Unreadable
        | almena_node::record::NotReadable::Refused => "unreadable_record",
    })?;

    Ok(taking_up(
        &app,
        running,
        &mut held,
        Up {
            node: joined,
            holding,
            which: wanted,
        },
    )
    .await)
}

/// What the interface asks for when it joins a network.
///
/// **The window says which network, and the zone is that network's own unless somebody running a
/// network of their own names another.** A seed given by hand stands in for the zone the way the
/// terminal's `--seed` does: it only ever says *somebody is there*, which is the safe direction,
/// and it is how a machine with no zone at all joins a node beside it.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Joining {
    /// Which network: `development` or `production`.
    pub which: String,
    /// The port to pull the record on, which is the one that gets published in the zone.
    ///
    /// Absent is what the preferences remember, and [`MESH`] where they remember nothing — the
    /// same port a start takes, so that the port this node is reachable on is decided in one
    /// place rather than typed again by whoever is joining.
    #[serde(default)]
    pub port: Option<u16>,
    /// Another zone to look in, or nothing for the network's own.
    #[serde(default)]
    pub zone: Option<String>,
    /// Seed records given by hand, written as the zone writes them, or nothing to ask the zone.
    #[serde(default)]
    pub seeds: Vec<String>,
}

/// Somebody already on that network to ask, from the seeds given by hand or from the zone.
async fn somebody_to_join(
    asked: &Joining,
    theirs: &str,
) -> Result<(almena_node::zone::Seed, Vec<almena_mesh::Multiaddr>), &'static str> {
    let told: Vec<String> = asked
        .seeds
        .iter()
        .filter(|record| !record.trim().is_empty())
        .cloned()
        .collect();
    if told.is_empty() {
        let zone = asked
            .zone
            .as_deref()
            .map(str::trim)
            .filter(|named| !named.is_empty())
            .unwrap_or(theirs);
        return where_to_join(zone).await;
    }
    log::info!("seeds_given count={}", told.len());
    told_where_to_join(&told)
}

/// Somebody to ask, from seed records given by hand rather than read from a zone.
///
/// One that cannot be read is left out with its reason in the records and the rest stand; none
/// readable is *nobody is there* said of a hand-typed line, which is the one to go and fix.
fn told_where_to_join(
    told: &[String],
) -> Result<(almena_node::zone::Seed, Vec<almena_mesh::Multiaddr>), &'static str> {
    let mut seeds = Vec::new();
    for record in told {
        match almena_node::zone::Seed::read(record) {
            Ok(seed) => seeds.push(seed),
            Err(why) => log::info!("seed_unusable reason={why:?}"),
        }
    }
    let first = seeds.first().cloned().ok_or("the_zone_is_unreadable")?;
    let dialling = seeds
        .iter()
        .filter_map(|one| almena_mesh::dialling(one).ok())
        .collect();
    Ok((first, dialling))
}

/// Somebody on that network to ask, and everywhere else worth dialling afterwards.
///
/// **Parsed, because what is needed from a seed is more than somewhere to dial.** The record names
/// the network, and that name is the anchor everything pulled is checked against — a node that took
/// whatever it was handed would be calling that the network it joined.
async fn where_to_join(
    zone: &str,
) -> Result<(almena_node::zone::Seed, Vec<almena_mesh::Multiaddr>), &'static str> {
    let looked = looked_at(zone).await?;
    let Some(seed) = looked.answer.seeds.first().cloned() else {
        // **Nothing readable is not the same as nothing at all**, and telling them apart is the
        // difference between *open one* and *go and fix the zone*. A zone that published a seed
        // this build cannot read has said somebody is there — so opening is refused, correctly,
        // and saying *nobody is there* would send whoever is looking at it round a loop.
        return Err(if looked.seeds.is_empty() {
            "nobody_is_there"
        } else {
            "the_zone_is_unreadable"
        });
    };
    let dialling = looked
        .answer
        .seeds
        .iter()
        .filter_map(|one| almena_mesh::dialling(one).ok())
        .collect();
    Ok((seed, dialling))
}

/// The directory, held, and the key that outlives every run — in the one order that is safe.
///
/// **The directory is taken before anything in it is read or written, including the key**: two
/// processes racing to make one would each think they had made it. And a directory that already
/// holds a record is a node that has a network; joining over it would be a second history for one
/// identity.
fn ready_to_come_back(
    app: &tauri::AppHandle,
    which: almena_node::Which,
) -> Result<
    (
        std::path::PathBuf,
        almena_node::directory::Held,
        almena_node::SigningKey,
    ),
    &'static str,
> {
    let directory = directory_of(app, which)?;
    let holding = almena_node::directory::hold(&directory).map_err(|why| match why {
        almena_node::directory::NotHeld::AlreadyHeld => "directory_held",
        almena_node::directory::NotHeld::NotWritable => "no_directory",
        almena_node::directory::NotHeld::CannotTell => "directory_cannot_be_held",
    })?;
    let key = almena_node::identity::load_or_make(&directory).map_err(|why| match why {
        almena_node::identity::NoIdentity::NoRandomness => "no_randomness",
        almena_node::identity::NoIdentity::Unreadable => "unreadable_identity",
        almena_node::identity::NoIdentity::NotWritable => "no_directory",
    })?;
    Ok((directory, holding, key))
}

/// The same, for a directory that must be holding **no** record — which is what joining needs.
///
/// **The two are one function apart and are kept apart on purpose.** Coming back requires a record
/// and joining refuses one, and a single function with a flag would be one place where getting the
/// flag round the wrong way means a second history for one identity.
fn ready(
    app: &tauri::AppHandle,
    which: almena_node::Which,
) -> Result<
    (
        std::path::PathBuf,
        almena_node::directory::Held,
        almena_node::SigningKey,
    ),
    &'static str,
> {
    let directory = directory_of(app, which)?;
    let holding = almena_node::directory::hold(&directory).map_err(|why| match why {
        almena_node::directory::NotHeld::AlreadyHeld => "directory_held",
        almena_node::directory::NotHeld::NotWritable => "no_directory",
        almena_node::directory::NotHeld::CannotTell => "directory_cannot_be_held",
    })?;
    if !matches!(
        almena_node::record::holding(&directory),
        almena_node::record::Holding::Nothing
    ) {
        return Err("already_on_a_network");
    }
    let key = almena_node::identity::load_or_make(&directory).map_err(|why| match why {
        almena_node::identity::NoIdentity::NoRandomness => "no_randomness",
        almena_node::identity::NoIdentity::Unreadable => "unreadable_identity",
        almena_node::identity::NoIdentity::NotWritable => "no_directory",
    })?;
    Ok((directory, holding, key))
}

/// Everything one seed has written down, asked for over the mesh.
///
/// **Paged by the answering node and asked for until it stops growing.** What comes back says how
/// much that node holds altogether, which is what tells a short answer from the end of a record —
/// a caller that folded the first page would join on part of a history with nothing saying so.
async fn pulled(
    key: &almena_node::SigningKey,
    network: &str,
    port: u16,
    address: &almena_mesh::Multiaddr,
) -> Result<Vec<Vec<u8>>, &'static str> {
    // **The network's name is in the protocol's own name**, so a seed on another network offers
    // nothing this node asks for and the two never speak (`SPECS.md §4.12`). That check is made by
    // listening under this name, before anything is pulled.
    let mut listening =
        almena_mesh::listening(key, network, port, almena_mesh::Carrying::ForNobody)
            .map_err(|_| "no_transport")?;
    listening
        .dial(address.clone())
        .map_err(|_| "seed_unreachable")?;

    let mut acts: Vec<Vec<u8>> = Vec::new();
    let taken = tokio::time::timeout(JOINING_WITHIN, async {
        loop {
            match listening.next().await {
                almena_mesh::Happened::Met(peer, _) => {
                    listening.ask(&peer, almena_mesh::sync::Ask::Since(0));
                }
                almena_mesh::Happened::Answered(peer, _, said) => {
                    let had = acts.len() as u64;
                    acts.extend(said.acts);
                    // Done when this node holds what that one said it holds. Asking again from
                    // where it got to is how a record larger than one answer arrives.
                    if acts.len() as u64 >= said.written {
                        return true;
                    }
                    // A page that added nothing is a node that has no more to give, whatever it
                    // said it held. Stopping is the honest end: asking the same question for ever
                    // would be a loop nobody could see.
                    if acts.len() as u64 == had {
                        return !acts.is_empty();
                    }
                    listening.ask(&peer, almena_mesh::sync::Ask::Since(acts.len() as u64));
                }
                almena_mesh::Happened::Unanswered(_, _, _) => return false,
                _ => {}
            }
        }
    })
    .await;

    match taken {
        Ok(true) => Ok(acts),
        Ok(false) => Err("seed_would_not_answer"),
        Err(_) => Err("seed_too_slow"),
    }
}

/// Taking that place, with the node already in hand.
///
/// **The one implementation, and the reason it is not the command.** A start brings the node up
/// by itself and an operator can ask for the same thing from the Network screen; two
/// implementations of it would be two nodes' worth of behaviour to keep in step. The command above
/// takes the lock; a start already holds it, and calling the command from inside one would be a
/// deadlock rather than a shortcut.
async fn taking_a_place(
    app: &tauri::AppHandle,
    running: &Running,
    serving: &almena_serve::Serving,
    asked: &Place,
) -> Result<(), &'static str> {
    let network = serving.node().read().await.network().as_str().to_owned();

    let which = running.which.lock().await.ok_or("no_network")?;
    let directory = directory_of(app, which)?;
    let key = almena_node::identity::load_or_make(&directory).map_err(|_| "unreadable_identity")?;
    let listening = listening_on(&key, &network, asked).await?;

    // **Said in the record, where it is counted, before anybody is told to come here.** A client
    // picks a mediator from what the record says a node offers, and a mailbox that answered
    // without having said so would be a service the network could not see.
    if asked.mediator {
        let said = serving
            .node()
            .write()
            .await
            .also_offering(almena_node::Capability::Mailbox, running.now());
        log::info!(
            "mediator_offered {}",
            if said {
                "written=now"
            } else {
                "written=before"
            }
        );
    }

    // Taken before the socket goes: afterwards nothing else holds it, and a reading that wanted a
    // peer count would have nobody to ask.
    *running.peers.lock().await = Some(listening.peers());
    // Both handles come off the socket here, before it is handed away: afterwards nothing
    // else holds either, and a screen wanting a figure then would have nobody to ask.
    *running.crossed.lock().await = Some(listening.crossed());
    *running.where_it_listens.lock().await = Some(listening.where_it_listens());

    // Held rather than let go of, so that ending the application can leave the mesh instead of
    // having the process taken away with a socket still open.
    running.tasks.lock().await.mesh = Some(tokio::spawn(almena_mesh::keeping::keeping_up(
        listening,
        std::sync::Arc::clone(serving.node()),
        running.dialling.lock().await.clone(),
        running.clock(),
        ASK_EVERY,
    )));

    // Written down where it happened, so that the next start takes the same port without being
    // told. The port is the one somebody publishes: a start that took a different one would make
    // the published record wrong.
    crate::preferences::remember_mesh(app, asked.port);
    Ok(())
}

/// What the interface asks for when the node takes its place on the mesh.
///
/// Grouped because they are one decision: where this node listens, whether it carries other
/// nodes, whether it holds post, and who it asks to carry it. The terminal takes the same one.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    /// The port to listen on, which is the one somebody publishes.
    pub port: u16,
    /// Whether this node carries other nodes' traffic.
    #[serde(default)]
    pub carry: bool,
    /// Whether this node holds post for other people, and says so in the record.
    #[serde(default)]
    pub mediator: bool,
    /// Relays to ask to carry this one, for a node that cannot be dialled.
    #[serde(default)]
    pub carried_by: Vec<String>,
}

/// A socket on the mesh, asked to be carried where that was asked, and driven until the operating
/// system has said where it can be reached.
///
/// Driven here only until then: being reachable is a fact the node reports, and afterwards the
/// mesh belongs to whatever is keeping up. A relay that will not carry us is one relay and not a
/// reason to stop — which of them answers is not this node's to decide.
async fn listening_on(
    key: &almena_node::SigningKey,
    network: &str,
    asked: &Place,
) -> Result<almena_mesh::Listening, &'static str> {
    let carrying = if asked.carry {
        almena_mesh::Carrying::ForOthers
    } else {
        almena_mesh::Carrying::ForNobody
    };
    let mut listening =
        almena_mesh::listening(key, network, asked.port, carrying).map_err(|why| match why {
            almena_mesh::NotListening::NoIdentity
            | almena_mesh::NotListening::NoTransport
            | almena_mesh::NotListening::Anonymous => "no_transport",
            almena_mesh::NotListening::AddressUnavailable => "mesh_address_unavailable",
        })?;
    for relay in &asked.carried_by {
        match listening.ask_to_be_carried_at(relay) {
            Ok(address) => log::info!("mesh_asked_to_be_carried relay={address}"),
            Err(why) => log::error!("mesh_relay_not_asked relay={relay} reason={why:?}"),
        }
    }
    let _ = tokio::time::timeout(REACHABLE_WITHIN, async {
        while listening.port().is_none() {
            let _ = listening.next().await;
        }
    })
    .await;
    match listening.port() {
        Some(port) => log::info!("mesh_port port={port}"),
        None => log::info!("mesh_port port=unknown"),
    }
    Ok(listening)
}

/// The pair an operator names to serve under a certificate of their own, or nothing for the
/// node's own key.
///
/// Grouped because they are one decision and one refusal: one without the other is a node that
/// would answer under its own key having been asked to answer under somebody's certificate.
struct Under {
    /// The certificate chain, as a path to a PEM file.
    certificate: Option<String>,
    /// The key that belongs to it, as a path to a PEM file.
    private_key: Option<String>,
}

/// Opening that door, with the node already in hand.
///
/// The other half of what [`taking_a_place`] is: one implementation for the start that does it by
/// itself and the operator who asks for it, and not the command, because a start already holds the
/// lock the command takes.
async fn serving_on(
    app: &tauri::AppHandle,
    running: &Running,
    serving: almena_serve::Serving,
    address: &str,
    under: Under,
) -> Result<(), &'static str> {
    let (under, how) = under_which(app, running, under).await?;

    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|_| "address_unavailable")?;

    log::info!("interface_serving address={address} under={how}");
    *running.serving_at.lock().await = Some(origin_of(&listener, address));

    // **Said in the record, where it is counted, once the door is open.** What a network has is
    // counted from what its nodes say they offer, and a node answering on an interface it never
    // announced would be a service the network could not see. After the bind and not before, so
    // that nothing is claimed for a socket that was refused; and once — the core writes nothing
    // when the record already says it.
    let said = serving
        .node()
        .write()
        .await
        .also_offering(almena_node::Capability::Interface, running.now());
    log::info!(
        "interface_offered {}",
        if said {
            "written=now"
        } else {
            "written=before"
        }
    );

    // The same clock the node has been keeping time by since its network opened. Building a second
    // one here would be this face deciding what epoch it is, which is a fact and not a face's.
    let telling = running.clock();

    // Held rather than let go of, so that ending the application shuts the door instead of
    // leaving it open until the process is taken away.
    running.tasks.lock().await.serving = Some(tokio::spawn(async move {
        while let Ok((io, _)) = listener.accept().await {
            let Some(room) = serving.room() else {
                continue;
            };
            let serving = serving.clone();
            let telling = telling.clone();
            let under = under.clone();
            tokio::spawn(async move {
                let _room = room;
                // One node behind every connection: what is served is decided in one place, and
                // this only wraps what the bytes travel inside.
                if let Ok(wrapped) = under.accept(io).await {
                    let _ = serving.connection(wrapped, telling).await;
                }
            });
        }
    }));

    // For the same reason the port is: the address is the one that gets published, so the next
    // start serves where this one did.
    crate::preferences::remember_interface(app, address);
    Ok(())
}

/// What the interface is served under: the operator's pair, or the node's own key.
///
/// A node asked to serve under a certificate that will not load does not come up under its own
/// key instead: whoever asked for one would be told all was well while what they meant to serve
/// under was not what was served. The word that comes back is what the records say.
async fn under_which(
    app: &tauri::AppHandle,
    running: &Running,
    under: Under,
) -> Result<(almena_tls::Accepting, &'static str), &'static str> {
    match (under.certificate, under.private_key) {
        (Some(certificate), Some(key)) => Ok((
            almena_tls::accepting(
                std::path::Path::new(&certificate),
                std::path::Path::new(&key),
            )
            .map_err(|why| match why {
                almena_tls::NoCertificate::NoChain => "no_certificate",
                almena_tls::NoCertificate::NoKey => "no_private_key",
                almena_tls::NoCertificate::NotAPair => "certificate_and_key_are_not_a_pair",
            })?,
            "a_certificate",
        )),
        (None, None) => {
            let which = running.which.lock().await.ok_or("no_network")?;
            let directory = directory_of(app, which)?;
            let key = almena_node::identity::load_or_make(&directory)
                .map_err(|_| "unreadable_identity")?;
            Ok((
                almena_tls::self_signed(&key.secret())
                    .map_err(|_| "certificate_and_key_are_not_a_pair")?,
                "own_key",
            ))
        }
        // One without the other is a node that would answer under its own key having been asked
        // to answer under somebody's certificate.
        _ => Err("no_private_key"),
    }
}

/// Where a bound listener is, written the way somebody would type it.
///
/// **What was actually bound and not what was asked for**: a port of nought is a real request, and
/// the answer to it is whatever the operating system granted. Only where the socket will not say
/// does the asked-for address stand in, which is the closest thing to true that is left. Always
/// `https`, because the interface is never served in the clear.
fn origin_of(listener: &tokio::net::TcpListener, asked: &str) -> String {
    let bound = listener
        .local_addr()
        .map_or_else(|_| asked.to_owned(), |at| at.to_string());
    format!("https://{bound}")
}

/// Erase this node from this machine, and leave a machine that is not a node.
///
/// # The order is the whole of it, again
///
/// **The network is told first, while there is still a node to tell it with.** Closing is an act
/// this node signs and appends to its own chain; once the files are gone there is no key to sign
/// it with and nothing to append it to, so an erase that deleted first would leave a node that
/// everybody else goes on expecting until the observers give up on it.
///
/// Then the node is stopped — the door, the mesh, the clock, and the directory let go — because
/// deleting files out from under a running node is the two-processes-over-one-record failure seen
/// from the other side. Then the directory goes: the key, the acts, the roots and the lock, which
/// is everything this node was. Then the notes the node kept in the preferences, so that the next
/// launch does not come back to a directory that is not there.
///
/// # It does not refuse over a node that is down
///
/// **A way out that needs a working node is not a way out.** The one person most likely to want
/// this is the one whose node will not come up, so nothing here is conditional on the node being
/// on a network, running, or reachable: what could not be said is logged and the erase goes on. A
/// node that was never announced as closed is one the record's observers will find silent, which
/// is a worse outcome than a clean close and a better one than a machine somebody cannot leave.
///
/// Erasing a machine that is not a node is not a failure. It is the state being asked for, and it
/// is already the case; the preferences are cleared anyway, because a note about a node that is
/// not there is the thing that would send the next launch somewhere strange.
///
/// # What goes with it that nobody asked about
///
/// **The government key, on the one machine that has one.** A node that opened its network keeps
/// that network's government key beside its record, and the directory is what this takes away — so
/// erasing the machine that opened a network is also giving up the ability to publish its core or
/// certify anybody on it, for ever. It is said rather than refused: a way out that some machines
/// cannot take is not a way out, and the machine that opened the network is not the one this
/// should be hardest for. What it must not be is silent, so it is logged when it happens.
///
/// # Errors
///
/// `no_directory` where the platform will not say where the data lives, and `not_erased` where
/// the files would not go — a directory somebody else has open, or one this process may not
/// write. Both leave the node where it was rather than half of it.
#[tauri::command]
pub async fn erase_this_node(
    app: tauri::AppHandle,
    running: tauri::State<'_, Running>,
) -> Result<(), &'static str> {
    // Whichever node is being erased: the one that is up, or — where none came up — whichever
    // directory still holds a record, which is the case this exists for.
    let which = match *running.which.lock().await {
        Some(which) => Some(which),
        None => which_to_come_back_to(&app),
    };
    let Some(which) = which else {
        // Nothing on disk to take away. The preferences are still cleared: a remembered network
        // over a directory that holds nothing is the one thing that could still be wrong here.
        crate::preferences::forget_the_node(&app);
        log::info!("node_erased_nothing_to_erase");
        return Ok(());
    };

    // Said first, and only where there is a node up to say it with. The guard is dropped before
    // stopping, which takes the same lock for writing.
    {
        let held = running.held.read().await;
        match held.as_ref() {
            Some(serving) => {
                let now = running.now();
                if serving.node().write().await.close_itself(now) {
                    log::info!("node_closed_before_erasing");
                } else {
                    log::warn!("node_not_closed_before_erasing reason=not_written_down");
                }
            }
            None => log::warn!("node_not_closed_before_erasing reason=no_network"),
        }
    }

    stopping(&running).await;

    let directory = directory_of(&app, which)?;
    // Said before it goes, because afterwards there is nothing to read it off.
    if almena_node::government::load(&directory).is_ok() {
        log::warn!("erasing_the_government_key which={}", worded(which));
    }
    match std::fs::remove_dir_all(&directory) {
        Ok(()) => log::info!("node_erased which={}", worded(which)),
        // Already gone is the state being asked for, not a failure to reach it.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            log::info!("node_erased_already_gone which={}", worded(which));
        }
        Err(error) => {
            log::error!("node_not_erased which={} reason={error}", worded(which));
            return Err("not_erased");
        }
    }

    crate::preferences::forget_the_node(&app);
    Ok(())
}

/// Open a network in a directory that is holding none, keeping the government's key beside it.
///
/// **The key is written before the record and taken away if the record does not start**, so the
/// two are never apart: a record without its government key is a network nobody can publish the
/// core on, and a key without a record is a file that would refuse the next open. Almena
/// Government's key belongs to the network and is made with it — opening a development network
/// again makes a new one, which is what opening a new network means.
fn first_time(
    directory: &std::path::Path,
    which: almena_node::Which,
    seeds: &[String],
    key: almena_node::SigningKey,
) -> Result<almena_node::Node, &'static str> {
    if !seeds.is_empty() {
        return Err("there_is_a_network");
    }
    let government = almena_node::fresh_key().map_err(|_| "no_randomness")?;
    let kept = almena_node::government::keep(directory, &government)
        .map_err(|_| "government_key_not_kept")?;

    // The one wall clock reading this platform ever writes down. Everything afterwards counts whole
    // hours from it, so it is read once, here, and never again.
    let began = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "no_clock")?
        .as_secs();

    let opening = almena_node::Opening {
        which,
        beginning: almena_node::Epoch::GENESIS,
        began,
    };
    let opened =
        almena_node::Node::open_in(directory, &opening, seeds, &government, key).map_err(|why| {
            match why {
                almena_node::NotOpened::ThisNodeAlreadyHasOne => "already_on_a_network",
                almena_node::NotOpened::ThereIsAlreadyANetwork(_) => "there_is_a_network",
                almena_node::NotOpened::TheRecordWouldNotStart => "record_would_not_start",
                // Production only: the core holds the format to its freeze checklist before
                // opening one, and refuses rather than opening a network on a format that is
                // still moving.
                almena_node::NotOpened::TheFormatIsNotFrozen(_) => "format_is_not_frozen",
            }
        });
    match opened {
        Ok(node) => {
            log::info!("government_key_at path={}", kept.display());
            Ok(node)
        }
        Err(why) => {
            let _ = std::fs::remove_file(&kept);
            Err(why)
        }
    }
}

/// Open a development network, on the zone's word that there is nobody to join.
///
/// # Why the window opens development and never production
///
/// **A development network is opened as often as it needs to be; a production one is opened once
/// in the history of the platform** (`SPECS.md §4.5`). A window that opened whichever network it
/// found empty would give every fresh install its own production network the first time the zone
/// was quiet — the accident that section calls the one that costs the most, and one an append-only
/// log does not undo.
///
/// So this refuses production **before anything happens**, on the argument rather than on what the
/// zone said: there is no ordering of events, no slow resolver and no mistyped word that reaches
/// the opening of a production network from this face. Opening production is a deliberate act at a
/// terminal, by whoever is doing it, and it stays there.
///
/// The zone is asked and its answer is what decides: nobody there means there is a network to open,
/// and **silence is not nobody**. That rule is the core's and is applied below the interface; what
/// this refuses on its own is only the network.
///
/// # Errors
///
/// `nobody_is_there_is_for_development` for production, and otherwise whatever the core said —
/// `there_is_a_network` where the zone names somebody, `zone_silent` where it did not answer at
/// all, and the directory's own refusals.
#[tauri::command]
pub async fn open_a_network(
    app: tauri::AppHandle,
    running: tauri::State<'_, Running>,
    which: String,
) -> Result<Facts, &'static str> {
    opening(app, &running, which).await.inspect_err(|why| {
        running.now_at(Phase::Failing(why));
    })
}

/// Open a network, with every refusal on its way out reported by the caller.
async fn opening(
    app: tauri::AppHandle,
    running: &tauri::State<'_, Running>,
    which: String,
) -> Result<Facts, &'static str> {
    let (wanted, zone) = which_of(&which)?;
    // Refused on the word and not on the answer. See above: there is no path from this face to a
    // production network being opened, and it is closed here rather than anywhere it could be
    // reached by an ordering of events.
    if matches!(wanted, almena_node::Which::Production) {
        return Err("nobody_is_there_is_for_development");
    }

    let mut held = running.held.write().await;
    if held.is_some() {
        return Err("already_on_a_network");
    }

    // Asked, and its answer is what decides. A zone naming somebody is a network to join rather
    // than one to open, and the core is what says so.
    let seeds = who_is_there(zone, running).await?;
    let (directory, holding, key) = ready(&app, wanted)?;
    running.reading_the_clock_for(wanted);
    let opened = first_time(&directory, wanted, &seeds, key)?;

    Ok(taking_up(
        &app,
        running,
        &mut held,
        Up {
            node: opened,
            holding,
            which: wanted,
        },
    )
    .await)
}

/// One peer this node is connected to, as a face draws it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    /// What it answers to on the mesh: its `PeerId`, which **is** its key with a prefix.
    pub peer: String,
    /// The address this connection is on, as a multiaddress.
    ///
    /// Where this node is talking to it in fact — dialled, or answered and observed — and not any
    /// address the record or the zone carries. It is never written down anywhere (§17.18).
    pub address: String,
    /// The last round trip to it in milliseconds, or nothing where none has come back yet.
    ///
    /// **Absent is not nought.** The first ping goes out after a connection settles, so a peer
    /// that has just arrived has no round trip — and a zero there would be the fastest connection
    /// on the list, invented.
    pub far: Option<u128>,
}

/// Who this node is connected to right now.
///
/// **A fact about sockets, and only that.** Who is a node the record knows is a different question
/// with a different answer — the census is in the log — and this is who is on the other end of a
/// connection at this moment. A node it has never heard of is not in the record; a node in the
/// record it is not talking to is not here.
///
/// An empty list and no list are different answers and both are drawn: `None` is a node with no
/// place on the mesh, where nobody has counted anything, and `Some([])` is a node that has taken
/// its place and is talking to nobody. A face that showed them the same way would be claiming a
/// measurement nobody took.
#[tauri::command]
pub async fn peers_connected(running: tauri::State<'_, Running>) -> Result<Option<Vec<Peer>>, ()> {
    Ok(running.peers.lock().await.as_ref().map(|peers| {
        peers
            .reached()
            .into_iter()
            .map(|(peer, reached)| Peer {
                peer: peer.to_string(),
                address: reached.address.to_string(),
                far: reached.far.map(|took| took.as_millis()),
            })
            .collect()
    }))
}

/// What has crossed this node's mesh, in bytes each way.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Crossed {
    /// Bytes of record traffic read off the wire since this node came up.
    pub taken: u64,
    /// Bytes of record traffic written to the wire since this node came up.
    pub given: u64,
}

/// What an operator hands to whoever keeps the zone, so that this node can be a seed.
///
/// # Why the node writes it and not the screen
///
/// The shape of these records is the platform's (`ZONES.md`), not a face's. Composed on a screen it
/// would be composed twice — once per face — and the two would drift, which for a record that is
/// **a commitment newcomers verify against** is worse than it sounds: a `_seed` with the wrong
/// `net=` sends whoever reads it to a network that is not this one.
///
/// # What it knows, and the one thing it does not
///
/// The port it actually bound, its own public key, and the name of its network: those three are
/// the parts nobody else can produce. **The host name is not among them** — it is the zone
/// keeper's to choose, so it is left as a placeholder rather than guessed at.
///
/// The addresses are what the operating system granted, with loopback and private ones dropped and
/// **relayed ones dropped too**: an address a relay lends answers for as long as somebody else
/// agrees to carry this node and stops without it being told, so a zone pointing at one is a zone
/// pointing at a door nobody is behind.
///
/// # It says where it thinks it is, not where the world sees it
///
/// A node knows what it bound. Behind a household router that is not what anybody else can dial,
/// and this node does not learn its observed address. So whoever keeps the zone checks the record
/// before publishing it — dial the address, see that the handshake key is the `peer=`, see that the
/// record handed over starts with the act `net=` names — which is what they have to do anyway.
///
/// `None` where there is no place on the mesh: a node that is not listening has no `_seed` to be.
#[tauri::command]
pub async fn seed_record(running: tauri::State<'_, Running>) -> Result<Option<String>, ()> {
    let Some(where_it_listens) = running.where_it_listens.lock().await.clone() else {
        return Ok(None);
    };
    let held = running.held.read().await;
    let Some(serving) = held.as_ref() else {
        return Ok(None);
    };
    let facts = serving.node().read().await.facts();
    let (Some(peer), Some(network)) = (facts.peer, facts.network) else {
        return Ok(None);
    };
    let Some(port) = where_it_listens.port() else {
        return Ok(None);
    };

    // Composed below the interface, so that the terminal and this window hand whoever keeps the
    // zone the very same record. What the interface adds is a button.
    Ok(Some(almena_mesh::seed_record(
        &peer,
        &network,
        port,
        running
            .serving_at
            .lock()
            .await
            .as_ref()
            .and_then(|origin| origin.rsplit(':').next()?.parse().ok()),
        &where_it_listens.all(),
    )))
}

/// How much record traffic has crossed this node, or nothing where there is no mesh.
///
/// **Record traffic and not every byte on the wire**, which is what the counters count: the acts,
/// the pages and the roots this node asked for and answered with. The handshake, the identify
/// exchange, the pings and anything a relay carries for somebody else are outside it — counting
/// those would mean wrapping the transport, and one figure mixing *what this node moved* with
/// *what its sockets cost* would answer neither question.
///
/// **Totals since this node came up, and never a rate.** A rate is two of these a moment apart,
/// and how far apart is a decision for whoever is drawing it. `None` is a node with no place on
/// the mesh: nothing has crossed because there is nothing for it to cross.
#[tauri::command]
pub async fn crossed(running: tauri::State<'_, Running>) -> Result<Option<Crossed>, ()> {
    Ok(running
        .crossed
        .lock()
        .await
        .as_ref()
        .map(|crossed| Crossed {
            taken: crossed.taken(),
            given: crossed.given(),
        }))
}

/// How many bytes this node is keeping on disk, or nothing where there is no node.
///
/// **What it costs to be this node**, which is the one figure about a node that a person running
/// one on their own machine actually wants: the key, the record, the roots and the entries, as
/// they are on disk right now. Measured rather than remembered — a stored figure would be wrong
/// the moment an epoch closed — and it is a walk of one directory, which holds a handful of files.
///
/// `None` is a node with no directory, which is not a size of nought: nothing was measured.
#[tauri::command]
pub async fn stored(
    app: tauri::AppHandle,
    running: tauri::State<'_, Running>,
) -> Result<Option<u64>, ()> {
    let Some(which) = *running.which.lock().await else {
        return Ok(None);
    };
    let Ok(directory) = directory_of(&app, which) else {
        return Ok(None);
    };
    let Ok(entries) = std::fs::read_dir(&directory) else {
        return Ok(None);
    };
    // Flat on purpose: a node's directory is a key, a lock and three record files, and a walk that
    // recursed would be describing a shape this node does not have.
    Ok(Some(
        entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.metadata().ok())
            .filter(std::fs::Metadata::is_file)
            .map(|metadata| metadata.len())
            .sum(),
    ))
}

/// Where this node serves its interface, or nothing where it serves none.
///
/// **Absent is a state and not a gap.** A node that has not been asked to serve has no origin, and
/// a plausible-looking address standing in for one would send somebody to a door that is not open.
#[tauri::command]
pub async fn interface_at(running: tauri::State<'_, Running>) -> Result<Option<String>, ()> {
    Ok(running.serving_at.lock().await.clone())
}

/// What this node will and will not do for whoever asks.
fn limits() -> almena_api::Limits {
    almena_api::Limits {
        per_connection: 600,
        window: 60,
        largest_act: 65_536,
        connections: 256,
    }
}

/// Which command offers which capability, for the check against the table both faces are held to.
///
/// **The window's own surface, written down where a test can hold it to the table.** Every
/// command registered in `lib.rs` is here, and every capability the table says the window offers
/// has a command here — so a command added without a row, or a row without a command, fails the
/// build rather than being found by somebody looking for it on the face that does not have it.
#[cfg(test)]
const COMMANDS: &[(&str, &[almena_node::facade::Capability])] = {
    use almena_node::facade::Capability;
    &[
        ("node_facts", &[Capability::Watch]),
        ("interface_at", &[Capability::Watch]),
        // What this node is doing is *seeing what this node is*, which is the capability the
        // terminal draws as its own status. No row of its own in the table: a second capability
        // for the same question would be a second answer to it.
        ("node_state", &[Capability::Watch]),
        // Who this node is talking to and what it keeps on disk are both *seeing what this
        // node is*, which is the one capability the terminal draws as its own status. No
        // rows of their own: a second capability for the same question would be a second
        // answer to it.
        ("peers_connected", &[Capability::Watch]),
        ("stored", &[Capability::Watch]),
        ("crossed", &[Capability::Watch]),
        ("seed_record", &[Capability::SayHowToFindMe]),
        // A start takes the mesh place and opens the door by itself, which is the same two
        // capabilities the terminal takes as flags. **They reach the window only this way now** —
        // there is no control that takes a port or an address — so the window offers them without
        // ever asking anybody about them, which is what the rows in the table say.
        ("come_up", &[Capability::JoinTheMesh, Capability::Serve]),
        // The one press on the first screen. It joins production and names no zone and no seed,
        // which is why `WhereToLook` is no longer on this row: the window has nowhere to say
        // where to look any more, and a row claiming otherwise would be this table lying.
        ("join_a_network", &[Capability::JoinNetwork]),
        // Development alone, and refused for production on the argument itself. The one
        // press a start makes falls through to it where the development zone names nobody,
        // which is what a network opened as often as it needs to be is for.
        ("open_a_network", &[Capability::OpenNetwork]),
        (
            "come_back",
            &[
                Capability::ComeBack,
                // A start is the whole of coming up, and the mesh and the interface are part of
                // it. Named here because this table is what the window really offers, and a
                // command that quietly does a third thing is exactly what it exists to catch.
                Capability::JoinTheMesh,
                Capability::Serve,
            ],
        ),
        // Not operating a node — the opposite, and the only thing in this application a person
        // cannot get out of any other way. It is in Settings.
        ("erase_this_node", &[Capability::EraseThisNode]),
    ]
};

#[cfg(test)]
mod tests {
    use super::{COMMANDS, Facts, Phase, Running, state_of, stopping};

    /// The wall clock's seconds since the Unix epoch, for a network that began just now.
    fn just_now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs())
    }

    #[test]
    fn a_clock_offset_file_moves_this_face_s_clock_as_it_is_written() {
        // **The knob the development network is walked with**, read on every look. The
        // environment is the process's, so this test names the file straight rather than
        // through it — what is under test is the clock reading the offset, not the variable.
        let file = std::env::temp_dir().join("almena-app-node-clock");
        let _ = std::fs::remove_file(&file);
        std::fs::write(&file, "5\n").expect("written");

        let running = Running::default();
        running
            .began
            .store(just_now(), std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            running.now(),
            almena_node::Epoch::GENESIS,
            "nothing read until an offset is settled"
        );
        *running
            .offset
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            std::sync::Arc::new(crate::clock::Offset::reading(file.clone()));
        assert_eq!(running.now(), almena_node::Epoch::new(5));

        std::fs::write(&file, "72").expect("written");
        assert_eq!(
            running.now(),
            almena_node::Epoch::new(72),
            "read again on every look"
        );
        let telling = running.clock();
        std::fs::remove_file(&file).expect("taken away");
        assert_eq!(
            telling(),
            almena_node::Epoch::GENESIS,
            "an absent file is nought, and a clock handed out reads the same file"
        );
    }

    #[tokio::test]
    async fn a_node_that_never_started_is_stopped_and_nothing_else() {
        // Stopped is a state and not a failure, and it is what a directory holding no record
        // answers. Nothing on it is invented: no network, no mesh, no door, and a peer count
        // nobody took stays absent rather than becoming a nought.
        let state = state_of(&Running::default()).await;
        assert_eq!(state.state, "stopped");
        assert_eq!(state.failing, None);
        assert_eq!(state.which, None);
        assert!(!state.mesh);
        assert!(!state.serving);
        assert_eq!(state.peers, None, "nobody counted, which is not nought");
    }

    #[tokio::test]
    async fn a_start_that_went_wrong_says_which_thing_by_its_identifier() {
        // The whole point of carrying the reason: *something is wrong* is not something anybody
        // can act on, and the word has to be the same word two operators comparing notes see.
        let running = Running::default();
        running.now_at(Phase::Failing("mesh_address_unavailable"));
        let state = state_of(&running).await;
        assert_eq!(state.state, "failing");
        assert_eq!(state.failing, Some("mesh_address_unavailable"));
    }

    #[test]
    fn the_four_states_are_the_four_and_only_failing_carries_a_reason() {
        // Four words, because the badge that draws them has four tones and no fifth. A state
        // added here without one is a state nothing can draw.
        assert_eq!(Phase::Stopped.worded(), "stopped");
        assert_eq!(Phase::Starting.worded(), "starting");
        assert_eq!(Phase::Running.worded(), "running");
        assert_eq!(Phase::Failing("no_transport").worded(), "failing");
        assert_eq!(Phase::Stopped.failing(), None);
        assert_eq!(Phase::Starting.failing(), None);
        assert_eq!(Phase::Running.failing(), None);
        assert_eq!(
            Phase::Failing("no_transport").failing(),
            Some("no_transport")
        );
    }

    #[tokio::test]
    async fn what_crosses_to_the_webview_says_the_state_the_same_way_every_time() {
        // The interface reads these keys by name, and `failing` has to arrive as `null` rather
        // than as a missing key or an empty string: absent is a state.
        let json = serde_json::to_string(&state_of(&Running::default()).await).expect("serialises");
        assert_eq!(
            json,
            r#"{"state":"stopped","failing":null,"which":null,"mesh":false,"serving":false,"peers":null}"#
        );
    }

    #[tokio::test]
    async fn stopping_lets_go_of_the_directory_so_the_next_process_can_be_this_node() {
        // What ending the application has to achieve, and the one part of it a test can hold:
        // the lock is the open file, so a directory this process can hold again is a directory
        // it let go of. A second hold while the first is alive is what proves the test is real.
        let directory = std::env::temp_dir().join("almena-app-node-stopping");
        std::fs::create_dir_all(&directory).expect("made");
        let holding = almena_node::directory::hold(&directory).expect("held");
        assert!(
            almena_node::directory::hold(&directory).is_err(),
            "a directory somebody holds cannot be held twice"
        );

        let running = Running::default();
        *running.held_directory.lock().await = Some(holding);
        *running.which.lock().await = Some(almena_node::Which::Development);
        *running.serving_at.lock().await = Some("https://127.0.0.1:8791".to_owned());
        running.now_at(Phase::Running);
        // A task standing in for the three a running node keeps: what matters is that stopping
        // ends it rather than leaving it to be taken away with the process.
        let forever = tokio::spawn(async { std::future::pending::<()>().await });
        running.tasks.lock().await.serving = Some(forever);

        stopping(&running).await;

        assert!(
            almena_node::directory::hold(&directory).is_ok(),
            "stopping did not let go of the directory"
        );
        let state = state_of(&running).await;
        assert_eq!(state.state, "stopped");
        assert_eq!(state.which, None);
        assert!(!state.serving, "the door is shut");
        assert!(
            running.tasks.lock().await.serving.is_none(),
            "nothing is left holding a task nobody will ask to stop again"
        );
    }

    #[tokio::test]
    async fn erasing_takes_the_directory_away_and_lets_go_of_it_first() {
        // The two halves that matter and that a test can hold without a Tauri app handle: the
        // files go, and the lock is let go before they do — a directory this process is still
        // standing in is one `remove_dir_all` fails on where the platform says so, and one that
        // comes back held where it does not.
        let directory = std::env::temp_dir().join("almena-app-node-erasing");
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("made");
        std::fs::write(directory.join("identity.key"), b"not really a key").expect("written");
        let holding = almena_node::directory::hold(&directory).expect("held");

        let running = Running::default();
        *running.held_directory.lock().await = Some(holding);
        *running.which.lock().await = Some(almena_node::Which::Development);
        running.now_at(Phase::Running);

        // What the command does between announcing and deleting, which is the part under test.
        stopping(&running).await;
        std::fs::remove_dir_all(&directory).expect("the directory was let go before deleting");

        assert!(
            !directory.exists(),
            "the key, the record and the roots are what erasing takes away"
        );
        assert_eq!(state_of(&running).await.state, "stopped");
        assert_eq!(
            state_of(&running).await.which,
            None,
            "nothing is left saying which network a node that is gone was on"
        );
    }

    #[test]
    fn erasing_a_directory_that_is_already_gone_is_the_state_being_asked_for() {
        // A node erased twice, or one whose directory somebody removed by hand, must not be a
        // refusal: the state asked for is *this machine is not a node*, and it is already true.
        let directory = std::env::temp_dir().join("almena-app-node-never-there");
        let _ = std::fs::remove_dir_all(&directory);
        assert_eq!(
            std::fs::remove_dir_all(&directory).unwrap_err().kind(),
            std::io::ErrorKind::NotFound,
            "the case the command reads as success rather than as a failure to reach it"
        );
    }

    #[tokio::test]
    async fn stopping_a_node_that_never_came_up_is_nothing() {
        // Ending the application runs this whether or not there was ever a node — a directory
        // holding no record reaches `RunEvent::Exit` like any other.
        let running = Running::default();
        stopping(&running).await;
        assert_eq!(state_of(&running).await.state, "stopped");
    }

    #[test]
    fn a_node_with_no_network_has_looked_at_nothing() {
        let facts: Facts = almena_node::Facts::default().into();
        assert!(facts.network.is_none());
        assert!(facts.identity.is_none());
        assert!(
            facts.written.is_none(),
            "never a zero for a count nobody took"
        );
        assert!(facts.root.is_none());
        assert!(facts.peers.is_none(), "nobody counted, which is not nought");
        assert!(facts.silent.is_none());
    }

    #[test]
    fn what_this_face_registers_is_what_the_table_says_it_offers() {
        // **The window's half of the parity check.** The table in the core says which
        // capabilities the window draws; this is the window saying which commands draw them, and
        // the two have to agree — plus the two that are not commands: the language, which is the
        // preferences', and the clock offset, which is the environment's and is read by the clock
        // every command runs on (`offset_for`).
        use almena_node::facade::{Capability, offered_by_window};
        let mut drawn: Vec<Capability> = COMMANDS
            .iter()
            .flat_map(|(_, capabilities)| capabilities.iter().copied())
            .collect();
        drawn.extend([Capability::Language, Capability::ClockOffset]);
        for capability in offered_by_window() {
            assert!(
                drawn.contains(&capability),
                "{capability:?}: the table says the window offers it and no command does"
            );
        }
        for capability in &drawn {
            assert!(
                offered_by_window().contains(capability),
                "{capability:?}: a command offers it and the table does not say so"
            );
        }
    }

    #[test]
    fn every_command_named_here_is_registered_and_every_node_command_registered_is_named_here() {
        // The list above is only worth anything if it is the real surface. `lib.rs` is where the
        // commands are registered, so it is read as text and held to the list both ways.
        let registered = include_str!("lib.rs");
        for (command, _) in COMMANDS {
            assert!(
                registered.contains(&format!("node::{command},")),
                "{command} is named here and not registered in lib.rs"
            );
        }
        for line in registered.lines() {
            let line = line.trim();
            if let Some(command) = line
                .strip_prefix("node::")
                .and_then(|rest| rest.strip_suffix(','))
            {
                assert!(
                    COMMANDS.iter().any(|(named, _)| named == &command),
                    "{command} is registered and not named here"
                );
            }
        }
    }

    #[test]
    fn every_fact_comes_across_from_the_core_unchanged() {
        // The one thing this conversion must never do is compute something. If it did, this face
        // and the terminal would answer the same question differently.
        let from_core = almena_node::Facts {
            network: Some("znetwork".to_owned()),
            identity: Some("aabb".to_owned()),
            written: Some(7),
            root: Some("ccdd".to_owned()),
            peer: Some("12D3KooWSomewhere".to_owned()),
        };
        let drawn: Facts = from_core.clone().into();

        assert_eq!(drawn.network, from_core.network);
        assert_eq!(drawn.identity, from_core.identity);
        assert_eq!(drawn.written, from_core.written);
        assert_eq!(drawn.root, from_core.root);
        assert_eq!(drawn.peer, from_core.peer);
        assert_eq!(drawn.peers, None, "the conversion never guesses at a count");
        assert_eq!(drawn.silent, None);
    }

    #[test]
    fn what_crosses_to_the_webview_keeps_an_absent_fact_absent() {
        // `null` has to arrive as `null`. A missing key, or a zero, would be the webview drawing a
        // measurement that was never taken.
        let json = serde_json::to_string(&Facts::default()).expect("serialises");
        assert_eq!(
            json,
            r#"{"network":null,"identity":null,"written":null,"root":null,"peer":null,"peers":null,"silent":null}"#
        );
    }
}
