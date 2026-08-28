//! The whole of a node, demonstrated over the wire.
//!
//! A network is opened, an identity is created by handing a signed act to the interface, and the
//! identity is then **resolved by composing its document here** rather than by being handed one.
//! Nothing below reaches into the node: every question and every answer goes over a socket as
//! HTTP, exactly as a client or a portal would ask it.
//!
//! # Why it composes the document instead of asking for one
//!
//! Because a node that handed over a finished document would be a source somebody has to believe,
//! and that is the one thing it must not become. What it hands over is materials — the head of a
//! chain, and each act in that chain in the bytes its author signed — and whoever is going to use
//! the identity walks the chain, checks the signatures on the way, and builds the document. If
//! that is not possible from what the interface serves, the interface is wrong, and this is what
//! would say so.
//!
//! # What this is not
//!
//! **It is not an independent implementation.** It parses acts with the same crate the node writes
//! them with, so it cannot catch the two agreeing on something wrong. What catches that is the
//! second implementation in the holder's application, held to the same corpus of bytes. This
//! catches something different and still worth catching: that everything a caller needs is
//! actually reachable through the interface, and that the answers are told apart on the wire.

#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use std::collections::BTreeMap;

use almena_api::{Limits, State};
use almena_format::cbor::{Value, read};
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::{Operation, Signed, create};
use almena_node::{Node, Opening, Which};
use almena_serve::Serving;
use almena_store::kind::Kind;
use almena_suite::ed25519;
use almena_time::Epoch;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

/// A development network with a fixed clock, so that this is never about what time it is.
fn opening() -> Opening {
    Opening {
        which: Which::Development,
        beginning: Epoch::GENESIS,
        began: 1_800_000_000,
    }
}

fn key(seed: u8) -> ed25519::SigningKey {
    ed25519::SigningKey::from_secret([seed; 32])
}

fn limits() -> Limits {
    Limits {
        per_connection: 600,
        window: 60,
        largest_act: 65_536,
        connections: 64,
    }
}

/// What one exchange over the wire came back with.
struct Answered {
    status: u16,
    fields: BTreeMap<u64, Value>,
}

impl Answered {
    /// What the node said happened.
    fn state(&self) -> u64 {
        let Some(&Value::Uint(state)) = self.fields.get(&3) else {
            panic!("every answer says what happened");
        };
        state
    }

    /// What it was asked for, if anything came with it.
    fn payload(&self) -> Option<&Value> {
        self.fields.get(&4)
    }
}

/// Ask the node something, over a real socket, as HTTP.
async fn ask(serving: &Serving, method: &str, path: &str, body: Vec<u8>) -> Answered {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a port");
    let address = listener.local_addr().expect("an address");

    let served = serving.clone();
    let server = tokio::spawn(async move {
        let (io, _) = listener.accept().await.expect("a connection");
        let _ = served.connection(io, || Epoch::GENESIS).await;
    });

    let mut stream = tokio::net::TcpStream::connect(address)
        .await
        .expect("connected");
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: node\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await.expect("sent");
    stream.write_all(&body).await.expect("sent");

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.expect("read");
    server.await.expect("served");

    let split = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("a complete response");
    let status = String::from_utf8_lossy(&raw[..split])
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("a status");

    let Ok(Value::Map(fields)) = read(&raw[split + 4..]) else {
        panic!("an answer is a canonical map");
    };
    Answered { status, fields }
}

/// A holder creation, signed by the control key it establishes.
fn an_identity(control: &ed25519::SigningKey) -> Operation {
    let public = control.verifying_key().bytes();
    let mut operation = create(
        Network::Development,
        Kind::HOLDER_CREATE.number(),
        1,
        Epoch::GENESIS,
        BTreeMap::from([(1, Value::Bytes(public.to_vec()))]),
    );
    let signature = control.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: operation.object.clone(),
        key: public.to_vec(),
        signature: signature.bytes(),
    });
    operation
}

/// An act adding a device, signed by the control key.
fn adding(
    identity: &Operation,
    device: &almena_suite::p256::SigningKey,
    control: &ed25519::SigningKey,
) -> Operation {
    let mut act = Operation {
        object: identity.object.clone(),
        previous: Some(Name::of(&identity.to_bytes())),
        kind: Kind::HOLDER_ADD_DEVICE.number(),
        version: 1,
        issued: Epoch::GENESIS,
        payload: BTreeMap::from([(1, Value::Bytes(device.verifying_key().bytes().to_vec()))]),
        signatures: Vec::new(),
    };
    let signature = control.sign(&act.signing_bytes());
    act.signatures.push(Signed {
        by: identity.object.clone(),
        key: control.verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });
    act
}

