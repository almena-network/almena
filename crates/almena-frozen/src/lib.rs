//! What has to be settled before a network opens for good, and a way to ask whether it is.
//!
//! A record is append-only (`SPECS.md §4.3`), so a network that opens with a hole in its format
//! keeps that hole for ever: nothing already written can be re-read later, and there is no version
//! two of a history. **Development is re-opened as often as it needs to be and production is
//! opened once** — so between those two facts sits a checklist, and this is it.
//!
//! # It runs; it is not a document
//!
//! Every item here is a probe against this build. Not a sentence somebody ticked: a thing that
//! composes an act, hands it to the reader that would have to apply it, and reports what came back.
//! A checklist made of prose is checked by whoever remembers to read it, which is the same as not
//! being checked at all — and this one is asked by [`almena_node`] before it will open a production
//! network, at the one moment the answer is worth anything.
//!
//! [`almena_node`]: https://docs.rs/almena-node
//!
//! # Three things are asked
//!
//! - **The criticality mark works** (`SPECS.md §4.8`, rule 4). An unknown odd field stops a
//!   reader; an unknown even one does not. Without it, every hole below that is a field is
//!   unprotected, and adding a field to an existing act becomes a silent disaster rather than an
//!   addition.
//! - **The six extension holes of `SPECS.md §18` are present**, each asked in the place it actually
//!   lives. Which holes those are and what covers each is not decided here: it is
//!   [`almena_format::holes::HOLES`], the section's own table, and what this crate adds is a probe
//!   per row. A row with no probe is reported as wanting, so the two cannot drift apart in silence.
//! - **The numbers are versioned.** A figure written as a constant cannot be changed after a
//!   network opens without re-deciding what every act already written meant. A figure written as a
//!   history can: a change appends a setting and everything before it is judged as it always was.
//!
//! # What it does not ask
//!
//! Whether the composition of Almena Government is fit to certify anybody (`SPECS.md §7.1`), which
//! is [`almena_store::government`] and is a gate on **certifying**, not on opening. The two are
//! deliberately separate milestones: a network can exist before there is anybody to trust in it,
//! and pretending otherwise would mean either opening late or certifying early.
//!
//! [`almena_store::government`]: https://docs.rs/almena-store

use almena_format::cbor::Value;
use almena_format::field::{Field, Unintelligible, Vocabulary};
use almena_format::identifier::Network;
use almena_format::operation::create;
use almena_store::kind::Kind;
use almena_time::Epoch;
use std::collections::BTreeMap;

/// One hole of `SPECS.md §18`, its declared protection, and the probe that puts it to the test.
///
/// **The roster is not here.** Which holes exist and what protects each is
/// [`almena_format::holes::HOLES`], which transcribes the section's own table; what this crate adds
/// is the other half — a probe per hole, run against this build. Keeping a second list of six here
/// would be two answers to one question, and the day they differed the checklist would be checking
/// something the format had stopped saying.
///
/// A hole in the table with no probe beside it is **wanting**, not passed over: an unverified hole
/// is exactly the thing a freeze must not wave through.
struct Probe {
    /// The name the table reserved it under.
    declared: &'static str,
    /// What it is called where somebody has to read it.
    called: &'static str,
    /// What went wrong, or nothing.
    put: fn() -> Option<String>,
}

/// One probe per hole of the table, matched to it by the name it was reserved under.
const PROBES: [Probe; 6] = [
    Probe {
        declared: "alcance del sello",
        called: "the scope of a seal",
        put: seal_scope,
    },
    Probe {
        declared: "referencias entre entradas del log",
        called: "references between entries",
        put: a_new_class_can_arrive,
    },
    Probe {
        declared: "anclaje externo de raíces",
        called: "an external anchor for roots",
        put: roots_are_not_entries,
    },
    Probe {
        declared: "tipo de prueba en credenciales",
        called: "the proof type of a credential",
        put: proof_type,
    },
    Probe {
        declared: "método de identificación del emisor",
        called: "how an issuer is identified",
        put: issuer_method,
    },
    Probe {
        declared: "método de una propuesta",
        called: "whether a proposal is open or blind",
        put: proposal_method,
    },
];

