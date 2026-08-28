//! Three nodes, one act, and somebody counting how many trees carry it.
//!
//! **This is what *several nodes* is for.** Everything else in this crate moves bytes; this is the
//! thing the moving is in aid of — an act told to one node ends up in three separate trees, kept by
//! three separate keys, each of which can be shown to carry it.
//!
//! # What the counter is allowed to know
//!
//! Only what somebody asking over the interface could get: each node's signed root for an epoch,
//! its proof that the act is in that root, and — resolved **from the record** — the key that node
//! is supposed to have. It is never handed a node's key by the node whose answer is being checked,
//! because that is the thing under suspicion.
//!
//! It does not use the transport. `exit_criterion.rs` in `almena-serve` exercises that; what is
//! under test here is the counting and the propagation that makes there be anything to count.

use std::sync::Arc;

use almena_node::{Answer, Epoch, Node, Opening, SigningKey, State, Which};
use almena_store::firm::{Carried, carried_by};
use almena_time::Epochs;
use tokio::sync::RwLock;

/// How long the three are given before it is called a failure.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(20);

/// How often a node asks whoever it knows.
///
/// **Longer than this test waits**, so that anything arriving arrived because somebody said so.
const OFTEN: std::time::Duration = std::time::Duration::from_secs(120);

/// A key nobody else in this test has.
fn key(seed: u8) -> SigningKey {
    SigningKey::from_secret([seed; 32])
}

/// A development network with a fixed clock, so a test is never about what time it is.
fn at() -> Opening {
    Opening {
        which: Which::Development,
        beginning: Epoch::GENESIS,
        began: 1_800_000_000,
    }
}

