//! Almena, the node for a computer that has no graphical system to run one from.
//!
//! The windowed application is a node on a computer with a screen. This is the same thing for
//! a machine in a rack: it brings a node up and keeps it up, and it draws what that node is
//! for whoever is watching. It is **not** the windowed application without a window — it has
//! no settings, no appearance, no notifications, no tray and no login entry, because every one
//! of those is something a person operates and the answer for a server is its own operating
//! system.
//!
//! Everything but the launch lives here rather than in `main.rs`, so that a test can link
//! against it — the same arrangement `almena-app` uses and for the same reason.
//!
//! Anything beyond bringing a node up arrives because somebody operating a real node asked for
//! it, and never because the windowed application has one.

pub mod arguments;
pub mod catalog;
pub mod clock;
pub mod language;
pub mod node;
pub mod preferences;
pub mod records;
pub mod serve;
pub mod view;

use log::{error, info, warn};

use crate::arguments::Arguments;
use crate::catalog::Catalog;
use crate::language::Language;
use crate::node::Node;

/// What this program calls itself, and therefore the name of every directory it keeps.
///
/// **Not the windowed application's.** That one is `network.almena.desktop`, declared in
/// `tauri.conf.json`, and the two are deliberately different: they are two programs, so they
/// keep separate directories, so they hold separate keys, so a machine running both is two
/// nodes. That is the decision and not a side effect of one: the network has no opinion about
/// two participants sharing hardware, and either program can be removed without reaching into
/// the other's data.
pub const IDENTIFIER: &str = "network.almena.cli";

/// The name this program's log files carry.
///
/// The binary a person types, and not the package, which is `almena-cli`. Two programs never
/// share a file, and `almena-app` is the other one's.
pub const PROGRAM: &str = "almena";

/// The certificate this run serves under: the operator's pair, or the node's own key.
///
/// **Serving in the clear is not a mode.** Every node has a key, so every node has a certificate:
/// one whose subject public key is the node's own, signed by that key, which whoever dials it pins
/// against the identity the zone or the record told them. An operator who already has a
/// certificate for the machine names two files instead — and a node asked to serve under files
/// that will not load does not come up under its own key instead, because whoever named them
/// would be told all was well while what they meant to serve under was not what was served.
fn certificate_of(
    arguments: &Arguments,
    node: &Node,
) -> Result<(almena_tls::Accepting, serve::Under), String> {
    match (&arguments.certificate, &arguments.private_key) {
        (Some(certificate), Some(key)) => almena_tls::accepting(certificate, key)
            .map(|accepting| (accepting, serve::Under::ACertificate))
            .map_err(|why| format!("{why:?}")),
        // Neither, or one without the other — which the parser refuses before this is reached.
        _ => {
            let key = node.identity().map_err(|why| format!("{why:?}"))?;
            almena_tls::self_signed(&key.secret())
                .map(|accepting| (accepting, serve::Under::OwnKey))
                .map_err(|why| format!("{why:?}"))
        }
    }
}

/// The zone this run looks in: the one named, or the network's own.
fn zone_of(arguments: &Arguments) -> &str {
    arguments
        .zone
        .as_deref()
        .unwrap_or(match arguments.network {
            crate::arguments::Network::Dev => crate::node::DEVELOPMENT_ZONE,
            crate::arguments::Network::Pro => crate::node::PRODUCTION_ZONE,
        })
}

/// Put the node on a network and on the mesh, as far as this run asked for.
///
/// # Errors
///
/// The code to leave with. Both of these are a refusal to come up rather than something to carry on
/// past: a node half on a network is one whose published records describe something that is not
/// there.
fn bring_up(arguments: &Arguments, node: &mut Node) -> Result<(), u8> {
    // **One flow, and what the run said is the only thing that varies.** Every start looks the
    // zone up and honours the seeds it was given; a directory holding a record comes back to its
    // network whatever was said. What differs is a directory holding nothing: `--open` makes a
    // network and refuses when somebody is there, `--join` takes the one that is there and refuses
    // when nobody is, and neither joins if it can and otherwise says there is no network yet.
    let zone = zone_of(arguments);
    let asking = if arguments.open {
        node.open(zone, &arguments.seeds, arguments.nobody_is_there)
    } else if arguments.join {
        node.join(zone, &arguments.seeds)
    } else {
        match node.take_part(zone, &arguments.seeds) {
            // Nothing to come back to and nobody to join is not a failure worth stopping over: a
            // node with no network still draws, still says what it is, and still has a network to
            // be given — and the records say which of the two words would give it one.
            Err(crate::node::Opening::NoNetwork) => {
                warn!("no_network_yet zone={zone}");
                Ok(())
            }
            other => other,
        }
    };
    if let Err(why) = asking {
        error!("network_not_taken reason={why:?}");
        return Err(1);
    }

    if let Some(port) = arguments.mesh
        && let Err(why) = node.join_the_mesh(&crate::node::Joining {
            port,
            carrying: if arguments.carry {
                almena_mesh::Carrying::ForOthers
            } else {
                almena_mesh::Carrying::ForNobody
            },
            carried_by: &arguments.carried_by,
            mediator: arguments.mediator,
        })
    {
        error!("mesh_not_joined reason={why:?}");
        return Err(1);
    }
    Ok(())
}