/// What one item of the checklist answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answered {
    /// It holds, and here is the probe that says so.
    Holds,
    /// It does not, and this is what went wrong.
    ///
    /// **A network must not open on one of these.** Whatever it names is something that cannot be
    /// corrected once a record exists, only lived with.
    Wanting(String),
}

/// One line of the checklist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// What is being asked.
    pub called: String,
    /// What the format's own table says keeps it open, where it is one of the holes.
    pub kept: Option<&'static [almena_format::holes::Protection]>,
    /// What the probe said.
    pub answered: Answered,
}

impl Item {
    /// Whether this line is one that would stop a network opening.
    #[must_use]
    pub const fn wanting(&self) -> bool {
        matches!(self.answered, Answered::Wanting(_))
    }
}

/// Run every item, in the order they are worth reading in.
///
/// The mark comes first because the holes that are fields rest on it, then the six holes, then the
/// numbers. Nothing short-circuits: an answer that is going to stop a network opening is worth
/// having beside every other, so that whoever reads it fixes what is wrong rather than what was
/// reported first.
#[must_use]
pub fn checklist() -> Vec<Item> {
    let mut items = vec![the_mark_works()];
    items.extend(almena_format::holes::HOLES.into_iter().map(present));
    items.push(the_numbers_are_histories());
    items
}

/// Every item that is not satisfied, which is empty when the format may be frozen.
#[must_use]
pub fn wanting() -> Vec<Item> {
    checklist().into_iter().filter(Item::wanting).collect()
}

/// Whether the format may be frozen, which is what a production network is opened against.
#[must_use]
pub fn frozen() -> bool {
    wanting().is_empty()
}

/// One line, from a name and what a probe said.
fn line(
    called: &str,
    kept: Option<&'static [almena_format::holes::Protection]>,
    wrong: Option<String>,
) -> Item {
    Item {
        called: called.to_owned(),
        kept,
        answered: wrong.map_or(Answered::Holds, Answered::Wanting),
    }
}

/// An act with one payload field in it, for handing to a reader.
fn carrying(field: u64, value: Value) -> almena_format::operation::Operation {
    create(
        Network::Development,
        Kind::HOLDER_CREATE.number(),
        1,
        Epoch::GENESIS,
        BTreeMap::from([(field, value)]),
    )
}

/// **The criticality mark, exercised rather than asserted** (`SPECS.md §4.8`, rule 4).
///
/// An unknown odd field has to stop a reader and an unknown even one has to not. The two halves are
/// equally load-bearing: without the first, a node applies an act it did not understand; without
/// the second, no field can ever be added at all.
fn the_mark_works() -> Item {
    const KNOWN: &[Field] = &[Field::new(2)];
    let vocabulary = Vocabulary::of(KNOWN);

    let odd = carrying(1_001, Value::Uint(1));
    let even = carrying(1_000, Value::Uint(1));

    let wrong = match (odd.understood(vocabulary), even.understood(vocabulary)) {
        (Err(Unintelligible::Field(_)), Ok(())) => None,
        (Ok(()), _) => {
            Some("an unknown odd field is passed over instead of stopping a reader".to_owned())
        }
        (_, Err(_)) => {
            Some("an unknown even field stops a reader, so no field could ever be added".to_owned())
        }
        (Err(other), Ok(())) => Some(format!("an unknown odd field is refused as {other:?}")),
    };
    line("the criticality mark decides by parity", None, wrong)
}

/// Ask one declared hole whether it is really there.
fn present(hole: almena_format::holes::Hole) -> Item {
    let Some(probe) = PROBES.iter().find(|probe| probe.declared == hole.name) else {
        // A hole the table reserves and nothing here checks. Reported as wanting rather than
        // skipped: an unverified hole is exactly what a freeze must not wave through, and the
        // silence of a missing probe reads identically to a passing one.
        return line(
            hole.name,
            Some(hole.protection),
            Some(format!(
                "{} is reserved and nothing here puts it to the test",
                hole.name
            )),
        );
    };
    line(probe.called, Some(hole.protection), (probe.put)())
}

