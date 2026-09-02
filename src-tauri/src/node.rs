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
    /// Which network the running node is for, and therefore which directory it lives in.
    ///
    /// Kept because every later command that reaches the directory — the mesh reading the key, the
    /// interface serving under it — has to reach the same one the node came up from.
    which: tokio::sync::Mutex<Option<almena_node::Which>>,
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

/// The resolver the zone is asked through: the machine's own, or the one `ALMENA_RESOLVER` names.
///
/// **A development knob, read by nothing a deployment sets.** A zone emulated on this machine
/// answers on a port of its own, and this is how the window is pointed at it — the same reading
/// the terminal's `--resolver` does, so a value that works for one works for the other.
///
/// # Errors
///
/// `zone_silent` when no resolver can be built: a machine with no resolver is a zone that cannot be
/// asked, which is a silence and never an empty zone.
fn resolver() -> Result<almena_lookup::Dns, &'static str> {
    match std::env::var("ALMENA_RESOLVER") {
        Ok(named) if !named.trim().is_empty() => {
            let server = almena_lookup::server(&named).map_err(|_| "resolver_not_an_address")?;
            almena_lookup::Dns::asking(&[server]).map_err(|_| "zone_silent")
        }
        _ => almena_lookup::Dns::of_this_machine().map_err(|_| "zone_silent"),
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

/// Open a network, on the zone's word that there is nobody to join.
///
/// # Opening is not joining, and the difference is the whole of this
///
/// **A node opens a network only when nobody is there**, and it learns that by reading that
/// network's zone — the same question asked of a different name. Development is opened again as
/// often as the format moves; **production is opened once in the history of the platform**, not
/// once per machine, so what stops a second one is not this function's manners but the zone
/// answering that somebody is already there.
///
/// # The freeze gate is the core's and is not enforced here
///
/// `Node::open_in` refuses to open production on a format that is still moving, and answers
/// `TheFormatIsNotFrozen`. Nothing in this layer repeats that check: a second implementation of a
/// rule is two rules that will one day disagree. What the interface above does with
/// [`freeze_checklist`] is show **why** before it happens, which is a different job.
///
/// # Errors
///
/// The reason it could not, as a stable identifier the interface translates — never a sentence.
#[tauri::command]
pub async fn open_a_network(
    app: tauri::AppHandle,
    running: tauri::State<'_, Running>,
    which: String,
    zone: Option<String>,
    nobody_is_there: Option<bool>,
) -> Result<Facts, &'static str> {
    let (wanted, theirs) = which_of(&which)?;
    // **Somebody's word instead of the zone's reaches development alone.** The whole defence
    // against a second production network is the zone being asked, and this is the one place
    // that defence is set aside — where a second network costs an afternoon.
    let nobody_is_there = nobody_is_there.unwrap_or(false);
    if nobody_is_there && wanted == almena_node::Which::Production {
        return Err("nobody_is_there_is_for_development");
    }
    let mut held = running.held.write().await;
    if held.is_some() {
        return Err("already_on_a_network");
    }

    // This node's own key belongs to the directory and outlives every run. Making one afresh each
    // time would be a different node every time, and anything published about it stale without
    // anybody being told. The directory is taken before any of it is read or written.
    let (directory, holding, key) = ready_to_come_back(&app, wanted)?;
    // Opening a network and coming back to the one already here are different acts, and doing the
    // first where the second belonged is how a directory ends up on a second network.
    let opened = match almena_node::record::holding(&directory) {
        almena_node::record::Holding::Unreadable(_) => return Err("unreadable_record"),
        almena_node::record::Holding::ARecord { .. } => almena_node::Node::rejoin(&directory, key)
            .map_err(|why| match why {
                almena_node::record::NotReadable::NotWritable => "no_directory",
                almena_node::record::NotReadable::DoesNotAddUp => "record_does_not_add_up",
                almena_node::record::NotReadable::AnotherNetwork => "not_the_promised_network",
                almena_node::record::NotReadable::Unreadable
                | almena_node::record::NotReadable::Refused => "unreadable_record",
            })?,
        almena_node::record::Holding::Nothing => {
            // The check that makes opening safe, and it is only a check if somebody looks.
            let seeds = if nobody_is_there {
                log::info!("zone_not_asked reason=nobody_is_there");
                Vec::new()
            } else {
                who_is_there(zone.as_deref().unwrap_or(theirs), &running).await?
            };
            first_time(&directory, wanted, &seeds, key)?
        }
    };
    Ok(taking_up(
        &app,
        &running,
        &mut held,
        Up {
            node: opened,
            holding,
            which: wanted,
        },
    )
    .await)
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
    tokio::spawn(
        running
            .timekeeping
            .clone()
            .keeping_time(serving.clone(), running.clock(), LOOK),
    );
    *held = Some(serving);
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
    let dns = resolver()?;
    let looked = almena_lookup::look_patiently(&dns, zone)
        .await
        .ok_or("zone_silent")?;

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
                // Production only: the core holds the format to its freeze checklist before opening
                // one, and refuses rather than opening a network on a format that is still moving. The
                // screen above shows the same checklist beforehand so that this is never the first
                // anybody hears of it.
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

/// One line of the format's freeze checklist, as the interface draws it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Line {
    /// What is being asked.
    pub called: String,
    /// What went wrong, where something did. `None` is a line that holds.
    ///
    /// **The reason travels and is not reduced to a flag.** Whoever is about to open a production
    /// network needs to know what to go and fix, and *something is wrong* is not that.
    pub wanting: Option<String>,
}

