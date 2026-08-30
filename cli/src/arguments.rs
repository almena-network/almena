//! What this program accepts on a command line, which is two things and no more.
//!
//! Neither of them names a peer, a network or an address: this build joins no network, so such a
//! flag would be accepted, used for nothing and refused by nothing. It arrives with the code that
//! can honour it.

use clap::Parser;

/// The command line, parsed.
///
/// **Nothing here names a peer by address.** A node is reached through the zone it reads or through
/// a `_seed` record given by hand, and both carry the identity of who answers — which is the whole
/// of what makes a redirected address a failed connection rather than a wrong node. An address on
/// its own says where to call and not who picks up, so there is nothing to give one to.
#[derive(Debug, Parser)]
#[command(
    name = "almena",
    version,
    about = "Almena, the node for a computer with no graphical system",
    long_about = None
)]
pub struct Arguments {
    /// Write records instead of drawing. Implied when there is no terminal.
    #[arg(long)]
    pub quiet: bool,

    /// Open a development network, on the word that there is nobody to join.
    ///
    /// **A node opens a network only when nobody is there**, and normally it finds that out by
    /// reading the zone. Nothing reads a zone yet, so this flag is somebody saying it — which is
    /// why it says *development* in its own name and cannot open anything else. Development can be
    /// opened again as often as it needs to be; a production network is opened once, ever, and
    /// nobody is going to do that on the strength of a promise typed at a terminal.
    #[arg(long)]
    pub open_development: bool,

    /// Open the production network, on the word that there is nobody to join.
    ///
    /// **Once, ever.** A record is append-only, so what this settles is settled for as long as that
    /// network exists — and the node holds the format to its own freeze checklist before it will do
    /// it, refusing rather than opening a network on a format that is still moving. That is the
    /// difference between the two networks and the reason there are two: development is re-opened
    /// whenever the format changes, and production is not re-opened at all.
    ///
    /// Read `--freeze-checklist` first. It answers the same question without opening anything.
    #[arg(long, conflicts_with = "open_development")]
    pub open_production: bool,

    /// Say whether the format this build writes is one a network may be opened on for good.
    ///
    /// **The question, without the act.** Every item is a probe against this build rather than a
    /// line somebody ticked, and reading it is how whoever is about to open a production network
    /// finds out what would happen before it happens. Nothing is opened, joined or written.
    #[arg(long)]
    pub freeze_checklist: bool,

    /// Serve the interface on this address, so clients and portals can ask.
    ///
    /// Reading is not authenticated and writing is handing over a signed act, so there is nothing
    /// to configure about who may ask — only where to listen.
    #[arg(long, value_name = "ADDRESS")]
    pub serve: Option<String>,

    /// Take a place on the mesh, listening on this port.
    ///
    /// **The port is chosen and not discovered**, because it is the one somebody publishes in the
    /// zone. A node that picked whatever was free would be a node whose published record is wrong
    /// the next time it starts.
    #[arg(long, value_name = "PORT")]
    pub mesh: Option<u16>,

    /// Carry other nodes' traffic, so machines that cannot be dialled can still be reached.
    ///
    /// **Volunteered, and it costs this machine's bandwidth.** Behind a household router there is
    /// no address anybody outside can knock on, and without somebody carrying them such machines
    /// could hold the record and answer nothing — so a network without permission needs nodes that
    /// do this, and it asks rather than assumes. Turning it on says so in the record, where it is
    /// counted like anything else a node offers.
    #[arg(long)]
    pub carry: bool,

    /// Ask this node to carry ours, and publish where that makes us reachable.
    ///
    /// For a node that cannot be dialled. The address has to say **which** node the relay is —
    /// `/p2p/<id>` at the end — because a circuit runs through somebody, and being carried by
    /// whoever happens to answer at a host and port is being carried by whoever took them.
    ///
    /// Asking is not being carried: whether a slot is granted is theirs to decide, and the address
    /// is published if and when one is.
    #[arg(long = "carried-by", value_name = "ADDRESS")]
    pub carried_by: Vec<String>,

    /// Show a challenge for whoever contributed this node to approve, good for this many epochs.
    ///
    /// **Whoever sustains the network earns the right to write on it, and that has to attach to
    /// somebody** — a node nobody claimed is a machine, and a machine cannot be credited. This is
    /// the node asking. Approving it is somebody else's to do, with the key their own chain
    /// authorises, and it happens where that key is.
    ///
    /// Short on purpose: one that ended up in a screenshot, a support bundle or this node's own log
    /// must not bind somebody's machine a year later.
    #[arg(long, value_name = "EPOCHS")]
    pub who_contributed_me: Option<u64>,

    /// Write down that somebody contributed this node: the challenge shown, then their approval.
    ///
    /// Both halves, because one of them alone binds nothing — the node saying it is the node's word
    /// about somebody, and their approval alone is somebody claiming a machine they may not hold.
    #[arg(long, value_names = ["CHALLENGE", "APPROVAL"], num_args = 2)]
    pub contributed_by: Option<Vec<String>>,

