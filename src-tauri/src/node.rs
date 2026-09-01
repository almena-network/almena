//! What this face draws when it draws a node.
//!
//! **It draws one; it is not one.** Everything a node reports about itself comes from the core, so
//! that the window and the terminal cannot start answering the same question differently. Nothing
//! here works a fact out.
//!
//! A node started here holds no network. Opening one means first knowing there is nobody to join,
//! and reading the zone is not built — so this reports having looked at nothing, which is a
//! different thing from reporting that there is nothing.
//!
//! # Why the shape is repeated instead of shared
//!
//! What crosses to the webview has to be serialisable, and the core does not serialise: it is
//! replicated into the holder's application, where every dependency is paid for twice. So the
//! shape is written out here and filled **only** from the core's own answer — no field is computed
//! on this side, which is what keeps the repetition from becoming a second opinion.

use serde::Serialize;

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
}

impl From<almena_node::Facts> for Facts {
    /// Straight across, field for field. Anything else would be this side deciding something.
    fn from(facts: almena_node::Facts) -> Self {
        Self {
            network: facts.network,
            identity: facts.identity,
            written: facts.written,
            root: facts.root,
            peer: facts.peer,
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

/// What epoch it is, counted from the instant this network began.
///
/// Built once when the network opens and carried by whatever needs the time, so that the one
/// wall-clock reading this platform ever writes down is not read again by anybody else.
fn clock(began: u64) -> impl Fn() -> almena_node::Epoch + Clone + Send + Sync + 'static {
    move || {
        let since = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(began, |over| over.as_secs());
        almena_node::Epoch::new(since.saturating_sub(began) / 3_600)
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
}

impl Running {
    /// What epoch it is, by this node's own clock.
    ///
    /// An epoch is whole hours since **this network's** beginning, so before one is opened there
    /// is no such instant and nothing to count from.
    fn now(&self) -> almena_node::Epoch {
        let began = self.began.load(std::sync::atomic::Ordering::Relaxed);
        let since = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        almena_node::Epoch::new(since.saturating_sub(began) / 3_600)
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
    let facts = serving.node().read().await.facts();
    Ok(facts.into())
}

/// Open a development network, on the operator's word that there is nobody to join.
///
/// **A node opens a network only when nobody is there.** Normally it learns that by reading the
/// zone; nothing reads one yet, so this takes somebody's word for it — which is why only
/// development can be opened this way. Development is opened again as often as it needs to be;
/// production is opened once, ever, and not on the strength of a button.
///
/// # Errors
///
/// The reason it could not, as a stable identifier the interface translates — never a sentence.
#[tauri::command]
pub async fn open_development_network(
    app: tauri::AppHandle,
    running: tauri::State<'_, Running>,
    zone: Option<String>,
) -> Result<Facts, &'static str> {
    let mut held = running.held.write().await;
    if held.is_some() {
        return Err("already_on_a_network");
    }

    // Almena Government's key belongs to the network and is made with it — opening a development
    // network again makes a new one, which is what opening a new network means.
    let government = almena_node::fresh_key().map_err(|_| "no_randomness")?;

    // This node's own key belongs to the directory and outlives every run. Making one afresh each
    // time would be a different node every time, and anything published about it stale without
    // anybody being told.
    let directory = tauri::Manager::path(&app)
        .app_data_dir()
        .map_err(|_| "no_directory")?;
    // Taken before anything in the directory is read or written, including the key: two processes
    // racing to make one would each think they had made it.
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
            let seeds = who_is_there(zone.as_deref().unwrap_or(DEVELOPMENT_ZONE), &running).await?;
            first_time(&directory, &seeds, &government, key)?
        }
    };

    let began = opened.began();
    let facts = opened.facts();
    running
        .began
        .store(began, std::sync::atomic::Ordering::Relaxed);

    *running.held_directory.lock().await = Some(holding);
    let serving = almena_serve::Serving::new(opened, limits());
    tokio::spawn(
        running
            .timekeeping
            .clone()
            .keeping_time(serving.clone(), clock(began), LOOK),
    );

    *held = Some(serving);
    Ok(facts.into())
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
    let dns = almena_lookup::Dns::of_this_machine().map_err(|_| "zone_silent")?;
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

/// Open a network in a directory that is holding none.
fn first_time(
    directory: &std::path::Path,
    seeds: &[String],
    government: &almena_node::SigningKey,
    key: almena_node::SigningKey,
) -> Result<almena_node::Node, &'static str> {
    // The one wall clock reading this platform ever writes down. Everything afterwards counts whole
    // hours from it, so it is read once, here, and never again.
    let began = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "no_clock")?
        .as_secs();

