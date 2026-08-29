//! Two people, one organisation, and a node that counts.
//!
//! **The phase's exit criterion where it is actually decided.** Everything here goes through
//! `Objects` the way an act handed to a node does: the owners are real accounts with real devices,
//! resolved from the record rather than named in the act, and the threshold is counted against the
//! set standing at the act's own moment.
//!
//! What it proves is the property the whole of `SPECS.md §8` rests on — **that a threshold is a
//! threshold**. An entity that says two owners have to agree is one where a single owner, signing
//! perfectly well with a key their own chain authorises, cannot move it.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, Signed, create};
use almena_store::chain::{Admitted, Answer, Objects, Refused, State};
use almena_store::entity::field;
use almena_store::kind::Kind;
use almena_suite::{ed25519, p256};
use almena_time::Epoch;

/// After everything the words alone asked for has landed (`SPECS.md §11.12`).
fn settled() -> Epoch {
    Epoch::new(almena_time::deadline::CONTROL_KEY_WAIT.count() + 1)
}

fn words(seed: u8) -> ed25519::SigningKey {
    ed25519::SigningKey::from_secret([seed; 32])
}

fn device(seed: u8) -> p256::SigningKey {
    p256::SigningKey::from_secret([seed; 32]).expect("a valid scalar")
}

/// One person's account, with one device on it, as of [`settled`].
fn a_person(objects: &mut Objects, control: u8, holds: u8) -> Did {
    let public = words(control).verifying_key().bytes();
    let mut created = create(
        Network::Development,
        Kind::HOLDER_CREATE.number(),
        1,
        Epoch::GENESIS,
        BTreeMap::from([(1, Value::Bytes(public.to_vec()))]),
    );
    let signature = words(control).sign(&created.signing_bytes());
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
            Value::Bytes(device(holds).verifying_key().bytes().to_vec()),
        )]),
        signatures: Vec::new(),
    };
    let signature = words(control).sign(&adding.signing_bytes());
    adding.signatures.push(Signed {
        by: whose.clone(),
        key: public.to_vec(),
        signature: signature.bytes(),
    });
    objects.admit(&adding, Epoch::GENESIS).expect("the asking");
    whose
}

/// The act that creates an entity, named by its first owner.
fn founding(owner: &Did, thresholds: (u64, u64, u64)) -> Operation {
    create(
        Network::Development,
        Kind::ENTITY_CREATE.number(),
        1,
        settled(),
        BTreeMap::from([
            (field::KEY, Value::Bytes(vec![9; 32])),
            (field::WHO, Value::Text(owner.to_string())),
            (field::ROUTINE, Value::Uint(thresholds.0)),
            (field::SEALING, Value::Uint(thresholds.1)),
            (field::GOVERNANCE, Value::Uint(thresholds.2)),
        ]),
    )
}

/// One act on an entity's chain.
fn on(entity: &Did, head: &Name, kind: Kind, payload: BTreeMap<u64, Value>) -> Operation {
    Operation {
        object: entity.clone(),
        previous: Some(head.clone()),
        kind: kind.number(),
        version: 1,
        issued: settled(),
        payload,
        signatures: Vec::new(),
    }
}

/// Sign it, as each of those people would from one of their devices.
fn signed_by(operation: &mut Operation, by: &[(&Did, u8)]) {
    let over = operation.signing_bytes();
    for (who, holds) in by {
        operation.signatures.push(Signed {
            by: (*who).clone(),
            key: device(*holds).verifying_key().bytes().to_vec(),
            signature: device(*holds).sign(&over).bytes(),
        });
    }
}

/// The entity, as the record says it stands.
fn entity_at(objects: &Objects, entity: &Did) -> almena_store::entity::Entity {
    match objects.resolve(entity.name()) {
        Answer::Here(State::Entity(entity)) => *entity,
        other => panic!("{other:?}"),
    }
}