    /// Say this node is no longer contributed by anybody.
    ///
    /// **The node alone.** Whoever claimed it agreed to be credited for what it served, and giving
    /// that up costs them nothing anybody could hold them to. Credit stops from here and never in
    /// arrears: what was served was served.
    #[arg(long)]
    pub contributed_by_nobody: bool,

    /// Close this node: say it stops counting, from now and for good.
    ///
    /// **The one way out of a node whose key is somebody else's** (`SPECS.md §4.1`). A node does not
    /// rotate — the only thing that governs it is the key that was lost — so it closes, and whoever
    /// operated it announces another. What it said stays said: its roots and its summaries are in
    /// the record for ever, and closing takes none of it back.
    ///
    /// **It does not come back.** This is not how a node is taken down for the afternoon; that is
    /// stopping the program. Coming back after this means a new node, with a new key and a new
    /// name, because one that returned would bring whoever took its key with it.
    #[arg(long)]
    pub close_this_node: bool,
    /// Be the node in this directory instead of the usual one.
    ///
    /// **A node is a directory with a key in it**, so this is how a machine runs more than one —
    /// which is the ordinary thing to want while building a network and testing what several nodes
    /// do. Two directories are two nodes with two keys and two names, and they are as separate as
    /// nodes on two machines.
    #[arg(long, value_name = "PATH")]
    pub directory: Option<std::path::PathBuf>,
    /// Join these instead of asking the zone. One `_seed` record each, written as the zone writes
    /// them.
    ///
    /// **For when there is no zone to ask** — a network being tried out on one machine, or a
    /// resolver that will not answer. It only ever says *somebody is there*, which is the safe
    /// direction: a node given a seed joins rather than opens, and no flag can make it open a
    /// network it has not established is missing.
    #[arg(long = "seed", value_name = "RECORD")]
    pub seeds: Vec<String>,
    /// Ask these servers for DNS instead of whatever this machine uses.
    ///
    /// **For a machine whose own resolver is not usable**, which is a real state and not a rare
    /// one: a resolver behind a VPN, one configured with servers it cannot reach, or one that
    /// answers every other tool on the machine in milliseconds and this one not at all. A node
    /// that cannot look up a zone cannot open a network, because reading silence as an empty zone
    /// is how a second network gets started — so being able to name a resolver is the difference
    /// between a machine that can take part and one that cannot.
    ///
    /// Addresses, not names: a resolver named by a name would need a resolver.
    #[arg(long = "resolver", value_name = "ADDRESS")]
    pub resolvers: Vec<std::net::IpAddr>,

    /// Look for somebody to join in this zone instead of the usual one.
    ///
    /// For an operator running a network of their own. What it is for is the check that makes
    /// opening safe: **a node opens a network only when the zone says nobody is there**, and
    /// pointing this at a zone that is not the network's would be answering that question about
    /// somebody else.
    #[arg(long, value_name = "ZONE")]
    pub zone: Option<String>,
    /// Serve under this certificate, in PEM. Needs `--private-key` beside it.
    ///
    /// **Without it the interface answers in the clear**, which is right for a node being tried out
    /// on the machine it runs on and wrong for one anybody else reaches. What a node answers is
    /// signed by whoever wrote it, so nothing in the middle can forge an act — but it can read
    /// every question, and on this platform the questions are a list of who is looking up whom.
    ///
    /// Nothing here obtains a certificate. An operator who has one should not have to explain that
    /// to a program, and one who has not is better served by whatever already issues certificates
    /// for the rest of that machine.
    #[arg(long, value_name = "PATH", requires = "private_key")]
    pub certificate: Option<std::path::PathBuf>,
    /// The private key for `--certificate`, in PEM.
    #[arg(long, value_name = "PATH", requires = "certificate")]
    pub private_key: Option<std::path::PathBuf>,
    /// Speak this language from now on — `en`, `es`. Remembered.
    ///
    /// What a person chooses overrules what the system says, and is remembered. Without this
    /// the environment was the only voice, which made this face of the node able to be told
    /// less than the other one — and neither face is the cut-down version of the other.
    ///
    /// Any tag is accepted rather than a list being checked here: what this build ships is the
    /// catalog directory's answer and not the parser's, and a tag with no catalog falls back to
    /// English the same way an unrecognised `LANG` always has.
    #[arg(long, value_name = "TAG")]
    pub language: Option<String>,
}

