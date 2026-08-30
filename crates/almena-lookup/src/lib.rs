//! Asking DNS what a zone publishes, and telling silence apart from an empty answer.
//!
//! What a node does with a zone's records is a rule and lives with the node. This is the part that
//! goes and gets them, kept apart so that the rule can be tested without a network and so that
//! nothing which merely *reads* a record has to link a resolver.
//!
//! # The distinction the whole thing turns on
//!
//! **A zone that is down is not a zone that is empty.**
//!
//! | What DNS said | What it means | What a node does |
//! | --- | --- | --- |
//! | Here are some records | Somebody is there | Join them |
//! | That name does not exist, or it has no records | **Nobody is there** | Open a network |
//! | Nothing, or a failure | *Nobody knows* | Work from what it knew before, and open nothing |
//!
//! Collapsing the last two is how a node opens a second network beside the one that was already
//! running: the zone was unreachable for a minute, the node read that as *nobody is there*, and now
//! there are two networks that say the same things about themselves and cannot be told apart by
//! anybody reading a label.
//!
//! So a name that does not exist is an **answer** and comes back as an empty list, and everything
//! else that goes wrong is [`Silent`].
//!
//! # No validation, and that is not an oversight
//!
//! The zone is the weak source here and is treated as one: what it publishes is a commitment to
//! check against, never a second place the truth lives. Validating it would make what a node found
//! in DNS harder to forge without making it any more authoritative — and a node that had *checked*
//! the zone would be more inclined to believe it, which is the wrong direction.

use std::net::IpAddr;

use almena_node::zone::{Answer, NotUsable};

/// Where nodes to join the mesh through are published, under the zone.
const SEED: &str = "_seed";

/// Where nodes serving the interface are published, under the zone.
const API: &str = "_api";

/// Where nodes that hold post are published, under the zone.
///
/// **Separate from `_api` because they are not the same offer.** Every node serves the interface;
/// holding somebody's post is a thing an operator switches on, and a client looking for a mediator
/// would otherwise have to ask every node in the zone whether it was one.
const MEDIATOR: &str = "_mediator";

/// The zone did not answer.
///
/// **Not the same as answering with nothing**, and the two must never be collapsed: one is *nobody
/// is there* and the other is *nobody knows*, and acting on the first when it was the second opens
/// a second network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Silent;

/// Something that can be asked for the text records at a name.
///
/// A trait so that everything above it can be tested without a network, and so that a machine with
/// a resolver of its own can be the thing that answers.
pub trait Records {
    /// The text records at `name`.
    ///
    /// An empty list means **the name answered and holds none**. A name that does not exist is one
    /// of those answers, not a failure.
    ///
    /// # Errors
    ///
    /// [`Silent`] when nothing came back at all.
    fn text(&self, name: &str) -> impl Future<Output = Result<Vec<String>, Silent>> + Send;
}

/// The one that talks to a real resolver.
pub struct Dns {
    /// Configured from the machine's own resolver settings, because a node that carried its own
    /// list of servers would ignore whatever its operator had already decided about DNS.
    resolver: hickory_resolver::TokioResolver,
}

impl Dns {
    /// Ask whatever this machine already uses for DNS.
    ///
    /// # Errors
    ///
    /// [`Silent`] when the machine will not say what its resolver is — on which a node cannot look
    /// anything up, and must not conclude that nobody is there.
    pub fn of_this_machine() -> Result<Self, Silent> {
        let builder = hickory_resolver::Resolver::builder_tokio().map_err(|_| Silent)?;
        Ok(Self {
            resolver: builder.build().map_err(|_| Silent)?,
        })
    }

    /// Ask these servers instead of the machine's own.
    ///
    /// For an operator whose machine resolves differently from the network it is joining, and for
    /// a test that wants a resolver it controls.
    ///
    /// # Errors
    ///
    /// [`Silent`] when a resolver cannot be built over those servers at all.
    pub fn asking(servers: &[IpAddr]) -> Result<Self, Silent> {
        use hickory_resolver::config::{NameServerConfig, ResolverConfig};

        // Both protocols, because a set of records can outgrow what fits in one datagram and a
        // node that could only ask over UDP would read a truncated zone as a short one.
        let configured = ResolverConfig::from_parts(
            None,
            Vec::new(),
            servers
                .iter()
                .map(|server| NameServerConfig::udp_and_tcp(*server))
                .collect(),
        );
        Ok(Self {
            resolver: hickory_resolver::Resolver::builder_with_config(
                configured,
                hickory_resolver::net::runtime::TokioRuntimeProvider::default(),
            )
            .build()
            .map_err(|_| Silent)?,
        })
    }
}

