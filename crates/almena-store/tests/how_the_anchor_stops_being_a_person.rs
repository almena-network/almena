//! Almena Government growing out of the one key it was born with.
//!
//! **The milestone `SPECS.md §7.1` puts before certifying anybody from outside**, and it is a
//! milestone rather than a date: nothing forces a large set of owners in order to write code, and
//! everything forces one before a third party stakes anything on that seal.
//!
//! What is checked here is that it is possible at all, that it uses the same mechanism as any other
//! organisation, and — the part that would be easy to get wrong — that **the genesis key stops
//! deciding the moment there is somebody to count**. A key that went on signing beside a threshold
//! would make naming owners a performance.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::{BTreeMap, BTreeSet};

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, Signed, create};
use almena_store::chain::{Answer, Objects, Refused, State};
use almena_store::entity::field;
use almena_store::genesis::{Opening, Which, open};
use almena_store::government::{self, Wanting};
use almena_store::kind::Kind;
use almena_suite::{ed25519, p256};
use almena_time::Epoch;

/// After everything the words alone asked for has landed (`SPECS.md §11.12`).
fn settled() -> Epoch {
    Epoch::new(almena_time::deadline::CONTROL_KEY_WAIT.now() + 1)
}

fn words(seed: u8) -> ed25519::SigningKey {
    ed25519::SigningKey::from_secret([seed; 32])
}

fn device(seed: u8) -> p256::SigningKey {
    p256::SigningKey::from_secret([seed; 32]).expect("a valid scalar")
}

/// One person's account, with one device on it.
fn a_person(objects: &mut Objects, seed: u8) -> Did {
    let public = words(seed).verifying_key().bytes();
    let mut created = create(
        Network::Production,
        Kind::HOLDER_CREATE.number(),
        1,
        Epoch::GENESIS,
        BTreeMap::from([(1, Value::Bytes(public.to_vec()))]),
    );
    let signature = words(seed).sign(&created.signing_bytes());
    created.signatures.push(Signed {
        by: created.object.clone(),
        key: public.to_vec(),
        signature: signature.bytes(),
    });
    let whose = created.object.clone();
    objects
        .admit(&created, Epoch::GENESIS)
        .expect("the account");

    let mut adding = Operation {
        object: whose.clone(),
        previous: Some(created.called()),
        kind: Kind::HOLDER_ADD_DEVICE.number(),
        version: 1,
        issued: Epoch::GENESIS,
        payload: BTreeMap::from([(
            1,
            Value::Bytes(device(seed).verifying_key().bytes().to_vec()),
        )]),
        signatures: Vec::new(),
    };
    let signature = words(seed).sign(&adding.signing_bytes());
    adding.signatures.push(Signed {
        by: whose.clone(),
        key: public.to_vec(),
        signature: signature.bytes(),
    });
    objects.admit(&adding, Epoch::GENESIS).expect("the asking");
    whose
}

/// The key the genesis gives Almena Government.
fn anchor() -> ed25519::SigningKey {
    ed25519::SigningKey::from_secret([200; 32])
}

/// A production network, opened, with its government resolving.
fn a_network() -> (Objects, Did, Name) {
    let opened = open(
        &Opening {
            which: Which::Production,
            beginning: Epoch::GENESIS,
            began: 1_800_000_000,
        },
        &[],
        false,
        &anchor(),
    )
    .expect("nobody to join");
    let mut objects = Objects::new();
    objects
        .admit(&opened.operation, Epoch::GENESIS)
        .expect("the record starts");
    (objects, opened.government, opened.operation.called())
}

/// One act on Almena Government's own chain.
fn on(government: &Did, head: &Name, kind: Kind, payload: BTreeMap<u64, Value>) -> Operation {
    Operation {
        object: government.clone(),
        previous: Some(head.clone()),
        kind: kind.number(),
        version: 1,
        issued: settled(),
        payload,
        signatures: Vec::new(),
    }
}

/// Signed by the key the genesis gave it.
fn by_the_anchor(operation: &mut Operation, government: &Did) {
    let signature = anchor().sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: government.clone(),
        key: anchor().verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });
}