/// Whether this build's format may be frozen, item by item.
///
/// # The question, without the act
///
/// **Nothing is opened, joined or written.** Every item is a probe against this build rather than a
/// line somebody ticked, and reading it is how whoever is about to open a production network finds
/// out what would happen before it happens.
///
/// It is the same list the core holds a production network to when one is opened, so this is a
/// preview of that answer and never a second opinion about it.
///
/// # Errors
///
/// None. It answers about this build and asks nothing of anybody.
#[tauri::command]
pub fn freeze_checklist() -> Vec<Line> {
    almena_frozen::checklist()
        .into_iter()
        .map(|item| Line {
            called: item.called,
            wanting: match item.answered {
                almena_frozen::Answered::Holds => None,
                almena_frozen::Answered::Wanting(why) => Some(why),
            },
        })
        .collect()
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
/// # Errors
///
/// The reason it could not, as a stable identifier the interface translates. **A directory that
/// holds nothing is not one of them**: it is `Ok(None)`, because having no network yet is a state
/// and not a failure.
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

    let Some(which) = which_to_come_back_to(&app) else {
        return Ok(None);
    };
    let (directory, holding, key) = ready_to_come_back(&app, which)?;
    let node = almena_node::Node::rejoin(&directory, key).map_err(|why| match why {
        almena_node::record::NotReadable::NotWritable => "no_directory",
        almena_node::record::NotReadable::DoesNotAddUp => "record_does_not_add_up",
        almena_node::record::NotReadable::AnotherNetwork => "not_the_promised_network",
        almena_node::record::NotReadable::Unreadable
        | almena_node::record::NotReadable::Refused => "unreadable_record",
    })?;

    // **Best effort, and silence is not fatal here.** The record is what this node is on; the zone
    // only says who else to dial once it takes its place on the mesh, and a node that refused to
    // come back because DNS was slow would be a node whose uptime depended on somebody else's.
    let zone = which_of(named(which)).map_or(DEVELOPMENT_ZONE, |(_, zone)| zone);
    match who_is_there(zone, &running).await {
        Ok(seeds) => log::info!("zone_read_on_rejoin seeds={}", seeds.len()),
        Err(why) => log::info!("zone_silent_on_rejoin zone={zone} reason={why}"),
    }
    Ok(Some(
        taking_up(
            &app,
            &running,
            &mut held,
            Up {
                node,
                holding,
                which,
            },
        )
        .await,
    ))
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
    let acts = pulled(&key, &network, asked.port, &address).await?;

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
        &running,
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
    pub port: u16,
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
    let dns = resolver()?;
    let looked = almena_lookup::look_patiently(&dns, zone)
        .await
        .ok_or("zone_silent")?;
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

/// Take a place on the mesh, listening on `port`.
///
/// The port is chosen and not discovered, because it is the one somebody publishes in the zone. A
/// node that took whatever was free would be a node whose published record is wrong the next time
/// it starts.
///
/// # Errors
///
/// The reason it could not, as a stable identifier the interface translates.
#[tauri::command]
pub async fn join_the_mesh(
    app: tauri::AppHandle,
    running: tauri::State<'_, Running>,
    asked: Place,
) -> Result<(), &'static str> {
    let held = running.held.read().await;
    let serving = held.as_ref().ok_or("no_network")?;
    let network = serving.node().read().await.network().as_str().to_owned();

    let which = running.which.lock().await.ok_or("no_network")?;
    let directory = directory_of(&app, which)?;
    let key = almena_node::identity::load_or_make(&directory).map_err(|_| "unreadable_identity")?;
    let listening = listening_on(&key, &network, &asked).await?;

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

    tokio::spawn(almena_mesh::keeping::keeping_up(
        listening,
        std::sync::Arc::clone(serving.node()),
        running.dialling.lock().await.clone(),
        running.clock(),
        ASK_EVERY,
    ));
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

/// Close whatever epochs this node owes, without waiting for its own clock to come round.
///
/// The clock does this on its own; asking is for the moment somebody does not want to wait for it.
/// Both go through one record of what has been closed, so asking twice is not two answers about
/// one epoch.
///
/// # Errors
///
/// The reason it could not, as a stable identifier the interface translates.
#[tauri::command]
pub async fn close_epoch(running: tauri::State<'_, Running>) -> Result<usize, &'static str> {
    let held = running.held.read().await;
    let Some(serving) = held.as_ref() else {
        return Err("no_network");
    };
    let closed = running.timekeeping.catch_up(serving, running.now()).await;
    Ok(closed)
}

/// Show a challenge for whoever contributed this node to approve.
///
/// **Whoever sustains the network earns the right to write on it, and that has to attach to
/// somebody.** A node nobody claimed is a machine, and a machine cannot be credited — so the node
/// asks, and approving it is somebody else's to do with the key their own chain authorises.
///
/// Good for `for_epochs` and then not: one that ended up in a screenshot or a support bundle must
/// not bind somebody's machine a year later. Nothing but this node remembers it was shown.
///
/// # Errors
///
/// The reason there is none to show, as a stable identifier.
#[tauri::command]
pub async fn who_contributed_me(
    running: tauri::State<'_, Running>,
    for_epochs: u64,
) -> Result<String, &'static str> {
    let held = running.held.read().await;
    let serving = held.as_ref().ok_or("no_network")?;
    let until = running
        .now()
        .plus(almena_node::Epochs(for_epochs))
        .ok_or("no_network")?;
    let challenge = serving
        .node()
        .read()
        .await
        .asking_who_contributed_me(until)
        .map_err(|_| "no_randomness")?;
    Ok(challenge.to_text())
}

/// Write down that somebody contributed this node, from what they handed back.
///
/// Both halves go in: the challenge this node showed, and their approval of it. One alone binds
/// nothing — the node saying it is the node's word about somebody, and an approval alone is
/// somebody claiming a machine they may not hold.
///
/// # Errors
///
/// `not_a_claim` when the text is not a challenge and an approval, and `not_theirs` when it read
/// and does not bind. **A binding that cannot be checked is not a weaker binding**: it would be
/// this node's word about somebody who never agreed.
#[tauri::command]
pub async fn contributed_by(
    running: tauri::State<'_, Running>,
    challenge: String,
    approval: String,
) -> Result<(), &'static str> {
    let held = running.held.read().await;
    let serving = held.as_ref().ok_or("no_network")?;
    let now = running.now();
    match serving
        .node()
        .write()
        .await
        .contributed_by_text(&challenge, &approval, now)
    {
        almena_node::Claimed::Written => Ok(()),
        almena_node::Claimed::NotAClaim => Err("not_a_claim"),
        almena_node::Claimed::NotTheirs => Err("not_theirs"),
    }
}

