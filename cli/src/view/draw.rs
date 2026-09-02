//! What the view draws, given a node and the words for it.
//!
//! Nothing here touches a terminal, reads a key or advances time. It takes a node and returns
//! the same frame for the same node every time, which is what lets what it draws be asserted
//! against `ratatui`'s `TestBackend` instead of looked at.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Stylize as _;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};

use crate::catalog::Catalog;
use crate::node::Node;

/// Every catalog key this view says out loud.
///
/// Here so that a test can assert each one exists in both catalogs. `tsc` does that job for
/// the frontend by typing every catalog against the English one, and nothing would otherwise
/// do it for a program written in Rust — where a key the catalogs lack would be a dotted key
/// printed at somebody rather than a test that failed.
pub const KEYS: &[&str] = &[
    "app.name",
    "app.version",
    "network.about.figure.network",
    "network.about.figure.identity",
    "network.about.figure.written",
    "network.about.figure.root",
    "network.about.figure.peer",
    "network.about.figure.peers",
    "network.about.figure.silent",
    "network.about.figure.interface",
    "network.about.figure.link",
    "network.peers.noNetworkTitle",
    "network.peers.noNetwork",
    "network.control.challengeShown",
    "cli.running",
    "cli.records",
    "cli.recordsNone",
    "cli.quit",
];

/// What a figure shows when nothing has been measured.
///
/// An em dash, and never a `0` or an empty column: a count of zero is a measurement, and this
/// is the absence of one.
const UNMEASURED: &str = "—";

/// Draws the whole view.
pub fn draw(frame: &mut Frame<'_>, node: &Node, catalog: &Catalog) {
    let block = Block::bordered().title(catalog.text("app.name"));
    let inside = block.inner(frame.area());
    frame.render_widget(block, frame.area());

    let rows = figures_of(node, catalog);
    // Every figure row, and never a fixed number of them: the list grew once past the space it
    // was given and the last three were clipped for a month before anybody noticed.
    let [heading, figures, middle, records, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(u16::try_from(rows.len()).unwrap_or(u16::MAX)),
        Constraint::Min(3),
        Constraint::Length(2),
        Constraint::Length(1),
    ])
    .areas(inside);

    draw_heading(frame, heading, catalog);
    draw_figures(frame, figures, &rows);
    // What goes in the middle is which emptiness this is, when there is one — a node on no network
    // — and otherwise the one thing shown to a person and gone: the challenge, when one was asked.
    if node.facts().network.is_none() {
        draw_explanation(frame, middle, catalog);
    } else if let Some(challenge) = node.challenge() {
        draw_challenge(frame, middle, challenge, catalog);
    }
    draw_records(frame, records, node, catalog);
    draw_footer(frame, footer, catalog);
}

