//! The seal, and the one name it unlocks.
//!
//! **Three levels that are not a gauge** (`SPECS.md §7.2`). Proving a domain is automatic and
//! proves control of a domain; it does **not** prove who somebody is, because
//! `banco-santander-clientes.com` is registrable by anybody. So the domain decides *which* name an
//! entity may take, and the seal decides *whether* it may take one at all — and without that
//! second half, the person who registered the look-alike would hold the look-alike name with
//! perfect technical legitimacy.
//!
//! What this walks is that division, and the two rules that hang off it: a reason is always
//! published and in both languages, and a withdrawal never reaches backwards.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, Signed, create};
use almena_store::certification::{Cause, Grade, Reason};
use almena_store::chain::{Admitted, Answer, Objects, Refused, State};
use almena_store::entity::field;
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
fn a_person(objects: &mut Objects, control: u8, holds: u8) -> Did {
    let public = words(control).verifying_key().bytes();
    let mut created = create(
        Network::Development,
        Kind::HOLDER_CREATE.number(),
        1,
        Epoch::GENESIS,
        BTreeMap::from([(1, Value::Bytes(public.to_vec()))]),
    );
    sign_with_words(&mut created, control);
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
    sign_with_words(&mut adding, control);
    objects.admit(&adding, Epoch::GENESIS).expect("the asking");
    whose
}

fn sign_with_words(operation: &mut Operation, control: u8) {
    let public = words(control).verifying_key().bytes();
    let signature = words(control).sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: public.to_vec(),
        signature: signature.bytes(),
    });
}

/// Sign an act as those owners would, from one of their devices each.
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

/// An organisation with one owner, at one and one.
fn an_entity(objects: &mut Objects, owner: &Did, key: u8) -> (Did, Name) {
    let mut founded = create(
        Network::Development,
        Kind::ENTITY_CREATE.number(),
        1,
        settled(),
        BTreeMap::from([
            (field::KEY, Value::Bytes(vec![key; 32])),
            (field::WHO, Value::Text(owner.to_string())),
            (field::ROUTINE, Value::Uint(1)),
            (field::SEALING, Value::Uint(1)),
            (field::GOVERNANCE, Value::Uint(1)),
        ]),
    );
    signed_by(&mut founded, &[(owner, key)]);
    let whose = founded.object.clone();
    objects.admit(&founded, settled()).expect("founded");
    (whose, founded.called())
}

/// One act on an existing chain.
fn on(whose: &Did, head: &Name, kind: Kind, payload: BTreeMap<u64, Value>) -> Operation {
    Operation {
        object: whose.clone(),
        previous: Some(head.clone()),
        kind: kind.number(),
        version: 1,
        issued: settled(),
        payload,
        signatures: Vec::new(),
    }
}

/// A reason, in the two languages the platform ships in.
fn reason() -> Value {
    Reason::carried(&BTreeMap::from([
        ("en".to_owned(), "checked, and it holds up".to_owned()),
        ("es".to_owned(), "comprobado, y se sostiene".to_owned()),
    ]))
}

/// The act that certifies somebody.
fn certifying(by: &Did, subject: &Did, grade: Grade, reason: Value) -> Operation {
    use almena_store::certification::field as certification;
    create(
        Network::Development,
        Kind::CERTIFICATION_ISSUE.number(),
        1,
        settled(),
        BTreeMap::from([
            (certification::BY, Value::Text(by.to_string())),
            (certification::SUBJECT, Value::Text(subject.to_string())),
            (certification::GRADE, Value::Uint(grade.number())),
            (certification::REASON, reason),
        ]),
    )
}

/// The entity, as the record says it stands.
fn entity_at(objects: &Objects, whose: &Did) -> almena_store::entity::Entity {
    match objects.resolve(whose.name()) {
        Answer::Here(State::Entity(entity)) => *entity,
        other => panic!("{other:?}"),
    }
}

/// The key Almena Government was opened with.
///
/// **One key and no owners, which is what the act that opens a network gives it.** Its composition
/// under `SPECS.md §7.1` — five owners and a threshold of three — is what `PLAN.md` F10 puts in
/// place for production; until then what signs in its name is this.
const ALMENA: u8 = 7;