    let opening = almena_node::Opening {
        which: almena_node::Which::Development,
        beginning: almena_node::Epoch::GENESIS,
        began,
    };
    almena_node::Node::open_in(directory, &opening, seeds, government, key).map_err(|why| match why
    {
        almena_node::NotOpened::ThisNodeAlreadyHasOne => "already_on_a_network",
        almena_node::NotOpened::ThereIsAlreadyANetwork(_) => "there_is_a_network",
        almena_node::NotOpened::TheRecordWouldNotStart => "record_would_not_start",
        // What is opened from here is development, which is re-opened whenever the format moves and
        // so is never asked whether the format may be frozen. Named rather than folded in, so that
        // the day it does arise nobody is sent looking in the wrong place.
        almena_node::NotOpened::TheFormatIsNotFrozen(_) => "format_is_not_frozen",
    })
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
    which: String,
    port: u16,
) -> Result<Facts, &'static str> {
    let mut held = running.held.write().await;
    if held.is_some() {
        return Err("already_on_a_network");
    }

    // **The window says which network, never which zone.** Where each one is published is the
    // network's own fact, and a name typed into an interface is a name that can be typed wrong —
    // which would mean joining whatever answered at it.
    let zone = match which.as_str() {
        "production" => PRODUCTION_ZONE,
        "development" => DEVELOPMENT_ZONE,
        _ => return Err("no_such_network"),
    };

    let (seed, dialling) = where_to_join(zone).await?;
    let network = seed.network().to_owned();
    let address = almena_mesh::dialling(&seed).map_err(|_| "no_transport")?;
    // Kept for afterwards, so that taking a place on the mesh does not ask the zone a second time
    // and get a different answer.
    *running.dialling.lock().await = dialling;

    let (directory, holding, key) = ready(&app)?;
    let acts = pulled(&key, &network, port, &address).await?;

    // The instant the network began, out of the act that opened it — which is the only place it is
    // written and the reason a newcomer counts epochs from where everybody else does.
    let began = almena_node::Node::began_in(&acts).ok_or("record_does_not_add_up")?;
    running
        .began
        .store(began, std::sync::atomic::Ordering::Relaxed);
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

    let facts = joined.facts();
    *running.held_directory.lock().await = Some(holding);
    let serving = almena_serve::Serving::new(joined, limits());
    tokio::spawn(
        running
            .timekeeping
            .clone()
            .keeping_time(serving.clone(), clock(began), LOOK),
    );
    *held = Some(serving);
    Ok(facts.into())
}

/// Somebody on that network to ask, and everywhere else worth dialling afterwards.
///
/// **Parsed, because what is needed from a seed is more than somewhere to dial.** The record names
/// the network, and that name is the anchor everything pulled is checked against — a node that took
/// whatever it was handed would be calling that the network it joined.
async fn where_to_join(
    zone: &str,
) -> Result<(almena_node::zone::Seed, Vec<almena_mesh::Multiaddr>), &'static str> {
    let dns = almena_lookup::Dns::of_this_machine().map_err(|_| "zone_silent")?;
    let looked = almena_lookup::look_patiently(&dns, zone)
        .await
        .ok_or("zone_silent")?;
    let seed = looked
        .answer
        .seeds
        .first()
        .cloned()
        .ok_or("nobody_is_there")?;
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
fn ready(
    app: &tauri::AppHandle,
) -> Result<
    (
        std::path::PathBuf,
        almena_node::directory::Held,
        almena_node::SigningKey,
    ),
    &'static str,
