//! What a node holds of a status list, and what it refuses to hold.
//!
//! # A node replicating one never reads it
//!
//! `SPECS.md §10.2` hosts the lists on the network, addressed by hash, and puts only the hash in the
//! log. So a node hashes the bytes it was handed and keeps them if the record names that hash —
//! and it never decodes a bitstring. That is rule one of `SPECS.md §4.8` in the place it would be
//! easiest to break: replication does not require understanding, and a change to the list format is
//! a change to issuers and verifiers and to nobody else.
//!
//! # And the obligation ends by expiry, with no operation at all
//!
//! Every credential a list covers carries its expiry **signed inside it**, so when the window
//! passes they are all dead and the list is thrown away whole rather than pruned. Nothing has to be
//! consulted and nobody has to say so — the same shape `SPECS.md §12.1` already has for a closed
//! organisation.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, Signed, create};
use almena_node::{Node, NotKept, Opening};
use almena_status::list::List;
use almena_store::element::field as element;
use almena_store::entity::field as entity;
use almena_store::genesis::Which;
use almena_store::kind::Kind;
use almena_store::status::field as status;
use almena_suite::{ed25519, p256};
use almena_time::cohort::Cohort;
use almena_time::{Clock, Epoch};

/// After everything the words alone asked for has landed (`SPECS.md §11.12`).
fn settled() -> Epoch {
    Epoch::new(almena_time::deadline::CONTROL_KEY_WAIT.now() + 1)
}

/// The instant this network's epoch zero begins.
const BEGAN: u64 = 1_800_000_000;

fn words(seed: u8) -> ed25519::SigningKey {
    ed25519::SigningKey::from_secret([seed; 32])
}

fn device(seed: u8) -> p256::SigningKey {
    p256::SigningKey::from_secret([seed; 32]).expect("a valid scalar")
}

fn a_node() -> Node {
    Node::open(
        &Opening {
            which: Which::Development,
            beginning: Epoch::GENESIS,
            began: BEGAN,
        },
        &[],
        &words(5),
        words(6),
    )
    .expect("nobody to join")
}

/// Sign an act as an account or an element signs its own.
fn by(operation: &mut Operation, who: &Did, key: &ed25519::SigningKey) {
    let signature = key.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: who.clone(),
        key: key.verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });
}

/// Sign an act as an owner would, from a device.
fn as_owner(operation: &mut Operation, owner: &Did, holds: u8) {
    let signature = device(holds).sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: owner.clone(),
        key: device(holds).verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });
}

/// The act that opens a status list, unsigned.
fn opening(empty: &List, issuer: &Did, cohort: Cohort) -> Operation {
    create(
        Network::Development,
        Kind::STATUS_LIST_PUBLISH_VERSION.number(),
        1,
        settled(),
        BTreeMap::from([
            (
                status::VERSION,
                Value::Bytes(empty.version().bytes().to_vec()),
            ),
            (status::COHORT, Value::Text(cohort.written())),
            (status::BY, Value::Text(issuer.to_string())),
        ]),
    )
}

/// The one owner an organisation was founded with.
fn owner_of(node: &Node, organisation: &Did) -> Option<Did> {
    match node.resolve(organisation.name(), settled()).answer {
        almena_store::chain::Answer::Here(almena_store::chain::State::Entity(held)) => {
            held.owners.iter().next().cloned()
        }
        _ => None,
    }
}

/// A record with an issuer in it, and the list object it opened.
struct Serving {
    node: Node,
    /// The issuer element.
    issuer: Did,
    /// The key it signs its own acts with.
    key: ed25519::SigningKey,
    /// The list it opened.
    list: Did,
    /// The act that opened it.
    opened: Name,
    /// The window the list covers.
    cohort: Cohort,
}

/// Somebody with an account and one device, and their organisation.
fn an_organisation(node: &mut Node) -> Did {
    let control = words(9);
    let mut account = create(
        Network::Development,
        Kind::HOLDER_CREATE.number(),
        1,
        Epoch::GENESIS,
        BTreeMap::from([(1, Value::Bytes(control.verifying_key().bytes().to_vec()))]),
    );
    let owner = account.object.clone();
    by(&mut account, &owner, &control);
    node.submit(&account, Epoch::GENESIS).expect("the account");

    let mut adding = Operation {
        object: owner.clone(),
        previous: Some(account.called()),
        kind: Kind::HOLDER_ADD_DEVICE.number(),
        version: 1,
        issued: Epoch::GENESIS,
        payload: BTreeMap::from([(1, Value::Bytes(device(22).verifying_key().bytes().to_vec()))]),
        signatures: Vec::new(),
    };
    by(&mut adding, &owner, &control);
    node.submit(&adding, Epoch::GENESIS).expect("the device");

    let mut founded = create(
        Network::Development,
        Kind::ENTITY_CREATE.number(),
        1,
        settled(),
        BTreeMap::from([
            (entity::KEY, Value::Bytes(vec![22; 32])),
            (entity::WHO, Value::Text(owner.to_string())),
            (entity::ROUTINE, Value::Uint(1)),
            (entity::SEALING, Value::Uint(1)),
            (entity::GOVERNANCE, Value::Uint(1)),
        ]),
    );
    as_owner(&mut founded, &owner, 22);
    node.submit(&founded, settled()).expect("founded");
    founded.object
}