/// Act as Almena Government, as far as this run asked for, and say what was written.
///
/// **A ceremony and not a service.** Each of these is one act against the record of the node that
/// opened the network, through that node's own admission — never a poke at the store — and the run
/// ends with it. The identifiers logged are what somebody publishes or pastes next: the
/// certification's, the reply's, and how much of the core was new.
///
/// # Errors
///
/// The code to leave with. A ceremony refused is a refusal and nothing to carry on past.
fn governing(arguments: &Arguments, node: &mut Node) -> Result<(), u8> {
    let reason = |arguments: &Arguments| {
        let mut said = std::collections::BTreeMap::new();
        if let Some(en) = &arguments.reason_en {
            said.insert("en".to_owned(), en.clone());
        }
        if let Some(es) = &arguments.reason_es {
            said.insert("es".to_owned(), es.clone());
        }
        said
    };

    if arguments.publish_core {
        match node.publish_core() {
            Ok(published) => info!(
                "core_published sources={} attributes={} purposes={} already={}",
                published.sources, published.attributes, published.purposes, published.already
            ),
            Err(why) => {
                error!("core_not_published reason={why:?}");
                return Err(1);
            }
        }
    }

    if let Some(subject) = &arguments.certify {
        let grade = arguments.grade.and_then(almena_node::Grade::of).ok_or(1)?;
        match node.certify(subject, grade, &reason(arguments)) {
            Ok(sealed) => info!("certified subject={subject} certification={sealed}"),
            Err(why) => {
                error!("not_certified subject={subject} reason={why:?}");
                return Err(1);
            }
        }
    }

    if let Some(to) = &arguments.reply {
        match node.reply(to, &reason(arguments)) {
            Ok(answered) => info!("replied to={to} reply={answered}"),
            Err(why) => {
                error!("not_replied to={to} reason={why:?}");
                return Err(1);
            }
        }
    }
    Ok(())
}

/// Show a challenge, record a claim, close this node, or let go of a claim — as far as this run
/// asked for.
///
/// **Whoever sustains the network earns the right to write on it, and that has to attach to
/// somebody.** A node nobody claimed is a machine, and a machine cannot be credited — so a node and
/// whoever contributed it say so together, in the node's own chain, where anybody can read it.
///
/// The challenge is drawn by the view, and printed where there is no view. It is a thing shown to
/// a person and gone: it never reaches the record, and the one place it is worth having is in
/// front of whoever is about to approve it — which, at a terminal that is about to open its
/// alternate screen, is the screen and not the scrollback underneath it.
///
/// # Errors
///
/// The code to leave with. A claim that did not go in is a refusal rather than something to carry
/// on past — the node came up, and what somebody asked it to say about who contributed it is not
/// what it says.
fn saying_who_contributed_it(arguments: &Arguments, node: &mut Node) -> Result<(), u8> {
    if let Some(epochs) = arguments.who_contributed_me {
        match node.asking_who_contributed_me(epochs) {
            Ok(challenge) => {
                if arguments.writes_records() {
                    println!("{challenge}");
                }
            }
            Err(why) => {
                error!("challenge_not_shown reason={why:?}");
                return Err(1);
            }
        }
    }

    if let Some(both) = arguments.contributed_by.as_ref()
        && let [challenge, approval] = both.as_slice()
        && let Err(why) = node.contributed_by(challenge, approval)
    {
        error!("claim_not_written reason={why:?}");
        return Err(1);
    }

    if arguments.close_this_node
        && let Err(why) = node.close_this_node()
    {
        error!("node_not_closed reason={why:?}");
        return Err(1);
    }

    if arguments.contributed_by_nobody
        && let Err(why) = node.contributed_by_nobody()
    {
        error!("not_let_go reason={why:?}");
        return Err(1);
    }
    Ok(())
}

