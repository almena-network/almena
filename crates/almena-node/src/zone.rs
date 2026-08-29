//! What the zone publishes, and how much of it is worth believing.
//!
//! A node finds its first neighbours through DNS, and a client and the Registry find somewhere to
//! ask. That is all the zone is for. **It says who exists; it never says how anybody is doing** —
//! availability, lag behind the last root and error rate are measured between nodes and published
//! by them, and putting any of it in DNS would be one party declaring which nodes are good, which
//! is exactly what measuring across nodes exists to avoid.
//!
//! # Records are attributes, and one of them says who answers
//!
//! Every record is `name=value` pairs separated by spaces, opening with the shape's version. That
//! is so a reader keeps what it needs and nothing else, and so the shape can gain an attribute
//! later without anything having to be re-encoded.
//!
//! ```text
//! _seed  v=1 host=madrid.example port=4001 peer=12D3KooW… net=zQm…
//! _api   v=1 url=https://madrid.example
//! ```
//!
//! **A seed without `peer` is refused, and that is not a formality.** An address on its own says
//! *where* to call and not *who* should pick up: the mesh dials towards an identity, so with one
//! the handshake checks who answered and a substituted address fails loudly, and without one a
//! connection to whoever answers simply succeeds. It is the only authentication a first contact
//! has.
//!
//! **A version a reader does not know stops it.** Taking the attributes it recognises and guessing
//! at the rest would be guessing about where to connect and to whom.
//!
//! **And what it fixes is a commitment to check against, never a second source of truth.** What a
//! node is, what it can do and how it is behaving live in its chain. If the zone and the chain
//! ever disagreed, the zone is the weak one and loses.
//!
//! # What this does not catch
//!
//! Whoever controls the answers rewrites the whole record — address *and* identity — and the check
//! passes, because the reference came down the same compromised channel. What it does catch is a
//! tampered address with the record intact, and somebody sitting in the middle of the network.
//!
//! Against a hijacked zone what there is, is **having synchronised once**: nodes announce
//! themselves in the record, so whoever already has the log has a signed census and no longer
//! needs the zone. Which means the zone can only mislead somebody **arriving for the first time**.
//! That is the worst possible moment to mislead anybody, and it is said out loud rather than
//! assumed away.
//!
//! # The last good set is kept
//!
//! A zone that stops answering must cost discovery, not operation. Whoever has a good set carries
//! on with it — without that, losing DNS turns off everything rather than just the finding of new
//! neighbours.

use std::collections::BTreeMap;

/// The version every record declares, so that changing the shape is possible at all.
///
/// An older reader meeting a version it does not know **stops**, rather than reading the parts it
/// recognises and guessing at the rest. Four bytes buys the ability to change the shape one day
/// without every reader that predates the change quietly misunderstanding it.
const VERSION: &str = "1";

/// What each attribute is called.
mod attribute {
    /// Which shape this record is written in.
    pub const VERSION: &str = "v";
    /// The name to resolve.
    pub const HOST: &str = "host";
    /// The port to reach it on.
    pub const PORT: &str = "port";
    /// Who has to be at the other end.
    pub const PEER: &str = "peer";
    /// Which network the node at the other end is on.
    pub const NETWORK: &str = "net";
    /// Where an interface is served.
    pub const URL: &str = "url";
}

/// Why a record was not usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotUsable {
    /// Written in a shape this reader does not know.
    ///
    /// Not an error to work around: a reader that took the attributes it recognised and guessed at
    /// the rest would be guessing about where to connect and to whom.
    AnotherVersion,
    /// An attribute this record cannot do without is missing.
    Incomplete,
    /// A seed that says where to call without saying who answers.
    NoIdentity,
    /// A seed that says who answers without saying which network they are on.
    ///
    /// **Somebody arriving for the first time cannot work it out.** What separates two networks is
    /// the name of the protocol nodes speak, and that name has the network's own inside it — so a
    /// newcomer with no record yet cannot even ask, and would have to take whatever it was handed
    /// and call that the network it joined.
    NoNetwork,
    /// Something that is not a set of attributes at all.
    Malformed,
    /// An interface address that is not one a browser would be allowed to call.
    NotSecure,
}

