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
pub mod language;
pub mod node;
pub mod preferences;
pub mod records;
pub mod serve;
pub mod view;

use clap::Parser as _;
use log::{error, info};

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

/// The certificate this run serves under, if it was given one.
///
/// A node asked to serve under a certificate that will not load does not come up serving in the
/// clear instead: whoever asked for one would be told all was well while every question put to
/// their node travelled in the open.
fn certificate_of(
    arguments: &Arguments,
) -> Result<Option<almena_tls::Accepting>, almena_tls::NoCertificate> {
    match (&arguments.certificate, &arguments.private_key) {
        (Some(certificate), Some(key)) => almena_tls::accepting(certificate, key).map(Some),
        // Neither, or one without the other — which the parser refuses before this is reached.
        _ => Ok(None),
    }
}

/// Put the node on a network and on the mesh, as far as this run asked for.
///
/// # Errors
///
/// The code to leave with. Both of these are a refusal to come up rather than something to carry on
/// past: a node half on a network is one whose published records describe something that is not
/// there.
fn bring_up(arguments: &Arguments, node: &mut Node) -> Result<(), u8> {
    if arguments.open_development
        && let Err(why) = node.open_development(
            arguments
                .zone
                .as_deref()
                .unwrap_or(crate::node::DEVELOPMENT_ZONE),
            &arguments.seeds,
        )
    {
        error!("network_not_opened reason={why:?}");
        return Err(1);
    }

    // **The other zone by default, because the zone is which network is being asked about.**
    // Pointing a production opening at the development zone would be asking *is anybody there* of
    // the wrong network, and the answer would be no for the wrong reason.
    if arguments.open_production
        && let Err(why) = node.open_production(
            arguments
                .zone
                .as_deref()
                .unwrap_or(crate::node::PRODUCTION_ZONE),
            &arguments.seeds,
        )
    {
        error!("network_not_opened reason={why:?}");
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
        })
    {
        error!("mesh_not_joined reason={why:?}");
        return Err(1);
    }
    Ok(())
}

/// Show a challenge, record a claim, or let go of one, as far as this run asked for.
///
/// **Whoever sustains the network earns the right to write on it, and that has to attach to
/// somebody.** A node nobody claimed is a machine, and a machine cannot be credited — so a node and
/// whoever contributed it say so together, in the node's own chain, where anybody can read it.
///
/// The challenge is printed rather than logged. It is a thing shown to a person and gone: it never
/// reaches the record, and the one place it is worth having is in front of whoever is about to
/// approve it.
///
/// # Errors
///
/// The code to leave with. A claim that did not go in is a refusal rather than something to carry
/// on past — the node came up, and what somebody asked it to say about who contributed it is not
/// what it says.
fn saying_who_contributed_it(arguments: &Arguments, node: &mut Node) -> Result<(), u8> {
    if let Some(epochs) = arguments.who_contributed_me {
        match node.asking_who_contributed_me(epochs) {
            Ok(challenge) => println!("{challenge}"),
            Err(why) => {
                error!("challenge_not_shown reason={why:?}");
                return Err(1);
            }
        }
    }

    if let Some(both) = arguments.contributed_by.as_deref()
        && let [challenge, approval] = both
        && let Err(why) = node.contributed_by(challenge, approval)
    {
        error!("not_claimed reason={why:?}");
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
fn listen(
    arguments: &Arguments,
    node: &Node,
    under: Option<almena_tls::Accepting>,
) -> Result<Option<serve::Listening>, String> {
    let Some(address) = arguments.serve.as_deref() else {
        return Ok(None);
    };
    let Some(serving) = node.serving().cloned() else {
        return Err(address.to_owned());
    };
    serve::start(address, serving, node, under)
        .map(Some)
        .ok_or_else(|| address.to_owned())
}

/// The node this run is about, in the directory and against the resolvers it was told.
fn held(arguments: &Arguments, records: Option<std::path::PathBuf>) -> Node {
    Node::in_directory(
        records,
        arguments.directory.clone(),
        arguments.resolvers.clone(),
    )
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
    let arguments = Arguments::parse();

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

    if let Err(code) = saying_who_contributed_it(&arguments, &mut node) {
        return code;
    }

    let under = match certificate_of(&arguments) {
        Ok(under) => under,
        Err(why) => {
            error!("interface_not_served reason={why:?}");
            return 1;
        }
    };

    let listening = match listen(&arguments, &node, under) {
        Ok(listening) => listening,
        Err(address) => {
            error!("interface_not_served address={address} reason=no_network");
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
    use super::settle;
    use crate::language::Language;

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
