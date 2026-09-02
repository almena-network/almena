//! The whole cycle, walked by the partner against a node it does not share a process with.
//!
//! **The library's own exit criterion, with a wire in the middle.** A node is opened and served
//! over a loopback socket under its own key; a holder is a handful of keys on this side of the
//! socket; and the partner — through the same errands the binary runs — puts its account on the
//! record, relates to the holder over the holder's mediator, issues against a template published
//! by the government, collects what the holder decided, serves a request a wallet answers, judges
//! the presentation, revokes, and judges the next presentation refused **by name**.
//!
//! Nothing below reaches into the partner: every step goes through `almena_partner::commands` and
//! `almena_partner::verifying`, and everything the holder does goes through the same envelope and
//! mediator code a wallet would run. What this cannot catch is the two ends agreeing on something
//! wrong — that is what the vectors held to the published numbers are for.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use almena_api::Limits;
use almena_credential::disclosure::Disclosure;
use almena_credential::issue::Issued;
use almena_credential::present::{Asked as Presenting, parts, show};
use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, Signed, create};
use almena_node::{Node, Opening, Which};
use almena_partner::commands::{Partner, collect, issue, keys, relate, revoke};
use almena_partner::directory::{Directory, hex};
use almena_partner::link::{Pointer, decoded, encoded};
use almena_partner::node::{self as client, peer_of};
use almena_partner::post::envelope::{self, Envelope};
use almena_partner::post::mediator;
use almena_partner::post::message::{HELLO, Message};
use almena_partner::post::peer::{Peer, written};
use almena_partner::relations::answered_by;
use almena_partner::verifying::{self, Asking, Outcome, Under};
use almena_sdk::errand::{self, Came};
use almena_serve::Serving;
use almena_store::attribute::{Shape as Kindly, Written, carried};
use almena_store::capability::Capability;
use almena_store::certification::{Grade, Reason};
use almena_store::element::field as element;
use almena_store::entity::field as entity;
use almena_store::kind::Kind;
use almena_store::template::{How, Shape};
use almena_suite::{ed25519, p256};
use almena_time::Epoch;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

/// The key Almena Government was opened with.
const ALMENA: u8 = 7;

/// The node's own key, which it serves under and is named by.
const NODE: u8 = 6;

/// The issuer element's own key, and the key it emits with.
const ISSUER_KEY: u8 = 33;
const ISSUANCE: u8 = 44;

/// The holder's words, device, and the key of its one relationship.
const HOLDER_WORDS: u8 = 2;
const HOLDER_DEVICE: u8 = 22;
const HOLDER_RELATION: u8 = 21;

/// After everything the words alone asked for has landed.
fn settled() -> u64 {
    almena_time::deadline::CONTROL_KEY_WAIT.now() + 1
}

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

fn signed_by_device(operation: &mut Operation, who: &Did, secret: [u8; 32]) {
    let key = p256::SigningKey::from_secret(secret).expect("a key");
    let over = operation.signing_bytes();
    operation.signatures.push(Signed {
        by: who.clone(),
        key: key.verifying_key().bytes().to_vec(),
        signature: key.sign(&over).bytes(),
    });
}

/// The node, served over a loopback socket under its own key, on a clock this test moves.
struct Served {
    serving: Serving,
    address: std::net::SocketAddr,
    clock: Arc<AtomicU64>,
    peer: String,
}