#[test]
fn two_people_create_a_two_of_two_and_neither_of_them_can_move_it_alone() {
    // **The phase's exit criterion.** Every signature below is made by a device key, and every one
    // of them is checked against the account that key belongs to — resolved from the record, never
    // taken from the act.
    let mut objects = Objects::new();
    let one = a_person(&mut objects, 1, 11);
    let other = a_person(&mut objects, 2, 22);

    // It starts as one owner and one approver, which is where `SPECS.md §8.2` says this starts.
    let mut founded = founding(&one, (1, 1, 1));
    signed_by(&mut founded, &[(&one, 11)]);
    let entity = founded.object.clone();
    assert_eq!(objects.admit(&founded, settled()), Ok(Admitted::Extended));
    assert_eq!(entity_at(&objects, &entity).owners.len(), 1);

    // The second person joins, on the threshold standing now — which is one.
    let mut adding = on(
        &entity,
        &founded.called(),
        Kind::ENTITY_ADD_OWNER,
        BTreeMap::from([(field::WHO, Value::Text(other.to_string()))]),
    );
    signed_by(&mut adding, &[(&one, 11)]);
    assert_eq!(objects.admit(&adding, settled()), Ok(Admitted::Extended));

    // And now they agree to need each other. Changing the threshold takes the threshold standing,
    // which is still one — it is what they are changing.
    let mut raising = on(
        &entity,
        &adding.called(),
        Kind::ENTITY_SET_THRESHOLD,
        BTreeMap::from([
            (field::ROUTINE, Value::Uint(1)),
            (field::SEALING, Value::Uint(1)),
            (field::GOVERNANCE, Value::Uint(2)),
        ]),
    );
    signed_by(&mut raising, &[(&one, 11)]);
    assert_eq!(objects.admit(&raising, settled()), Ok(Admitted::Extended));

    // **From here neither of them moves it alone**, and that is the whole of what a threshold is.
    let mut alone = on(
        &entity,
        &raising.called(),
        Kind::ENTITY_ADD_OWNER,
        BTreeMap::from([(field::WHO, Value::Text(one.to_string()))]),
    );
    signed_by(&mut alone, &[(&one, 11)]);
    assert_eq!(
        objects.admit(&alone, settled()),
        Err(Refused::NotAuthorised),
        "one owner, signing perfectly well, is not two"
    );

    // Together they do.
    let mut together = on(
        &entity,
        &raising.called(),
        Kind::ENTITY_ADD_MANAGER,
        BTreeMap::from([(field::WHO, Value::Text(other.to_string()))]),
    );
    signed_by(&mut together, &[(&one, 11), (&other, 22)]);
    assert_eq!(objects.admit(&together, settled()), Ok(Admitted::Extended));
    assert_eq!(entity_at(&objects, &entity).managers.len(), 1);
}

/// The act that creates an issuer under that entity.
fn an_issuer(of: &Did, role: u64) -> Operation {
    use almena_store::element::field as element;
    create(
        Network::Development,
        Kind::ISSUER_CREATE.number(),
        1,
        settled(),
        BTreeMap::from([
            (element::KEY, Value::Bytes(vec![5; 32])),
            (element::OF, Value::Text(of.to_string())),
            (element::ROLE, Value::Uint(role)),
        ]),
    )
}

