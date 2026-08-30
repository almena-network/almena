//! One credential from end to end: issued, shown, checked, revoked, refused.
//!
//! **The phase's exit criterion, walked once.** An issuer built on this library issues against a
//! template; the holder sees every attribute before accepting; a verifier built on the same library
//! checks the signature, the binding, the template and the revocation; the issuer revokes; and the
//! next presentation is refused — **telling *revoked* from *could not be verified***, which is the
//! conformance requirement `SPECS.md §17.12` exists for.
//!
//! Every fact the verifier uses comes from a record built here act by act, because the point of the
//! library living in the node's own repository is that the rules it speaks are the rules the node
//! applies (`SPECS.md §13`).

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::BTreeMap;

use almena_credential::verify::{Fault, Missing};
use almena_credential::{Method, Status};
use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, Signed, create};
use almena_sdk::issuer::{Issuing, issue, place, publishing, republishing};
use almena_sdk::request::{Request, Wanted, holds_up, purposes};
use almena_sdk::verifier::{Against, Policy, Resolved, Says, verify};
use almena_status::list::{AT_LEAST, List};
use almena_status::wanted::{Reached, what_is_known};
use almena_store::attribute::{Shape as Kindly, Written, carried};
use almena_store::certification::{Grade, Reason};
use almena_store::chain::{Answer, Objects, State};
use almena_store::element::field as element;
use almena_store::entity::field as entity;
use almena_store::kind::Kind;
use almena_store::template::{How, Shape, Version};
use almena_suite::{ed25519, p256};
use almena_time::cohort::Cohort;
use almena_time::{Clock, Epoch};

/// After everything the words alone asked for has landed (`SPECS.md §11.12`).
fn settled() -> Epoch {
    Epoch::new(almena_time::deadline::CONTROL_KEY_WAIT.now() + 1)
}

/// The key Almena Government was opened with.
const ALMENA: u8 = 7;

fn words(seed: u8) -> ed25519::SigningKey {
    ed25519::SigningKey::from_secret([seed; 32])
}

fn device(seed: u8) -> p256::SigningKey {
    p256::SigningKey::from_secret([seed; 32]).expect("a valid scalar")
}

fn as_almena(operation: &mut Operation, who: &Did) {
    let signature = words(ALMENA).sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: who.clone(),
        key: words(ALMENA).verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });
}

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

/// Everything the walk below needs, once the record has been built.
struct Built {
    objects: Objects,
    /// Almena Government.
    almena: Did,
    /// The organisation that issues.
    university: Did,
    /// Whoever owns it.
    owner: Did,
    /// Its issuer element, which is what signs credentials and publishes status lists.
    issuer: Did,
    /// The key that element signs its own acts with.
    issuer_key: u8,
    /// The key it emits credentials with.
    issuance: u8,
    /// The template version a credential is issued against — a **credential's** shape.
    template: Version,
    /// The template version a request is made under — a **request's** shape.
    ///
    /// Two objects and not one, because they are two different questions: what an issuer puts in,
    /// and what a verifier may ask for. A template that was both would be a shape nobody could
    /// compare against anything.
    asking: Version,
    /// The attributes it names.
    attributes: (Name, Name),
}