/// Turn the interface on, if this run asked for it.
///
/// Serving runs beside whatever this face is doing, not instead of it. A node that had to stop
/// being drawn in order to answer questions would be one of the two faces able to do something the
/// other cannot, which is the arrangement this whole design refuses.
///
/// # Errors
///
/// The address that was asked for, when there is no network to serve on it. A node with a network
/// has somewhere for the work to run and one without has nothing to serve, so the two absences are
/// the same refusal.
fn listen(arguments: &Arguments, node: &mut Node) -> Result<Option<serve::Listening>, String> {
    let Some(address) = arguments.serve.as_deref() else {
        return Ok(None);
    };
    let Some(serving) = node.serving().cloned() else {
        return Err(format!("address={address} reason=no_network"));
    };
    let (under, how) =
        certificate_of(arguments, node).map_err(|why| format!("address={address} reason={why}"))?;
    let listening = serve::start(address, serving, node, under, how)
        .ok_or_else(|| format!("address={address} reason=no_runtime"))?;
    node.serving_at(address);
    Ok(Some(listening))
}

/// The node this run is about, in the directory and against the resolvers it was told, with its
/// clock moved by the file it was given if it was given one.
///
/// The parser has already refused the clock file beside production, so a file here is a
/// development node's and nothing is checked again.
fn held(arguments: &Arguments, records: Option<std::path::PathBuf>) -> Node {
    let node = Node::in_directory(
        records,
        arguments.directory.clone(),
        arguments.resolvers.clone(),
        arguments.network.which(),
    );
    match &arguments.clock_offset_file {
        Some(file) => node.reading_the_clock_offset_from(file.clone()),
        None => node,
    }
}

/// Print whether the format this build writes is one a network may be opened on for good.
///
/// **Printed rather than logged, and it opens nothing.** It is a thing shown to a person who is
/// about to do something once, and the one place it is worth having is in front of them before they
/// do it. Every line is a probe that has just run against this build — not a list somebody keeps up
/// to date — and a line that is wanting is one that cannot be corrected after a record exists.
fn freeze_checklist() -> u8 {
    let items = almena_frozen::checklist();
    let wanting = items.iter().filter(|item| item.wanting()).count();

    for item in &items {
        let held = match &item.answered {
            almena_frozen::Answered::Holds => "holds".to_owned(),
            almena_frozen::Answered::Wanting(why) => format!("wanting — {why}"),
        };
        match item.kept {
            // Written out rather than printed as a list, because a reader of this is deciding
            // whether to do something once and the mechanism is half of what they are deciding on.
            Some(kept) => {
                let by: Vec<String> = kept.iter().map(|one| format!("{one:?}")).collect();
                println!("{}: {held} — kept by {}", item.called, by.join(" and "));
            }
            None => println!("{}: {held}", item.called),
        }
    }

    if wanting == 0 {
        println!("\nThe format may be frozen: a production network may be opened on it.");
        return 0;
    }
    println!(
        "\n{wanting} of {} not met. A production network opened on this format would keep what is missing for as long as it exists.",
        items.len()
    );
    1
}