impl Records for Dns {
    async fn text(&self, name: &str) -> Result<Vec<String>, Silent> {
        match self.resolver.txt_lookup(name).await {
            Ok(found) => Ok(found
                .answers()
                .iter()
                .filter_map(|record| match &record.data {
                    hickory_resolver::proto::rr::RData::TXT(text) => Some(text),
                    // Anything else at that name is not this node's business, and refusing the
                    // whole answer because somebody put a comment there would be brittle.
                    _ => None,
                })
                .map(|text| {
                    // One record can arrive in several strings. DNS puts them back together end to
                    // end, and a reader that treated them as separate records would see one line
                    // cut in two and refuse both halves.
                    text.txt_data
                        .iter()
                        .map(|part| String::from_utf8_lossy(part).into_owned())
                        .collect::<String>()
                })
                .collect()),
            // The name is not there, or it is and holds no text. Both are the zone answering, and
            // what it answered is *nobody is here*.
            Err(why) if why.is_no_records_found() => Ok(Vec::new()),
            Err(_) => Err(Silent),
        }
    }
}

/// What one look at a zone found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Looked {
    /// What was published under `_seed`, exactly as written.
    ///
    /// **Kept unparsed as well as parsed**, because the two answer different questions. What a node
    /// connects to has to be understood; whether anybody is there at all does not — a line nobody
    /// can read is still somebody having published something, and *open a network only when nobody
    /// is there* has to be answered on that and not on this build's ability to parse.
    pub seeds: Vec<String>,
    /// What could be made of everything the zone published.
    pub answer: Answer,
    /// What was there and unusable.
    ///
    /// **It never spoils the rest**: one node publishing a bad line must not cost everybody else
    /// their way in, and a zone is a small hand-maintained thing where a typo is a normal event
    /// rather than an attack.
    pub refused: Vec<NotUsable>,
}

/// How long a zone is given to answer.
///
/// **A node that waited for ever would have no answer and no way to say so**, which is worse than
/// either of the two a zone can give: nobody is there, or nobody knows. Long enough for a resolver
/// that has to go and ask, and short enough that a node which cannot come up says so while somebody
/// is still watching it try.
pub const PATIENCE: std::time::Duration = std::time::Duration::from_secs(10);

/// One look at a zone.
///
/// [`None`] when the zone did not answer — which is *nobody knows* and must never be read as
/// *nobody is there*. A zone that answered and holds nothing comes back as a [`Looked`] with
/// nothing in it, which is a different thing and the one a node may act on.
pub async fn look(records: &impl Records, zone: &str) -> Option<Looked> {
    // **All three at once, because they are three independent questions.** Asked one after another
    // they share one budget and spend the sum of three round trips — which is how a zone that
    // answers every one of them in milliseconds still comes back as a silence when the resolver is
    // having a slow minute. Nothing here reads one answer to decide the next.
    let (seed, api, mediator) = (under(SEED, zone), under(API, zone), under(MEDIATOR, zone));
    let (seeds, api, mediators) = tokio::join!(
        records.text(&seed),
        records.text(&api),
        records.text(&mediator),
    );
    looked(seeds, api, mediators)
}

/// The same, given up on if it takes longer than [`PATIENCE`].
///
/// Running out of time is a silence and not an empty zone, which is the distinction everything
/// here turns on: one is *nobody knows* and the other is *nobody is there*.
pub async fn look_patiently(records: &impl Records, zone: &str) -> Option<Looked> {
    tokio::time::timeout(PATIENCE, look(records, zone))
        .await
        .ok()
        .flatten()
}

/// What to make of the two answers.
fn looked(
    seeds: Result<Vec<String>, Silent>,
    api: Result<Vec<String>, Silent>,
    mediators: Result<Vec<String>, Silent>,
) -> Option<Looked> {
    // Any part going quiet makes the whole look a silence. A zone part of which is unreachable has
    // not told this node that nobody is there, and part of an answer is the shape a node would most
    // easily mistake for one.
    let (Ok(seeds), Ok(api), Ok(mediators)) = (seeds, api, mediators) else {
        return None;
    };

    let (answer, refused) = Answer::read(&seeds, &api, &mediators);
    Some(Looked {
        seeds,
        answer,
        refused,
    })
}

/// A name under a zone, written so that it means only itself.
///
/// **The trailing dot is the whole of this function.** Without it the name is relative, and a
/// machine with a search domain — a company network, a VPN, anything that appends a suffix — asks
/// for `_seed.the.zone.the-search-domain` first. A resolver that does not answer that at all
/// leaves the node waiting, and waiting is indistinguishable from a zone that is down.
///
/// It costs a round trip even where it works, because the answer everybody wanted was the second
/// question rather than the first.
fn under(what: &str, zone: &str) -> String {
    format!("{what}.{}.", zone.trim_end_matches('.'))
}

