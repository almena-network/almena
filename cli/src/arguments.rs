//! What this program accepts on a command line.
//!
//! Every flag here is honoured by the code behind it, and the ones that are one decision are held
//! together by the parser rather than by whoever remembers: a certificate without its key, a join
//! that is also an open, *nobody is there* said of the production network — each is refused before
//! anything starts, because a node half configured is a node whose published records describe
//! something that is not there.

use clap::{Parser, ValueEnum};

/// Which network a run is about.
///
/// Two, and there will never be a third from here: `SPECS.md §4.5` names two zones, and a network
/// nobody publishes a zone for is one nobody can join.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Network {
    /// Development, which is opened again as often as it needs to be.
    Dev,
    /// The real one, opened once.
    Pro,
}

impl Network {
    /// The same choice, as the node's own word for it.
    #[must_use]
    pub const fn which(self) -> almena_node::Which {
        match self {
            Self::Dev => almena_node::Which::Development,
            Self::Pro => almena_node::Which::Production,
        }
    }
}

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

    /// Which network this node is for: `dev` or `pro`.
    ///
    /// **Chosen once and never mixed.** It decides which zone the node reads, which network it
    /// would open, and — the part that is not obvious — **where the node lives**: a node for one
    /// network keeps its key, its record and its roots in a directory of its own, so a node for the
    /// other cannot read any of them.
    ///
    /// Everything else already separated the two: the act that opened a network is inside the
    /// record, its hash is the network's name, and the mesh protocol carries that name so two
    /// networks have nothing to negotiate. **The key is what none of that covered** — thirty-two
    /// bytes with no network in them — and development is where directories get copied and machines
    /// get shared, so one key across both would mean a careless afternoon there costing a node in
    /// production.
    #[arg(long, value_enum, default_value_t = Network::Dev)]
    pub network: Network,

    /// Open that network, on the word that there is nobody to join.
    ///
    /// **The same act for both, and what differs is what is at stake.** A node opens a network only
    /// when nobody is there and it finds that out by reading that network's zone — the same
    /// question asked of a different zone. Development is opened again as often as it needs to be;
    /// **production is opened once, ever**, and the node holds the format to its own freeze
    /// checklist first, refusing rather than opening a network on a format that is still moving.
    ///
    /// Without it a node comes back to the network its directory already holds and opens nothing —
    /// which is what every start after the first wants, and what keeps a restart from ever becoming
    /// a second network.
    ///
    /// Read `--freeze-checklist` before opening production. It answers the same question with
    /// nothing at stake. **It refuses when somebody is there**: a zone that names a node is a
    /// network to join, and `--join` is the word for that.
    #[arg(long, conflicts_with = "join")]
    pub open: bool,

    /// Join that network, through whoever its zone names.
    ///
    /// **The other half of `--open`, and the one every node but the first wants.** It asks the
    /// zone — or takes `--seed` — for somebody already there, pulls the record from them, checks
    /// it is the network the zone promised, and announces itself on it. When nobody is there it
    /// refuses rather than opening: opening is a different act, said out loud with its own flag.
    ///
    /// Neither flag is needed on a start after the first: a directory holding a record comes back
    /// to its network. And a fresh directory given neither joins when the zone names somebody,
    /// which is what the window does, and otherwise comes up on no network and says so.
    #[arg(long, conflicts_with = "open")]
    pub join: bool,

    /// Open the development network without asking its zone, on your word that nobody is there.
    ///
    /// **Development only, and the parser refuses it beside `--network pro`.** The whole defence
    /// against a second production network is the zone being asked; this is the one place that
    /// defence is set aside, and it is set aside where a second network costs an afternoon rather
    /// than the platform. For a machine with no resolver at all, or a network being tried out with
    /// nothing published anywhere.
    #[arg(long, requires = "open")]
    pub nobody_is_there: bool,

    /// Add the epochs written in this file to the clock, re-reading it on every look.
    ///
    /// **Development only, and the parser refuses it beside `--network pro`.** A device the words
    /// add waits three days before it may sign, so a network opened this morning cannot be walked
    /// this morning on the wall's clock; the file holds one integer, the epochs to add, and a test
    /// moves it while the node runs. A file that is absent or holds no integer counts as nought,
    /// said once in the records. A node whose clock somebody can move signs roots for hours that
    /// have not happened, which is why production never reads one.
    #[arg(long = "clock-offset-file", value_name = "PATH")]
    pub clock_offset_file: Option<std::path::PathBuf>,

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

    /// Hold post for people whose device is not on: run a mailbox, and say so in the record.
    ///
    /// **Volunteered, like carrying, and said where it is counted.** A client chooses a mediator
    /// from what the zone and the record name, and the record is what says this node is one — so
    /// turning this on writes an act on the node's own chain, once, and the mailbox answers from
    /// then on. Nothing held is replicated, signed by this node, or kept past it.
    #[arg(long)]
    pub mediator: bool,

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
    ///
    /// `ip` or `ip:port`, and a bare address means the port DNS is spoken on. The port is for the
    /// one case where a resolver answers somewhere unusual, which on this platform is a zone
    /// emulated on the machine itself: `--resolver 127.0.0.1:5300`.
    #[arg(long = "resolver", value_name = "ADDRESS", value_parser = resolver)]
    pub resolvers: Vec<std::net::SocketAddr>,

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

    /// Publish the core Almena maintains, as Almena Government, and leave.
    ///
    /// **A government ceremony, run against the directory of the node that opened the network**,
    /// which is where that network's government key was kept. Sources first, then the attributes
    /// copied from them, then the closed list of purposes — each an act through this node's own
    /// admission, and each skipped where the record already holds it, so running it twice costs
    /// nothing. The node is not left running: a ceremony is an act, and the run ends with it.
    #[arg(long)]
    pub publish_core: bool,

    /// Certify this entity, as Almena Government, and leave.
    ///
    /// Needs `--grade` and a reason in both languages, because a decision with no published reason
    /// is arbitrariness and one half the readers cannot read is not published.
    #[arg(long, value_name = "DID", requires_all = ["grade", "reason_en", "reason_es"])]
    pub certify: Option<String>,

    /// Which grade `--certify` gives: 1 basic, 2 verified, 3 reinforced.
    #[arg(long, value_name = "N", requires = "certify", value_parser = clap::value_parser!(u64).range(1..=3))]
    pub grade: Option<u64>,

    /// Answer this asking to be certified, as Almena Government, with a refusal, and leave.
    ///
    /// The act named is the entity's own asking, on its own chain. The reply is published beside it
    /// for ever, which is what makes a refusal something anybody can read and judge.
    #[arg(long, value_name = "ACT", requires_all = ["reason_en", "reason_es"])]
    pub reply: Option<String>,

    /// Why, in English, for `--certify` or `--reply`.
    #[arg(long, value_name = "TEXT")]
    pub reason_en: Option<String>,

    /// Why, in Spanish, for `--certify` or `--reply`.
    #[arg(long, value_name = "TEXT")]
    pub reason_es: Option<String>,
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