/// Brings a node up, draws it or writes about it, and takes it down again.
///
/// Returns the code the process should exit with: `0` unless the terminal itself failed.
#[must_use]
pub fn run() -> u8 {
    let arguments = Arguments::parsed();

    // **Before anything else, because it is instead of everything else.** Nothing is opened,
    // joined, served or written: the run answers one question and leaves.
    if arguments.freeze_checklist {
        return freeze_checklist();
    }

    let writes_records = arguments.writes_records();

    let directories = almena_paths::Paths::for_application(IDENTIFIER);
    let logs = directories.logs().ok();
    let destination = records::install(PROGRAM, logs.as_deref(), writes_records);

    let configuration = directories.configuration().ok();
    let language = settle(arguments.language.as_deref(), configuration.as_deref());

    let mut node = held(&arguments, destination);
    let mut code = 0;

    if let Err(code) = bring_up(&arguments, &mut node) {
        return code;
    }

    // **A ceremony is an act and then the run ends.** The node came up on its record so that the
    // act went through its own admission; staying up afterwards would be a node started by
    // accident, drawing itself for nobody.
    if arguments.is_a_ceremony() {
        let code = match governing(&arguments, &mut node) {
            Ok(()) => 0,
            Err(code) => code,
        };
        node.stop();
        return code;
    }

    if let Err(code) = saying_who_contributed_it(&arguments, &mut node) {
        return code;
    }

    let listening = match listen(&arguments, &mut node) {
        Ok(listening) => listening,
        Err(why) => {
            error!("interface_not_served {why}");
            return 1;
        }
    };

    if writes_records {
        // Nothing to draw and nobody to draw it for: the node is up, and it stays up until the
        // operating system says otherwise.
        info!("waiting_for_stop");
        wait_for_stop();
    } else {
        let catalog = Catalog::of(language);
        if let Err(failure) = view::run(&node, &catalog) {
            error!("view_failed reason={failure}");
            code = 1;
        }
    }

    if let Some(listening) = listening {
        listening.stop();
    }
    node.stop();
    code
}

/// The language this run speaks, remembering it if this run is where it was asked for.
///
/// **Asked for now, else chosen before, else what the environment says**, and the order is the
/// whole of it: a person's choice overrules the system, and it is remembered so they are only
/// asked once. The remembering happens here rather than after the view, so that a run which
/// ends badly still leaves the choice made.
///
/// A language with no catalog is not a refusal to start; `Language::settled` falls back.
fn settle(asked: Option<&str>, configuration: Option<&std::path::Path>) -> Language {
    if let Some(asked) = asked {
        preferences::remember(configuration, asked);
        return Language::from_tag(asked);
    }

    Language::settled(preferences::chosen(configuration).as_deref())
}

/// Blocks until the operating system asks this process to stop.
///
/// `Ctrl-C` at a terminal and the `SIGTERM` a service manager sends both arrive here, so a
/// node stopped either way says so in its records instead of vanishing mid-line.
fn wait_for_stop() {
    let (tell, wait) = std::sync::mpsc::channel();

    if ctrlc::set_handler(move || {
        // A failed send means the receiver is gone, which means the program is already on its
        // way out. There is nothing to report and nobody to report it to.
        let _ = tell.send(());
    })
    .is_err()
    {
        error!("stop_signal_not_installed");
        return;
    }

    // A failed receive means every sender was dropped, which cannot happen while the handler
    // holds one — but it is still a reason to stop waiting rather than to wait for ever.
    let _ = wait.recv();
    info!("stop_requested");
}

#[cfg(test)]
mod tests {
    use super::{bring_up, held, listen, settle};
    use crate::arguments::Arguments;
    use crate::language::Language;
    use crate::node::Node;

    /// A directory of this test's own, removed when it is done with it.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("almena-settle-{name}"));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// How many nodes the record counts as offering this, by this node's own reading of it.
    ///
    /// The figure the registry draws and never the announcement itself: a node that said it
    /// serves without being counted for it would have said it nowhere.
    fn counted_offering(node: &Node, capability: almena_node::Capability) -> Option<usize> {
        let serving = node.serving()?;
        let now = node.now()?;
        serving
            .node()
            .blocking_read()
            .running(now)
            .answer
            .offering
            .get(&capability)
            .copied()
    }

    #[test]
    fn a_node_started_with_serve_says_so_in_the_record() {
        // **Where the capacity figures are drawn from.** A node answering on an interface it never
        // announced is one the network cannot count, so serving says `Interface` on the node's
        // own chain the moment the socket is bound — and once, however many times it is bound.
        let scratch = Scratch::new("serves");
        let directory = scratch.0.to_str().expect("a path").to_owned();
        let arguments = Arguments::try_parsed_from([
            "almena",
            "--network",
            "dev",
            "--open",
            "--nobody-is-there",
            "--serve",
            "127.0.0.1:0",
            "--directory",
            &directory,
        ])
        .expect("a command line this face parses");
        let mut node = held(&arguments, None);
        bring_up(&arguments, &mut node).expect("opens on somebody's word");
        assert_eq!(
            counted_offering(&node, almena_node::Capability::Interface),
            Some(0),
            "nothing served yet, so nothing said"
        );

        let listening = listen(&arguments, &mut node)
            .expect("serves")
            .expect("was asked to");
        // Bound on the node's own work rather than here, so said a moment after being asked.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while counted_offering(&node, almena_node::Capability::Interface) != Some(1)
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert_eq!(
            counted_offering(&node, almena_node::Capability::Interface),
            Some(1),
            "the record counts this node as serving"
        );
        listening.stop();

        // Served again — a restart, a second address — is said no second time.
        let again = listen(&arguments, &mut node)
            .expect("serves")
            .expect("was asked to");
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(
            counted_offering(&node, almena_node::Capability::Interface),
            Some(1),
            "one node, one announcement"
        );
        again.stop();
        node.stop();
    }