#[cfg(test)]
mod tests {
    use super::{Records, Silent, look, under};
    use std::collections::BTreeMap;

    /// A zone this test wrote, so that nothing here needs a network.
    struct Wrote(BTreeMap<String, Result<Vec<String>, Silent>>);

    impl Records for Wrote {
        async fn text(&self, name: &str) -> Result<Vec<String>, Silent> {
            self.0.get(name).cloned().unwrap_or(Ok(Vec::new()))
        }
    }

    fn zone(entries: &[(&str, Result<Vec<String>, Silent>)]) -> Wrote {
        Wrote(
            entries
                .iter()
                .map(|(name, answer)| ((*name).to_owned(), answer.clone()))
                .collect(),
        )
    }

    const SEED: &str =
        "v=1 host=madrid.example port=4001 peer=12D3KooWExampleMadrid net=zQmSomeGenesis";
    const SERVED: &str = "v=1 url=https://madrid.example";

    #[test]
    fn a_name_is_asked_for_absolutely_and_never_relative_to_anything() {
        // **The bug this exists to stop.** A name without a trailing dot is relative, so a machine
        // with a search domain asks for `_seed.the.zone.the-search-domain` first — and a resolver
        // that does not answer that at all leaves the node waiting, which is indistinguishable
        // from a zone that is down.
        assert_eq!(
            under("_seed", "dev.almena.network"),
            "_seed.dev.almena.network."
        );
    }

    #[test]
    fn a_zone_written_the_way_a_zone_file_writes_it_gives_the_same_name() {
        // People copy zones across from a zone file, where the trailing dot is how they are
        // written — and two dots in the middle is not a name.
        assert_eq!(
            under("_seed", "dev.almena.network."),
            under("_seed", "dev.almena.network")
        );
    }

    #[tokio::test]
    async fn a_zone_with_nodes_in_it_gives_them_back() {
        let looked = look(
            &zone(&[
                ("_seed.dev.almena.network.", Ok(vec![SEED.to_owned()])),
                ("_api.dev.almena.network.", Ok(vec![SERVED.to_owned()])),
            ]),
            "dev.almena.network",
        )
        .await
        .expect("the zone answered");

        assert_eq!(looked.answer.seeds.len(), 1);
        assert_eq!(looked.answer.api.len(), 1);
        assert!(looked.refused.is_empty());
    }

    #[tokio::test]
    async fn a_zone_that_holds_nothing_says_nobody_is_there() {
        // This is the answer a node opens a network on, so it has to be an answer and not an
        // absence.
        let looked = look(&zone(&[]), "dev.almena.network")
            .await
            .expect("the zone answered");
        assert!(looked.answer.is_empty() && looked.seeds.is_empty());
    }

    #[tokio::test]
    async fn a_zone_that_did_not_answer_is_not_a_zone_that_is_empty() {
        // The distinction the whole crate exists for. Collapsing these is how a node opens a
        // second network beside the one that was already running.
        assert!(
            look(
                &zone(&[("_seed.dev.almena.network.", Err(Silent))]),
                "dev.almena.network",
            )
            .await
            .is_none(),
            "and nothing is concluded from it"
        );
    }

    #[tokio::test]
    async fn half_an_answer_is_a_silence_and_not_half_a_zone() {
        // Half an answer is the shape a node would most easily mistake for a whole one.
        assert!(
            look(
                &zone(&[
                    ("_seed.dev.almena.network.", Ok(vec![SEED.to_owned()])),
                    ("_api.dev.almena.network.", Err(Silent)),
                ]),
                "dev.almena.network",
            )
            .await
            .is_none()
        );

        // Every part, including the newest one. A part added later that did not make the whole
        // look a silence would be a part through which the distinction quietly stopped holding.
        assert!(
            look(
                &zone(&[
                    ("_seed.dev.almena.network.", Ok(vec![SEED.to_owned()])),
                    ("_api.dev.almena.network.", Ok(vec![SERVED.to_owned()])),
                    ("_mediator.dev.almena.network.", Err(Silent)),
                ]),
                "dev.almena.network",
            )
            .await
            .is_none()
        );
    }