/// A resolver's address as clap hands it over: `ip` or `ip:port`.
///
/// The reading is `almena_lookup`'s; this only turns its refusal into a sentence `clap` can print,
/// because the lookup crate has no business knowing there is a command line.
fn resolver(written: &str) -> Result<std::net::SocketAddr, String> {
    almena_lookup::server(written)
        .map_err(|_| "an IP address, with :port when DNS is not on 53 — never a name".to_owned())
}

impl Arguments {
    /// The command line, parsed, with the one rule `clap` cannot hold on its own applied.
    ///
    /// **`--nobody-is-there` and `--clock-offset-file` beside `--network pro` are refused here,
    /// and the process leaves as it would for any other conflict.** A conflict between a flag and
    /// a *value* is not one `clap` declares, and the rules matter too much to be left to the code
    /// that would otherwise honour them: opening production without asking its zone is the one
    /// accident this platform cannot undo, and a production node whose clock can be moved is one
    /// signing roots for hours that have not happened.
    #[must_use]
    pub fn parsed() -> Self {
        match Self::try_parsed_from(std::env::args_os()) {
            Ok(arguments) => arguments,
            Err(refusal) => refusal.exit(),
        }
    }

    /// The same, from any command line, for a test to hold the rule to.
    ///
    /// # Errors
    ///
    /// Whatever `clap` refused, or the development-only rule above as a `clap` error of the same
    /// shape, so that both print the same way.
    pub fn try_parsed_from<I, T>(arguments: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        use clap::CommandFactory as _;
        let parsed = Self::try_parse_from(arguments)?;
        if parsed.nobody_is_there && parsed.network == Network::Pro {
            return Err(Self::command().error(
                clap::error::ErrorKind::ArgumentConflict,
                "--nobody-is-there is for development: production is opened on the zone's word and never on yours",
            ));
        }
        if parsed.clock_offset_file.is_some() && parsed.network == Network::Pro {
            return Err(Self::command().error(
                clap::error::ErrorKind::ArgumentConflict,
                "--clock-offset-file is for development: production keeps the wall's clock and nobody moves it",
            ));
        }
        Ok(parsed)
    }

    /// Whether this run is a government ceremony rather than a node being brought up to stay.
    ///
    /// Publishing the core, certifying and answering are acts on the record and then the run
    /// ends: a node that stayed up drawing after a ceremony would be a node started by accident.
    #[must_use]
    pub fn is_a_ceremony(&self) -> bool {
        self.publish_core || self.certify.is_some() || self.reply.is_some()
    }

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