#[test]
fn nobody_hangs_an_issuer_off_an_organisation_they_do_not_govern() {
    // **The second half of the bidirectional link `SPECS.md §2.3` asks for**, and it takes no
    // second act: the issuer names its parent, and the act only enters the record because the
    // parent's owners signed it. An acknowledgement act instead would leave a window in which an
    // issuer claims an organisation that has not agreed.
    let mut objects = Objects::new();
    let one = a_person(&mut objects, 1, 11);
    let stranger = a_person(&mut objects, 3, 33);

    let mut founded = founding(&one, (1, 1, 1));
    signed_by(&mut founded, &[(&one, 11)]);
    let entity = founded.object.clone();
    objects.admit(&founded, settled()).expect("founded");

    let mut theirs = an_issuer(&entity, 1);
    signed_by(&mut theirs, &[(&stranger, 33)]);
    assert_eq!(
        objects.admit(&theirs, settled()),
        Err(Refused::NotAuthorised),
        "signing perfectly well, for an organisation that is not theirs"
    );

    let mut ours = an_issuer(&entity, 1);
    signed_by(&mut ours, &[(&one, 11)]);
    let issuer = ours.object.clone();
    assert_eq!(objects.admit(&ours, settled()), Ok(Admitted::Extended));
    match objects.resolve(issuer.name()) {
        Answer::Here(State::Element(element)) => {
            assert_eq!(element.of, entity, "and it names the entity back");
            assert_eq!(element.issuance, None, "and issues nothing yet");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_issuer_emits_nothing_until_the_owners_authorise_the_key_it_emits_with() {
    // Creating one is routine; **authorising the key credentials are actually emitted with is a
    // sealing act** (`SPECS.md §4.11`, `§8.2`), which is the act that turns a configuration into
    // something that can sign in the world's face.
    use almena_store::element::field as element;
    let mut objects = Objects::new();
    let one = a_person(&mut objects, 1, 11);
    let other = a_person(&mut objects, 2, 22);

    // Routine costs one owner here; sealing costs two. Governance stays at one until there are
    // two owners to reach it with — see the test below for what founding above your own reach does.
    let mut founded = founding(&one, (1, 2, 1));
    signed_by(&mut founded, &[(&one, 11)]);
    let entity = founded.object.clone();
    objects.admit(&founded, settled()).expect("founded");

    let mut adding = on(
        &entity,
        &founded.called(),
        Kind::ENTITY_ADD_OWNER,
        BTreeMap::from([(field::WHO, Value::Text(other.to_string()))]),
    );
    signed_by(&mut adding, &[(&one, 11)]);
    objects
        .admit(&adding, settled())
        .expect("governance is one");

    let mut made = an_issuer(&entity, 1);
    signed_by(&mut made, &[(&one, 11)]);
    let issuer = made.object.clone();
    objects
        .admit(&made, settled())
        .expect("creating one is routine, which costs one here");

    let authorising = |signers: &[(&Did, u8)]| {
        let mut act = on(
            &issuer,
            &made.called(),
            Kind::ISSUER_SET_ISSUANCE_KEY,
            BTreeMap::from([(element::ISSUANCE, Value::Bytes(vec![4; 32]))]),
        );
        signed_by(&mut act, signers);
        act
    };

    assert_eq!(
        objects.admit(&authorising(&[(&one, 11)]), settled()),
        Err(Refused::NotAuthorised),
        "one owner is not the sealing threshold"
    );
    assert_eq!(
        objects.admit(&authorising(&[(&one, 11), (&other, 22)]), settled()),
        Ok(Admitted::Extended)
    );
    match objects.resolve(issuer.name()) {
        Answer::Here(State::Element(element)) => {
            assert_eq!(element.issuance, Some(vec![4; 32]));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_entity_founded_above_its_own_reach_is_born_without_a_quorum() {
    // **And it is not refused**, because refusing would be a node deciding an organisation's
    // governance for it — and because the way back exists: emergency continuity needs one surviving
    // owner and puts the set back (`SPECS.md §8.3`). What the record does instead is say plainly
    // that the quorum is not there, which is what `has_quorum` is for.
    let mut objects = Objects::new();
    let one = a_person(&mut objects, 1, 11);

    let mut founded = founding(&one, (1, 1, 2));
    signed_by(&mut founded, &[(&one, 11)]);
    let entity = founded.object.clone();
    assert_eq!(objects.admit(&founded, settled()), Ok(Admitted::Extended));
    assert!(!entity_at(&objects, &entity).has_quorum());

    // It cannot add the second owner it would need, because adding one is governance.
    let mut adding = on(
        &entity,
        &founded.called(),
        Kind::ENTITY_ADD_OWNER,
        BTreeMap::from([(field::WHO, Value::Text(one.to_string()))]),
    );
    signed_by(&mut adding, &[(&one, 11)]);
    assert_eq!(
        objects.admit(&adding, settled()),
        Err(Refused::NotAuthorised)
    );

    // Continuity can, at a threshold of one, and after sixty days in which any surviving owner
    // could have said no.
    let other = a_person(&mut objects, 2, 22);
    let mut continuity = on(
        &entity,
        &founded.called(),
        Kind::ENTITY_CONTINUITY,
        BTreeMap::from([(field::WHO, Value::Text(other.to_string()))]),
    );
    signed_by(&mut continuity, &[(&one, 11)]);
    assert_eq!(
        objects.admit(&continuity, settled()),
        Ok(Admitted::Extended)
    );

    let waited = Epoch::new(settled().number() + almena_store::entity::CONTINUITY_WAITS.count());
    assert!(
        entity_at(&objects, &entity).come_due(waited).has_quorum(),
        "and the way back was there"
    );
}

#[test]
fn a_stranger_signing_perfectly_well_is_not_an_owner() {
    // The signature checks; the key is theirs; and their account is nothing to do with this entity.
    // What decides is the set of owners the record holds, and nothing on the act.
    let mut objects = Objects::new();
    let one = a_person(&mut objects, 1, 11);
    let stranger = a_person(&mut objects, 3, 33);

    let mut founded = founding(&one, (1, 1, 1));
    signed_by(&mut founded, &[(&one, 11)]);
    let entity = founded.object.clone();
    objects.admit(&founded, settled()).expect("founded");

    let mut theirs = on(
        &entity,
        &founded.called(),
        Kind::ENTITY_ADD_OWNER,
        BTreeMap::from([(field::WHO, Value::Text(stranger.to_string()))]),
    );
    signed_by(&mut theirs, &[(&stranger, 33)]);
    assert_eq!(
        objects.admit(&theirs, settled()),
        Err(Refused::NotAuthorised)
    );
}

#[test]
fn an_owner_who_stopped_their_own_account_is_not_signing_for_anybody() {
    // **Freezing denies everything and concedes nothing** (`SPECS.md §11.12`), and signing for an
    // organisation is conceding. Somebody who stopped their account because a device was taken must
    // not go on governing an entity from it — which is exactly what a thief would use it for.
    let mut objects = Objects::new();
    let one = a_person(&mut objects, 1, 11);

    let mut founded = founding(&one, (1, 1, 1));
    signed_by(&mut founded, &[(&one, 11)]);
    let entity = founded.object.clone();
    objects.admit(&founded, settled()).expect("founded");

    // The words alone stop the account, which is the one act that does not wait.
    let head = objects.head(one.name()).expect("a chain").clone();
    let mut freeze = Operation {
        object: one.clone(),
        previous: Some(head),
        kind: Kind::HOLDER_FREEZE.number(),
        version: 1,
        issued: settled(),
        payload: BTreeMap::new(),
        signatures: Vec::new(),
    };
    let public = words(1).verifying_key().bytes();
    let signature = words(1).sign(&freeze.signing_bytes());
    freeze.signatures.push(Signed {
        by: one.clone(),
        key: public.to_vec(),
        signature: signature.bytes(),
    });
    objects
        .admit(&freeze, settled())
        .expect("freezing is immediate");

    let mut theirs = on(
        &entity,
        &founded.called(),
        Kind::ENTITY_ADD_MANAGER,
        BTreeMap::from([(field::WHO, Value::Text(one.to_string()))]),
    );
    signed_by(&mut theirs, &[(&one, 11)]);
    assert_eq!(
        objects.admit(&theirs, settled()),
        Err(Refused::NotAuthorised)
    );
}

/// The act that writes down a domain this organisation says it holds.
fn claiming(entity: &Did, head: &Name, domain: &str) -> Operation {
    on(
        entity,
        head,
        Kind::ENTITY_ADD_DOMAIN,
        BTreeMap::from([(field::DOMAIN, Value::Text(domain.to_owned()))]),
    )
}

#[test]
fn a_domain_belongs_to_one_organisation_until_that_one_lets_it_go() {
    // **DNS takes several records at one name**, so an administrator can publish two claims and both
    // pass the bidirectional check (`SPECS.md §7.5`). Without this the register would hold two
    // organisations each entitled to the same name, with nothing from outside to tell them apart —
    // and the tie-break is not the register's to make: the second waits until the first releases,
    // which puts the decision back with whoever controls the domain.
    let mut objects = Objects::new();
    let one = a_person(&mut objects, 1, 11);
    let other = a_person(&mut objects, 2, 22);

    let mut first = founding(&one, (1, 1, 1));
    signed_by(&mut first, &[(&one, 11)]);
    let ours = first.object.clone();
    objects.admit(&first, settled()).expect("founded");

    let mut second = founding(&other, (1, 1, 1));
    signed_by(&mut second, &[(&other, 22)]);
    let theirs = second.object.clone();
    objects.admit(&second, settled()).expect("founded");

    let mut claimed = claiming(&ours, &first.called(), "almena.network");
    signed_by(&mut claimed, &[(&one, 11)]);
    assert_eq!(objects.admit(&claimed, settled()), Ok(Admitted::Extended));

    let mut also = claiming(&theirs, &second.called(), "almena.network");
    signed_by(&mut also, &[(&other, 22)]);
    assert_eq!(
        objects.admit(&also, settled()),
        Err(Refused::NotAuthorised),
        "a second claim on a name already bound"
    );

    // Proving it again is not a second claim: it is the revalidation `SPECS.md §7.4` asks for every
    // thirty days, and refusing it would make a domain impossible to keep.
    let mut again = claiming(&ours, &claimed.called(), "almena.network");
    signed_by(&mut again, &[(&one, 11)]);
    assert_eq!(objects.admit(&again, settled()), Ok(Admitted::Extended));

    // And when the first lets it go, the second can have it.
    let mut released = on(
        &ours,
        &again.called(),
        Kind::ENTITY_REMOVE_DOMAIN,
        BTreeMap::from([(field::DOMAIN, Value::Text("almena.network".to_owned()))]),
    );
    signed_by(&mut released, &[(&one, 11)]);
    objects.admit(&released, settled()).expect("given up");

    let mut theirs_now = claiming(&theirs, &second.called(), "almena.network");
    signed_by(&mut theirs_now, &[(&other, 22)]);
    assert_eq!(
        objects.admit(&theirs_now, settled()),
        Ok(Admitted::Extended)
    );
}

#[test]
fn an_owner_who_changed_phone_keeps_their_place() {
    // **An owner is a root identifier and not a key** (`SPECS.md §8.5`). Binding a key would make
    // every rotation of one person a governance operation in every entity they belong to.
    let mut objects = Objects::new();
    let one = a_person(&mut objects, 1, 11);

    let mut founded = founding(&one, (1, 1, 1));
    signed_by(&mut founded, &[(&one, 11)]);
    let entity = founded.object.clone();
    objects.admit(&founded, settled()).expect("founded");

    // A second device joins that account, approved by the one already on it.
    let head = objects.head(one.name()).expect("a chain").clone();
    let mut adding = Operation {
        object: one.clone(),
        previous: Some(head),
        kind: Kind::HOLDER_ADD_DEVICE.number(),
        version: 1,
        issued: settled(),
        payload: BTreeMap::from([(1, Value::Bytes(device(19).verifying_key().bytes().to_vec()))]),
        signatures: Vec::new(),
    };
    let over = adding.signing_bytes();
    adding.signatures.push(Signed {
        by: one.clone(),
        key: device(11).verifying_key().bytes().to_vec(),
        signature: device(11).sign(&over).bytes(),
    });
    objects
        .admit(&adding, settled())
        .expect("a device already on the account approves");

    // And the entity is signed from the new one, without the entity having been told anything.
    let mut from_the_laptop = on(
        &entity,
        &founded.called(),
        Kind::ENTITY_ADD_MANAGER,
        BTreeMap::from([(field::WHO, Value::Text(one.to_string()))]),
    );
    signed_by(&mut from_the_laptop, &[(&one, 19)]);
    assert_eq!(
        objects.admit(&from_the_laptop, settled()),
        Ok(Admitted::Extended)
    );
}