/// **A certification carrying a field this build has no meaning for is unintelligible.**
///
/// The scope of a seal is `SPECS.md §18`'s own example of a critical field, and the failure it
/// names is exact: a certification with a scope read as a certification without limits. So the
/// probe is a certification act carrying an odd number the vocabulary does not know, and what it
/// has to produce is a refusal to apply.
fn seal_scope() -> Option<String> {
    let vocabulary = almena_store::certification::vocabulary();
    // **A missing free number is a failure and never a pass.** Reported rather than shrugged at:
    // a hole with nowhere left to put its field is a hole that has closed.
    let Some(free) = (1..512_u64)
        .map(|number| number * 2 - 1)
        .find(|number| !vocabulary.fields.contains(&Field::new(*number)))
    else {
        return Some("a certification has no odd field number left for a scope".to_owned());
    };

    let act = create(
        Network::Development,
        Kind::CERTIFICATION_ISSUE.number(),
        1,
        Epoch::GENESIS,
        BTreeMap::from([(free, Value::Text("everything and nothing".to_owned()))]),
    );
    match act.understood(vocabulary) {
        Err(Unintelligible::Field(_)) => None,
        _ => Some(format!(
            "a certification carrying field {free} is applied by a reader that has no meaning for it"
        )),
    }
}

/// **An act of a class this build has never heard of is kept, and its object resolves to nothing.**
///
/// This is `SPECS.md §4.8` rules 1 and 2 together, and `SPECS.md §18` leans on both for the hole it
/// protects with an unknown type: the annotation that lets one entry refer to another is a class of
/// its own, so a build that refused unknown classes would refuse the very thing that hole is for —
/// and would hold a different history from every build that came after it.
fn a_new_class_can_arrive() -> Option<String> {
    use almena_store::chain::{Answer, Objects, Reason};

    // As above: no free number left is a failure, not something to pass over.
    let Some(free) =
        (1..1_024_u64).find(|number| Kind::new(*number).is_some_and(|kind| !kind.known()))
    else {
        return Some(
            "there is no act number left for a class that has not been designed yet".to_owned(),
        );
    };
    let signing = almena_suite::ed25519::SigningKey::from_secret([37; 32]);
    let mut act = create(
        Network::Development,
        free,
        1,
        Epoch::GENESIS,
        BTreeMap::from([(1, Value::Text("whatever it says".to_owned()))]),
    );
    let signature = signing.sign(&act.signing_bytes());
    act.signatures.push(almena_format::operation::Signed {
        by: act.object.clone(),
        key: signing.verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });

    let mut objects = Objects::new();
    if let Err(refused) = objects.admit(&act, Epoch::GENESIS) {
        return Some(format!(
            "an act of class {free} is refused as {refused:?} instead of being kept and passed on"
        ));
    }
    match objects.resolve(act.object.name()) {
        Answer::CannotResolve(Reason::Unintelligible) => None,
        other => Some(format!(
            "an object of class {free} answers {other:?} instead of saying it cannot be resolved"
        )),
    }
}

/// **A root is not an entry, and firmness only ever counts what it was handed.**
///
/// `SPECS.md §4.12` keeps roots out of the log — a root per node per epoch would be pure accounting
/// in the one thing everybody stores for ever — and `SPECS.md §18` builds on that: an anchor is a
/// separate artefact addressed by hash, so a reader that ignores one **loses firmness and never
/// misstates it**. Two things have to hold for that, and both are asked.
///
/// First, a root's own bytes are not an act. If they could be read as one, an anchor would be a
/// field on something in the record and would need the criticality mark instead. Second, firmness
/// is a count of independent trees: handed nothing, it says nought rather than guessing, which is
/// precisely what makes ignoring an anchor safe.
fn roots_are_not_entries() -> Option<String> {
    use almena_format::identifier::{Did, Name};
    use almena_store::root::Root;

    let network = Name::of(b"a network");
    let root = Root {
        network: network.clone(),
        node: Did::new(Network::Development, Name::of(b"a node")),
        epoch: Epoch::GENESIS,
        size: 1,
        root: almena_suite::digest::Digest::of(b"a tree"),
    };
    let bytes = root.to_bytes();
    if Root::read(&bytes).is_none() {
        return Some("a root does not read back as the artefact it was signed as".to_owned());
    }
    if almena_format::cbor::read(&bytes)
        .ok()
        .and_then(|value| almena_format::operation::read(&value))
        .is_some()
    {
        return Some(
            "a root reads as an act, so an anchor would be a field in the record".to_owned(),
        );
    }

    let nothing = almena_store::firm::footing(&carrying(2, Value::Uint(1)), &network, &[], 3);
    (nothing.trees != 0 || nothing.roots.is_some())
        .then(|| "firmness claims trees nobody handed it".to_owned())
}