    use super::{Arguments, Network};

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
        // consequential is the default — and without it a run comes back to the network its
        // directory already holds, which is what every start after the first wants.
        assert!(!Arguments::parse_from(["almena"]).open);
        assert!(Arguments::parse_from(["almena", "--open"]).open);
    }

    #[test]
    fn development_is_the_network_a_run_is_about_unless_it_says_otherwise() {
        // **The default is the one that is opened again as often as it needs to be.** Production is
        // opened once, ever, so reaching it is something a run says out loud.
        assert_eq!(Arguments::parse_from(["almena"]).network, Network::Dev);
        assert_eq!(
            Arguments::parse_from(["almena", "--network", "pro"]).network,
            Network::Pro
        );
    }

    #[test]
    fn a_run_is_about_one_network_and_there_is_nowhere_to_say_two() {
        // Two networks over one directory would be a second history for one identity. It is not a
        // rule anybody enforces here: it falls out of the choice being one value rather than two
        // flags that could both be given.
        assert!(Arguments::try_parse_from(["almena", "--network", "both"]).is_err());
        assert!(Arguments::try_parse_from(["almena", "--network", "prod"]).is_err());
    }

    #[test]
    fn the_checklist_can_be_read_without_opening_anything() {
        // The whole point of it being a flag of its own: whoever is about to open a production
        // network finds out what would happen before it happens.
        let asked = Arguments::parse_from(["almena", "--freeze-checklist"]);
        assert!(asked.freeze_checklist);
        assert!(!asked.open);
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
    fn opening_and_joining_are_two_flags_and_never_both() {
        // One is *make a network* and the other is *take the one that is there*; a run that asked
        // for both would be asking for whichever the zone happened to answer.
        assert!(Arguments::try_parse_from(["almena", "--open", "--join"]).is_err());
        assert!(Arguments::parse_from(["almena", "--join"]).join);
    }

    #[test]
    fn nobody_is_there_is_refused_for_production_and_taken_for_development() {
        // **The one accident this platform cannot undo**, refused at the parser and not left to
        // the code that would honour it. Development is where a second network costs an afternoon.
        assert!(
            Arguments::try_parsed_from([
                "almena",
                "--network",
                "pro",
                "--open",
                "--nobody-is-there"
            ])
            .is_err()
        );
        let taken = Arguments::try_parsed_from([
            "almena",
            "--network",
            "dev",
            "--open",
            "--nobody-is-there",
        ])
        .expect("development may be opened on somebody's word");
        assert!(taken.nobody_is_there);
        assert!(
            Arguments::try_parsed_from(["almena", "--nobody-is-there"]).is_err(),
            "and it means nothing without --open"
        );
    }

    #[test]
    fn a_clock_offset_is_refused_for_production_and_taken_for_development() {
        // A production node whose clock somebody can move signs roots for hours that have not
        // happened — refused at the parser, like opening on somebody's word, and for development
        // it is how the three-day wait passes on the morning a network opens.
        assert!(
            Arguments::try_parsed_from([
                "almena",
                "--network",
                "pro",
                "--clock-offset-file",
                "/tmp/clock"
            ])
            .is_err()
        );
        let taken = Arguments::try_parsed_from([
            "almena",
            "--network",
            "dev",
            "--clock-offset-file",
            "/tmp/clock",
        ])
        .expect("development may move its clock");
        assert_eq!(
            taken.clock_offset_file.as_deref(),
            Some(std::path::Path::new("/tmp/clock"))
        );
        assert_eq!(
            Arguments::try_parsed_from(["almena"])
                .expect("parses")
                .clock_offset_file,
            None,
            "and nothing is read unless a file is named"
        );
    }

    #[test]
    fn a_resolver_is_an_address_with_or_without_a_port() {
        let bare = Arguments::parse_from(["almena", "--resolver", "127.0.0.1"]);
        assert_eq!(bare.resolvers[0].port(), almena_lookup::DNS_PORT);
        let with = Arguments::parse_from([
            "almena",
            "--resolver",
            "127.0.0.1:5300",
            "--resolver",
            "[::1]:5300",
        ]);
        assert_eq!(with.resolvers.len(), 2);
        assert_eq!(with.resolvers[0].port(), 5300);
        assert!(
            Arguments::try_parse_from(["almena", "--resolver", "dns.example"]).is_err(),
            "a resolver named by a name would need a resolver"
        );
    }

    #[test]
    fn a_ceremony_needs_its_reason_in_both_languages() {
        // A decision with no published reason is arbitrariness, and one half the readers cannot
        // read is not published — refused before a key is touched.
        assert!(
            Arguments::try_parse_from(["almena", "--certify", "did:almena:dev:z1", "--grade", "1"])
                .is_err()
        );
        assert!(
            Arguments::try_parse_from([
                "almena",
                "--certify",
                "did:almena:dev:z1",
                "--grade",
                "4",
                "--reason-en",
                "a",
                "--reason-es",
                "b"
            ])
            .is_err()
        );
        let sealed = Arguments::parse_from([
            "almena",
            "--certify",
            "did:almena:dev:z1",
            "--grade",
            "2",
            "--reason-en",
            "a",
            "--reason-es",
            "b",
        ]);
        assert!(sealed.is_a_ceremony());
        assert_eq!(sealed.grade, Some(2));
        assert!(Arguments::try_parse_from(["almena", "--reply", "zQm1"]).is_err());
        assert!(Arguments::parse_from(["almena", "--publish-core"]).is_a_ceremony());
        assert!(!Arguments::parse_from(["almena"]).is_a_ceremony());
    }

    #[test]
    fn a_mediator_is_asked_for_and_never_assumed() {
        assert!(!Arguments::parse_from(["almena"]).mediator);
        assert!(Arguments::parse_from(["almena", "--mediator"]).mediator);
    }

    /// Which flag offers which capability, for the check against the table both faces are held to.
    const BY_FLAG: &[(&str, &[almena_node::facade::Capability])] = {
        use almena_node::facade::Capability;
        &[
            ("quiet", &[Capability::Watch]),
            (
                "network",
                &[Capability::OpenNetwork, Capability::JoinNetwork],
            ),
            ("open", &[Capability::OpenNetwork]),
            ("join", &[Capability::JoinNetwork]),
            ("nobody_is_there", &[Capability::NobodyIsThere]),
            ("clock_offset_file", &[Capability::ClockOffset]),
            ("freeze_checklist", &[Capability::FreezeChecklist]),
            ("serve", &[Capability::Serve]),
            ("mesh", &[Capability::JoinTheMesh]),
            ("carry", &[Capability::JoinTheMesh]),
            ("mediator", &[Capability::Mediator]),
            ("carried_by", &[Capability::JoinTheMesh]),
            ("who_contributed_me", &[Capability::SayWhoContributedIt]),
            ("contributed_by", &[Capability::SayWhoContributedIt]),
            ("contributed_by_nobody", &[Capability::SayWhoContributedIt]),
            ("close_this_node", &[Capability::CloseThisNode]),
            ("directory", &[Capability::Directory]),
            ("seeds", &[Capability::WhereToLook]),
            ("resolvers", &[Capability::WhereToLook]),
            ("zone", &[Capability::WhereToLook]),
            ("certificate", &[Capability::Certificate]),
            ("private_key", &[Capability::Certificate]),
            ("publish_core", &[Capability::PublishCore]),
            ("certify", &[Capability::Certify]),
            ("grade", &[Capability::Certify]),
            ("reply", &[Capability::Reply]),
            ("reason_en", &[Capability::Certify, Capability::Reply]),
            ("reason_es", &[Capability::Certify, Capability::Reply]),
            ("language", &[Capability::Language]),
        ]
    };

    #[test]
    fn what_this_face_parses_is_what_the_table_says_it_offers() {
        // **The terminal's half of the parity check.** The table in the core says which
        // capabilities the terminal draws; `BY_FLAG` maps every flag `clap` parses onto them — plus
        // the three that are not flags: coming back is the absence of one, closing an epoch is a
        // key in the view, and watching is the view itself — and the two lists are held to each
        // other. A flag added without a row, or a row without a flag, fails here.
        use almena_node::facade::{Capability, offered_by_terminal};
        use clap::CommandFactory as _;

        let mut drawn: Vec<Capability> = BY_FLAG
            .iter()
            .flat_map(|(_, capabilities)| capabilities.iter().copied())
            .collect();
        drawn.extend([
            Capability::ComeBack,
            Capability::CloseEpoch,
            Capability::Watch,
        ]);

        for argument in Arguments::command().get_arguments() {
            let id = argument.get_id().as_str();
            if id == "help" || id == "version" {
                continue;
            }
            assert!(
                BY_FLAG.iter().any(|(flag, _)| *flag == id),
                "--{id} is parsed and maps onto no capability"
            );
        }
        for capability in offered_by_terminal() {
            assert!(
                drawn.contains(&capability),
                "{capability:?}: the table says the terminal offers it and no flag does"
            );
        }
        for capability in &drawn {
            assert!(
                offered_by_terminal().contains(capability),
                "{capability:?}: a flag offers it and the table does not say so"
            );
        }
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