    #[tokio::test]
    async fn a_mailbox_published_in_the_zone_is_read_as_one() {
        let looked = look(
            &zone(&[
                ("_seed.dev.almena.network.", Ok(vec![SEED.to_owned()])),
                ("_mediator.dev.almena.network.", Ok(vec![SERVED.to_owned()])),
            ]),
            "dev.almena.network",
        )
        .await
        .expect("answered");
        assert_eq!(looked.answer.mediators.len(), 1);
        assert!(
            looked.answer.api.is_empty(),
            "and it is not mistaken for the interface, which is a different offer"
        );
    }

    #[tokio::test]
    async fn a_zone_with_somebody_in_it_stops_a_node_opening_a_second_network() {
        // The composition this crate exists for, end to end: what the zone published reaches the
        // rule, and the rule is the one that refuses. Nothing between them decides anything.
        let looked = look(
            &zone(&[("_seed.dev.almena.network.", Ok(vec![SEED.to_owned()]))]),
            "dev.almena.network",
        )
        .await
        .expect("answered");

        let opening = almena_node::Opening {
            which: almena_node::Which::Development,
            beginning: almena_node::Epoch::GENESIS,
            began: 1_800_000_000,
        };
        let key = |seed: u8| almena_suite::ed25519::SigningKey::from_secret([seed; 32]);

        assert!(
            matches!(
                almena_node::Node::open(&opening, &looked.seeds, &key(5), key(6)),
                Err(almena_node::NotOpened::ThereIsAlreadyANetwork(_))
            ),
            "somebody is there, so there is a network to join and not one to open"
        );
    }

    #[tokio::test]
    async fn a_zone_nobody_is_in_lets_a_node_open_one() {
        // The other half. Without it the check above would pass on a node that never opens at all.
        let looked = look(&zone(&[]), "dev.almena.network")
            .await
            .expect("answered");

        let opening = almena_node::Opening {
            which: almena_node::Which::Development,
            beginning: almena_node::Epoch::GENESIS,
            began: 1_800_000_000,
        };
        let key = |seed: u8| almena_suite::ed25519::SigningKey::from_secret([seed; 32]);

        assert!(almena_node::Node::open(&opening, &looked.seeds, &key(5), key(6)).is_ok());
    }

    #[tokio::test]
    async fn one_bad_record_does_not_cost_everybody_else_their_way_in() {
        // The zone is a small hand-maintained thing, where a typo is a normal event rather than an
        // attack.
        let looked = look(
            &zone(&[(
                "_seed.dev.almena.network.",
                Ok(vec!["nonsense".to_owned(), SEED.to_owned()]),
            )]),
            "dev.almena.network",
        )
        .await
        .expect("answered");

        assert_eq!(looked.answer.seeds.len(), 1);
        assert_eq!(
            looked.refused.len(),
            1,
            "and what was wrong is said, not swallowed"
        );
        assert_eq!(
            looked.seeds.len(),
            2,
            "but somebody published two things, and that is what decides whether anybody is there"
        );
    }
}

/// Proving that a domain is an entity's, which is a different question from finding a node.
///
/// **The two uses of DNS in this platform must not be confused** (`SPECS.md §4.5`). One is
/// discovery — where to knock to join a mesh — and it is what everything above answers. This is the
/// other: a **bidirectional link** in which the domain points at an entity and the entity's own
/// record names the domain, so that neither half alone proves anything.
///
/// # Nothing here decides whether an act is valid, and that is deliberate
///
/// A node cannot make admitting an act depend on a DNS lookup. Validity is settled against the act
/// and the record, so that any node replaying the same history reaches the same answer at any hour
/// — and a rule that asked the network would make two honest nodes disagree because one of them was
/// asked while a resolver was down. So the record carries **the claim and the date it was last
/// proved**, and checking it is done by whoever is about to rely on it: the entity's own app when
/// it revalidates, and a reader when it matters (`SPECS.md §7.4`).
pub mod domain {
    use super::{Records, Silent};

    /// Where an entity's claim on a domain is published, under that domain.
    ///
    /// Underscore-prefixed, like every other record this platform reads, so that it cannot collide
    /// with a host somebody owns.
    const AT: &str = "_almena";

    /// The version the shape opens with, as every record here does.
    const SHAPE: &str = "v=1";

    /// Where the identifier sits inside the record.
    const DID: &str = "did=";

