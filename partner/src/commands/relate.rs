//! `relate`: take up a relationship with a holder who showed a code.
//!
//! The holder's identifier arrived out of band — a link, a code read off a screen — and carries
//! the holder's keys and mediators inside it. This end mints an identifier of its own on a key
//! made for this relationship and used in no other, tells its own mediator to route it, and
//! **writes first**: an introduction is a name with nobody at the other end until a message
//! arrives, and the message is what says who arrived. The hello carries nothing; what it is for
//! is being sealed with the key this end's identifier names.

use crate::commands::Partner;
use crate::directory::hex;
use crate::failed::Failed;
use crate::node::Node;
use crate::post::message::{HELLO, Message};
use crate::post::peer::Peer;
use crate::relations::Relations;
use crate::{chain, link};

/// What `relate` leaves behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Related {
    /// What this end is called in the new relationship, which is what the holder now holds.
    pub mine: String,
    /// What the far end is called.
    pub theirs: String,
    /// How many of the holder's mediators took the hello.
    pub reached: usize,
}

/// Take up a relationship with whoever the link names, delivering at those mediators of ours.
///
/// `mediators` are the services this end's identifier names as where to deliver to it, each
/// written `host:port 12D3KooW…` — the address and the identity of the node that runs it, as the
/// service travels inside the identifier — or `host:port` alone for a mediator on the node this
/// partner was told. Empty means that node itself.
///
/// # Errors
///
/// `link_not_a_meeting`, `relate_not_a_peer`, `relate_mediator_not_one`, `partner_no_entropy`,
/// what declaring fails with, and `send_nowhere_to_send` when no mediator of the holder's took the
/// hello — in which case the relationship is still kept, because the far end may simply be
/// unreachable this minute.
pub async fn run(partner: &Partner, link: &str, mediators: Vec<String>) -> Result<Related, Failed> {
    let (keys, account) = partner.identity()?;
    let theirs = link::met(link)?;
    let far = Peer::read(&theirs).map_err(|_| Failed::new("relate_not_a_peer"))?;
    let theirs = far.to_did();

    let mediators = if mediators.is_empty() {
        vec![(partner.node.address.clone(), partner.node.peer.clone())]
    } else {
        mediators
            .iter()
            .map(|named| mediator_named(named, &partner.node))
            .collect::<Result<Vec<_>, _>>()?
    };
    let secret = drawn_scalar()?;
    let key =
        p256::SecretKey::from_slice(&secret).map_err(|_| Failed::new("partner_no_entropy"))?;
    let mine = Peer::on(&key.public_key(), mediators);
    let relation = Relations::minted(&secret, &mine, Some(theirs.clone()));
    let mine = relation.mine.clone();

    let mut relations = partner.directory.relations()?;
    relations.keep(relation.clone());
    partner.directory.keep_relations(&relations)?;
    log::info!("relation_kept mine={mine} theirs={theirs}");

    // **Before writing to them**, because the answer comes back to the address just minted and a
    // mediator that has not been told about it has nowhere to put one.
    let network = chain::network(&partner.node).await?;
    partner.declare(&keys, &account, network.epoch).await?;
    log::info!("relations_declared count={}", relations.addresses().len());

    let delivered = partner
        .send(&relation, HELLO, &mine, serde_json::json!({}))
        .await?;
    log::info!(
        "hello_delivered theirs={theirs} reached={} asked={}",
        delivered.reached,
        delivered.asked
    );
    Ok(Related {
        mine,
        theirs,
        reached: delivered.reached,
    })
}

/// One mediator as the command line names it: `host:port 12D3KooW…`, or `host:port` for one on
/// the node this partner was told.
///
/// Checked as a node would be dialled — an address with a port and a peer that is a key — so that
/// what goes into this end's identifier is somewhere a counterparty can actually write to.
fn mediator_named(named: &str, ours: &Node) -> Result<(String, String), Failed> {
    let (address, peer) = named
        .trim()
        .split_once(' ')
        .map_or((named.trim(), ours.peer.as_str()), |(address, peer)| {
            (address.trim(), peer.trim())
        });
    let there = Node::at(address, peer).map_err(|_| Failed::new("relate_mediator_not_one"))?;
    Ok((there.address, there.peer))
}

/// Thirty-two bytes that are a P-256 scalar.
fn drawn_scalar() -> Result<[u8; 32], Failed> {
    for _ in 0..8 {
        let mut secret = [0u8; 32];
        getrandom::fill(&mut secret).map_err(|_| Failed::new("partner_no_entropy"))?;
        if p256::SecretKey::from_slice(&secret).is_ok() {
            return Ok(secret);
        }
    }
    Err(Failed::new("partner_no_entropy"))
}

/// The hello, as it would be sealed: kept here so that the message shape has one author.
#[must_use]
pub fn hello(mine: &str, theirs: &str) -> Message {
    Message::new(mine, HELLO, mine, theirs, serde_json::json!({}))
}

/// The public half of a relationship key, for the records.
#[must_use]
pub fn shown(secret: &[u8; 32]) -> String {
    p256::SecretKey::from_slice(secret)
        .map(|key| hex(&crate::post::peer::written(&key.public_key())))
        .unwrap_or_default()
}