/// A holder account, signed by the key that will control it.
fn an_account(control: &SigningKey) -> almena_format::operation::Operation {
    use almena_format::cbor::Value;
    use almena_format::identifier::Network;
    use almena_format::operation::{Signed, create};
    use std::collections::BTreeMap;

    let public = control.verifying_key().bytes();
    let mut operation = create(
        Network::Development,
        almena_store::kind::Kind::HOLDER_CREATE.number(),
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

/// Run a node until it says where it can be reached, on an address this test may dial.
async fn reachable(listening: &mut almena_mesh::Listening) -> libp2p::Multiaddr {
    loop {
        if let almena_mesh::Happened::Reachable(address) = listening.next().await
            && address.to_string().starts_with("/ip4/127.0.0.1")
        {
            return address;
        }
    }
}

/// Wait for something to become true, and say whether it did.
async fn until<F, Fut>(yet: F) -> bool
where
    F: Fn() -> Fut,
    Fut: Future<Output = bool>,
{
    tokio::time::timeout(PATIENCE, async {
        while !yet().await {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .is_ok()
}

/// What one node would answer somebody counting, using only what it offers anybody.
///
/// The key is resolved from the record rather than taken from the node being asked — which is the
/// whole difference between counting and tallying. A client would resolve it wherever it liked; the
/// answer is the same everywhere, because it is the record's and not anybody's opinion.
async fn what_it_says(
    node: &Arc<RwLock<Node>>,
    record: &Arc<RwLock<Node>>,
    named: &almena_format::identifier::Name,
    epoch: Epoch,
) -> Option<Carried> {
    let node = node.read().await;
    let (at, path, published) = node.inclusion_in(named, epoch, epoch).answer?;

    let Answer::Here(State::Node { key }) =
        record.read().await.resolve(node.did().name(), epoch).answer
    else {
        return None;
    };

    Some(Carried {
        node: node.did().clone(),
        key,
        published,
        at,
        path,
    })
}

/// How much each node has written down, so that a failure says which one is behind.
async fn written(nodes: &[Arc<RwLock<Node>>]) -> Vec<usize> {
    let mut out = Vec::new();
    for node in nodes {
        out.push(node.read().await.written());
    }
    out
}

/// Three nodes on one network, each dialling only the one before it.
///
/// **A chain and not a star**, so that an act has to travel two hops to reach the last. It only
/// gets there because each node that takes it tells the ones it knows — which is the difference
/// between a mesh and everybody talking to one server.
async fn three_in_a_chain() -> Vec<Arc<RwLock<Node>>> {
    let government = key(5);
    let mut nodes = Vec::new();
    let mut places = Vec::new();

    for seed in [6u8, 7, 8] {
        let Ok(node) = Node::open(&at(), &[], &government, key(seed)) else {
            panic!("there is nobody to join")
        };
        let network = node.network().as_str().to_owned();
        let Ok(mut place) = almena_mesh::listen(&key(seed), &network, 0) else {
            panic!("every node should get a place on the mesh")
        };
        let Ok(address) = tokio::time::timeout(PATIENCE, reachable(&mut place)).await else {
            panic!("every node should be listening well within that")
        };
        nodes.push(Arc::new(RwLock::new(node)));
        places.push((place, address));
    }

    let mut behind: Vec<libp2p::Multiaddr> = Vec::new();
    for (which, (place, address)) in places.into_iter().enumerate() {
        tokio::spawn(almena_mesh::keeping::keeping_up(
            place,
            Arc::clone(&nodes[which]),
            behind,
            || Epoch::GENESIS,
            OFTEN,
        ));
        behind = vec![address];
    }
    nodes
}

/// Whether every node resolves that name.
async fn all_hold(nodes: &[Arc<RwLock<Node>>], named: &almena_format::identifier::Name) -> bool {
    for node in nodes {
        if !matches!(
            node.read().await.resolve(named, Epoch::GENESIS).answer,
            Answer::Here(_)
        ) {
            return false;
        }
    }
    true
}

/// Whether the record names every one of these nodes.
///
/// **Until it does there is nobody to count.** A counter resolves each node's key from the record;
/// one whose own announcement has not arrived yet is one whose answer cannot be checked, and so
/// does not count — correctly, and not what is under test here.
async fn record_names_them_all(nodes: &[Arc<RwLock<Node>>]) -> bool {
    let record = nodes[0].read().await;
    for node in nodes {
        let named = node.read().await.did().name().clone();
        if !matches!(
            record.resolve(&named, Epoch::GENESIS).answer,
            Answer::Here(State::Node { .. })
        ) {
            return false;
        }
    }
    true
}

#[tokio::test(flavor = "multi_thread")]
async fn an_act_told_to_one_node_ends_up_in_three_trees() {
    let nodes = three_in_a_chain().await;

    let account = an_account(&key(9));
    // Two different names, and they are not interchangeable: an object is called by the hash of the
    // act that created it *without* its own name and signatures inside, and an act is called by the
    // hash of all of it. Resolving takes the first; proving where it was written takes the second.
    let object = account.object.name().clone();
    let act = almena_format::identifier::Name::of(&account.to_bytes());
    nodes[0]
        .write()
        .await
        .submit(&account, Epoch::GENESIS)
        .expect("taken");

    assert!(
        until(|| async { all_hold(&nodes, &object).await }).await,
        "the act reached all three; written: {:?}",
        written(&nodes).await
    );
    assert!(
        until(|| async { record_names_them_all(&nodes).await }).await,
        "the record names all three nodes"
    );

    // Each closes an epoch after it, so each has signed a tree that carries it.
    let after = Epoch::GENESIS.plus(Epochs(1)).expect("no overflow");
    for node in &nodes {
        node.write().await.close_owed(&[after]);
    }

    let mut answers = Vec::new();
    for node in &nodes {
        if let Some(said) = what_it_says(node, &nodes[0], &act, after).await {
            answers.push(said);
        }
    }

    let network = nodes[0].read().await.network().clone();
    let (count, refused) = carried_by(&account, &network, &answers);

    assert!(refused.is_empty(), "nothing was thrown out: {refused:?}");
    assert_eq!(
        count, 3,
        "three separate trees, kept by three separate keys, each shown to carry it"
    );
}
