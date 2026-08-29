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

    let Answer::Here(State::Node { key, .. }) =
        record.read().await.resolve(node.did().name(), epoch).answer
    else {
        return None;
    };

    Some(Carried {
        node: node.did().clone(),
        key,
        // What the record says about that key and that node, resolved the same way the key itself
        // is — never taken from the node being counted, which is the thing under suspicion.
        contradicted: record.read().await.contradicted(&key),
        reachable: record.read().await.reachable_at(node.did().name()),
        // Where the counter itself reached them, which is its own observation — never where the
        // node being counted says it was.
        found_at: record.read().await.found_at(&key),
        // What other nodes wrote down about it, whether the share-out deals it this act, and
        // whether it handed it over when asked. This counter has asked nobody and read no
        // summaries, and says so rather than filling the gaps with noughts.
        watched: almena_store::firm::Watched::Nobody,
        dealt: record
            .read()
            .await
            .falls_to_me(named, almena_node::COPIES_OF_HISTORY, epoch),
        serving: almena_store::firm::Serving::NotAsked,
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
    let act = account.called();
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

/// Put into the record proof that the third node signed two roots for one epoch.
///
/// Its own key signs both, because that is the only thing that can be proved against a node — and
/// all whoever publishes it has to do is point at them.
async fn caught_signing_twice(nodes: &[Arc<RwLock<Node>>], epoch: Epoch) {
    let (network, its_did, its_key) = {
        let caught = nodes[2].read().await;
        (caught.network().clone(), caught.did().clone(), caught.key())
    };
    let two_ways = |over: &[u8]| almena_store::root::Root {
        network: network.clone(),
        node: its_did.clone(),
        epoch,
        size: 4,
        root: almena_suite::digest::Digest::of(over),
    };
    let (one, other) = {
        let node = nodes[2].read().await;
        (
            node.publish(&two_ways(b"one history")),
            node.publish(&two_ways(b"another history")),
        )
    };

    assert!(
        nodes[0].write().await.write_down(&one, &other, epoch),
        "evidence anybody can check"
    );
    assert!(nodes[0].read().await.contradicted(&its_key));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_node_that_signed_two_histories_stops_being_one_of_the_trees() {
    // **What counting is for, from the other side.** Several trees would all have had to be wrong
    // in the same way, by people who never had to agree on anything. One kept by somebody already
    // shown to have signed two histories is not evidence of that — and the record is what says so,
    // not the node being counted and not whoever is doing the counting.
    let nodes = three_in_a_chain().await;

    let account = an_account(&key(9));
    let object = account.object.clone();
    let act = account.called();
    nodes[0]
        .write()
        .await
        .submit(&account, Epoch::GENESIS)
        .expect("taken");

    assert!(
        until(|| async { all_hold(&nodes, object.name()).await }).await,
        "the act reached all three; written: {:?}",
        written(&nodes).await
    );
    assert!(until(|| async { record_names_them_all(&nodes).await }).await);

    let after = Epoch::GENESIS.plus(Epochs(1)).expect("no overflow");
    for node in &nodes {
        node.write().await.close_owed(&[after]);
    }

    let network = nodes[0].read().await.network().clone();
    let mut answers = Vec::new();
    for node in &nodes {
        if let Some(said) = what_it_says(node, &nodes[0], &act, after).await {
            answers.push(said);
        }
    }
    assert_eq!(carried_by(&account, &network, &answers).0, 3);

    // Now one of them is shown to have signed two roots for one epoch, into the record itself.
    caught_signing_twice(&nodes, after).await;

    // And the same answers now count for two, with the third refused for a stated reason.
    let mut again = Vec::new();
    for node in &nodes {
        if let Some(said) = what_it_says(node, &nodes[0], &act, after).await {
            again.push(said);
        }
    }
    let (count, refused) = carried_by(&account, &network, &again);
    assert_eq!(count, 2, "two trees left, not three");
    assert_eq!(refused.len(), 1);
    assert_eq!(
        refused[0].1,
        almena_store::firm::NotCounted::Contradicted,
        "and it says why rather than quietly counting one fewer"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn how_spread_out_the_trees_are_is_a_fact_anybody_can_read() {
    // **It does not prove independence, and nothing can.** Nodes are open and nobody says who runs
    // one. What this gives somebody who wants independence is the ability to **ask for it instead
    // of assuming it**: three trees from one place are one place's word, whoever signed them.
    let nodes = three_in_a_chain().await;

    let account = an_account(&key(9));
    let object = account.object.clone();
    let act = account.called();
    nodes[0]
        .write()
        .await
        .submit(&account, Epoch::GENESIS)
        .expect("taken");
    assert!(until(|| async { all_hold(&nodes, object.name()).await }).await);
    assert!(until(|| async { record_names_them_all(&nodes).await }).await);

    // Two of them say they are in one place and the third somewhere else.
    let where_each = [
        "/ip4/198.51.100.7/tcp/4001",
        "/ip4/198.51.100.7/tcp/4002",
        "/ip4/203.0.113.9/tcp/4001",
    ];
    for (node, at) in nodes.iter().zip(where_each) {
        let said = node.write().await.offering(
            almena_node::Saying {
                offers: &std::collections::BTreeSet::new(),
                version: 1,
                reachable: &std::collections::BTreeSet::from([at.to_owned()]),
            },
            Epoch::GENESIS,
        );
        assert!(said, "a node may say where it is");
    }
    assert!(until(|| async { all_say_where(&nodes).await }).await);

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

    let spread = almena_store::firm::spread_of(&answers);
    assert_eq!(spread.nodes, 3, "three separate trees, kept by three keys");
    assert_eq!(spread.places, 2, "and only two places between them");
    assert_eq!(spread.nowhere, 0, "all three said where they are");
}

/// Whether every node's record says where every node is.
async fn all_say_where(nodes: &[Arc<RwLock<Node>>]) -> bool {
    let record = nodes[0].read().await;
    for node in nodes {
        let named = node.read().await.did().name().clone();
        if record.reachable_at(&named).is_empty() {
            return false;
        }
    }
    true
}

#[tokio::test(flavor = "multi_thread")]
async fn where_a_node_was_really_reached_is_this_nodes_own_and_not_the_records() {
    // **Publishing an address costs nothing; answering on one had to work.** So the two are kept
    // apart: what a node says about itself goes in the record everybody holds, and where this one
    // found it stays here. Folding the second into the first would make one node's experience
    // everybody's truth.
    let nodes = three_in_a_chain().await;

    let account = an_account(&key(9));
    let object = account.object.clone();
    nodes[0]
        .write()
        .await
        .submit(&account, Epoch::GENESIS)
        .expect("taken");
    assert!(until(|| async { all_hold(&nodes, object.name()).await }).await);
    assert!(until(|| async { record_names_them_all(&nodes).await }).await);

    // The middle node dialled the first and the last dialled the middle, so each of those has
    // really reached somebody — and says so from its own experience.
    assert!(
        until(|| async { somebody_was_reached(&nodes).await }).await,
        "a node that dialled somebody has reached them somewhere"
    );

    // And nobody said where they were, so the record has nothing about it — which is a different
    // fact from having been reached, and stays a different fact.
    for node in &nodes {
        let held = node.read().await;
        for other in &nodes {
            let named = other.read().await.did().name().clone();
            assert!(
                held.reachable_at(&named).is_empty(),
                "the record carries only what a node published about itself"
            );
        }
    }
}

/// Whether any of them has actually reached any of the others.
async fn somebody_was_reached(nodes: &[Arc<RwLock<Node>>]) -> bool {
    for node in nodes {
        let held = node.read().await;
        for other in nodes {
            let its_key = other.read().await.key();
            if !held.found_at(&its_key).is_empty() {
                return true;
            }
        }
    }
    false
}

#[tokio::test(flavor = "multi_thread")]
async fn a_network_smaller_than_the_copies_it_wants_keeps_everything_and_that_is_right() {
    // **The effective share is never more than there are nodes.** Three nodes and five copies
    // wanted means every act falls to all three, so nothing is let go — and that is the rule
    // working, not the rule failing. A network does not start sharing out until it is bigger than
    // the number of copies it is trying to keep.
    //
    // What is under test here is that letting go **takes nothing away** while that is true: every
    // line stays, every tree is the one its node signed over, and nothing stops being held.
    let nodes = three_in_a_chain().await;

    let mut accounts = Vec::new();
    for which in 0..24u8 {
        let account = an_account(&key(60 + which));
        nodes[0]
            .write()
            .await
            .submit(&account, Epoch::GENESIS)
            .expect("taken");
        accounts.push(account);
    }
    assert!(
        until(|| async { all_hold(&nodes, accounts[23].object.name()).await }).await,
        "everything reached all three first"
    );
    assert!(until(|| async { record_names_them_all(&nodes).await }).await);

    let before: Vec<usize> = written(&nodes).await;
    for node in &nodes {
        node.write()
            .await
            .let_go_of_what_is_not_mine(Epoch::GENESIS);
    }

    // Every node kept every line, so every tree is the one it signed over.
    assert_eq!(written(&nodes).await, before, "and no entry went anywhere");

    // Nobody let go of anything, because everything falls to all three.
    for node in &nodes {
        assert!(
            node.read().await.not_got().is_empty(),
            "a network smaller than the copies it wants keeps everything"
        );
    }

    // And every act is still held by every one of them.
    for account in &accounts {
        for node in &nodes {
            assert!(
                node.read().await.holds(&account.called()),
                "nothing stopped being held"
            );
        }
    }
}
