//! Two nodes left to it, and an act that reaches one of them from the other.
//!
//! The difference from the other test here is that **nobody drives them**. There is no loop in this
//! file asking or answering: both nodes are handed to the thing that keeps them up to date and are
//! left alone, and the assertion is that a record which only one of them held ends up in both.
//!
//! That is the whole of what a mesh is for, and the first point at which *several nodes* means
//! anything at all.

use std::sync::Arc;

use almena_node::{Answer, Epoch, Node, Opening, SigningKey, Which};
use tokio::sync::RwLock;

/// How long the two are given before it is called a failure.
///
/// Well under the interval at which they ask each other anything, so that an act arriving inside it
/// arrived because somebody said so rather than because somebody got round to asking.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(20);

/// How often a node asks whoever it knows.
///
/// **Deliberately longer than this test waits.** If an act still arrives, it arrived because
/// somebody said it had — which is the thing under test. Left short, a passing test would prove
/// only that the asking interval is short.
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

/// Put both nodes on the mesh and hand them over.
///
/// **After this nothing asks anybody anything on their behalf.** The second is given the first's
/// address the way a zone would give it, and both are left to keep themselves up to date.
async fn leave_them_to_it(
    network: &str,
    holder: &Arc<RwLock<Node>>,
    joiner: &Arc<RwLock<Node>>,
) -> almena_mesh::keeping::Watched {
    let (Ok(mut first), Ok(mut second)) = (
        almena_mesh::listen(&key(6), network, 0),
        almena_mesh::listen(&key(7), network, 0),
    ) else {
        panic!("both should get a place on the mesh")
    };

    let Ok(address) = tokio::time::timeout(PATIENCE, reachable(&mut first)).await else {
        panic!("the first should be listening well within that")
    };
    if tokio::time::timeout(PATIENCE, reachable(&mut second))
        .await
        .is_err()
    {
        panic!("the second should be listening well within that")
    }

    tokio::spawn(almena_mesh::keeping::keeping_up(
        first,
        Arc::clone(holder),
        Vec::new(),
        || Epoch::GENESIS,
        OFTEN,
    ));
    let joiner = Arc::clone(joiner);
    let watched = almena_mesh::keeping::Watched::default();
    let watching = watched.clone();
    tokio::spawn(async move {
        let mut second = second;
        almena_mesh::keeping::watching(
            almena_mesh::keeping::Present {
                listening: &mut second,
                node: &joiner,
                watched: &watching,
            },
            vec![address],
            || Epoch::GENESIS,
            OFTEN,
        )
        .await;
    });
    watched
}

/// Wait for something to become true, and say whether it did.
///
/// The nodes are left to themselves, so nothing here can be told when they have done something —
/// only asked whether they have yet.
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

#[tokio::test(flavor = "multi_thread")]
async fn an_act_one_node_took_reaches_the_other_without_anybody_asking() {
    let government = key(5);
    let holder = Node::open(&at(), &[], &government, key(6)).expect("nobody to join");
    let joiner = Node::open(&at(), &[], &government, key(7)).expect("nobody to join");
    assert_eq!(holder.network(), joiner.network(), "one network");
    let network = holder.network().as_str().to_owned();

    let account = an_account(&key(9));
    let named = account.object.name().clone();
    assert!(
        matches!(
            joiner.resolve(&named, Epoch::GENESIS).answer,
            Answer::DoesNotExist
        ),
        "and the second one has never heard of the account"
    );

    let holder = Arc::new(RwLock::new(holder));
    let joiner = Arc::new(RwLock::new(joiner));

    // What it witnesses is the other test's subject; this one is only about the act arriving.
    let _ = leave_them_to_it(&network, &holder, &joiner).await;

    // Told to one of them, the way anybody tells a node anything.
    holder
        .write()
        .await
        .submit(&account, Epoch::GENESIS)
        .expect("taken");

    assert!(
        until(|| async {
            matches!(
                joiner.read().await.resolve(&named, Epoch::GENESIS).answer,
                Answer::Here(_)
            )
        })
        .await,
        "the account reached the other node on its own"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn what_one_node_signed_comes_back_witnessed_by_the_other() {
    // **How a node makes its own honesty checkable by people who cannot see its tree.** Showing one
    // root to one person and another to somebody else would leave two roots carrying different
    // witnesses, and the pair is the proof. So the word has to come back.
    let government = key(5);
    let holder = Node::open(&at(), &[], &government, key(6)).expect("nobody to join");
    let joiner = Node::open(&at(), &[], &government, key(7)).expect("nobody to join");
    let network = holder.network().as_str().to_owned();

    let holder = Arc::new(RwLock::new(holder));
    let joiner = Arc::new(RwLock::new(joiner));
    let watched = leave_them_to_it(&network, &holder, &joiner).await;

    assert!(
        until(|| async { !watched.witnessed().await.everybody().is_empty() }).await,
        "a signed root arrived from the other node and checked out against its own key"
    );
    assert!(
        watched.witnessed().await.contradictions().is_empty(),
        "and nobody said two things about one epoch"
    );

    assert!(
        until(|| async {
            let node = holder.read().await;
            node.root_at(Epoch::GENESIS)
                .is_some_and(|root| !node.publish(root).witnesses.is_empty())
        })
        .await,
        "somebody else's word that they saw this node's root came back to it"
    );

    let node = holder.read().await;
    let root = node.root_at(Epoch::GENESIS).expect("a root");
    assert!(
        node.publish(root)
            .witnesses
            .iter()
            .all(|seen| seen.checks(root)),
        "and every one of them is really that key's word about that root"
    );
}