/// A record with a real genesis, and one entity that has proved a domain.
struct Set {
    objects: Objects,
    /// Almena Government, as the act that opened this network created it.
    almena: Did,
    /// The entity being certified.
    theirs: Did,
    /// Its head.
    theirs_head: Name,
    /// Whoever owns that.
    theirs_owner: Did,
}

/// A network, and an organisation in it that has proved `panaderia.example`.
fn a_set() -> Set {
    let mut objects = Objects::new();
    let opened = almena_store::genesis::open(
        &almena_store::genesis::Opening {
            which: almena_store::genesis::Which::Development,
            beginning: Epoch::GENESIS,
            began: 1_800_000_000,
        },
        &[],
        false,
        &words(ALMENA),
    )
    .expect("nobody to join");
    objects
        .admit(&opened.operation, Epoch::GENESIS)
        .expect("the network");

    let theirs_owner = a_person(&mut objects, 2, 22);
    let (theirs, theirs_head) = an_entity(&mut objects, &theirs_owner, 22);

    let mut proving = on(
        &theirs,
        &theirs_head,
        Kind::ENTITY_ADD_DOMAIN,
        BTreeMap::from([(field::DOMAIN, Value::Text("panaderia.example".to_owned()))]),
    );
    signed_by(&mut proving, &[(&theirs_owner, 22)]);
    objects.admit(&proving, settled()).expect("proved");

    Set {
        objects,
        almena: opened.government,
        theirs,
        theirs_head: proving.called(),
        theirs_owner,
    }
}

/// The act by which Almena Government certifies somebody, signed with the key it was opened with.
fn seal_from_almena(set: &Set, subject: &Did, grade: Grade, reason: Value) -> Operation {
    let mut act = certifying(&set.almena, subject, grade, reason);
    let public = words(ALMENA).verifying_key().bytes();
    let signature = words(ALMENA).sign(&act.signing_bytes());
    act.signatures.push(Signed {
        by: set.almena.clone(),
        key: public.to_vec(),
        signature: signature.bytes(),
    });
    act
}

#[test]
fn a_domain_alone_does_not_buy_a_name() {
    // **The seal decides whether, and the domain decides which** (`SPECS.md §7.5`). Proving a
    // domain is automatic and proves control of a domain; it does not prove who anybody is. Without
    // this, whoever registered a look-alike would hold the look-alike name with perfect technical
    // legitimacy.
    let mut set = a_set();
    let mut claiming = on(
        &set.theirs,
        &set.theirs_head,
        Kind::ENTITY_SET_ALIAS,
        BTreeMap::from([(field::ALIAS, Value::Text("panaderia".to_owned()))]),
    );
    signed_by(&mut claiming, &[(&set.theirs_owner, 22)]);
    assert_eq!(
        set.objects.admit(&claiming, settled()),
        Err(Refused::NotAuthorised),
        "a proved domain and no seal"
    );
    assert_eq!(entity_at(&set.objects, &set.theirs).alias, None);
}

#[test]
fn a_name_has_to_derive_from_a_domain_the_entity_proved() {
    // A free name would be a name anybody could pick, which is what deriving exists to prevent.
    let mut set = a_set();
    let sealing = seal_from_almena(&set, &set.theirs, Grade::Basic, reason());
    set.objects.admit(&sealing, settled()).expect("certified");

    for made_up in [
        "banco",
        "panaderia.example",
        "Panaderia",
        "pan aderia",
        "-pan",
    ] {
        let mut claiming = on(
            &set.theirs,
            &set.theirs_head,
            Kind::ENTITY_SET_ALIAS,
            BTreeMap::from([(field::ALIAS, Value::Text(made_up.to_owned()))]),
        );
        signed_by(&mut claiming, &[(&set.theirs_owner, 22)]);
        assert!(
            set.objects.admit(&claiming, settled()).is_err(),
            "{made_up}"
        );
    }
}