/// **The proof type is present, closed, and refused when unknown** (`SPECS.md §9.1`, `§18`).
///
/// Closed because a field that has travelled from day one is known to every reader, so parity never
/// fires for it and an unknown value would otherwise be read as the nearest known one. Refused
/// rather than defaulted, because a verifier that supposed the type it knows would be making the
/// implicit assumption `SPECS.md §9.1` exists to forbid.
fn proof_type() -> Option<String> {
    if almena_credential::Proof::of("something-nobody-has-defined").is_some() {
        return Some("a proof type this build does not know is read as one it does".to_owned());
    }
    (almena_credential::Proof::of(almena_credential::Proof::Disclosure.name())
        != Some(almena_credential::Proof::Disclosure))
    .then(|| "the one proof type this build knows does not read back".to_owned())
}

/// **The issuer's identification method, on the same terms** (`SPECS.md §9.1`, `§18`).
///
/// With one thing more, which is why it is asked separately: the method is read *before* anything
/// has been verified, so what decides which are acceptable is the verifier's own list and never the
/// list a credential proposes.
fn issuer_method() -> Option<String> {
    if almena_credential::Method::of("x509-or-whatever-comes-next").is_some() {
        return Some(
            "an identification method this build does not know is read as one it does".to_owned(),
        );
    }
    (almena_credential::Method::of(almena_credential::Method::Almena.name())
        != Some(almena_credential::Method::Almena))
    .then(|| "the one identification method this build knows does not read back".to_owned())
}

/// **A proposal's method — open or blind — is a critical field with a closed vocabulary.**
///
/// The vote of `SPECS.md §14` is not built: it waits for the first public vote Almena convenes, and
/// building it early is the thing `SPECS.md §15.2` exists to prevent. So what is asked here is what
/// is asked of an unwritten act: that the mechanism it will need already works, and that nothing
/// has been written under that class in the meantime — because a proposal written before the field
/// existed would be one whose method a later reader had to guess.
fn proposal_method() -> Option<String> {
    const METHOD: Field = Field::new(1);
    const KNOWN: &[Field] = &[METHOD];
    const OPEN: &[Value] = &[Value::Uint(1)];
    const CLOSED: &[(Field, &[Value])] = &[(METHOD, OPEN)];

    if !METHOD.is_critical() {
        return Some("the field a proposal's method would take is not a critical one".to_owned());
    }
    let vocabulary = Vocabulary::with_closed(KNOWN, CLOSED);
    let blind = create(
        Network::Development,
        Kind::PROPOSAL_OPEN.number(),
        1,
        Epoch::GENESIS,
        BTreeMap::from([(METHOD.number(), Value::Uint(2))]),
    );
    match blind.understood(vocabulary) {
        Err(Unintelligible::Value(_)) => None,
        _ => Some(
            "a value outside a closed vocabulary is read as the one this build knows".to_owned(),
        ),
    }
}