/// A DID document, as much of one as this build's acts can establish.
#[derive(Debug, PartialEq, Eq)]
struct Document {
    /// Who it is about.
    id: String,
    /// The key that governs the account, which is what invokes a capability over it.
    capability_invocation: Vec<String>,
    /// How many acts had to be fetched to build this.
    depth: usize,
    /// The keys that operate it, one per device.
    ///
    /// They project as authentication because that is what they do — and the projection is
    /// information for whoever reads with ordinary tools, never the source of permission. What may
    /// sign what is decided by the chain this walked, not by anything written here.
    authentication: Vec<String>,
}

/// Build the document for `name`, asking the node only for materials.
///
/// This is the part that matters. It walks the chain backwards from the head, fetching each act in
/// the bytes its author signed, checks that the first one names itself, verifies every signature,
/// and only then says what the identity is. At no point does it ask the node what the answer is.
async fn compose(serving: &Serving, name: &Name) -> Document {
    let chain = fetch(serving, name).await;

    // The first act has to name itself, or this identity is not the one that was asked about.
    let first = chain.first().expect("a chain has a first act");
    assert!(
        first.names_itself(),
        "a creation names itself or it is not one"
    );
    assert_eq!(first.object.name(), name);

    let (control, devices) = walk(&chain);
    Document {
        id: Did::new(Network::Development, name.clone()).to_string(),
        depth: chain.len(),
        capability_invocation: vec![written_out(&control.expect("a control key"))],
        authentication: devices.iter().map(|key| written_out(key)).collect(),
    }
}

/// Every act of an object's chain, oldest first, fetched one at a time by hash.
///
/// Backwards from the head, because that is the only direction the chain points: each act names
/// the one it follows, and nothing names what follows it.
async fn fetch(serving: &Serving, name: &Name) -> Vec<Operation> {
    let answered = ask(
        serving,
        "GET",
        &format!("/object/{}", name.as_str()),
        Vec::new(),
    )
    .await;
    assert_eq!(
        answered.state(),
        State::Here as u64,
        "it has to be resolvable"
    );

    let Some(Value::Text(head)) = answered.payload() else {
        panic!("resolving hands over where to start reading");
    };

    let mut chain = Vec::new();
    let mut at = Name::parse(head).expect("a name");
    loop {
        let fetched = ask(serving, "GET", &format!("/act/{}", at.as_str()), Vec::new()).await;
        assert_eq!(
            fetched.state(),
            State::Here as u64,
            "the act has to be there"
        );
        let Some(Value::Bytes(bytes)) = fetched.payload() else {
            panic!("an act comes back as the bytes its author signed");
        };
        assert_eq!(Name::of(bytes), at, "what came back is what was asked for");

        let act = almena_format::operation::read(&read(bytes).expect("canonical")).expect("an act");
        let previous = act.previous.clone();
        chain.push(act);

        match previous {
            Some(earlier) => at = earlier,
            None => break,
        }
    }

    chain.reverse();
    chain
}

/// Read the chain forwards, checking every signature, and say what it established.
///
/// **Which curve made a signature is never guessed from the length of a key.** The state so far
/// says which key it is, and therefore which curve: the control key is the one the chain
/// established, and anything else has to be a device the chain already added.
fn walk(chain: &[Operation]) -> (Option<[u8; 32]>, Vec<Vec<u8>>) {
    let mut control: Option<[u8; 32]> = None;
    let mut devices: Vec<Vec<u8>> = Vec::new();

    for act in chain {
        let signature = act.signatures.first().expect("every act is signed");
        let established = control.is_some_and(|key| key.as_slice() == signature.key.as_slice());

        if established || control.is_none() {
            let key: [u8; 32] = signature
                .key
                .as_slice()
                .try_into()
                .expect("the control key is thirty-two bytes");
            let verifying = ed25519::VerifyingKey::from_bytes(key).expect("a key");
            let made = ed25519::Signature::from_bytes(signature.signature);
            assert_eq!(
                verifying.verify(&act.signing_bytes(), &made),
                Ok(()),
                "every signature checks out, or nothing here is worth composing"
            );
        } else {
            assert!(
                devices.contains(&signature.key),
                "signed by a key this chain never authorised"
            );
        }

        match Kind::new(act.kind) {
            Some(Kind::HOLDER_CREATE) => {
                let Some(Value::Bytes(key)) = act.payload.get(&1) else {
                    panic!("a creation establishes a key");
                };
                control = Some(key.as_slice().try_into().expect("thirty-two bytes"));
            }
            Some(Kind::HOLDER_ADD_DEVICE) => {
                let Some(Value::Bytes(key)) = act.payload.get(&1) else {
                    panic!("adding a device names one");
                };
                devices.push(key.clone());
            }
            _ => {}
        }
    }

    (control, devices)
}

