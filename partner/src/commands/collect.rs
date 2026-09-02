//! `collect`: bring in this partner's post, and write down what it says.
//!
//! Three steps and not one: ask every mediator, put together what they said, and only then tell
//! each of them what arrived. Everything is written down before anything is confirmed, because a
//! mediator drops a message on being told, and a confirmation sent first would lose a message to
//! a crash in the one direction nobody can recover from.
//!
//! What a message opens into is believed only once the key that sealed it is one the far end's
//! identifier carries. Opening proves who sealed it; the relationship says whether that is who
//! this end is talking to.

use std::collections::BTreeSet;

use almena_sdk::errand;

use crate::chain;
use crate::commands::Partner;
use crate::failed::Failed;
use crate::post::envelope::{self, Envelope};
use crate::post::mediator::{self, Collection, Held};
use crate::post::message::{HELLO, Message};
use crate::post::peer::Peer;
use crate::relations::{Relations, answered_by};

/// One message, as far as this partner could get with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arrived {
    /// What it is called.
    pub called: String,
    /// Which mediator it came from.
    pub mediator: String,
    /// What it said, when it opened and its sender was who the relationship is with.
    pub said: Option<Message>,
    /// Why it was not taken as said, when it was not: an identifier, never a sentence.
    pub set_aside: Option<String>,
}

/// One round of collecting, from every mediator this partner's identifiers name.
///
/// # Errors
///
/// What the directory or the node fails with. A mediator that did not answer is logged and
/// passed over rather than failing the round: it is not a mediator with an empty mailbox.
pub async fn run(partner: &Partner) -> Result<Vec<Arrived>, Failed> {
    let (keys, account) = partner.identity()?;
    let mut relations = partner.directory.relations()?;
    let network = chain::network(&partner.node).await?;
    let device = keys.device_key()?;
    partner.declare(&keys, &account, network.epoch).await?;

    let mut heard: Vec<((String, String), Collection)> = Vec::new();
    for service in mediators_of(&relations) {
        let Ok(there) = partner.mediator_at(&service) else {
            log::info!("mediator_not_dialled service={}", service.0);
            continue;
        };
        match mediator::collect(&there, &account, &device, network.epoch).await {
            Ok(collection) => heard.push((service, collection)),
            Err(why) => log::info!("mediator_silent service={} reason={why}", service.0),
        }
    }

    let mut arrived = Vec::new();
    let mut seen = BTreeSet::new();
    for (service, collection) in &heard {
        for held in collection.waiting.iter().chain(&collection.ringing) {
            if !seen.insert(held.called.clone()) {
                continue;
            }
            let one = opened(held, &service.0, &mut relations);
            noted(partner, &one, network.epoch)?;
            arrived.push(one);
        }
    }
    partner.directory.keep_relations(&relations)?;

    // **Only now**, once everything is written down.
    for (service, collection) in &heard {
        let names: Vec<String> = collection
            .waiting
            .iter()
            .chain(&collection.ringing)
            .map(|held| held.called.clone())
            .collect();
        if names.is_empty() {
            continue;
        }
        if let Ok(there) = partner.mediator_at(service)
            && let Err(why) =
                mediator::confirm(&there, &account, &device, names, network.epoch).await
        {
            log::info!("post_not_confirmed mediator={} reason={why}", service.0);
        }
    }
    log::info!("post_collected count={}", arrived.len());
    Ok(arrived)
}

/// Every service this partner's own identifiers name, once each: an address and the node it is
/// pinned to.
fn mediators_of(relations: &Relations) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = relations
        .all()
        .iter()
        .filter_map(|relation| Peer::read(&relation.mine).ok())
        .flat_map(|mine| mine.delivered_to)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Open one message, and say what it is, or why it was set aside.
fn opened(held: &Held, mediator: &str, relations: &mut Relations) -> Arrived {
    let set_aside = |why: &str| Arrived {
        called: held.called.clone(),
        mediator: mediator.to_owned(),
        said: None,
        set_aside: Some(why.to_owned()),
    };
    let Some(relation) = relations.addressed(&held.relation).cloned() else {
        return set_aside("post_not_one_of_mine");
    };
    let Ok(key) = relation.key() else {
        return set_aside("relations_key_invalid");
    };
    let Ok(sealed) = serde_json::from_slice::<Envelope>(&held.sealed) else {
        return set_aside("post_not_an_envelope");
    };
    let (body, sealed_by) = match envelope::open(&key, &sealed) {
        Ok(opened) => opened,
        Err(why) => return set_aside(&format!("post_not_opened why={why:?}")),
    };
    let Ok(message) = serde_json::from_slice::<Message>(&body) else {
        return set_aside("post_not_a_message");
    };
    // The far end this relationship is with, or — on an introduction nobody has answered — the
    // far end the message proves itself to be.
    let from_them = match relation.theirs.as_deref() {
        Some(_) => relations.from_them(&held.relation, &sealed_by).is_ok(),
        None => match answered_by(&message.from, &sealed_by) {
            Some(theirs) => {
                let mut met = relation.clone();
                met.theirs = Some(theirs);
                relations.keep(met);
                true
            }
            None => false,
        },
    };
    if !from_them {
        return set_aside("post_not_from_them");
    }
    Arrived {
        called: held.called.clone(),
        mediator: mediator.to_owned(),
        said: Some(message),
        set_aside: None,
    }
}

/// Write down what a message said, where it is one this partner keeps a record of.
fn noted(partner: &Partner, arrived: &Arrived, epoch: u64) -> Result<(), Failed> {
    let Some(message) = &arrived.said else {
        log::info!(
            "post_set_aside called={} why={}",
            arrived.called,
            arrived.set_aside.as_deref().unwrap_or_default()
        );
        return Ok(());
    };
    match message.kind.as_str() {
        HELLO => log::info!("hello_arrived from={}", message.from),
        kind if kind == errand::kind::DECIDED => {
            let (Some(credential), Some(taken)) = (
                message.body["credential"].as_str(),
                message.body["taken"].as_bool(),
            ) else {
                log::info!("decided_unreadable from={}", message.from);
                return Ok(());
            };
            let mut issued = partner.directory.issued()?;
            match issued.get_mut(credential) {
                Some(record) => record.decided = Some(taken),
                None => log::info!("decided_about_nothing_issued credential={credential}"),
            }
            partner.directory.keep_issued(&issued)?;
            log::info!("decided credential={credential} taken={taken} epoch={epoch}");
        }
        other => log::info!("post_kind_unknown kind={other} from={}", message.from),
    }
    Ok(())
}
