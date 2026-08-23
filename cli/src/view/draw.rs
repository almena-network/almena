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
/// do it for a program written in Rust — which is the half of
/// `.agents/rules/catalog-parity.md` a type checker cannot reach from here.
pub const KEYS: &[&str] = &[
    "app.name",
    "app.version",
    "network.about.figure.network",
    "network.about.figure.identity",
    "network.about.figure.peers",
    "network.peers.noNetworkTitle",
    "network.peers.noNetwork",
    "cli.running",
    "cli.records",
    "cli.recordsNone",
    "cli.quit",
];

/// What a figure shows when nothing has been measured.
///
/// An em dash, and never a `0` or an empty column. `.agents/rules/honest-emptiness.md`: a
/// count of zero is a measurement, and this is the absence of one.
const UNMEASURED: &str = "—";

/// Draws the whole view.
pub fn draw(frame: &mut Frame<'_>, node: &Node, catalog: &Catalog) {
    let block = Block::bordered().title(catalog.text("app.name"));
    let inside = block.inner(frame.area());
    frame.render_widget(block, frame.area());

    let [heading, figures, explanation, records, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(2),
        Constraint::Length(1),
    ])
    .areas(inside);

    draw_heading(frame, heading, catalog);
    draw_figures(frame, figures, node, catalog);
    draw_explanation(frame, explanation, catalog);
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

/// The three things a node reports about itself, none of them measured yet.
fn draw_figures(frame: &mut Frame<'_>, area: Rect, node: &Node, catalog: &Catalog) {
    let rows = [
        (
            catalog.text("network.about.figure.network"),
            node.network().map(ToOwned::to_owned),
        ),
        (
            catalog.text("network.about.figure.identity"),
            node.identity().map(ToOwned::to_owned),
        ),
        (
            catalog.text("network.about.figure.peers"),
            node.peers().map(|count| count.to_string()),
        ),
    ];

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

    /// What the view draws for a node, one string per line of the terminal.
    fn drawn(language: Language) -> Vec<String> {
        let backend = TestBackend::new(WIDTH, 20);
        let mut terminal = Terminal::new(backend).expect("a test terminal");
        let node = Node::start(None);
        let catalog = Catalog::of(language);

        terminal
            .draw(|frame| draw(frame, &node, &catalog))
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

    /// The three figure rows, which are the three lines after the border and the heading.
    fn figures(screen: &[String]) -> &[String] {
        screen.get(3..6).unwrap_or_default()
    }

    #[test]
    fn every_figure_is_a_dash_because_nothing_was_measured() {
        let screen = drawn(Language::English);
        let figures = figures(&screen);

        // Asserted on the figure rows rather than on the whole screen, because the prose below
        // them legitimately contains an em dash of its own. This is the test that fails the day
        // somebody draws a zero for a count nobody took.
        assert_eq!(figures.len(), 3, "{screen:?}");
        for row in figures {
            assert!(row.ends_with(UNMEASURED), "{row}");
            assert!(!row.contains('0'), "a zero was drawn: {row}");
        }
    }

    #[test]
    fn the_view_says_which_emptiness_this_is() {
        let screen = drawn(Language::English).join("\n");

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
        let screen = drawn(Language::English).join("\n");
        assert!(screen.to_lowercase().contains('q'), "{screen}");
    }

    #[test]
    fn the_same_view_is_drawn_in_spanish() {
        let english = drawn(Language::English);
        let spanish = drawn(Language::Spanish);

        assert_ne!(english, spanish, "the Spanish view is the English one");
        for row in figures(&spanish) {
            assert!(
                row.ends_with(UNMEASURED),
                "the figures changed with the language: {row}"
            );
        }
    }
}
