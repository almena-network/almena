//! What this program accepts on a command line, which is four things and no more.

use clap::Parser;

/// The command line, parsed.
///
/// There is no argument naming a peer, a network or an address. This build joins no network,
/// so one would be accepted, used for nothing and refused by nothing — and Almena being
/// IPv6-only means accepting an address also means refusing every IPv4 one, which is a refusal
/// there is nowhere to write yet. The flag arrives with the code that can honour both halves.
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
    fn the_surface_is_valid() {
        // `clap` builds the parser at run time, so a mistake in the attributes above is a
        // panic on first use rather than a compile error. This is what turns it back into a
        // test failure.
        use clap::CommandFactory as _;
        Arguments::command().debug_assert();
    }
}