/// A record with a certified university, its issuer, and one template published.
#[allow(clippy::too_many_lines, reason = "it is a record built act by act")]
fn built() -> Built {
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
    let almena = opened.government;

    let owner = a_person(&mut objects, 2, 22);
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
    signed_by(&mut founded, &[(&owner, 22)]);
    objects.admit(&founded, settled()).expect("founded");
    let university = founded.object;

    // The seal, which is the gate on publishing to the catalogue.
    let mut sealing = create(
        Network::Development,
        Kind::CERTIFICATION_ISSUE.number(),
        1,
        settled(),
        BTreeMap::from([
            (
                almena_store::certification::field::BY,
                Value::Text(almena.to_string()),
            ),
            (
                almena_store::certification::field::SUBJECT,
                Value::Text(university.to_string()),
            ),
            (
                almena_store::certification::field::GRADE,
                Value::Uint(Grade::Basic.number()),
            ),
            (
                almena_store::certification::field::REASON,
                Reason::carried(&BTreeMap::from([
                    ("en".to_owned(), "checked".to_owned()),
                    ("es".to_owned(), "comprobado".to_owned()),
                ])),
            ),
        ]),
    );
    as_almena(&mut sealing, &almena);
    objects.admit(&sealing, settled()).expect("certified");

    // The issuer element, and the key its owners authorise it to emit with.
    let issuer_key = 33;
    let issuance = 44;
    let mut hung = create(
        Network::Development,
        Kind::ISSUER_CREATE.number(),
        1,
        settled(),
        BTreeMap::from([
            (
                element::KEY,
                Value::Bytes(words(issuer_key).verifying_key().bytes().to_vec()),
            ),
            (element::OF, Value::Text(university.to_string())),
            (element::ROLE, Value::Uint(1)),
        ]),
    );
    signed_by(&mut hung, &[(&owner, 22)]);
    objects.admit(&hung, settled()).expect("hung");
    let issuer = hung.object.clone();

    let mut authorising = Operation {
        object: issuer.clone(),
        previous: Some(hung.called()),
        kind: Kind::ISSUER_SET_ISSUANCE_KEY.number(),
        version: 1,
        issued: settled(),
        payload: BTreeMap::from([(
            element::ISSUANCE,
            Value::Bytes(device(issuance).verifying_key().bytes().to_vec()),
        )]),
        signatures: Vec::new(),
    };
    signed_by(&mut authorising, &[(&owner, 22)]);
    objects.admit(&authorising, settled()).expect("authorised");

    // The catalogue: a source, two attributes, and the template that names them.
    let source = admit(&mut objects, &almena);
    let born = publish_attribute(&mut objects, &almena, &source, "birthdate", false);
    let over = publish_attribute(&mut objects, &almena, &source, "age_over_18", true);
    let template = publish_template(
        &mut objects,
        &almena,
        Shape::Credential,
        &[(&born, How::Value, true), (&over, How::Predicate, false)],
    );
    let asking = publish_template(
        &mut objects,
        &almena,
        Shape::Request,
        &[(&over, How::Predicate, true)],
    );

    Built {
        objects,
        almena,
        university,
        owner,
        issuer,
        issuer_key,
        issuance,
        template,
        asking,
        attributes: (born.name().clone(), over.name().clone()),
    }
}

/// Almena admits a source.
fn admit(objects: &mut Objects, almena: &Did) -> Did {
    use almena_store::source::field as source;
    let mut admitting = create(
        Network::Development,
        Kind::SOURCE_ADMIT.number(),
        1,
        settled(),
        BTreeMap::from([
            (source::NAME, Value::Text("openid-connect-core".to_owned())),
            (source::AT, Value::Text("https://openid.net".to_owned())),
            (source::VERSION, Value::Text("1.0".to_owned())),
            (source::BY, Value::Text(almena.to_string())),
        ]),
    );
    as_almena(&mut admitting, almena);
    objects.admit(&admitting, settled()).expect("admitted");
    admitting.object
}

/// Labels in the two languages the platform ships in.
fn labels(what: &str) -> Value {
    carried(
        &["en", "es"]
            .iter()
            .map(|tag| ((*tag).to_owned(), format!("{what} ({tag})")))
            .collect::<Written>(),
    )
}

/// Almena publishes one attribute of the core.
fn publish_attribute(
    objects: &mut Objects,
    almena: &Did,
    source: &Did,
    claim: &str,
    predicate: bool,
) -> Did {
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
        (attribute::SOURCE, Value::Text(source.to_string())),
        (
            attribute::DEFINITION,
            Value::Text(format!("what {claim} means")),
        ),
        (attribute::LABELS, labels(claim)),
        (attribute::BY, Value::Text(almena.to_string())),
    ]);
    if predicate {
        payload.insert(attribute::PREDICATE, Value::Uint(1));
    }
    let mut publishing = create(
        Network::Development,
        Kind::ATTRIBUTE_PUBLISH.number(),
        1,
        settled(),
        payload,
    );
    as_almena(&mut publishing, almena);
    objects.admit(&publishing, settled()).expect("published");
    publishing.object
}