fn written_out(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[tokio::test]
async fn a_node_opens_a_network_takes_an_identity_and_lets_it_be_composed() {
    let node = Node::open(&opening(), &[], &key(5), key(6)).expect("nobody to join");
    let serving = Serving::new(node, limits());

    // An identity nobody has created yet does not exist. **The first of the three answers.**
    let control = key(9);
    let identity = an_identity(&control);
    let name = identity.object.name().clone();

    let before = ask(
        &serving,
        "GET",
        &format!("/object/{}", name.as_str()),
        Vec::new(),
    )
    .await;
    assert_eq!(
        before.state(),
        State::DoesNotExist as u64,
        "and the status is 200, because the question was served"
    );
    assert_eq!(before.status, 200);

    // Handing over the signed act is the whole of creating it. There is nothing to log in to.
    let taken = ask(&serving, "POST", "/acts", identity.to_bytes()).await;
    assert_eq!(taken.state(), State::Taken as u64);

    // A device, so that the chain is more than one act and composing it means walking one.
    let device = almena_suite::p256::SigningKey::from_secret([21; 32]).expect("a valid scalar");
    let added = adding(&identity, &device, &control);
    assert_eq!(
        ask(&serving, "POST", "/acts", added.to_bytes())
            .await
            .state(),
        State::Taken as u64
    );

    // And now it can be composed, from materials, without believing the node about anything.
    let document = compose(&serving, &name).await;
    assert_eq!(
        document.capability_invocation,
        vec![written_out(&control.verifying_key().bytes())],
        "the key that governs the account, established by the act that created it"
    );
    assert_eq!(
        document.authentication,
        vec![written_out(&device.verifying_key().bytes())],
        "and the device it added, found by walking the chain rather than by being told"
    );
    assert_eq!(
        document.depth, 2,
        "two acts had to be fetched and walked, not one answer taken on trust"
    );
    assert!(document.id.starts_with("did:almena:dev:"));
}

#[tokio::test]
async fn opening_a_network_leaves_a_trust_anchor_that_resolves_over_the_wire() {
    // Everything on this network is checked against it, so it has to be reachable the same way
    // everything else is — from the very first act, before anybody has done anything at all.
    let node = Node::open(&opening(), &[], &key(5), key(6)).expect("nobody to join");
    let network = node.network().clone();
    let serving = Serving::new(node, limits());

    let anchor = ask(
        &serving,
        "GET",
        &format!("/object/{}", network.as_str()),
        Vec::new(),
    )
    .await;
    assert_eq!(anchor.status, 200);
    assert_eq!(anchor.state(), State::Here as u64);
}

#[tokio::test]
async fn an_identity_whose_history_this_build_cannot_read_stops_resolving() {
    // **The second of the three answers**, and the one that has to be told apart from the first:
    // saying *it does not exist* about something that does would be a lie, and serving the state
    // from before an act nobody understood would be a worse one, because nobody would notice.
    let node = Node::open(&opening(), &[], &key(5), key(6)).expect("nobody to join");
    let serving = Serving::new(node, limits());

    let control = key(9);
    let identity = an_identity(&control);
    let name = identity.object.name().clone();
    ask(&serving, "POST", "/acts", identity.to_bytes()).await;

    let mut newer = Operation {
        object: identity.object.clone(),
        previous: Some(Name::of(&identity.to_bytes())),
        kind: 9_999,
        version: 1,
        issued: Epoch::GENESIS,
        payload: BTreeMap::new(),
        signatures: Vec::new(),
    };
    let signature = control.sign(&newer.signing_bytes());
    newer.signatures.push(Signed {
        by: identity.object.clone(),
        key: control.verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });

    let stored = ask(&serving, "POST", "/acts", newer.to_bytes()).await;
    assert_eq!(
        stored.state(),
        State::Taken as u64,
        "an act nobody understands is still written down and passed on"
    );

    let after = ask(
        &serving,
        "GET",
        &format!("/object/{}", name.as_str()),
        Vec::new(),
    )
    .await;
    assert_eq!(after.state(), State::CannotResolve as u64);
    assert_eq!(after.status, 200, "it is an answer, not a failed request");
}

#[tokio::test]
async fn the_third_answer_exists_in_the_vocabulary_before_anything_produces_it() {
    // *Not here* — the object is asleep and its state is held elsewhere. Nothing produces it,
    // because nothing shares anything out yet, and it is in the vocabulary from the first day
    // anyway: a contract without it breeds clients that meet it for the first time in production
    // and treat it as an error.
    assert_eq!(State::NotHere as u64, 4);
    assert_ne!(State::NotHere as u64, State::DoesNotExist as u64);
    assert_ne!(State::NotHere as u64, State::CannotResolve as u64);
}