impl Arguments {
    /// Whether this run should write records rather than draw a view.
    ///
    /// True when it was asked for, and true when there is no terminal to draw on — a unit file
    /// that forgot the flag gets the behaviour it meant rather than a program fighting for a
    /// terminal that is not there.
    ///
    /// The flag exists on top of that detection because somebody at a real terminal is
    /// entitled to say they would rather read records, and because a behaviour reachable only
    /// by taking something away is one nobody can ask for.
    #[must_use]
    pub fn writes_records(&self) -> bool {
        self.quiet || !std::io::IsTerminal::is_terminal(&std::io::stdout())
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    use super::Arguments;

    #[test]
    fn a_certificate_without_its_key_is_refused() {
        // The two are one decision. A node that took a certificate and served in the clear because
        // no key came with it would be telling somebody it was private when it was not.
        assert!(
            Arguments::try_parse_from(["almena", "--certificate", "/tmp/cert.pem"]).is_err(),
            "and the other way round too"
        );
        assert!(Arguments::try_parse_from(["almena", "--private-key", "/tmp/cert.key"]).is_err());
    }

    #[test]
    fn a_certificate_with_its_key_is_taken() {
        let arguments = Arguments::parse_from([
            "almena",
            "--certificate",
            "/tmp/cert.pem",
            "--private-key",
            "/tmp/cert.key",
        ]);
        assert!(arguments.certificate.is_some() && arguments.private_key.is_some());
    }

    #[test]
    fn quiet_is_off_unless_it_is_asked_for() {
        let arguments = Arguments::parse_from(["almena"]);
        assert!(!arguments.quiet);
    }

    #[test]
    fn quiet_is_read_from_the_command_line() {
        let arguments = Arguments::parse_from(["almena", "--quiet"]);
        assert!(arguments.quiet);
    }

    #[test]
    fn asking_for_quiet_is_enough_on_its_own() {
        // The other half of `writes_records` depends on whether the test runner gave us a
        // terminal, which is not ours to decide. This is the half that is.
        let arguments = Arguments::parse_from(["almena", "--quiet"]);
        assert!(arguments.writes_records());
    }

    #[test]
    fn no_language_is_asked_for_unless_it_is_asked_for() {
        assert_eq!(Arguments::parse_from(["almena"]).language, None);
    }

    #[test]
    fn a_language_is_read_from_the_command_line() {
        let arguments = Arguments::parse_from(["almena", "--language", "es"]);
        assert_eq!(arguments.language.as_deref(), Some("es"));
    }

    #[test]
    fn a_tag_with_no_catalog_is_still_accepted_here() {
        // Refusing it at the parser would put the list of languages back into code, which
        // adding a language must never require. What happens to it is `language.rs`'s decision.
        let arguments = Arguments::parse_from(["almena", "--language", "fr"]);
        assert_eq!(arguments.language.as_deref(), Some("fr"));
    }

    #[test]
    fn opening_a_network_is_off_unless_it_is_asked_for() {
        // It opens a network, which on production is a thing that happens once ever. Nothing that
        // consequential is the default.
        assert!(!Arguments::parse_from(["almena"]).open_development);
        assert!(Arguments::parse_from(["almena", "--open-development"]).open_development);
        assert!(!Arguments::parse_from(["almena"]).open_production);
        assert!(Arguments::parse_from(["almena", "--open-production"]).open_production);
    }

    #[test]
    fn a_run_cannot_ask_for_both_networks() {
        // Two networks over one directory would be a second history for one identity, and the two
        // flags mean opposite things about how often a network may be opened at all.
        assert!(
            Arguments::try_parse_from(["almena", "--open-development", "--open-production"])
                .is_err()
        );
    }

    #[test]
    fn the_checklist_can_be_read_without_opening_anything() {
        // The whole point of it being a flag of its own: whoever is about to open a production
        // network finds out what would happen before it happens.
        let asked = Arguments::parse_from(["almena", "--freeze-checklist"]);
        assert!(asked.freeze_checklist);
        assert!(!asked.open_production);
        assert!(!asked.open_development);
    }

    #[test]
    fn each_network_is_opened_by_the_flag_that_names_it() {
        // **Named rather than switched**, because the two are not the same act done twice.
        // Development is opened again whenever the format moves; production is opened once and is
        // held to the freeze checklist before it is. A single flag with an argument would make them
        // look like one thing with a setting.
        assert!(Arguments::parse_from(["almena", "--open-development"]).open_development);
        assert!(Arguments::parse_from(["almena", "--open-production"]).open_production);
        assert!(!Arguments::parse_from(["almena", "--open-production"]).open_development);
    }

    #[test]
    fn nothing_is_served_unless_an_address_is_given() {
        assert_eq!(Arguments::parse_from(["almena"]).serve, None);
        assert_eq!(
            Arguments::parse_from(["almena", "--serve", "127.0.0.1:8080"])
                .serve
                .as_deref(),
            Some("127.0.0.1:8080")
        );
    }

    #[test]
    fn the_surface_is_valid() {
        // `clap` builds the parser at run time, so a mistake in the attributes above is a
        // panic on first use rather than a compile error. This is what turns it back into a
        // test failure.
        use clap::CommandFactory as _;
        Arguments::command().debug_assert();
    }
}