impl Served {
    async fn up() -> Self {
        let opening = Opening {
            which: Which::Development,
            beginning: Epoch::GENESIS,
            began: 1_800_000_000,
        };
        let mut node =
            Node::open(&opening, &[], &words(ALMENA), words(NODE)).expect("nobody to join");
        assert!(node.also_offering(Capability::Mailbox, Epoch::GENESIS));
        let serving = Serving::new(
            node,
            Limits {
                per_connection: 600,
                window: 60,
                largest_act: 65_536,
                connections: 64,
            },
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a port");
        let address = listener.local_addr().expect("an address");
        let clock = Arc::new(AtomicU64::new(0));
        let acceptor = almena_tls::self_signed(&[NODE; 32]).expect("the node's own certificate");
        let (served, ticking) = (serving.clone(), Arc::clone(&clock));
        tokio::spawn(async move {
            loop {
                let Ok((io, _)) = listener.accept().await else {
                    continue;
                };
                let (served, ticking, acceptor) =
                    (served.clone(), Arc::clone(&ticking), acceptor.clone());
                tokio::spawn(async move {
                    if let Ok(wrapped) = acceptor.accept(io).await {
                        let _ = served
                            .connection(wrapped, move || {
                                Epoch::new(ticking.load(Ordering::Relaxed))
                            })
                            .await;
                    }
                });
            }
        });
        Self {
            serving,
            address,
            clock,
            peer: peer_of(&words(NODE).verifying_key().bytes()),
        }
    }

    fn origin(&self) -> String {
        format!("https://{}", self.address)
    }

    fn client(&self) -> client::Node {
        client::Node::at(&self.origin(), &self.peer).expect("a node")
    }

    fn now(&self) -> Epoch {
        Epoch::new(self.clock.load(Ordering::Relaxed))
    }

    async fn submit(&self, operation: &Operation) {
        self.serving
            .node()
            .write()
            .await
            .submit(operation, self.now())
            .expect("taken");
    }

    async fn government(&self) -> Did {
        self.serving.node().read().await.government().clone()
    }
}

/// A person's account with one device, put on the record directly.
async fn a_person(served: &Served, control: u8, holds: u8) -> Did {
    let public = words(control).verifying_key().bytes();
    let mut created = create(
        Network::Development,
        Kind::HOLDER_CREATE.number(),
        1,
        served.now(),
        BTreeMap::from([(1, Value::Bytes(public.to_vec()))]),
    );
    let whose = created.object.clone();
    let signature = words(control).sign(&created.signing_bytes());
    created.signatures.push(Signed {
        by: whose.clone(),
        key: public.to_vec(),
        signature: signature.bytes(),
    });
    served.submit(&created).await;
    let mut adding = Operation {
        object: whose.clone(),
        previous: Some(created.called()),
        kind: Kind::HOLDER_ADD_DEVICE.number(),
        version: 1,
        issued: served.now(),
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
    served.submit(&adding).await;
    whose
}

/// What the government publishes and the owner founds, once the record holds the two accounts.
struct Built {
    issuer: Did,
    university: Did,
    credential_version: Name,
    request_version: Name,
    born: Name,
    over: Name,
}

/// The university, its seal, its issuer element, and the catalogue — as the SDK's own test builds them.
#[allow(clippy::too_many_lines, reason = "it is a record built act by act")]
async fn built(served: &Served, owner: &Did, owner_device: [u8; 32]) -> Built {
    let almena = served.government().await;
    let at = served.now();
    let mut founded = create(
        Network::Development,
        Kind::ENTITY_CREATE.number(),
        1,
        at,
        BTreeMap::from([
            (entity::KEY, Value::Bytes(vec![22; 32])),
            (entity::WHO, Value::Text(owner.to_string())),
            (entity::ROUTINE, Value::Uint(1)),
            (entity::SEALING, Value::Uint(1)),
            (entity::GOVERNANCE, Value::Uint(1)),
        ]),
    );
    signed_by_device(&mut founded, owner, owner_device);
    served.submit(&founded).await;
    let university = founded.object.clone();

    let mut sealing = create(
        Network::Development,
        Kind::CERTIFICATION_ISSUE.number(),
        1,
        at,
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
    served.submit(&sealing).await;

    let mut hung = create(
        Network::Development,
        Kind::ISSUER_CREATE.number(),
        1,
        at,
        BTreeMap::from([
            (
                element::KEY,
                Value::Bytes(words(ISSUER_KEY).verifying_key().bytes().to_vec()),
            ),
            (element::OF, Value::Text(university.to_string())),
            (element::ROLE, Value::Uint(1)),
        ]),
    );
    signed_by_device(&mut hung, owner, owner_device);
    served.submit(&hung).await;
    let issuer = hung.object.clone();

    let mut authorising = Operation {
        object: issuer.clone(),
        previous: Some(hung.called()),
        kind: Kind::ISSUER_SET_ISSUANCE_KEY.number(),
        version: 1,
        issued: at,
        payload: BTreeMap::from([(
            element::ISSUANCE,
            Value::Bytes(device(ISSUANCE).verifying_key().bytes().to_vec()),
        )]),
        signatures: Vec::new(),
    };
    signed_by_device(&mut authorising, owner, owner_device);
    served.submit(&authorising).await;

    let source = admit(served, &almena).await;
    let born = publish_attribute(served, &almena, &source, "birthdate", false).await;
    let over = publish_attribute(served, &almena, &source, "age_over_18", true).await;
    let credential_version = publish_template(
        served,
        &almena,
        Shape::Credential,
        &[(&born, How::Value, true), (&over, How::Predicate, false)],
    )
    .await;
    let request_version = publish_template(
        served,
        &almena,
        Shape::Request,
        &[(&over, How::Predicate, true)],
    )
    .await;
    Built {
        issuer,
        university,
        credential_version,
        request_version,
        born: born.name().clone(),
        over: over.name().clone(),
    }
}

async fn admit(served: &Served, almena: &Did) -> Did {
    use almena_store::source::field as source;
    let mut admitting = create(
        Network::Development,
        Kind::SOURCE_ADMIT.number(),
        1,
        served.now(),
        BTreeMap::from([
            (source::NAME, Value::Text("openid-connect-core".to_owned())),
            (source::AT, Value::Text("https://openid.net".to_owned())),
            (source::VERSION, Value::Text("1.0".to_owned())),
            (source::BY, Value::Text(almena.to_string())),
        ]),
    );
    as_almena(&mut admitting, almena);
    served.submit(&admitting).await;
    admitting.object
}

fn labels(what: &str) -> Value {
    carried(
        &["en", "es"]
            .iter()
            .map(|tag| ((*tag).to_owned(), format!("{what} ({tag})")))
            .collect::<Written>(),
    )
}

async fn publish_attribute(
    served: &Served,
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
        served.now(),
        payload,
    );
    as_almena(&mut publishing, almena);
    served.submit(&publishing).await;
    publishing.object
}

async fn publish_template(
    served: &Served,
    almena: &Did,
    shape: Shape,
    asks: &[(&Did, How, bool)],
) -> Name {
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
        served.now(),
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
    served.submit(&publishing).await;
    publishing.called()
}

/// The holder: an account on the record, and one relationship key it seals and signs with.
struct Holder {
    account: Did,
    mine: Peer,
}

impl Holder {
    fn key(&self) -> ::p256::SecretKey {
        ::p256::SecretKey::from_slice(&[HOLDER_RELATION; 32]).expect("a key")
    }

    /// Everything waiting at the holder's mediator, opened with the relationship's key.
    async fn collected(&self, served: &Served) -> Vec<(Message, Vec<u8>, Envelope)> {
        let epoch = served.now().number();
        let collection = mediator::collect(
            &served.client(),
            &self.account,
            &device(HOLDER_DEVICE),
            epoch,
        )
        .await
        .expect("collected");
        let mut opened = Vec::new();
        for held in &collection.waiting {
            assert_eq!(
                held.relation,
                self.mine.to_did(),
                "filed under the holder's own identifier"
            );
            let sealed: Envelope = serde_json::from_slice(&held.sealed).expect("an envelope");
            let (body, sealed_by) = envelope::open(&self.key(), &sealed).expect("opens");
            let message: Message = serde_json::from_slice(&body).expect("a message");
            opened.push((message, sealed_by, sealed));
        }
        let names = collection
            .waiting
            .iter()
            .map(|held| held.called.clone())
            .collect();
        mediator::confirm(
            &served.client(),
            &self.account,
            &device(HOLDER_DEVICE),
            names,
            epoch,
        )
        .await
        .expect("confirmed");
        opened
    }

    /// Seal a message for the partner and hand it to the partner's mediators.
    async fn send(&self, served: &Served, message: &Message) {
        let partner_mine = message.to.first().expect("addressed");
        let far = Peer::read(partner_mine).expect("the partner's identifier");
        let sealed = envelope::seal(
            &self.key(),
            &far.seals,
            &serde_json::to_vec(message).expect("json"),
        )
        .expect("sealed");
        let bytes = serde_json::to_vec(&sealed).expect("json");
        for (address, peer) in &far.delivered_to {
            assert_eq!(peer, &served.peer, "pinned to the node that runs it");
            let there = client::Node::at(address, peer).expect("a mediator");
            mediator::deliver(&there, partner_mine, &bytes, mediator::HELD_FOR)
                .await
                .expect("delivered");
        }
    }
}

/// One plain HTTP exchange with the verifier's endpoint.
async fn http(
    address: std::net::SocketAddr,
    method: &str,
    target: &str,
    body: &str,
) -> (u16, serde_json::Value) {
    let mut stream = tokio::net::TcpStream::connect(address)
        .await
        .expect("connected");
    let request = format!(
        "{method} {target} HTTP/1.1\r\nHost: verifier\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).await.expect("sent");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("read");
    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("a response");
    let status = String::from_utf8_lossy(&raw[..split])
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("a status");
    (
        status,
        serde_json::from_slice(&raw[split + 4..]).expect("json"),
    )
}

/// The pointer inside a `present` link.
fn pointer_in(link: &str) -> Pointer {
    let query = link
        .strip_prefix("almena://present?request=")
        .expect("a present link");
    serde_json::from_str(&decoded(query).expect("percent-encoded")).expect("a pointer")
}

/// A presentation of that credential showing those attributes, answering that request.
fn presented(
    holder: &Holder,
    written: &str,
    request: &serde_json::Value,
    showing: &[&str],
    at: u64,
) -> String {
    let taken = parts(written).expect("a credential");
    let issued = Issued {
        jwt: taken.jwt.to_owned(),
        disclosures: taken
            .disclosures
            .iter()
            .map(|one| Disclosure::read(one).expect("a disclosure"))
            .collect(),
    };
    let purpose: BTreeMap<String, String> = request["credentials"][0]["claims"]
        .as_array()
        .expect("claims")
        .iter()
        .map(|claim| {
            (
                claim["path"][0].as_str().expect("a path").to_owned(),
                claim["purpose"].as_str().expect("a purpose").to_owned(),
            )
        })
        .collect();
    let binding = p256::SigningKey::from_secret([HOLDER_RELATION; 32]).expect("a key");
    let _ = holder;
    show(
        &issued,
        showing,
        &Presenting {
            nonce: request["nonce"].as_str().expect("a nonce").to_owned(),
            audience: request["aud"].as_str().expect("an audience").to_owned(),
            at: Epoch::new(at),
            purpose,
        },
        &binding,
    )
    .expect("presented")
    .written
}

/// Where the vectors go, when somebody asked for them.
fn vectors() -> Option<std::path::PathBuf> {
    let dir = std::env::var("ALMENA_PARTNER_VECTORS_DIR").ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(std::path::PathBuf::from(dir))
}

fn export(name: &str, value: &serde_json::Value) {
    if let Some(dir) = vectors() {
        std::fs::write(
            dir.join(name),
            serde_json::to_string_pretty(value).expect("json"),
        )
        .expect("written");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[allow(clippy::too_many_lines, reason = "it is one cycle, walked end to end")]
async fn a_credential_is_issued_shown_checked_revoked_and_then_refused_by_name_over_the_wire() {
    almena_partner::records::install();
    let served = Served::up().await;
    let scratch = std::env::temp_dir().join(format!("almena-partner-cycle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let partner = Partner {
        directory: Directory::at(&scratch).expect("a directory"),
        node: served.client(),
    };

    // ---- A stranger's key is not the node: the pin is the whole of the check. ----
    let stranger = client::Node::at(&served.origin(), &peer_of(&[9; 32])).expect("an address");
    assert_eq!(
        almena_partner::chain::network(&stranger)
            .await
            .unwrap_err()
            .to_string(),
        "node_not_that_node"
    );

    // ---- The partner puts its own account on the record; the holder is put there directly. ----
    let made = keys::run(&partner).await.expect("keys");
    assert!(made.submitted);
    let again = keys::run(&partner).await.expect("keys again");
    assert_eq!(
        again.account, made.account,
        "and a second run is the same partner"
    );
    assert_eq!(
        made.element.len(),
        64,
        "the element's 32 Ed25519 bytes, hexadecimal"
    );
    assert_eq!(
        made.issuance.len(),
        66,
        "the 33 compressed P-256 bytes, hexadecimal"
    );
    assert_eq!(
        (&again.element, &again.issuance),
        (&made.element, &made.issuance),
        "and the same issuer element"
    );
    let holder_account = a_person(&served, HOLDER_WORDS, HOLDER_DEVICE).await;
    served.clock.store(settled(), Ordering::Relaxed);

    let owner_device = partner
        .directory
        .keys_held()
        .expect("keys")
        .expect("made")
        .device;
    let built = built(&served, &made.account, owner_device).await;

    // ---- The holder shows a code; the partner takes it up and says hello. ----
    let holder = Holder {
        account: holder_account.clone(),
        mine: Peer::on(
            &::p256::SecretKey::from_slice(&[HOLDER_RELATION; 32])
                .expect("a key")
                .public_key(),
            vec![(served.address.to_string(), served.peer.clone())],
        ),
    };
    mediator::carry(
        &served.client(),
        &holder.account,
        &device(HOLDER_DEVICE),
        vec![holder.mine.to_did()],
        served.now().number(),
    )
    .await
    .expect("declared");
    let link = format!("almena://meet?who={}", encoded(&holder.mine.to_did()));
    let related = relate::run(&partner, &link, Vec::new())
        .await
        .expect("related");
    assert_eq!(related.theirs, holder.mine.to_did());
    assert_eq!(related.reached, 1);

    let arrived = holder.collected(&served).await;
    assert_eq!(arrived.len(), 1, "the hello, once");
    let (hello, sealed_by, sealed) = &arrived[0];
    assert_eq!(hello.kind, HELLO);
    assert_eq!(hello.from, related.mine);
    assert_eq!(
        answered_by(&hello.from, sealed_by).as_deref(),
        Some(related.mine.as_str()),
        "the far end is named by a claim it proved"
    );
    let partner_relation = partner
        .directory
        .relations()
        .expect("relations")
        .whose_far_end_is(&holder.mine.to_did())
        .cloned()
        .expect("kept");
    export(
        "envelope.json",
        &serde_json::json!({
            "what": "a HELLO the partner sealed for the holder; open it with the recipient's secret and check the sender",
            "envelope": sealed,
            "recipient_secret_hex": hex(&[HOLDER_RELATION; 32]),
            "recipient_public_compressed_hex": hex(&written(&holder.key().public_key())),
            "sender_secret_hex": partner_relation.secret,
            "sender_public_compressed_hex": hex(sealed_by),
            "opens_to": hello,
        }),
    );

    // ---- The partner issues against the template; the holder collects and accepts. ----
    let offered = issue::run(
        &partner,
        &issue::Asked {
            to: holder.mine.to_did(),
            issuer: built.issuer.clone(),
            issuance_key: [ISSUANCE; 32],
            issuer_key: [ISSUER_KEY; 32],
            template: built.credential_version.clone(),
            attributes: BTreeMap::from([
                (built.born.clone(), serde_json::json!("1815-12-10")),
                (built.over.clone(), serde_json::json!(true)),
            ]),
            expires: 20_000,
            revocable: true,
            identifier: Some("one-degree".to_owned()),
            came: Came::Unasked,
            renews: None,
        },
    )
    .await
    .expect("issued");
    assert_eq!(offered.identifier, "one-degree");
    let arrived = holder.collected(&served).await;
    assert_eq!(arrived.len(), 1);
    let (offer, _, _) = &arrived[0];
    assert_eq!(offer.kind, errand::kind::OFFER);
    assert_eq!(offer.body["came"], "unasked");
    let credential = offer.body["credential"]
        .as_str()
        .expect("the credential")
        .to_owned();
    assert_eq!(
        parts(&credential).expect("a credential").disclosures.len(),
        3,
        "two attributes and the name"
    );
    holder
        .send(
            &served,
            &Message::new(
                "one-degree",
                errand::kind::DECIDED,
                &holder.mine.to_did(),
                &related.mine,
                errand::decided("one-degree", true),
            ),
        )
        .await;
    let collected = collect::run(&partner).await.expect("collected");
    assert_eq!(collected.len(), 1);
    assert_eq!(
        collected[0].said.as_ref().expect("opened").kind,
        errand::kind::DECIDED
    );
    assert_eq!(
        partner
            .directory
            .issued()
            .expect("issued")
            .get("one-degree")
            .expect("kept")
            .decided,
        Some(true)
    );

    // ---- The partner asks, against the request template, for less than it could. ----
    let asking = Asking {
        verifier: built.university.clone(),
        template: built.request_version.clone(),
        accepts: vec![built.credential_version.clone()],
        asks: vec![(
            built.over.clone(),
            "to sell something age-restricted".to_owned(),
        )],
        serve: "127.0.0.1:0".to_owned(),
        path: "/present".to_owned(),
        under: Under::Nothing,
        require_revocable: true,
    };
    let started = verifying::start(&partner, &asking).await.expect("serving");
    let pointer = pointer_in(&started.link);
    assert_eq!(pointer.verifier, built.university.to_string());
    assert_eq!(pointer.nonce, started.pointer.nonce);
    let at = pointer
        .at
        .strip_prefix("http://")
        .expect("plain, on loopback");
    let (address, path) = at.split_once('/').expect("a path");
    let address: std::net::SocketAddr = address.parse().expect("an address");
    let (status, request) = http(
        address,
        "GET",
        &format!("/{path}?nonce={}", encoded(&pointer.nonce)),
        "",
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(request["authorised_by"], built.request_version.as_str());
    assert_eq!(
        request["credentials"][0]["meta"]["vct_values"][0],
        built.credential_version.as_str()
    );
    export("request.json", &request);
    let (gone, _) = http(address, "GET", "/present?nonce=another", "").await;
    assert_eq!(gone, 410);

    let presentation = presented(
        &holder,
        &credential,
        &request,
        &[built.over.as_str()],
        served.now().number(),
    );
    let (status, answered) = http(
        address,
        "POST",
        "/present",
        &serde_json::json!({ "nonce": pointer.nonce, "presentation": presentation }).to_string(),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(answered["outcome"], "accepted", "{answered}");
    let judged = started.judged().await.expect("judged");
    assert_eq!(judged.outcome, Outcome::Accepted);
    export(
        "presentation.json",
        &serde_json::json!({
            "what": "the holder's answer to request.json, and what the verifier said",
            "posted": { "nonce": pointer.nonce, "presentation": presentation },
            "answered": answered,
        }),
    );

    // ---- The partner revokes; the holder is told; the next presentation is refused by name. ----
    let revoked = revoke::run(&partner, "one-degree", [ISSUER_KEY; 32])
        .await
        .expect("revoked");
    assert!(revoked.told);
    let arrived = holder.collected(&served).await;
    assert_eq!(arrived[0].0.kind, errand::kind::REVOKED);
    assert_eq!(arrived[0].0.body["index"], revoked.index);

    let started = verifying::start(&partner, &asking)
        .await
        .expect("serving again");
    let pointer = pointer_in(&started.link);
    let (_, request) = http(
        started.address,
        "GET",
        &format!("/present?nonce={}", encoded(&pointer.nonce)),
        "",
    )
    .await;
    let presentation = presented(
        &holder,
        &credential,
        &request,
        &[built.over.as_str()],
        served.now().number(),
    );
    let (status, answered) = http(
        started.address,
        "POST",
        "/present",
        &serde_json::json!({ "nonce": pointer.nonce, "presentation": presentation }).to_string(),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(answered["outcome"], "not_what_was_asked");
    assert_eq!(
        answered["why"], "revoked",
        "refused by name, and never as *could not verify*"
    );
    assert_eq!(
        started.judged().await.expect("judged").outcome,
        Outcome::NotWhatWasAsked
    );

    let _ = std::fs::remove_dir_all(&scratch);
}