/// Signed by those people, each from one of their devices.
fn by(operation: &mut Operation, owners: &[&Did], seeds: &[u8]) {
    let over = operation.signing_bytes();
    for (who, seed) in owners.iter().zip(seeds) {
        operation.signatures.push(Signed {
            by: (*who).clone(),
            key: device(*seed).verifying_key().bytes().to_vec(),
            signature: device(*seed).sign(&over).bytes(),
        });
    }
}

/// What the record says Almena Government is right now.
fn composition(objects: &Objects, government: &Did) -> almena_store::entity::Entity {
    match objects.resolve(government.name()) {
        Answer::Here(State::Government { body, .. }) => *body,
        other => panic!("the anchor has to resolve, got {other:?}"),
    }
}

/// Name each of them an owner in turn, and hand back the head of the chain.
///
/// **The first by the key and every one after by the owners already named**, which is the whole
/// shape of the bootstrap: the key is what signs while there is nobody to count, and stops the
/// moment there is.
fn naming_all_of_them(objects: &mut Objects, government: &Did, from: Name, people: &[Did]) -> Name {
    let mut head = from;
    for (at, person) in people.iter().enumerate() {
        let mut naming = on(
            government,
            &head,
            Kind::ENTITY_ADD_OWNER,
            BTreeMap::from([(field::WHO, Value::Text(person.to_string()))]),
        );
        if at == 0 {
            by_the_anchor(&mut naming, government);
        } else {
            let already: Vec<&Did> = people[..at].iter().collect();
            let seeds: Vec<u8> = (1..=u8::try_from(at).expect("a handful")).collect();
            by(&mut naming, &already, &seeds);
        }
        objects
            .admit(&naming, settled())
            .unwrap_or_else(|why| panic!("owner {at} should be named: {why:?}"));
        head = naming.called();
    }
    head
}

/// Raise the thresholds, signed by every owner there is.
fn raising(
    objects: &mut Objects,
    government: &Did,
    head: &Name,
    people: &[Did],
    to: (u64, u64, u64),
) -> Name {
    let mut act = on(
        government,
        head,
        Kind::ENTITY_SET_THRESHOLD,
        BTreeMap::from([
            (field::ROUTINE, Value::Uint(to.0)),
            (field::SEALING, Value::Uint(to.1)),
            (field::GOVERNANCE, Value::Uint(to.2)),
        ]),
    );
    let everyone: Vec<&Did> = people.iter().collect();
    let seeds: Vec<u8> = (1..=u8::try_from(people.len()).expect("a handful")).collect();
    by(&mut act, &everyone, &seeds);
    objects.admit(&act, settled()).expect("the thresholds");
    act.called()
}

#[test]
fn a_network_opens_with_its_anchor_in_one_pair_of_hands_and_says_so() {
    // `SPECS.md §7.9`: a self-signed root is the only way a web of trust starts. What would be
    // wrong is not saying it, which is why the reading exists at all.
    let (objects, government, _) = a_network();
    let body = composition(&objects, &government);

    assert!(government::one_pair_of_hands(&body));
    assert_eq!(
        government::counted(&body),
        vec![Wanting::NobodyIsAnOwnerYet],
        "one thing to say, and it is the one worth saying"
    );
}

#[test]
fn the_anchor_names_its_first_owner_with_the_key_it_was_born_with() {
    // There is nobody to count yet, so counting a set that does not exist would be counting nothing
    // at all. The key is what signs, and this is the only stretch in which it does.
    let (mut objects, government, genesis) = a_network();
    let first = a_person(&mut objects, 1);

    let mut naming = on(
        &government,
        &genesis,
        Kind::ENTITY_ADD_OWNER,
        BTreeMap::from([(field::WHO, Value::Text(first.to_string()))]),
    );
    by_the_anchor(&mut naming, &government);
    objects.admit(&naming, settled()).expect("the first owner");

    let body = composition(&objects, &government);
    assert_eq!(body.owners, BTreeSet::from([first]));
    assert!(!government::one_pair_of_hands(&body));
}