/// The version, and that the node is up.
fn draw_heading(frame: &mut Frame<'_>, area: Rect, catalog: &Catalog) {
    let version = catalog.filled("app.version", "version", env!("CARGO_PKG_VERSION"));
    let lines = vec![
        Line::from(catalog.text("cli.running")),
        Line::from(Span::from(version).dim()),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

/// What a node reports about itself, read from the core and not assembled here.
///
/// The same figures the windowed face draws, from the same place — which is what keeps the two
/// from answering the same question differently. The peer count is the mesh socket's and the
/// interface address is the run's; neither is a fact the record holds, and both are drawn as what
/// they are.
fn figures_of(node: &Node, catalog: &Catalog) -> Vec<(String, Option<String>)> {
    let facts = node.facts();
    vec![
        (catalog.text("network.about.figure.network"), facts.network),
        (
            catalog.text("network.about.figure.identity"),
            facts.identity,
        ),
        (
            catalog.text("network.about.figure.written"),
            facts.written.map(|count| count.to_string()),
        ),
        (catalog.text("network.about.figure.root"), facts.root),
        (catalog.text("network.about.figure.peer"), facts.peer),
        (
            catalog.text("network.about.figure.peers"),
            node.peers().map(|count| count.to_string()),
        ),
        (
            catalog.text("network.about.figure.silent"),
            node.silent().map(|count| count.to_string()),
        ),
        (
            catalog.text("network.about.figure.interface"),
            node.interface_at().map(ToOwned::to_owned),
        ),
        (catalog.text("network.about.figure.link"), node.link()),
    ]
}

/// The figure rows, one line each, with the labels lined up.
fn draw_figures(frame: &mut Frame<'_>, area: Rect, rows: &[(String, Option<String>)]) {
    let width = rows.iter().map(|(label, _)| label.len()).max().unwrap_or(0);
    let lines: Vec<Line<'_>> = rows
        .iter()
        .map(|(label, value)| {
            Line::from(vec![
                Span::from(format!("{label:<width$}  ")).dim(),
                Span::from(value.clone().unwrap_or_else(|| UNMEASURED.to_owned())),
            ])
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), area);
}

/// The challenge, for whoever contributed this node to approve.
///
/// Text and the link beneath it, and no code drawn in blocks: nothing this program links encodes
/// one, and a dependency taken on for a square of characters is a dependency. The windowed face
/// draws the same challenge as a code; the text is the same string and approves the same thing.
fn draw_challenge(frame: &mut Frame<'_>, area: Rect, challenge: &str, catalog: &Catalog) {
    let lines = vec![
        Line::from(Span::from(catalog.text("network.control.challengeShown")).bold()),
        Line::from(challenge.to_owned()),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

/// Which emptiness this is, and why it is that one.
fn draw_explanation(frame: &mut Frame<'_>, area: Rect, catalog: &Catalog) {
    let lines = vec![
        Line::from(Span::from(catalog.text("network.peers.noNetworkTitle")).bold()),
        Line::from(catalog.text("network.peers.noNetwork")),
    ];

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

/// Where this node's records are going, which is a thing an operator has to be told.
fn draw_records(frame: &mut Frame<'_>, area: Rect, node: &Node, catalog: &Catalog) {
    let destination = node.records().map_or_else(
        || catalog.text("cli.recordsNone"),
        |path| path.display().to_string(),
    );

    let line = Line::from(vec![
        Span::from(format!("{}  ", catalog.text("cli.records"))).dim(),
        Span::from(destination),
    ]);

    frame.render_widget(Paragraph::new(line).wrap(Wrap { trim: true }), area);
}

/// How to leave.
fn draw_footer(frame: &mut Frame<'_>, area: Rect, catalog: &Catalog) {
    frame.render_widget(
        Paragraph::new(Line::from(Span::from(catalog.text("cli.quit")).dim())),
        area,
    );
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::{UNMEASURED, draw};
    use crate::catalog::Catalog;
    use crate::language::Language;
    use crate::node::Node;

    /// The width the view is drawn at in these tests.
    const WIDTH: u16 = 72;

    /// What the view draws for a node on no network, one string per line of the terminal.
    fn drawn(language: Language) -> Vec<String> {
        drawn_of(&Node::start(None), language)
    }

    /// What the view draws for `node`, one string per line of the terminal.
    fn drawn_of(node: &Node, language: Language) -> Vec<String> {
        let backend = TestBackend::new(WIDTH, 24);
        let mut terminal = Terminal::new(backend).expect("a test terminal");
        let catalog = Catalog::of(language);

        terminal
            .draw(|frame| draw(frame, node, &catalog))
            .expect("a frame");

        let cells: Vec<&str> = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();

        cells
            .chunks(WIDTH as usize)
            // The border is the block's, not the content's, and every assertion below is about
            // what was written inside it.
            .map(|row| row.concat().trim_matches('│').trim_end().to_owned())
            .collect()
    }

    /// How many figure rows the view draws, which is every one `figures_of` lists.
    const FIGURES: usize = 9;

    /// The figure rows, which are the lines after the border and the heading.
    fn figures(screen: &[String]) -> &[String] {
        screen.get(3..3 + FIGURES).unwrap_or_default()
    }

    /// A directory of this test's own, removed when it is done with it.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("almena-cli-draw-{name}"));
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
    fn every_figure_is_a_dash_because_nothing_was_measured() {
        let screen = drawn(Language::source());
        let figures = figures(&screen);

        // Asserted on the figure rows rather than on the whole screen, because the prose below
        // them legitimately contains an em dash of its own. This is the test that fails the day
        // somebody draws a zero for a count nobody took — and the day a row is clipped again.
        assert_eq!(figures.len(), FIGURES, "{screen:?}");
        for row in figures {
            assert!(row.ends_with(UNMEASURED), "{row}");
            assert!(!row.contains('0'), "a zero was drawn: {row}");
        }
    }

    #[test]
    fn a_node_on_a_network_draws_every_row_and_no_explanation_of_an_emptiness() {
        // **The other half.** The rows past the sixth used to be clipped, and the sentence about
        // there being no network was drawn on every frame, network or not.
        let scratch = Scratch::new("network");
        let mut node = Node::in_directory(
            None,
            Some(scratch.0.clone()),
            Vec::new(),
            almena_node::Which::Development,
        );
        node.open("dev.almena.network", &[], true)
            .expect("development opens on somebody's word");
        node.serving_at("127.0.0.1:8791");

        let screen = drawn_of(&node, Language::source());
        let rows = figures(&screen);
        assert_eq!(rows.len(), FIGURES, "{screen:?}");
        assert!(rows[0].ends_with(&node.facts().network.expect("a network")));
        assert!(
            rows[6].ends_with('0'),
            "silent is a count, and nought: {}",
            rows[6]
        );
        assert!(rows[7].ends_with("127.0.0.1:8791"), "{}", rows[7]);
        assert!(
            rows[8].contains("almena://node?address=127.0.0.1:8791&peer="),
            "{}",
            rows[8]
        );
        assert!(!screen.join("\n").contains("No network"), "{screen:?}");
        node.stop();
    }

    #[test]
    fn the_view_says_which_emptiness_this_is() {
        let screen = drawn(Language::source()).join("\n");

        // "No network" and not "no peers": there being no network to count peers on and there
        // being a network with nobody on it are two different facts, and this view is only
        // entitled to the first.
        assert!(screen.contains("No network"), "{screen}");
        assert!(!screen.contains("No peers"), "{screen}");
    }

    #[test]
    fn the_view_says_how_to_leave() {
        // A full-screen program that does not say how to get out of it is one people kill from
        // another window.
        let screen = drawn(Language::source()).join("\n");
        assert!(screen.to_lowercase().contains('q'), "{screen}");
    }

    #[test]
    fn the_same_view_is_drawn_in_spanish() {
        let english = drawn(Language::source());
        let spanish = drawn(Language::from_tag("es"));

        assert_ne!(english, spanish, "the Spanish view is the English one");
        for row in figures(&spanish) {
            assert!(
                row.ends_with(UNMEASURED),
                "the figures changed with the language: {row}"
            );
        }
    }
}