> {
    let directory = tauri::Manager::path(app)
        .app_data_dir()
        .map_err(|_| "no_directory")?;
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
    port: u16,
    carry: bool,
    carried_by: Vec<String>,
) -> Result<(), &'static str> {
    let held = running.held.read().await;
    let serving = held.as_ref().ok_or("no_network")?;
    let network = serving.node().read().await.network().as_str().to_owned();

    let directory = tauri::Manager::path(&app)
        .app_data_dir()
        .map_err(|_| "no_directory")?;
    let key = almena_node::identity::load_or_make(&directory).map_err(|_| "unreadable_identity")?;

    let carrying = if carry {
        almena_mesh::Carrying::ForOthers
    } else {
        almena_mesh::Carrying::ForNobody
    };
    let mut listening =
        almena_mesh::listening(&key, &network, port, carrying).map_err(|why| match why {
            almena_mesh::NotListening::NoIdentity
            | almena_mesh::NotListening::NoTransport
            | almena_mesh::NotListening::Anonymous => "no_transport",
            almena_mesh::NotListening::AddressUnavailable => "mesh_address_unavailable",
        })?;

    // A relay that will not carry us is one relay, not a reason to stop: which of them answers is
    // not this node's to decide, and the answer arrives later either way.
    for relay in &carried_by {
        match listening.ask_to_be_carried_at(relay) {
            Ok(address) => log::info!("mesh_asked_to_be_carried relay={address}"),
            Err(why) => log::error!("mesh_relay_not_asked relay={relay} reason={why:?}"),
        }
    }

    // Driven here only until the operating system has said where this node can be reached; that
    // is a fact the node reports, and afterwards the mesh belongs to whatever is keeping up.
    let _ = tokio::time::timeout(REACHABLE_WITHIN, async {
        while listening.port().is_none() {
            let _ = listening.next().await;
        }
    })
    .await;

    tokio::spawn(almena_mesh::keeping::keeping_up(
        listening,
        std::sync::Arc::clone(serving.node()),
        running.dialling.lock().await.clone(),
        clock(running.began.load(std::sync::atomic::Ordering::Relaxed)),
        ASK_EVERY,
    ));
    Ok(())
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
/// # Errors
///
/// The reason it could not, as a stable identifier.
#[tauri::command]
pub async fn serve_interface(
    running: tauri::State<'_, Running>,
    address: String,
    certificate: Option<String>,
    private_key: Option<String>,
) -> Result<(), &'static str> {
    let held = running.held.read().await;
    let Some(serving) = held.as_ref().cloned() else {
        return Err("no_network");
    };
    // A node asked to serve under a certificate that will not load does not come up serving in the
    // clear instead: whoever asked for one would be told all was well while every question put to
    // their node travelled in the open.
    let under = match (certificate, private_key) {
        (Some(certificate), Some(key)) => Some(
            almena_tls::accepting(
                std::path::Path::new(&certificate),
                std::path::Path::new(&key),
            )
            .map_err(|why| match why {
                almena_tls::NoCertificate::NoChain => "no_certificate",
                almena_tls::NoCertificate::NoKey => "no_private_key",
                almena_tls::NoCertificate::NotAPair => "certificate_and_key_are_not_a_pair",
            })?,
        ),
        (None, None) => None,
        // One without the other is a node that would answer in the clear having been asked not to.
        _ => return Err("no_private_key"),
    };

    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|_| "address_unavailable")?;

    *running.serving_at.lock().await = Some(origin_of(&listener, &address, under.is_some()));

    // The same clock the node has been keeping time by since its network opened. Building a second
    // one here would be this face deciding what epoch it is, which is a fact and not a face's.
    let telling = clock(running.began.load(std::sync::atomic::Ordering::Relaxed));

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
                // Two ways in and one node behind them: what is served is decided in one place, and
                // this only chooses what the bytes travelled inside.
                match under {
                    Some(accepting) => {
                        if let Ok(wrapped) = accepting.accept(io).await {
                            let _ = serving.connection(wrapped, telling).await;
                        }
                    }
                    None => {
                        let _ = serving.connection(io, telling).await;
                    }
                }
            });
        }
    });
    Ok(())
}

/// Where a bound listener is, written the way somebody would type it.
///
/// **What was actually bound and not what was asked for**: a port of nought is a real request, and
/// the answer to it is whatever the operating system granted. Only where the socket will not say
/// does the asked-for address stand in, which is the closest thing to true that is left.
fn origin_of(listener: &tokio::net::TcpListener, asked: &str, secure: bool) -> String {
    let bound = listener
        .local_addr()
        .map_or_else(|_| asked.to_owned(), |at| at.to_string());
    format!("{}://{bound}", if secure { "https" } else { "http" })
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

#[cfg(test)]
mod tests {
    use super::Facts;

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
    }

    #[test]
    fn what_crosses_to_the_webview_keeps_an_absent_fact_absent() {
        // `null` has to arrive as `null`. A missing key, or a zero, would be the webview drawing a
        // measurement that was never taken.
        let json = serde_json::to_string(&Facts::default()).expect("serialises");
        assert_eq!(
            json,
            r#"{"network":null,"identity":null,"written":null,"root":null,"peer":null}"#
        );
    }
}