#[test]
fn once_there_is_an_owner_the_genesis_key_no_longer_speaks() {
    // **The part that would be easy to get wrong.** A key that went on signing beside a threshold
    // would make naming owners a performance: the set would exist and the one key would still be
    // able to do everything it could do before.
    let (mut objects, government, genesis) = a_network();
    let first = a_person(&mut objects, 1);
    let second = a_person(&mut objects, 2);

    let mut naming = on(
        &government,
        &genesis,
        Kind::ENTITY_ADD_OWNER,
        BTreeMap::from([(field::WHO, Value::Text(first.to_string()))]),
    );
    by_the_anchor(&mut naming, &government);
    objects.admit(&naming, settled()).expect("the first owner");

    let mut again = on(
        &government,
        &naming.called(),
        Kind::ENTITY_ADD_OWNER,
        BTreeMap::from([(field::WHO, Value::Text(second.to_string()))]),
    );
    by_the_anchor(&mut again, &government);
    assert_eq!(
        objects.admit(&again, settled()),
        Err(Refused::NotAuthorised),
        "the key that opened the network is not a way round the owners it named"
    );
}

#[test]
fn it_grows_to_the_composition_the_section_asks_for_and_the_reading_says_when() {
    // The whole of `SPECS.md §7.1`'s record half, walked: five owners, sealing at three, governance
    // at a majority — each act signed by whoever the threshold in force at that moment says.
    let (mut objects, government, genesis) = a_network();
    let people: Vec<Did> = (1..=5).map(|seed| a_person(&mut objects, seed)).collect();
    let head = naming_all_of_them(&mut objects, &government, genesis, &people);

    // Five owners at one and one is not a composition; the reading says exactly which parts.
    let body = composition(&objects, &government);
    assert_eq!(body.owners.len(), 5);
    assert_eq!(
        government::counted(&body),
        vec![
            Wanting::SealingTooLow { is: 1 },
            Wanting::GovernanceIsNotAMajority { is: 1, of: 5 },
        ]
    );

    raising(&mut objects, &government, &head, &people, (1, 3, 3));
    let body = composition(&objects, &government);
    assert!(
        government::counted(&body).is_empty(),
        "and now the record says what `SPECS.md §7.1` asks of it"
    );

    // And the fourth criterion is not in here, because it cannot be: owners are root identifiers
    // and root identifiers are anonymous. With nothing declared they count as five places; with a
    // declaration that puts them in one company they count as one, and only the declaration knows.
    assert!(government::fit(&body, &BTreeMap::new()).is_empty());
    let all_at_one_desk = body
        .owners
        .iter()
        .map(|owner| {
            (
                owner.clone(),
                government::Declared {
                    organisation: "One Company".to_owned(),
                    jurisdiction: "one country".to_owned(),
                },
            )
        })
        .collect();
    assert_eq!(
        government::fit(&body, &all_at_one_desk).len(),
        2,
        "one company and one country, which fail differently and are said separately"
    );
}

#[test]
fn three_of_five_cannot_change_who_governs_once_it_takes_a_majority() {
    // What raising the threshold bought, checked rather than assumed: a threshold that is not a
    // threshold is the failure every part of `SPECS.md §8` is built to prevent.
    let (mut objects, government, genesis) = a_network();
    let people: Vec<Did> = (1..=5).map(|seed| a_person(&mut objects, seed)).collect();
    let head = naming_all_of_them(&mut objects, &government, genesis, &people);
    let head = raising(&mut objects, &government, &head, &people, (1, 3, 4));

    let mut three = on(
        &government,
        &head,
        Kind::ENTITY_REMOVE_OWNER,
        BTreeMap::from([(field::WHO, Value::Text(people[4].to_string()))]),
    );
    by(
        &mut three,
        &[&people[0], &people[1], &people[2]],
        &[1, 2, 3],
    );
    assert_eq!(
        objects.admit(&three, settled()),
        Err(Refused::NotAuthorised),
        "three of five is not more than half of five"
    );
}
