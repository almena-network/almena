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

/// The act by which an organisation asks to be certified, on its own chain.
fn asking(whose: &Did, head: &Name, grade: u64, note: Option<&str>) -> Operation {
    let mut payload = BTreeMap::from([(field::GRADE, Value::Uint(grade))]);
    if let Some(note) = note {
        payload.insert(field::NOTE, Value::Text(note.to_owned()));
    }
    on(whose, head, Kind::CERTIFICATION_REQUEST, payload)
}

/// A refusal signed with the key Almena Government was opened with.
fn refusal_from_almena(set: &Set, to: &Name, said: Value) -> Operation {
    let mut act = replying(to, said);
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
fn asking_to_be_certified_is_an_act_on_the_entity_s_own_chain() {
    // **Nobody writes in somebody else's chain**, and that cuts both ways: the asking is the
    // entity's, so it lands in the entity's history — signed by its owners at what a routine act
    // costs them, and readable there by anybody deciding whether to answer it.
    let mut set = a_set();
    let mut request = asking(
        &set.theirs,
        &set.theirs_head,
        Grade::Verified.number(),
        Some("we bake bread, and we can show the oven"),
    );
    signed_by(&mut request, &[(&set.theirs_owner, 22)]);
    assert_eq!(
        set.objects.admit(&request, settled()),
        Ok(Admitted::Extended),
        "one owner at one-and-one is enough to ask"
    );

    let held = entity_at(&set.objects, &set.theirs);
    let recorded = held
        .requests
        .get(&request.called())
        .expect("the asking is in the entity's own state, under the act that asked");
    assert_eq!(recorded.grade, Grade::Verified);
    assert_eq!(
        recorded.note.as_deref(),
        Some("we bake bread, and we can show the oven")
    );
    assert_eq!(recorded.at, settled());
}

#[test]
fn a_grade_the_seal_does_not_number_is_not_a_request_for_the_nearest_one() {
    // The same closed vocabulary the seal has, and the same reason: *a grade slightly above the one
    // I know* is exactly the reading that would be dangerous. The act is kept and the entity stops
    // resolving here, which is what every closed vocabulary costs an old reader.
    let mut set = a_set();
    let mut request = asking(&set.theirs, &set.theirs_head, 4, None);
    signed_by(&mut request, &[(&set.theirs_owner, 22)]);
    assert_eq!(
        set.objects.admit(&request, settled()),
        Ok(Admitted::Extended),
        "kept and passed on"
    );
    assert!(
        matches!(
            set.objects.resolve(set.theirs.name()),
            Answer::CannotResolve(_)
        ),
        "and not read as a request for the third grade"
    );
}

#[test]
fn almena_answers_a_request_with_a_refusal_and_a_stranger_does_not() {
    // **A reply names a request exactly as it names a certification** (`SPECS.md §7.10`): the
    // refusal and the asking stand side by side for ever, with a reason in both languages, and the
    // party asked is the only one who may answer — found from the request, resolved where the
    // record is, never from what the reply says about itself.
    let mut set = a_set();
    let mut request = asking(&set.theirs, &set.theirs_head, Grade::Basic.number(), None);
    signed_by(&mut request, &[(&set.theirs_owner, 22)]);
    set.objects.admit(&request, settled()).expect("asked");
    let asked = request.called();

    let stranger = a_person(&mut set.objects, 4, 44);
    let mut theirs = replying(&asked, reason());
    signed_by(&mut theirs, &[(&stranger, 44)]);
    assert_eq!(
        set.objects.admit(&theirs, settled()),
        Err(Refused::NotAuthorised),
        "whoever was not asked does not answer"
    );

    let mut own = replying(&asked, reason());
    signed_by(&mut own, &[(&set.theirs_owner, 22)]);
    assert_eq!(
        set.objects.admit(&own, settled()),
        Err(Refused::NotAuthorised),
        "and neither does the party that asked — an answer to oneself is not an answer"
    );

    let half = Reason::carried(&BTreeMap::from([("es".to_owned(), "no".to_owned())]));
    assert_eq!(
        set.objects
            .admit(&refusal_from_almena(&set, &asked, half), settled()),
        Err(Refused::Malformed),
        "a refusal half the readers cannot read is not published beside anything"
    );

    let refusal = refusal_from_almena(&set, &asked, reason());
    assert_eq!(
        set.objects.admit(&refusal, settled()),
        Ok(Admitted::Extended),
        "Almena, with the key it was opened with, answers"
    );
    match set.objects.resolve(refusal.object.name()) {
        Answer::Here(State::Reply(said)) => {
            assert_eq!(said.to, asked, "it points at the act that asked");
            assert_eq!(said.by, set.almena, "and says who is answering");
        }
        other => panic!("{other:?}"),
    }

    // A reply naming an act nobody asked with is a reply to nothing.
    let nowhere = refusal_from_almena(&set, &Name::of(b"an act nobody wrote"), reason());
    assert_eq!(
        set.objects.admit(&nowhere, settled()),
        Err(Refused::NotAuthorised)
    );
}

#[test]
fn a_reply_is_about_whoever_the_decision_it_answers_was_about() {
    // **The act names a decision and never a party**, so who a reply is about is the record's to
    // say: the subject of the seal it answers, or the entity whose own chain carries the asking it
    // refuses. That is what puts the answer beside the decision when anybody asks what has been
    // said about an entity — read off the act alone, a reply would be about nobody.
    let mut set = a_set();
    let sealing = seal_from_almena(&set, &set.theirs, Grade::Basic, reason());
    let seal = sealing.object.clone();
    set.objects.admit(&sealing, settled()).expect("certified");
    assert_eq!(
        set.objects.subject_of(&sealing),
        Some(set.theirs.clone()),
        "a seal says who it is about in its own bytes"
    );

    let mut answer = replying(seal.name(), reason());
    signed_by(&mut answer, &[(&set.theirs_owner, 22)]);
    set.objects.admit(&answer, settled()).expect("answered");
    assert_eq!(
        set.objects.subject_of(&answer),
        Some(set.theirs.clone()),
        "the answer to a seal is about the seal's subject"
    );

    let mut request = asking(
        &set.theirs,
        &set.theirs_head,
        Grade::Verified.number(),
        None,
    );
    signed_by(&mut request, &[(&set.theirs_owner, 22)]);
    set.objects.admit(&request, settled()).expect("asked");
    assert_eq!(
        set.objects.subject_of(&request),
        None,
        "an asking is the entity's own act, on its own chain"
    );
    let refusal = refusal_from_almena(&set, &request.called(), reason());
    set.objects.admit(&refusal, settled()).expect("refused");
    assert_eq!(
        set.objects.subject_of(&refusal),
        Some(set.theirs.clone()),
        "and the refusal of an asking is about the entity that asked"
    );

    let nowhere = refusal_from_almena(&set, &Name::of(b"an act nobody wrote"), reason());
    assert_eq!(
        set.objects.subject_of(&nowhere),
        None,
        "a reply to nothing is about nobody"
    );
}

/// What the asking below says, word for word.
const NOTE: &str = "we bake bread, and we can show the oven";

/// The asking for the second grade with that note, following the entity's proof of its domain,
/// as the bytes every reader names it by — unsigned, because the name leaves the signatures out.
const REQUEST: &str = "a701783e6469643a616c6d656e613a6465763a7a516d5a35364466766e416f53746a6f536e46346a554b354c6f5a4e453954396b377a356e51475776616f3143525402782f7a516d6573573373634470364d4764356475563373347a7a787678574267726762563566377a48755a6b6175793336031836040105184906a2130214782777652062616b652062726561642c20616e642077652063616e2073686f7720746865206f76656e0780";

/// What that asking is called.
const REQUEST_NAME: &str = "zQmQWrLUwNwMePVDbWkYsudZF73LzTPLFMQtcVSU8Emf2sj";

/// Bytes as two hexadecimal digits each.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn an_asking_composed_here_is_these_bytes_and_this_name() {
    // **Pinned so that another composer can be held to them.** Whoever composes an asking for an
    // entity to sign — a portal, an app — has to produce the bytes this record admits under the
    // name this record gives them; the inputs are written beside the hex so that the other side
    // composes from the same facts and compares, rather than guessing at a field order.
    let set = a_set();
    let request = asking(
        &set.theirs,
        &set.theirs_head,
        Grade::Verified.number(),
        Some(NOTE),
    );
    let written = hex(&request.to_bytes());
    let called = request.called();

    if let Ok(dir) = std::env::var("ALMENA_VECTORS_DIR") {
        let text = format!(
            "# A CERTIFICATION_REQUEST (kind 54) the node's store composes and admits, for another\n\
             # composer to be held to. Pinned in almena/crates/almena-store/tests/what_the_seal_decides.rs,\n\
             # test `an_asking_composed_here_is_these_bytes_and_this_name`.\n\
             #\n\
             # Inputs:\n\
             #   object (the entity's DID) = {entity}\n\
             #   previous (its head: the act that proved panaderia.example) = {head}\n\
             #   kind = 54, version = 1\n\
             #   issued (epoch) = {issued}\n\
             #   payload = {{19: grade 2 (uint), 20: note (text) = {note:?}}}\n\
             #   signatures = [] (the name is over the naming bytes, which leave signatures out;\n\
             #                    the entity's owners sign these bytes at the routine threshold)\n\
             #\n\
             # The act's name (what a REPLY answering it names in field 1):\n\
             REQUEST_NAME={called}\n\
             #\n\
             # Operation::to_bytes() of the unsigned act, as hexadecimal:\n\
             REQUEST={written}\n",
            entity = set.theirs,
            head = set.theirs_head.as_str(),
            issued = settled().number(),
            note = NOTE,
            called = called.as_str(),
        );
        std::fs::create_dir_all(&dir).expect("a directory for the vectors");
        std::fs::write(std::path::Path::new(&dir).join("request54.txt"), text)
            .expect("the vector is written");
    }

    assert_eq!(written, REQUEST, "the bytes moved: re-export the vector");
    assert_eq!(called.as_str(), REQUEST_NAME);
    assert_eq!(
        act_read(&request.to_bytes()).called(),
        called,
        "and they read back under the same name"
    );
}

/// An act read back off its own bytes.
fn act_read(bytes: &[u8]) -> Operation {
    let value = almena_format::cbor::read(bytes).expect("canonical bytes");
    almena_format::operation::read(&value).expect("an act")
}