    /// What a domain says about itself.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Says {
        /// It names that entity, so the domain's half of the link holds.
        Names,
        /// It answered and names somebody else, or nobody.
        ///
        /// **An answer and not a failure**, and the difference is the same one this whole module
        /// turns on: a domain that says nothing has said *nobody here claims this*, which is a fact
        /// worth acting on, and a domain that could not be asked has said nothing at all.
        DoesNot,
    }

    /// Whether that domain names that entity.
    ///
    /// **A domain may name more than one**, because DNS takes several records at one name — so what
    /// is asked here is whether *this* entity is among them and never which of them wins. The
    /// tie-break belongs to whoever controls the domain, by removing the record they did not mean
    /// to publish, and never to a register choosing (`SPECS.md §7.5`).
    ///
    /// # Errors
    ///
    /// [`Silent`] when the domain did not answer, which is *nobody knows* and must never be read as
    /// *nobody is there*.
    pub async fn asked(records: &impl Records, domain: &str, entity: &str) -> Result<Says, Silent> {
        let published = records
            .text(&format!("{AT}.{}.", domain.trim_end_matches('.')))
            .await?;
        Ok(
            match published.iter().any(|record| claims(record, entity)) {
                true => Says::Names,
                false => Says::DoesNot,
            },
        )
    }

    /// Whether one record claims that entity.
    fn claims(record: &str, entity: &str) -> bool {
        let mut parts = record.split_whitespace();
        if parts.next() != Some(SHAPE) {
            // A shape this build does not read. Passed over rather than guessed at: reading a later
            // version as if it were this one is how a record ends up meaning what nobody wrote.
            return false;
        }
        parts
            .filter_map(|part| part.strip_prefix(DID))
            .any(|named| named == entity)
    }

    #[cfg(test)]
    mod tests {
        use super::{Says, asked};
        use crate::{Records, Silent};
        use std::collections::BTreeMap;

        struct Wrote(BTreeMap<String, Result<Vec<String>, Silent>>);

        impl Records for Wrote {
            async fn text(&self, name: &str) -> Result<Vec<String>, Silent> {
                self.0.get(name).cloned().unwrap_or(Ok(Vec::new()))
            }
        }

        fn zone(entries: &[(&str, Result<Vec<String>, Silent>)]) -> Wrote {
            Wrote(
                entries
                    .iter()
                    .map(|(name, answer)| ((*name).to_owned(), answer.clone()))
                    .collect(),
            )
        }

        const ENTITY: &str = "did:almena:dev:zAnEntity";

        #[tokio::test]
        async fn a_domain_that_names_the_entity_proves_its_half_of_the_link() {
            let published = zone(&[(
                "_almena.almena.network.",
                Ok(vec![format!("v=1 did={ENTITY}")]),
            )]);
            assert_eq!(
                asked(&published, "almena.network", ENTITY).await,
                Ok(Says::Names)
            );
        }

        #[tokio::test]
        async fn a_domain_may_name_more_than_one_and_this_asks_only_about_ours() {
            // DNS takes several records at one name, so an administrator can publish two. The
            // tie-break belongs to whoever controls the domain and never to a register choosing.
            let published = zone(&[(
                "_almena.almena.network.",
                Ok(vec![
                    "v=1 did=did:almena:dev:zSomebodyElse".to_owned(),
                    format!("v=1 did={ENTITY}"),
                ]),
            )]);
            assert_eq!(
                asked(&published, "almena.network", ENTITY).await,
                Ok(Says::Names)
            );
        }

        #[tokio::test]
        async fn a_domain_that_names_somebody_else_has_answered() {
            // *Nobody here claims this* is a fact worth acting on, and is not the same as silence.
            let published = zone(&[(
                "_almena.almena.network.",
                Ok(vec!["v=1 did=did:almena:dev:zSomebodyElse".to_owned()]),
            )]);
            assert_eq!(
                asked(&published, "almena.network", ENTITY).await,
                Ok(Says::DoesNot)
            );
            assert_eq!(
                asked(&zone(&[]), "almena.network", ENTITY).await,
                Ok(Says::DoesNot),
                "and so has one that publishes nothing"
            );
        }

        #[tokio::test]
        async fn a_domain_that_did_not_answer_is_not_a_domain_that_denied_it() {
            assert_eq!(
                asked(
                    &zone(&[("_almena.almena.network.", Err(Silent))]),
                    "almena.network",
                    ENTITY
                )
                .await,
                Err(Silent)
            );
        }

        #[tokio::test]
        async fn a_record_in_a_shape_this_build_does_not_read_is_passed_over() {
            // Reading a later version as if it were this one is how a record ends up meaning
            // something nobody wrote.
            let published = zone(&[(
                "_almena.almena.network.",
                Ok(vec![
                    format!("v=2 did={ENTITY}"),
                    format!("did={ENTITY}"),
                    format!("v=1 owner={ENTITY}"),
                ]),
            )]);
            assert_eq!(
                asked(&published, "almena.network", ENTITY).await,
                Ok(Says::DoesNot)
            );
        }
    }
}