/// **Every number the protocol rests on is a history and not a constant.**
///
/// A constant cannot be changed after a network opens without re-deciding what every act already
/// written meant, and there is no way for a reader to tell which reading a node used. A history
/// can: a change appends a setting, and an act is judged by what was in force when it was issued.
///
/// The deadlines are walked from their own roster, so a deadline added later is covered without
/// anybody remembering to add it here.
fn the_numbers_are_histories() -> Item {
    let mut wrong: Option<String> = None;
    let numbers = almena_time::deadline::ALL.into_iter().chain([
        ("summarise every", almena_store::parameter::SUMMARISE_EVERY),
        (
            "control pending most",
            almena_store::parameter::CONTROL_PENDING_MOST,
        ),
        (
            "copies of a status list",
            almena_store::share::COPIES_OF_A_STATUS_LIST,
        ),
        ("copies of history", almena_store::share::COPIES_OF_HISTORY),
    ]);

    for (called, parameter) in numbers {
        let settings = parameter.settings();
        if settings.first().map(|(from, _)| *from) != Some(Epoch::GENESIS) {
            wrong = Some(format!(
                "{called} has no answer for the epochs before its first setting"
            ));
            break;
        }
        if !settings.windows(2).all(|pair| pair[0].0 < pair[1].0) {
            wrong = Some(format!(
                "{called} has settings out of order, so one decides what an earlier act meant"
            ));
            break;
        }
    }
    line("the numbers are versioned and run forwards", None, wrong)
}

#[cfg(test)]
mod tests {
    use super::{Answered, PROBES, checklist, frozen, wanting};
    use almena_format::holes::{HOLES, Protection};

    #[test]
    fn the_format_this_build_writes_may_be_frozen() {
        // **The check the whole crate exists for.** Everything under it says why one item holds;
        // this says that all of them do, which is the question a production genesis puts.
        let wanting = wanting();
        assert!(
            wanting.is_empty(),
            "the format is not ready to freeze: {wanting:#?}"
        );
        assert!(frozen());
    }

    #[test]
    fn every_hole_the_format_reserves_has_a_probe_beside_it() {
        // **The two halves held to each other.** The table says which holes exist and what covers
        // each; this crate says whether they are really there. Two independent lists of six would
        // be two answers to one question, and the day they differed the checklist would be
        // checking something the format had stopped saying.
        for hole in HOLES {
            assert!(
                PROBES.iter().any(|probe| probe.declared == hole.name),
                "{} is reserved and nothing puts it to the test",
                hole.name
            );
        }
        for probe in PROBES {
            assert!(
                HOLES.iter().any(|hole| hole.name == probe.declared),
                "{} is probed and is not a hole the format reserves",
                probe.declared
            );
        }
    }

    #[test]
    fn each_hole_is_on_the_checklist_under_the_protection_it_was_reserved_with() {
        // Six, and the point of carrying the protection beside each is that they are not all the
        // same kind of thing — believing they were is the mistake the section itself corrects.
        let items = checklist();
        for (hole, probe) in HOLES.into_iter().zip(PROBES) {
            let found = items
                .iter()
                .find(|item| item.called == probe.called)
                .unwrap_or_else(|| panic!("{} is not on the checklist", hole.name));
            assert_eq!(found.kept, Some(hole.protection), "{}", hole.name);
            assert_eq!(found.answered, Answered::Holds, "{}", hole.name);
        }
        assert_eq!(
            items.iter().filter(|item| item.kept.is_none()).count(),
            2,
            "the mark and the numbers, which are not holes"
        );
    }

    #[test]
    fn all_three_mechanisms_are_in_use() {
        // The section's own correction, made checkable: reserving numbers for holes that are not
        // fields and leaving unprotected the ones that are is what happens when they are treated
        // as one mechanism.
        let used: Vec<Protection> = checklist()
            .into_iter()
            .filter_map(|item| item.kept)
            .flatten()
            .copied()
            .collect();
        for mechanism in [
            Protection::Criticality,
            Protection::UnknownType,
            Protection::ClosedVocabulary,
        ] {
            assert!(used.contains(&mechanism), "{mechanism:?} is unused");
        }
    }

    #[test]
    fn nothing_short_circuits() {
        // Every line is asked, so that whoever reads a failure fixes what is wrong rather than what
        // happened to be reported first.
        assert_eq!(checklist().len(), HOLES.len() + 2);
    }
}