    #[test]
    fn a_clock_offset_file_moves_the_node_s_clock_as_it_is_written() {
        // **The knob the development network is walked with.** The node's own reading of the
        // epoch — the one every act it admits is placed at — is the wall's plus what the file
        // says, and the file is read again on every look so the days pass while the node runs.
        let scratch = Scratch::new("clock");
        std::fs::create_dir_all(&scratch.0).expect("a directory");
        let clock = scratch.0.join("clock");
        std::fs::write(&clock, "5\n").expect("written");
        let directory = scratch.0.to_str().expect("a path").to_owned();
        let file = clock.to_str().expect("a path").to_owned();
        let arguments = Arguments::try_parsed_from([
            "almena",
            "--network",
            "dev",
            "--open",
            "--nobody-is-there",
            "--clock-offset-file",
            &file,
            "--directory",
            &directory,
        ])
        .expect("a command line this face parses");
        let mut node = held(&arguments, None);
        bring_up(&arguments, &mut node).expect("opens on somebody's word");

        // The network opened a moment ago, so the wall's epoch is nought and what is left is
        // the file's.
        assert_eq!(node.now(), Some(almena_node::Epoch::new(5)));
        std::fs::write(&clock, "72").expect("written");
        assert_eq!(
            node.now(),
            Some(almena_node::Epoch::new(72)),
            "read again on every look"
        );
        std::fs::remove_file(&clock).expect("taken away");
        assert_eq!(
            node.now(),
            Some(almena_node::Epoch::GENESIS),
            "an absent file is nought, and the clock is the wall's"
        );
        node.stop();
    }

    #[test]
    fn without_a_clock_offset_file_the_clock_is_the_wall_s() {
        let scratch = Scratch::new("wall");
        let directory = scratch.0.to_str().expect("a path").to_owned();
        let arguments = Arguments::try_parsed_from([
            "almena",
            "--network",
            "dev",
            "--open",
            "--nobody-is-there",
            "--directory",
            &directory,
        ])
        .expect("a command line this face parses");
        let mut node = held(&arguments, None);
        bring_up(&arguments, &mut node).expect("opens on somebody's word");
        assert_eq!(node.now(), Some(almena_node::Epoch::GENESIS));
        node.stop();
    }

    #[test]
    fn asking_for_a_language_settles_on_it_and_remembers_it() {
        let scratch = Scratch::new("asked");
        let directory = Some(scratch.0.as_path());

        assert_eq!(settle(Some("es"), directory).tag(), "es");
        // And the next run, which asks for nothing, gets the same answer.
        assert_eq!(settle(None, directory).tag(), "es");
    }

    #[test]
    fn a_later_choice_replaces_an_earlier_one() {
        let scratch = Scratch::new("replaced");
        let directory = Some(scratch.0.as_path());

        settle(Some("es"), directory);
        assert_eq!(settle(Some("en"), directory).tag(), "en");
        assert_eq!(settle(None, directory).tag(), "en");
    }

    #[test]
    fn asking_for_a_language_with_no_catalog_stops_nothing() {
        // It is remembered as asked — a build that ships French later will honour it — and this
        // run falls back rather than refusing to start.
        let scratch = Scratch::new("unshipped");
        let directory = Some(scratch.0.as_path());

        assert_eq!(settle(Some("fr"), directory), Language::source());
        assert_eq!(
            crate::preferences::chosen(directory).as_deref(),
            Some("fr"),
            "what was asked for is what is stored, not what this build could do with it"
        );
    }

    #[test]
    fn with_nothing_asked_and_nothing_stored_the_environment_decides() {
        let scratch = Scratch::new("environment");
        // Whatever this machine's environment says, it is a language there is a catalog for.
        let settled = settle(None, Some(scratch.0.as_path()));
        assert!(Language::available().any(|available| available == settled));
    }
}
