//! The catalogue, walked in the order it has to be built in.
//!
//! **Everything references by hash, so publishing out of order means republishing.** Sources first,
//! then the attributes whose definitions are copied from them, then the tags a request is
//! classified under, and only then the templates that name all three.
//!
//! What it proves is what `SPECS.md §9.4` says the catalogue is *for*: not that two systems
//! understand each other — a schema would do for that — but that **asking for more than you need is
//! visible**. The holder does not choose the template; their power is to accept, to refuse, and to
//! say no to what is optional. So the only thing left against excess is that what is asked for is
//! public, comparable and refusable, and a declared baseline is what turns comparing from counting
//! attributes into seeing the excess on its own.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, Signed, create};
use almena_store::attribute::{Shape as Kindly, Written, carried};
use almena_store::certification::{Grade, Reason};
use almena_store::chain::{Admitted, Answer, Objects, Refused, State};
use almena_store::entity::field as entity;
use almena_store::kind::Kind;
use almena_store::template::{How, Shape};
use almena_suite::{ed25519, p256};
use almena_time::Epoch;

/// After everything the words alone asked for has landed (`SPECS.md §11.12`).
fn settled() -> Epoch {
    Epoch::new(almena_time::deadline::CONTROL_KEY_WAIT.count() + 1)
}

/// The key Almena Government was opened with. Its composition under `SPECS.md §7.1` is F10's.
const ALMENA: u8 = 7;

fn words(seed: u8) -> ed25519::SigningKey {
    ed25519::SigningKey::from_secret([seed; 32])
}

fn device(seed: u8) -> p256::SigningKey {
    p256::SigningKey::from_secret([seed; 32]).expect("a valid scalar")
}

/// Sign an act with a key that is a whole party's, the way Almena Government signs.
fn as_almena(operation: &mut Operation, who: &Did) {
    let public = words(ALMENA).verifying_key().bytes();
    let signature = words(ALMENA).sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: who.clone(),
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
    let whose = created.object.clone();
    let signature = words(control).sign(&created.signing_bytes());
    created.signatures.push(Signed {
        by: whose.clone(),
        key: public.to_vec(),
        signature: signature.bytes(),
    });
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

/// An organisation with one owner, at one and one.
fn an_entity(objects: &mut Objects, owner: &Did, key: u8) -> Did {
    let mut founded = create(
        Network::Development,
        Kind::ENTITY_CREATE.number(),
        1,
        settled(),
        BTreeMap::from([
            (entity::KEY, Value::Bytes(vec![key; 32])),
            (entity::WHO, Value::Text(owner.to_string())),
            (entity::ROUTINE, Value::Uint(1)),
            (entity::SEALING, Value::Uint(1)),
            (entity::GOVERNANCE, Value::Uint(1)),
        ]),
    );
    signed_by(&mut founded, &[(owner, key)]);
    objects.admit(&founded, settled()).expect("founded");
    founded.object
}

/// A reason, in the two languages the platform ships in.
fn reason() -> Value {
    Reason::carried(&BTreeMap::from([
        ("en".to_owned(), "checked, and it holds up".to_owned()),
        ("es".to_owned(), "comprobado, y se sostiene".to_owned()),
    ]))
}

/// Labels, in the languages given.
fn labels(what: &str, languages: &[&str]) -> Value {
    carried(
        &languages
            .iter()
            .map(|tag| ((*tag).to_owned(), format!("{what} ({tag})")))
            .collect::<Written>(),
    )
}

/// The record, with Almena Government open and one certified organisation in it.
struct Set {
    objects: Objects,
    /// Almena Government, as the act that opened this network created it.
    almena: Did,
    /// An organisation Almena has certified.
    theirs: Did,
    /// Whoever owns it.
    theirs_owner: Did,
}

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
    let theirs = an_entity(&mut objects, &theirs_owner, 22);

    Set {
        objects,
        almena: opened.government,
        theirs,
        theirs_owner,
    }
}