/// Say this node is no longer contributed by anybody.
///
/// **The node alone.** Whoever claimed it agreed to be credited for what it served, and giving that
/// up costs them nothing anybody could hold them to. Credit stops from here and never in arrears:
/// what was served was served.
///
/// # Errors
///
/// The reason it could not be written down, as a stable identifier.
#[tauri::command]
pub async fn contributed_by_nobody(running: tauri::State<'_, Running>) -> Result<(), &'static str> {
    let held = running.held.read().await;
    let serving = held.as_ref().ok_or("no_network")?;
    let now = running.now();
    serving
        .node()
        .write()
        .await
        .contributed_by_nobody(now)
        .then_some(())
        .ok_or("not_written_down")
}

/// Serve the interface on `address`, so clients and portals can ask.
///
/// Reading is not authenticated and writing is handing over a signed act, so there is nothing to
/// configure about who may ask — only where to listen.
///
/// **Serving in the clear is not a mode.** Every node has a key, so every node has a certificate:
/// one whose subject public key is the node's own, signed by that key, which whoever dials it pins
/// against the identity the zone or the record told them. An operator who already has a
/// certificate for the machine names two files instead.
///
/// # Errors
///
/// The reason it could not, as a stable identifier.
#[tauri::command]
pub async fn serve_interface(
    app: tauri::AppHandle,
    running: tauri::State<'_, Running>,
    address: String,
    certificate: Option<String>,
    private_key: Option<String>,
) -> Result<(), &'static str> {
    let held = running.held.read().await;
    let Some(serving) = held.as_ref().cloned() else {
        return Err("no_network");
    };
    let (under, how) = under_which(&app, &running, certificate, private_key).await?;

    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|_| "address_unavailable")?;

    log::info!("interface_serving address={address} under={how}");
    *running.serving_at.lock().await = Some(origin_of(&listener, &address));

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

    tokio::spawn(async move {
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
    });
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
    certificate: Option<String>,
    private_key: Option<String>,
) -> Result<(almena_tls::Accepting, &'static str), &'static str> {
    match (certificate, private_key) {
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

/// Close this node, so that it stops counting.
///
/// **The one way out of a node whose key is somebody else's**, and not how a node is taken down
/// for the afternoon: a closed node does not come back, and coming back means announcing a new one
/// with a new key and a new name. What it said stays said — its roots and summaries are in the
/// record for ever — and what changes is the census the share-out is drawn from.
///
/// # Errors
///
/// `no_network` where there is none, and `not_written_down` where the record would not take it.
#[tauri::command]
pub async fn close_this_node(running: tauri::State<'_, Running>) -> Result<(), &'static str> {
    let held = running.held.read().await;
    let serving = held.as_ref().ok_or("no_network")?;
    let now = running.now();
    serving
        .node()
        .write()
        .await
        .close_itself(now)
        .then_some(())
        .ok_or("not_written_down")
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
        (
            "open_a_network",
            &[
                Capability::OpenNetwork,
                Capability::WhereToLook,
                Capability::NobodyIsThere,
            ],
        ),
        ("freeze_checklist", &[Capability::FreezeChecklist]),
        (
            "join_a_network",
            &[Capability::JoinNetwork, Capability::WhereToLook],
        ),
        ("come_back", &[Capability::ComeBack]),
        (
            "serve_interface",
            &[Capability::Serve, Capability::Certificate],
        ),
        ("close_epoch", &[Capability::CloseEpoch]),
        (
            "join_the_mesh",
            &[Capability::JoinTheMesh, Capability::Mediator],
        ),
        ("who_contributed_me", &[Capability::SayWhoContributedIt]),
        ("contributed_by", &[Capability::SayWhoContributedIt]),
        ("contributed_by_nobody", &[Capability::SayWhoContributedIt]),
        ("close_this_node", &[Capability::CloseThisNode]),
    ]
};

#[cfg(test)]
mod tests {
    use super::{COMMANDS, Facts, Running};

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