/// Almena publishes the template a credential is issued against.
fn publish_template(
    objects: &mut Objects,
    almena: &Did,
    shape: Shape,
    asks: &[(&Did, How, bool)],
) -> Version {
    use almena_store::template::field as template;
    let mut listed: Vec<(String, u64, u64)> = asks
        .iter()
        .map(|(who, how, required)| (who.to_string(), how.number(), u64::from(*required)))
        .collect();
    listed.sort();
    let mut publishing = create(
        Network::Development,
        Kind::TEMPLATE_PUBLISH.number(),
        1,
        settled(),
        BTreeMap::from([
            (template::KIND, Value::Uint(shape.number())),
            (
                template::ATTRIBUTES,
                Value::Array(
                    listed
                        .into_iter()
                        .map(|(who, how, required)| {
                            Value::Array(vec![
                                Value::Text(who),
                                Value::Uint(how),
                                Value::Uint(required),
                            ])
                        })
                        .collect(),
                ),
            ),
            (template::BY, Value::Text(almena.to_string())),
        ]),
    );
    as_almena(&mut publishing, almena);
    objects.admit(&publishing, settled()).expect("published");
    let Answer::Here(State::Template(held)) = objects.resolve(publishing.object.name()) else {
        panic!("it resolves")
    };
    held.latest().expect("one version").clone()
}

/// This network's clock.
fn clock() -> Clock {
    Clock::from_unix(1_800_000_000).expect("an instant")
}

/// What the record says about the issuer, as a verifier would resolve it.
fn resolved(built: &Built) -> Resolved {
    let Answer::Here(State::Element(element)) = built.objects.resolve(built.issuer.name()) else {
        panic!("the issuer resolves")
    };
    let Answer::Here(State::Entity(entity)) = built.objects.resolve(built.university.name()) else {
        panic!("its organisation resolves")
    };
    Resolved {
        issuance_key: element.issuance.as_ref().and_then(|held| {
            <[u8; p256::PUBLIC_KEY_WIDTH]>::try_from(held.as_slice())
                .ok()
                .and_then(|bytes| p256::VerifyingKey::from_bytes(bytes).ok())
        }),
        closed: entity.closed.is_some(),
    }
}

const METHODS: &[Method] = &[Method::Almena];

fn policy<'a>() -> Policy<'a> {
    Policy {
        methods: METHODS,
        revocable: true,
        closed_issuers: false,
    }
}