#[test]
fn with_the_seal_and_the_domain_the_name_is_taken_and_nobody_else_may_have_it() {
    let mut set = a_set();
    let sealing = seal_from_almena(&set, &set.theirs, Grade::Basic, reason());
    set.objects.admit(&sealing, settled()).expect("certified");

    let mut claiming = on(
        &set.theirs,
        &set.theirs_head,
        Kind::ENTITY_SET_ALIAS,
        BTreeMap::from([(field::ALIAS, Value::Text("panaderia".to_owned()))]),
    );
    signed_by(&mut claiming, &[(&set.theirs_owner, 22)]);
    assert_eq!(
        set.objects.admit(&claiming, settled()),
        Ok(Admitted::Extended)
    );

    let held = entity_at(&set.objects, &set.theirs).alias.expect("claimed");
    assert_eq!(held.name, "panaderia");
    assert_eq!(held.from, "panaderia.example");
    assert_eq!(
        held.claimed_by,
        claiming.called(),
        "and it says which act claimed it, which is what a reader checks for firmness"
    );

    // Somebody else who proves a domain with the same first label and holds the seal still may not
    // take it: a name carries somebody's reputation, and two holders is the confusion §7.5 exists
    // to prevent.
    let third = a_person(&mut set.objects, 3, 33);
    let (elsewhere, head) = an_entity(&mut set.objects, &third, 33);
    let sealing = seal_from_almena(&set, &elsewhere, Grade::Basic, reason());
    set.objects.admit(&sealing, settled()).expect("certified");

    let mut proving = on(
        &elsewhere,
        &head,
        Kind::ENTITY_ADD_DOMAIN,
        BTreeMap::from([(field::DOMAIN, Value::Text("panaderia.other".to_owned()))]),
    );
    signed_by(&mut proving, &[(&third, 33)]);
    set.objects.admit(&proving, settled()).expect("proved");

    let mut also = on(
        &elsewhere,
        &proving.called(),
        Kind::ENTITY_SET_ALIAS,
        BTreeMap::from([(field::ALIAS, Value::Text("panaderia".to_owned()))]),
    );
    signed_by(&mut also, &[(&third, 33)]);
    assert_eq!(
        set.objects.admit(&also, settled()),
        Err(Refused::NotAuthorised)
    );
}

#[test]
fn a_seal_withdrawn_for_risk_is_immediate_and_reaches_nothing_behind_it() {
    // **Never retroactive** (`SPECS.md §4.3`, `§7.3`, `§7.8`): what was signed while the seal stood
    // goes on being valid, evaluated against the moment of the act. What changes is forward.
    use almena_store::certification::field as certification;
    let mut set = a_set();
    let sealing = seal_from_almena(&set, &set.theirs, Grade::Basic, reason());
    let seal = sealing.object.clone();
    set.objects.admit(&sealing, settled()).expect("certified");

    let later = Epoch::new(settled().number() + 10);
    let mut taking = Operation {
        object: seal.clone(),
        previous: Some(sealing.called()),
        kind: Kind::CERTIFICATION_REVOKE.number(),
        version: 1,
        issued: later,
        payload: BTreeMap::from([
            (certification::SUBJECT, Value::Text(set.theirs.to_string())),
            (certification::CAUSE, Value::Uint(Cause::Risk.number())),
            (certification::REASON, reason()),
        ]),
        signatures: Vec::new(),
    };
    let public = words(ALMENA).verifying_key().bytes();
    let signature = words(ALMENA).sign(&taking.signing_bytes());
    taking.signatures.push(Signed {
        by: set.almena.clone(),
        key: public.to_vec(),
        signature: signature.bytes(),
    });
    assert_eq!(set.objects.admit(&taking, later), Ok(Admitted::Extended));

    let Answer::Here(State::Certification(held)) = set.objects.resolve(seal.name()) else {
        panic!("it resolves")
    };
    assert!(!held.stands(later), "gone, now");
    assert!(
        held.stands(Epoch::new(later.number() - 1)),
        "and standing the moment before, which is what nothing behind it means"
    );
    assert_eq!(
        held.withdrawn.as_ref().map(|gone| gone.cause),
        Some(Cause::Risk)
    );

    // And the reason is published beside the decision, in both languages.
    let gone = held.withdrawn.as_ref().expect("withdrawn");
    assert_eq!(
        gone.reason.languages(),
        ["en", "es"],
        "a reason half the readers cannot read is not a published reason"
    );
}

#[test]
fn a_seal_says_nothing_without_a_reason_published_in_both_languages() {
    // `SPECS.md §7.10` makes the seal a permission, and a gate with no published reason is
    // arbitrariness. Refused at admission, so that it is never in the record without one.
    let mut set = a_set();
    let half = Reason::carried(&BTreeMap::from([("en".to_owned(), "checked".to_owned())]));
    let only_one = seal_from_almena(&set, &set.theirs, Grade::Basic, half);
    assert_eq!(
        set.objects.admit(&only_one, settled()),
        Err(Refused::Malformed)
    );
}