/// The attributes of one record, as written.
fn attributes(record: &str) -> Result<BTreeMap<&str, &str>, NotUsable> {
    let record = record.trim();
    if record.is_empty() {
        return Err(NotUsable::Malformed);
    }

    let mut found = BTreeMap::new();
    for pair in record.split_whitespace() {
        let Some((name, value)) = pair.split_once('=') else {
            return Err(NotUsable::Malformed);
        };
        if name.is_empty() || value.is_empty() {
            return Err(NotUsable::Malformed);
        }
        found.insert(name, value);
    }

    match found.get(attribute::VERSION) {
        Some(&VERSION) => Ok(found),
        Some(_) => Err(NotUsable::AnotherVersion),
        None => Err(NotUsable::Malformed),
    }
}

/// A node to start from: where it is, and who it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seed {
    /// The name to resolve. Whether it answers over IPv4, IPv6 or both is that host's business:
    /// a name carries whichever address records it has, and the dialler takes what it finds.
    host: String,
    /// The port to reach it on.
    port: u16,
    /// Who has to be at the other end, which is what a connection is checked against.
    peer: String,
    /// Which network they are on: the name of the act that opened it.
    ///
    /// **A commitment, never a second source of truth.** It is what lets somebody arriving for the
    /// first time speak to that node at all — the protocol two nodes negotiate carries the
    /// network's name, and a newcomer has no record to take it from. What arrives afterwards is
    /// checked against this; if the two disagree, this is the weak one and it loses.
    network: String,
}

impl Seed {
    /// Read a `_seed` record.
    ///
    /// # Errors
    ///
    /// [`NotUsable::NoIdentity`] when it carries no peer identity. **That is not a formality.** The
    /// mesh dials towards an identity, not towards an address: with one, the handshake checks who
    /// answered and a substituted address fails loudly; without one, a connection to whoever picked
    /// up succeeds and there is nothing to notice. It is the only authentication a first contact
    /// has — everything afterwards comes from the signed record, which whoever has synchronised
    /// once no longer needs this for.
    pub fn read(record: &str) -> Result<Self, NotUsable> {
        let found = attributes(record)?;

        let host = *found.get(attribute::HOST).ok_or(NotUsable::Incomplete)?;
        let port = found
            .get(attribute::PORT)
            .ok_or(NotUsable::Incomplete)?
            .parse::<u16>()
            .map_err(|_| NotUsable::Malformed)?;
        let peer = *found.get(attribute::PEER).ok_or(NotUsable::NoIdentity)?;
        let network = *found.get(attribute::NETWORK).ok_or(NotUsable::NoNetwork)?;

        Ok(Self {
            host: host.to_owned(),
            port,
            peer: peer.to_owned(),
            network: network.to_owned(),
        })
    }

    /// The name to resolve.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The port to reach it on.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// Who has to be at the other end.
    #[must_use]
    pub fn peer(&self) -> &str {
        &self.peer
    }

    /// Which network they say they are on.
    ///
    /// A claim to check what arrives against, not a fact. A node that pulled a record whose first
    /// act does not have this name has been handed a different network from the one the zone
    /// named, and the record is what counts.
    #[must_use]
    pub fn network(&self) -> &str {
        &self.network
    }
}

/// Somewhere the platform's interface is served.
///
/// **The same record whether it is published under `_api` or under `_mediator`**, and one parser
/// for both, because they say the same thing: here is an origin, reachable securely. What differs
/// is the name it was published at — which is the zone saying *this one also holds post* — and a
/// second format would be a second way to get the same line wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint(String);