/// The whole errand, once.
#[test]
#[allow(clippy::too_many_lines, reason = "it is one errand, walked end to end")]
fn a_credential_is_issued_shown_checked_revoked_and_then_refused_by_name() {
    let mut built = built();
    let (born, over) = built.attributes.clone();
    let expires = Epoch::new(20_000);
    let cohort = Cohort::of(&clock(), expires).expect("a window");

    // ---- The issuer opens a status list for the quarter these credentials expire in. ----
    let empty = List::empty();
    let mut opening = publishing(
        Network::Development,
        &empty,
        &built.issuer,
        cohort,
        settled(),
    );
    // **Signed by the element's own key**, because revoking has to cost what issuing costs.
    let signature = words(built.issuer_key).sign(&opening.signing_bytes());
    opening.signatures.push(Signed {
        by: built.issuer.clone(),
        key: words(built.issuer_key).verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });
    built.objects.admit(&opening, settled()).expect("opened");
    let list = opening.object.clone();

    // ---- The holder's wallet makes a key for this credential, and the issuer signs. ----
    let binding = device(55);
    let status = place(&list, AT_LEAST).expect("a place");
    let Status::Revocable { index, .. } = status else {
        panic!("revocable")
    };
    let attributes = BTreeMap::from([
        (born.clone(), serde_json::json!("1815-12-10")),
        (over.clone(), serde_json::json!(true)),
    ]);
    let held = issue(
        &Issuing {
            issuer: &built.issuer,
            template: &built.template,
            identifier: "one-degree",
            attributes: &attributes,
            holder: &binding.verifying_key(),
            between: (settled(), expires),
            status: status.clone(),
        },
        &device(built.issuance),
    )
    .expect("issued");

    // **The holder sees every attribute before accepting** (`SPECS.md §9.5`): they are all in hand,
    // because the disclosures travel with the credential and only what is shown later is chosen.
    assert_eq!(held.disclosures.len(), 3, "two attributes and the name");

    // ---- A verifier asks, against the template, for less than it could. ----
    let request = Request {
        template: built.asking.called.clone(),
        // **The credential shape it takes the data from**, which is a different object from the
        // one authorising the request.
        accepts: vec![built.template.called.clone()],
        nonce: "a-nonce".to_owned(),
        audience: "did:almena:dev:zAVerifier".to_owned(),
        wants: vec![Wanted {
            attribute: over.clone(),
            how: How::Predicate,
            required: true,
            from_credential: true,
            purpose: "to sell something age-restricted".to_owned(),
        }],
    };
    assert_eq!(
        holds_up(&request, &built.asking),
        Ok(()),
        "the request is inside the template that authorises it"
    );

    let shown = almena_credential::present::show(
        &held,
        &[over.as_str()],
        &almena_credential::present::Asked {
            nonce: request.nonce.clone(),
            audience: request.audience.clone(),
            at: Epoch::new(1_000),
            purpose: purposes(&request),
        },
        &binding,
    )
    .expect("presented")
    .written;

    // ---- The verifier checks it: signature, binding, template and revocation. ----
    let mut list_bytes = List::empty();
    let fresh = Reached {
        freshest: current(&built, &list),
        served: Some(list_bytes.clone()),
    };
    let Says::Proved(proved) = verify(
        &shown,
        &Against {
            request: &request,
            resolved: &resolved(&built),
            policy: &policy(),
            revocation: what_is_known(&fresh, index),
            now: Epoch::new(1_000),
        },
    ) else {
        panic!("it holds up")
    };
    assert_eq!(proved.attributes.len(), 1, "and only what was asked for");
    assert_eq!(proved.attributes[over.as_str()], serde_json::json!(true));
    assert!(
        !proved.attributes.contains_key(born.as_str()),
        "the date of birth never left the wallet"
    );

    // ---- The issuer revokes: a new version, and its hash in the record. ----
    list_bytes.revoke(index);
    let mut again = republishing(&list_bytes, &list, opening.called(), Epoch::new(1_100));
    let signature = words(built.issuer_key).sign(&again.signing_bytes());
    again.signatures.push(Signed {
        by: built.issuer.clone(),
        key: words(built.issuer_key).verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });
    built
        .objects
        .admit(&again, Epoch::new(1_100))
        .expect("revoked");

    // ---- And the next presentation is refused, **by name**. ----
    let after = Reached {
        freshest: current(&built, &list),
        served: Some(list_bytes.clone()),
    };
    assert_eq!(
        verify(
            &shown,
            &Against {
                request: &request,
                resolved: &resolved(&built),
                policy: &policy(),
                revocation: what_is_known(&after, index),
                now: Epoch::new(1_200),
            },
        ),
        Says::NotValid(Fault::Revoked)
    );

    // **And a verifier that could not reach the bytes says so, and never says *invalid***
    // (`SPECS.md §17.12`). One is the holder's problem at the counter and one is nobody's.
    let unreachable = Reached {
        freshest: current(&built, &list),
        served: None,
    };
    assert_eq!(
        verify(
            &shown,
            &Against {
                request: &request,
                resolved: &resolved(&built),
                policy: &policy(),
                revocation: what_is_known(&unreachable, index),
                now: Epoch::new(1_200),
            },
        ),
        Says::CouldNotVerify(Missing::StatusUnavailable)
    );

    // And a replica still serving the version from before the revocation is **stale**, which is
    // also not a verdict about the credential: the hash in the record says what the bytes are not.
    let old = Reached {
        freshest: current(&built, &list),
        served: Some(empty),
    };
    assert_eq!(
        verify(
            &shown,
            &Against {
                request: &request,
                resolved: &resolved(&built),
                policy: &policy(),
                revocation: what_is_known(&old, index),
                now: Epoch::new(1_200),
            },
        ),
        Says::CouldNotVerify(Missing::StatusStale)
    );

    // The owner is still whoever owned it: nothing above touched who governs anything.
    assert_ne!(built.owner, built.almena);
}

/// The version hash the record names for that list, which is what a verifier compares against.
fn current(built: &Built, list: &Did) -> Option<almena_suite::digest::Digest> {
    let Answer::Here(State::StatusList(held)) = built.objects.resolve(list.name()) else {
        return None;
    };
    let bytes = <[u8; 32]>::try_from(held.latest()?.hash.as_slice()).ok()?;
    Some(almena_suite::digest::Digest::from_bytes(bytes))
}