/// An issuer, hung from an organisation, with one status list open.
fn serving(empty: &List) -> Serving {
    let mut node = a_node();
    let organisation = an_organisation(&mut node);
    // The one owner it was founded with, whose device signs for it.
    let Some(owner) = owner_of(&node, &organisation) else {
        panic!("it has one")
    };

    // The issuer element, whose own key is what publishes a list.
    let key = words(33);
    let mut hung = create(
        Network::Development,
        Kind::ISSUER_CREATE.number(),
        1,
        settled(),
        BTreeMap::from([
            (
                element::KEY,
                Value::Bytes(key.verifying_key().bytes().to_vec()),
            ),
            (element::OF, Value::Text(organisation.to_string())),
            (element::ROLE, Value::Uint(1)),
        ]),
    );
    as_owner(&mut hung, &owner, 22);
    node.submit(&hung, settled()).expect("hung");
    let issuer = hung.object.clone();

    let cohort = Cohort::of(
        &Clock::from_unix(BEGAN).expect("an instant"),
        Epoch::new(20_000),
    )
    .expect("a window");
    let mut opening = opening(empty, &issuer, cohort);
    by(&mut opening, &issuer, &key);
    node.submit(&opening, settled()).expect("opened");

    Serving {
        node,
        issuer,
        key,
        list: opening.object.clone(),
        opened: opening.called(),
        cohort,
    }
}

#[test]
fn a_node_keeps_the_version_the_record_names_and_nothing_else() {
    // **The whole of the check.** Bytes nobody's record names are bytes somebody wanted stored, and
    // a node that took those is one anybody can fill under the name of a service.
    let empty = List::empty();
    let mut serving = serving(&empty);

    assert_eq!(
        serving
            .node
            .keep_list(empty.written().into_bytes(), settled()),
        Ok(serving.list.name().clone())
    );
    assert_eq!(
        serving.node.list(empty.version().bytes()),
        Some(empty.written().into_bytes()),
        "and it serves back exactly what it was handed"
    );

    let invented = b"anything at all".to_vec();
    assert_eq!(
        serving.node.keep_list(invented, settled()),
        Err(NotKept::NotNamed)
    );
}

#[test]
fn only_the_version_in_force_is_kept_and_the_one_before_it_is_let_go_of() {
    // `SPECS.md §10.2` keeps **no history of contents**. An older version is not something to
    // store: it is something no verifier may use, because the hash in the record says so.
    let empty = List::empty();
    let mut serving = serving(&empty);
    serving
        .node
        .keep_list(empty.written().into_bytes(), settled())
        .expect("kept");

    let mut revoked = List::empty();
    revoked.revoke(4242);
    let mut again = Operation {
        object: serving.list.clone(),
        previous: Some(serving.opened.clone()),
        kind: Kind::STATUS_LIST_PUBLISH_VERSION.number(),
        version: 1,
        issued: Epoch::new(1_000),
        payload: BTreeMap::from([(
            status::VERSION,
            Value::Bytes(revoked.version().bytes().to_vec()),
        )]),
        signatures: Vec::new(),
    };
    let issuer = serving.issuer.clone();
    by(&mut again, &issuer, &serving.key);
    serving
        .node
        .submit(&again, Epoch::new(1_000))
        .expect("revoked");

    // The old bytes are no longer the version in force, so a node that still had them lets go.
    serving.node.forget_past_lists(Epoch::new(1_000));
    assert_eq!(serving.node.list(empty.version().bytes()), None);
    assert_eq!(
        serving
            .node
            .keep_list(empty.written().into_bytes(), Epoch::new(1_000)),
        Err(NotKept::NotNamed),
        "and it will not take them again"
    );
    assert!(
        serving
            .node
            .keep_list(revoked.written().into_bytes(), Epoch::new(1_000))
            .is_ok()
    );
}

#[test]
fn a_list_whose_window_has_passed_is_thrown_away_whole() {
    // **No operation, and nothing to consult.** Every credential it covered carries an expiry
    // signed inside it, so all of them are dead and the credential itself proves it.
    let empty = List::empty();
    let mut serving = serving(&empty);
    serving
        .node
        .keep_list(empty.written().into_bytes(), settled())
        .expect("kept");

    let over = serving
        .cohort
        .over(&Clock::from_unix(BEGAN).expect("an instant"))
        .expect("a calendar");
    serving.node.forget_past_lists(over);
    assert_eq!(serving.node.list(empty.version().bytes()), None);
    assert_eq!(
        serving.node.keep_list(empty.written().into_bytes(), over),
        Err(NotKept::WindowPast),
        "and taking it back would be holding a copy of something nobody may use"
    );
}