impl Endpoint {
    /// Read an `_api` record.
    ///
    /// # Errors
    ///
    /// [`NotUsable::NotSecure`] for anything a browser would refuse to call from a page it loaded
    /// securely, which is every consumer this record exists for.
    pub fn read(record: &str) -> Result<Self, NotUsable> {
        let found = attributes(record)?;
        let url = *found.get(attribute::URL).ok_or(NotUsable::Incomplete)?;

        if !url.starts_with("https://") {
            return Err(NotUsable::NotSecure);
        }
        Ok(Self(url.to_owned()))
    }

    /// Where to ask.
    #[must_use]
    pub fn origin(&self) -> &str {
        &self.0
    }
}

/// What one lookup of the zone came back with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answer {
    /// Nodes to join the mesh through.
    pub seeds: Vec<Seed>,
    /// Nodes serving the interface.
    pub api: Vec<Endpoint>,
    /// Nodes that have said they hold post.
    ///
    /// **A starting point and not a permission.** What the zone publishes is where to look; whether
    /// a node actually runs a mailbox is that node's own announcement in the record, and the client
    /// that picks one checks there. A zone that named a mediator which does not hold post costs
    /// somebody a wasted question and nothing else (`SPECS.md §6.2`).
    pub mediators: Vec<Endpoint>,
}

impl Answer {
    /// Read a zone's records, keeping what is usable and saying what was not.
    ///
    /// **An unusable record does not spoil the answer.** One node publishing a bad line must not
    /// cost everybody else their way in, and the zone is a small hand-maintained thing where a
    /// typo is a normal event rather than an attack.
    #[must_use]
    pub fn read(seeds: &[String], api: &[String], mediators: &[String]) -> (Self, Vec<NotUsable>) {
        let mut refused = Vec::new();
        let mut answer = Self::default();

        for record in seeds {
            match Seed::read(record) {
                Ok(seed) => answer.seeds.push(seed),
                Err(why) => refused.push(why),
            }
        }
        for (records, into) in [(api, &mut answer.api), (mediators, &mut answer.mediators)] {
            for record in records {
                match Endpoint::read(record) {
                    Ok(endpoint) => into.push(endpoint),
                    Err(why) => refused.push(why),
                }
            }
        }
        (answer, refused)
    }

    /// Whether the zone offered anything at all.
    ///
    /// This is what decides whether a node opens a network or joins one, and it is the reason a
    /// lookup that failed must never be reported as an empty zone: **an empty zone means nobody is
    /// there, and a node that believes it opens a second network.**
    ///
    /// A mediator counts, even though it is not a way into the mesh. The question this answers is
    /// *is anybody running this network*, and somebody publishing a mailbox for it is somebody
    /// running it — so a zone with mediators and no seeds is a zone with a problem to fix rather
    /// than an invitation to start a second network beside the first.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seeds.is_empty() && self.api.is_empty() && self.mediators.is_empty()
    }
}

/// The last set that worked, kept so that losing the zone costs discovery and not operation.
#[derive(Debug, Clone, Default)]
pub struct Remembered {
    last: Option<Answer>,
}

impl Remembered {
    /// Nothing looked up yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Take in what a lookup came back with, and say what to work from now.
    ///
    /// `answer` is [`None`] when the zone did not answer at all — **which is not the same as
    /// answering with nothing**, and the two must never be collapsed into one another. A zone that
    /// is down leaves the last good set in place; a zone that is genuinely empty replaces it.
    pub fn looked_up(&mut self, answer: Option<Answer>) -> Option<&Answer> {
        if let Some(answer) = answer {
            self.last = Some(answer);
        }
        self.last.as_ref()
    }