/// The act by which the party a decision was about answers it.
fn replying(to: &Name, said: Value) -> Operation {
    use almena_store::reply::field as reply;
    create(
        Network::Development,
        Kind::REPLY_PUBLISH.number(),
        1,
        settled(),
        BTreeMap::from([
            (reply::TO, Value::Text(to.as_str().to_owned())),
            (reply::SAID, said),
        ]),
    )
}

#[test]
fn the_decision_and_the_answer_stand_side_by_side_and_nobody_moderates_the_answer() {
    // **There is no authority above Almena** (`SPECS.md §7.8`), so appealing *to Almena* would be
    // asking it to re-read itself. What fits instead is that both are published and stay published:
    // whoever chooses their own root of trust reads the two and judges. A reply Almena could
    // withhold would be a right Almena grants, which is the one thing it must not be.
    let mut set = a_set();
    let sealing = seal_from_almena(&set, &set.theirs, Grade::Basic, reason());
    let seal = sealing.object.clone();
    set.objects.admit(&sealing, settled()).expect("certified");

    let mut answer = replying(seal.name(), reason());
    signed_by(&mut answer, &[(&set.theirs_owner, 22)]);
    assert_eq!(
        set.objects.admit(&answer, settled()),
        Ok(Admitted::Extended),
        "the party the decision was about answers, and nobody has to agree"
    );
    match set.objects.resolve(answer.object.name()) {
        Answer::Here(State::Reply(said)) => {
            assert_eq!(said.to, *seal.name());
            assert_eq!(said.by, set.theirs, "and it says who is answering");
            assert_eq!(said.said.languages(), ["en", "es"]);
        }
        other => panic!("{other:?}"),
    }

    // And it is published once and never edited: a reply somebody could revise after the fact
    // would be one whose meaning depends on when it is read.
    let mut again = Operation {
        object: answer.object.clone(),
        previous: Some(answer.called()),
        kind: Kind::REPLY_PUBLISH.number(),
        version: 1,
        issued: settled(),
        payload: answer.payload.clone(),
        signatures: Vec::new(),
    };
    signed_by(&mut again, &[(&set.theirs_owner, 22)]);
    assert_eq!(
        set.objects.admit(&again, settled()),
        Err(Refused::NotAuthorised)
    );
}

#[test]
fn a_stranger_does_not_answer_in_somebody_else_s_name() {
    // An act that named its own author would let anybody publish a reply as anybody. Who may answer
    // comes from the decision, resolved where the record is.
    let mut set = a_set();
    let sealing = seal_from_almena(&set, &set.theirs, Grade::Basic, reason());
    let seal = sealing.object.clone();
    set.objects.admit(&sealing, settled()).expect("certified");

    let stranger = a_person(&mut set.objects, 4, 44);
    let mut theirs = replying(seal.name(), reason());
    signed_by(&mut theirs, &[(&stranger, 44)]);
    assert_eq!(
        set.objects.admit(&theirs, settled()),
        Err(Refused::NotAuthorised)
    );
}

#[test]
fn a_certification_is_the_issuer_s_to_take_back_and_nobody_else_s() {
    // It is an object with a chain of its own, so being certified never means letting another party
    // append to your own history — and the subject does not get to edit what was said about them.
    use almena_store::certification::field as certification;
    let mut set = a_set();
    let sealing = seal_from_almena(&set, &set.theirs, Grade::Basic, reason());
    let seal = sealing.object.clone();
    set.objects.admit(&sealing, settled()).expect("certified");

    let mut theirs = Operation {
        object: seal.clone(),
        previous: Some(sealing.called()),
        kind: Kind::CERTIFICATION_REVOKE.number(),
        version: 1,
        issued: settled(),
        payload: BTreeMap::from([
            (certification::SUBJECT, Value::Text(set.theirs.to_string())),
            (certification::CAUSE, Value::Uint(Cause::Risk.number())),
            (certification::REASON, reason()),
        ]),
        signatures: Vec::new(),
    };
    // Signed by the subject's own owner, perfectly well.
    signed_by(&mut theirs, &[(&set.theirs_owner, 22)]);
    assert_eq!(
        set.objects.admit(&theirs, settled()),
        Err(Refused::NotAuthorised)
    );
}