/// Certify that organisation, which is the gate on publishing to the catalogue.
fn certify(set: &mut Set, subject: &Did) {
    use almena_store::certification::field as certification;
    let mut sealing = create(
        Network::Development,
        Kind::CERTIFICATION_ISSUE.number(),
        1,
        settled(),
        BTreeMap::from([
            (certification::BY, Value::Text(set.almena.to_string())),
            (certification::SUBJECT, Value::Text(subject.to_string())),
            (certification::GRADE, Value::Uint(Grade::Basic.number())),
            (certification::REASON, reason()),
        ]),
    );
    let almena = set.almena.clone();
    as_almena(&mut sealing, &almena);
    set.objects.admit(&sealing, settled()).expect("certified");
}

/// Almena admits a source to copy definitions from.
fn admit_a_source(set: &mut Set) -> Did {
    use almena_store::source::field as source;
    let mut admitting = create(
        Network::Development,
        Kind::SOURCE_ADMIT.number(),
        1,
        settled(),
        BTreeMap::from([
            (source::NAME, Value::Text("OpenID Connect".to_owned())),
            (
                source::AT,
                Value::Text("https://openid.net/specs".to_owned()),
            ),
            (source::VERSION, Value::Text("1.0 errata 2".to_owned())),
            (source::BY, Value::Text(set.almena.to_string())),
        ]),
    );
    let almena = set.almena.clone();
    as_almena(&mut admitting, &almena);
    set.objects.admit(&admitting, settled()).expect("admitted");
    admitting.object
}

/// Whose act it is, and the signatures that speak for them.
///
/// A pair rather than two arguments: which party publishes and who signed for it are one question,
/// and holding them apart at every call is how a test ends up publishing under a name nobody signed
/// for and passing anyway.
struct By<'a> {
    who: &'a Did,
    sign: &'a [(&'a Did, u8)],
}

impl<'a> By<'a> {
    /// Almena Government, which signs with the one key the genesis gave it.
    const fn almena(who: &'a Did) -> Self {
        Self { who, sign: &[] }
    }