    /// What is being worked from, if anything ever was.
    #[must_use]
    pub fn known(&self) -> Option<&Answer> {
        self.last.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::{Answer, Endpoint, NotUsable, Remembered, Seed};

    const ONE: &str =
        "v=1 host=madrid.example port=4001 peer=12D3KooWExampleMadrid net=zQmSomeGenesis";
    const TWO: &str =
        "v=1 host=barcelona.example port=4001 peer=12D3KooWExampleBarcelona net=zQmSomeGenesis";
    const SERVED: &str = "v=1 url=https://madrid.example";

    #[test]
    fn a_seed_carries_where_and_who() {
        let seed = Seed::read(ONE).expect("usable");
        assert_eq!(seed.host(), "madrid.example");
        assert_eq!(seed.port(), 4001);
        assert_eq!(seed.peer(), "12D3KooWExampleMadrid");
    }

    #[test]
    fn a_seed_with_no_identity_is_refused() {
        // The mesh dials towards an identity. Without one, a connection to whoever answers simply
        // succeeds, and a substituted address is something nobody can notice.
        assert_eq!(
            Seed::read("v=1 host=madrid.example port=4001"),
            Err(NotUsable::NoIdentity)
        );
    }

    #[test]
    fn a_seed_missing_where_to_call_is_refused_as_incomplete() {
        // Told apart from a missing identity: one is a record nobody finished, the other is a
        // record that would work and must not.
        assert_eq!(
            Seed::read("v=1 port=4001 peer=12D3KooWExampleMadrid"),
            Err(NotUsable::Incomplete)
        );
        assert_eq!(
            Seed::read("v=1 host=madrid.example peer=12D3KooWExampleMadrid"),
            Err(NotUsable::Incomplete)
        );
    }

    #[test]
    fn a_shape_this_reader_does_not_know_stops_it() {
        // Reading the attributes it recognises and guessing at the rest would be guessing about
        // where to connect and to whom.
        assert_eq!(
            Seed::read("v=2 host=madrid.example port=4001 peer=12D3KooWExampleMadrid"),
            Err(NotUsable::AnotherVersion)
        );
        assert_eq!(
            Endpoint::read("v=99 url=https://madrid.example"),
            Err(NotUsable::AnotherVersion)
        );
    }

    #[test]
    fn an_attribute_this_reader_has_never_heard_of_is_passed_over() {
        // Which is what the attributes are for: the shape gains one and nothing that predates it
        // has to be re-encoded. Adding meaning that changes how a record is read is what the
        // version is for instead.
        let seed = Seed::read(&format!("{ONE} region=eu weight=3")).expect("usable");
        assert_eq!(seed.host(), "madrid.example");
        assert_eq!(seed.peer(), "12D3KooWExampleMadrid");
    }

    #[test]
    fn a_seed_that_does_not_say_which_network_is_refused() {
        // Somebody arriving for the first time cannot work it out: what separates two networks is
        // the name of the protocol they speak, and that name has the network's own inside it. A
        // newcomer with no record has nothing to build it from, and would end up calling whatever
        // it was handed the network it joined.
        assert_eq!(
            Seed::read("v=1 host=madrid.example port=4001 peer=12D3KooWExampleMadrid"),
            Err(NotUsable::NoNetwork)
        );
    }

    #[test]
    fn a_seed_says_which_network_it_is_speaking_for() {
        let seed = Seed::read(ONE).expect("usable");
        assert_eq!(seed.network(), "zQmSomeGenesis");
    }

    #[test]
    fn something_that_is_not_a_set_of_attributes_is_refused() {
        for record in [
            "",
            "   ",
            "/dns/madrid.example/tcp/4001/p2p/12D3KooW",
            "host=madrid.example port=4001",
            "v=1 host= port=4001 peer=x",
            "v=1 hostmadrid.example port=4001",
            "v=1 host=madrid.example port=four peer=x",
        ] {
            assert!(Seed::read(record).is_err(), "{record:?}");
        }
    }

    #[test]
    fn surrounding_space_does_not_make_a_record_unusable() {
        // Records are typed by hand into a zone file.
        assert_eq!(Seed::read(&format!("  {ONE}  ")), Seed::read(ONE));
    }

    #[test]
    fn an_interface_has_to_be_one_a_browser_would_call() {
        // Every consumer of this record is a browser or something standing in for one.
        assert_eq!(
            Endpoint::read(SERVED).expect("usable").origin(),
            "https://madrid.example"
        );
        assert_eq!(
            Endpoint::read("v=1 url=http://madrid.example"),
            Err(NotUsable::NotSecure)
        );
        assert_eq!(Endpoint::read("v=1"), Err(NotUsable::Incomplete));
    }

    #[test]
    fn a_zone_with_a_mailbox_in_it_is_a_zone_somebody_is_running() {
        // **Not a way into the mesh, and still somebody being there.** The question `is_empty`
        // answers is whether anybody is running this network at all; a zone publishing a mailbox
        // for it and no seeds is a zone with a problem to fix, and a node that read it as *nobody
        // is here* would open a second network beside the first.
        let (answer, refused) = Answer::read(&[], &[], &[SERVED.to_owned()]);
        assert!(refused.is_empty());
        assert_eq!(answer.mediators.len(), 1);
        assert!(!answer.is_empty());

        // And the record is held to the same bar as any other origin: a browser will not call it
        // from a page it loaded securely, so publishing it is publishing something nobody can use.
        let (_, refused) = Answer::read(&[], &[], &["v=1 url=http://madrid.example".to_owned()]);
        assert_eq!(refused, vec![NotUsable::NotSecure]);
    }

    #[test]
    fn one_bad_record_does_not_cost_everybody_else_their_way_in() {
        // A zone is a small hand-maintained thing, and a typo in it is a normal event.
        let (answer, refused) = Answer::read(
            &[
                ONE.to_owned(),
                "v=1 host=broken.example port=4001".to_owned(),
                TWO.to_owned(),
            ],
            &[
                SERVED.to_owned(),
                "v=1 url=http://barcelona.example".to_owned(),
            ],
            &[],
        );

        assert_eq!(answer.seeds.len(), 2);
        assert_eq!(answer.api.len(), 1);
        assert_eq!(refused, vec![NotUsable::NoIdentity, NotUsable::NotSecure]);
    }

    #[test]
    fn a_zone_that_answered_with_nothing_means_nobody_is_there() {
        // Which is what decides whether a node opens a network or joins one.
        let (answer, refused) = Answer::read(&[], &[], &[]);
        assert!(answer.is_empty());
        assert!(refused.is_empty());
    }

    #[test]
    fn a_zone_that_did_not_answer_is_not_a_zone_that_answered_with_nothing() {
        // Collapsing the two is how a node opens a second network: it asks, gets no reply, reads
        // that as *nobody is here*, and opens one beside the network that was already running.
        let mut remembered = Remembered::new();
        let (good, _) = Answer::read(&[ONE.to_owned()], &[], &[]);

        remembered.looked_up(Some(good.clone()));
        assert_eq!(remembered.known(), Some(&good));

        remembered.looked_up(None);
        assert_eq!(
            remembered.known(),
            Some(&good),
            "the zone being down leaves what was working in place"
        );

        let (empty, _) = Answer::read(&[], &[], &[]);
        remembered.looked_up(Some(empty.clone()));
        assert_eq!(
            remembered.known(),
            Some(&empty),
            "and a zone that really is empty replaces it"
        );
    }

    #[test]
    fn nothing_is_known_until_something_answers() {
        let mut remembered = Remembered::new();
        assert_eq!(remembered.known(), None);
        assert_eq!(remembered.looked_up(None), None);
    }

    #[test]
    fn the_zone_says_who_exists_and_nothing_about_how_they_are_doing() {
        // There is nowhere in any of these types to put availability, lag or an error rate, and
        // that is the design: putting them here would be one party declaring which nodes are
        // good, which is what measuring across nodes exists to avoid. An attribute claiming to
        // carry one is passed over like any other nobody asked for.
        let seed = Seed::read(&format!("{ONE} availability=0.99")).expect("usable");
        let kept = format!("{seed:?}");
        for absent in ["availability", "lag", "error", "score", "rank"] {
            assert!(!kept.contains(absent), "{absent} has no place here");
        }
    }
}