    /// Anybody else, whose acts carry the signatures of owners its own chain authorises.
    const fn owners(who: &'a Did, sign: &'a [(&'a Did, u8)]) -> Self {
        Self { who, sign }
    }
}

/// Sign an act the way `by` says, hand it to the store, and give back what it named.
fn submit(set: &mut Set, mut act: Operation, by: &By<'_>) -> Result<Did, Refused> {
    if by.sign.is_empty() {
        let almena = set.almena.clone();
        as_almena(&mut act, &almena);
    } else {
        signed_by(&mut act, by.sign);
    }
    set.objects.admit(&act, settled()).map(|_| act.object)
}

/// One attribute, published by a party that holds the seal.
fn publish_an_attribute(
    set: &mut Set,
    from: &Did,
    by: &By<'_>,
    claim: &str,
    predicate: bool,
) -> Result<Did, Refused> {
    use almena_store::attribute::field as attribute;
    let mut payload = BTreeMap::from([
        (attribute::CLAIM, Value::Text(claim.to_owned())),
        (
            attribute::TYPE,
            Value::Uint(if predicate {
                Kindly::Boolean.number()
            } else {
                Kindly::Date.number()
            }),
        ),
        (attribute::SOURCE, Value::Text(from.to_string())),
        (
            attribute::DEFINITION,
            Value::Text(format!("what {claim} means, copied in")),
        ),
        (attribute::LABELS, labels(claim, &["en", "es"])),
        (attribute::BY, Value::Text(by.who.to_string())),
    ]);
    if predicate {
        payload.insert(attribute::PREDICATE, Value::Uint(1));
    }
    let publishing = create(
        Network::Development,
        Kind::ATTRIBUTE_PUBLISH.number(),
        1,
        settled(),
        payload,
    );
    submit(set, publishing, by)
}

/// Almena adds a tag to the closed list.
fn add_a_tag(set: &mut Set, name: &str) -> Did {
    use almena_store::tag::field as tag;
    let mut adding = create(
        Network::Development,
        Kind::TAG_ADD.number(),
        1,
        settled(),
        BTreeMap::from([
            (tag::NAME, Value::Text(name.to_owned())),
            (tag::LABELS, labels(name, &["en", "es"])),
            (tag::BY, Value::Text(set.almena.to_string())),
        ]),
    );
    let almena = set.almena.clone();
    as_almena(&mut adding, &almena);
    set.objects.admit(&adding, settled()).expect("added");
    adding.object
}

/// What a template asks for, in the strictly ascending order the record wants it in.
fn asked_in_order(asks: &[(&Did, How, bool)]) -> Vec<Value> {
    let mut listed: Vec<(String, u64, u64)> = asks
        .iter()
        .map(|(who, how, required)| (who.to_string(), how.number(), u64::from(*required)))
        .collect();
    listed.sort();
    listed
        .into_iter()
        .map(|(named, how, required)| {
            Value::Array(vec![
                Value::Text(named),
                Value::Uint(how),
                Value::Uint(required),
            ])
        })
        .collect()
}

/// A template, published by whoever signs it.
fn publish_a_template(
    set: &mut Set,
    by: &By<'_>,
    asks: &[(&Did, How, bool)],
    tags: &[&Did],
    derives: Option<&Did>,
) -> Result<Did, Refused> {
    use almena_store::template::field as template;
    let mut payload = BTreeMap::from([
        (template::KIND, Value::Uint(Shape::Request.number())),
        (template::ATTRIBUTES, Value::Array(asked_in_order(asks))),
        (template::BY, Value::Text(by.who.to_string())),
    ]);
    if !tags.is_empty() {
        payload.insert(
            template::TAGS,
            Value::Array(
                tags.iter()
                    .map(|tag| Value::Text(tag.to_string()))
                    .collect(),
            ),
        );
    }
    if let Some(from) = derives {
        payload.insert(template::DERIVES, Value::Text(from.to_string()));
    }

    let publishing = create(
        Network::Development,
        Kind::TEMPLATE_PUBLISH.number(),
        1,
        settled(),
        payload,
    );
    submit(set, publishing, by)
}

#[test]
fn almena_publishes_the_core_and_the_gate_is_the_seal() {
    // **The phase's exit criterion, first clause.** A source is admitted, the core is published from
    // it, and a party with no seal publishes nothing — while using what is published needs no
    // permission at all, because a template is a shape and not a licence.
    let mut set = a_set();
    let source = admit_a_source(&mut set);

    let almena = set.almena.clone();
    let born = publish_an_attribute(&mut set, &source, &By::almena(&almena), "birthdate", false)
        .expect("Almena's own");
    assert!(matches!(
        set.objects.resolve(born.name()),
        Answer::Here(State::Attribute(_))
    ));

    // The organisation has no seal yet, so it publishes nothing.
    let theirs = set.theirs.clone();
    let owner = set.theirs_owner.clone();
    assert_eq!(
        publish_an_attribute(
            &mut set,
            &source,
            &By::owners(&theirs, &[(&owner, 22)]),
            "employer",
            false
        ),
        Err(Refused::NotAuthorised)
    );

    // With the seal, it does.
    certify(&mut set, &theirs);
    assert!(
        publish_an_attribute(
            &mut set,
            &source,
            &By::owners(&theirs, &[(&owner, 22)]),
            "employer",
            false
        )
        .is_ok()
    );
}

#[test]
fn an_attribute_cannot_come_from_a_source_nobody_admitted() {
    // The definition copied in came from somewhere; the source is the record saying **where** was
    // agreed. Without that, *fix and copy* would be copying from anywhere at all.
    let mut set = a_set();
    let almena = set.almena.clone();
    let nowhere = Did::new(Network::Development, Name::of(b"a source nobody admitted"));
    assert_eq!(
        publish_an_attribute(&mut set, &nowhere, &By::almena(&almena), "birthdate", false),
        Err(Refused::NotAuthorised)
    );
}

#[test]
fn a_template_names_attributes_that_exist_and_tags_from_the_closed_list() {
    // **A template references rather than defines** (`SPECS.md §9.4`), so one naming something not
    // published would be a shape nobody can read — and a purpose invented for the occasion would be
    // one declared so as to be compared with nobody.
    let mut set = a_set();
    let source = admit_a_source(&mut set);
    let almena = set.almena.clone();
    let born = publish_an_attribute(&mut set, &source, &By::almena(&almena), "birthdate", false)
        .expect("published");
    let tag = add_a_tag(&mut set, "age-verification");

    let invented = Did::new(
        Network::Development,
        Name::of(b"an attribute nobody published"),
    );
    assert_eq!(
        publish_a_template(
            &mut set,
            &By::almena(&almena),
            &[(&invented, How::Value, true)],
            &[&tag],
            None
        ),
        Err(Refused::NotAuthorised)
    );

    let own_tag = Did::new(Network::Development, Name::of(b"a purpose of my own"));
    assert_eq!(
        publish_a_template(
            &mut set,
            &By::almena(&almena),
            &[(&born, How::Value, true)],
            &[&own_tag],
            None
        ),
        Err(Refused::NotAuthorised)
    );

    assert!(
        publish_a_template(
            &mut set,
            &By::almena(&almena),
            &[(&born, How::Value, true)],
            &[&tag],
            None
        )
        .is_ok()
    );
}

#[test]
fn a_predicate_may_only_be_asked_of_an_attribute_that_says_it_answers_one() {
    // A site with an age restriction has no business knowing a date of birth. Asking a plain date
    // for an answer, though, would be asking for something nobody undertook to be able to give.
    let mut set = a_set();
    let source = admit_a_source(&mut set);
    let almena = set.almena.clone();
    let born = publish_an_attribute(&mut set, &source, &By::almena(&almena), "birthdate", false)
        .expect("published");
    let over = publish_an_attribute(&mut set, &source, &By::almena(&almena), "age_over_18", true)
        .expect("published");

    assert_eq!(
        publish_a_template(
            &mut set,
            &By::almena(&almena),
            &[(&born, How::Predicate, true)],
            &[],
            None
        ),
        Err(Refused::NotAuthorised)
    );
    assert!(
        publish_a_template(
            &mut set,
            &By::almena(&almena),
            &[(&over, How::Predicate, true)],
            &[],
            None
        )
        .is_ok()
    );
}

#[test]
fn a_derived_template_shows_its_excess_against_the_baseline_it_declared() {
    // **The exit criterion's second clause.** Comparing stops being counting attributes: the author
    // declares the baseline, and the difference is what the catalogue puts on the page.
    let mut set = a_set();
    let source = admit_a_source(&mut set);
    let almena = set.almena.clone();
    let over = publish_an_attribute(&mut set, &source, &By::almena(&almena), "age_over_18", true)
        .expect("published");
    let born = publish_an_attribute(&mut set, &source, &By::almena(&almena), "birthdate", false)
        .expect("published");
    let name = publish_an_attribute(&mut set, &source, &By::almena(&almena), "given_name", false)
        .expect("published");
    let tag = add_a_tag(&mut set, "age-verification");

    let baseline = publish_a_template(
        &mut set,
        &By::almena(&almena),
        &[(&over, How::Predicate, true)],
        &[&tag],
        None,
    )
    .expect("published");

    // A certified organisation derives from it and asks for two things more.
    let theirs = set.theirs.clone();
    let owner = set.theirs_owner.clone();
    certify(&mut set, &theirs);
    let greedy = publish_a_template(
        &mut set,
        &By::owners(&theirs, &[(&owner, 22)]),
        &[
            (&over, How::Predicate, true),
            (&born, How::Value, true),
            (&name, How::Value, false),
        ],
        &[&tag],
        Some(&baseline),
    )
    .expect("published");

    let (Answer::Here(State::Template(line)), Answer::Here(State::Template(theirs))) = (
        set.objects.resolve(baseline.name()),
        set.objects.resolve(greedy.name()),
    ) else {
        panic!("both resolve")
    };
    let theirs = theirs.latest().expect("one version");
    assert_eq!(
        theirs.derives,
        Some(line.latest().expect("one").called.clone()).map(|_| baseline.name().clone())
    );

    let more = almena_store::template::beyond(theirs, line.latest().expect("one"));
    assert_eq!(more.len(), 2, "the date of birth and the name");
    assert!(more.iter().any(|asked| asked.attribute == *born.name()));
    assert!(more.iter().any(|asked| asked.attribute == *name.name()));
}

#[test]
fn using_a_published_template_needs_no_permission_at_all() {
    // **A template is not a licence, it is a shape** (`SPECS.md §9.4`). What the seal unlocks is
    // creating; using is open, which is what makes the catalogue an ecosystem rather than a set of
    // silos — and it is why an organisation with no seal is not shut out of anything but authoring.
    let mut set = a_set();
    let source = admit_a_source(&mut set);
    let almena = set.almena.clone();
    let born = publish_an_attribute(&mut set, &source, &By::almena(&almena), "birthdate", false)
        .expect("published");
    let published = publish_a_template(
        &mut set,
        &By::almena(&almena),
        &[(&born, How::Value, true)],
        &[],
        None,
    )
    .expect("published");

    // Anybody at all resolves it and reads what it asks for, seal or no seal.
    let Answer::Here(State::Template(held)) = set.objects.resolve(published.name()) else {
        panic!("anybody may read it")
    };
    assert_eq!(held.latest().expect("one").asks.len(), 1);
}

#[test]
fn translating_an_attribute_does_not_change_its_identifier_or_break_a_template() {
    // **The exit criterion's last clause.** Translating adds how something reads, never what it
    // means — so the templates referencing it and the credentials already issued go on standing.
    use almena_store::attribute::field as attribute;
    let mut set = a_set();
    let source = admit_a_source(&mut set);
    let almena = set.almena.clone();
    let born = publish_an_attribute(&mut set, &source, &By::almena(&almena), "birthdate", false)
        .expect("published");
    let published = publish_a_template(
        &mut set,
        &By::almena(&almena),
        &[(&born, How::Value, true)],
        &[],
        None,
    )
    .expect("published");

    let head = set.objects.head(born.name()).expect("a chain").clone();
    let mut translating = Operation {
        object: born.clone(),
        previous: Some(head),
        kind: Kind::ATTRIBUTE_TRANSLATE.number(),
        version: 1,
        issued: settled(),
        payload: BTreeMap::from([(attribute::LABELS, labels("birthdate", &["fr"]))]),
        signatures: Vec::new(),
    };
    as_almena(&mut translating, &almena);
    assert_eq!(
        set.objects.admit(&translating, settled()),
        Ok(Admitted::Extended)
    );

    let Answer::Here(State::Attribute(after)) = set.objects.resolve(born.name()) else {
        panic!("it resolves")
    };
    assert_eq!(after.languages().len(), 3);
    assert_eq!(after.claim, "birthdate", "and what it means did not move");

    let Answer::Here(State::Template(template)) = set.objects.resolve(published.name()) else {
        panic!("and the template that names it is untouched")
    };
    assert_eq!(
        template.latest().expect("one").asks[0].attribute,
        *born.name()
    );
}

#[test]
fn the_whole_catalogue_can_be_listed_by_what_each_object_is() {
    // **What the public view is served out of** (`SPECS.md §13.6`). A page per tag and a page per
    // template are comparisons, and a comparison needs the whole set — which is why there is no
    // private template and no arrangement outside the catalogue in the first place.
    let mut set = a_set();
    let source = admit_a_source(&mut set);
    let almena = set.almena.clone();
    let born = publish_an_attribute(&mut set, &source, &By::almena(&almena), "birthdate", false)
        .expect("published");
    let tag = add_a_tag(&mut set, "age-verification");
    let published = publish_a_template(
        &mut set,
        &By::almena(&almena),
        &[(&born, How::Value, true)],
        &[&tag],
        None,
    )
    .expect("published");

    let listed = set.objects.catalogue();
    assert_eq!(listed.sources, vec![source.name().clone()]);
    assert_eq!(listed.attributes, vec![born.name().clone()]);
    assert_eq!(listed.tags, vec![tag.name().clone()]);
    assert_eq!(listed.templates, vec![published.name().clone()]);

    // And nothing that is not the catalogue is on any shelf: the organisations and the people whose
    // acts are in the same record are not entries in it.
    assert!(!listed.templates.contains(set.theirs.name()));
}
