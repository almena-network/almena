//! Talking to a mediator: declaring, delivering, taking, confirming.
//!
//! **A mediator is a letterbox, and this is the walk to it.** It hands sealed bytes over, brings
//! sealed bytes back, and confirms only once what came back is written down — because a mediator
//! drops a message on being told it arrived, and a confirmation sent before the message was kept
//! would lose it to a crash in the one direction nobody can recover from.
//!
//! # The relationship named in a delivery is the recipient's
//!
//! A mediator files a delivery under the relationship the letter names, and only when the
//! recipient declared that relationship — which it does by its **own** peer identifiers. So the
//! letter carries the address the sender was given, twice: once as where it goes and once as the
//! relationship it is filed under. Naming the sender's own identifier there would land every
//! message at the doorbell, where nothing opens it.

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::Did;
use almena_mailbox::asking::{Asking, Errand};
use almena_suite::p256;
use almena_time::Epoch;

use crate::answer::State;
use crate::failed::Failed;
use crate::node::{self, Node};

/// Where each part of a delivery goes.
mod letter {
    /// Which relationship it is filed under: the recipient's own identifier.
    pub const RELATION: u64 = 1;
    /// The message itself, sealed.
    pub const SEALED: u64 = 3;
    /// How long the sender asks it be held.
    pub const FOR: u64 = 5;
}

/// Where each part of a collection comes back.
mod collected {
    /// What is waiting in this device's mailbox.
    pub const WAITING: u64 = 1;
    /// What is waiting at the doorbell.
    pub const RINGING: u64 = 3;
}

/// Where each part of one message comes back.
mod message {
    /// Its name.
    pub const CALLED: u64 = 1;
    /// Which relationship it came from.
    pub const RELATION: u64 = 3;
    /// The sealed bytes.
    pub const SEALED: u64 = 5;
    /// The first epoch at which it is no longer held.
    pub const UNTIL: u64 = 7;
}

/// How long a message asks to be held for, in epochs: three days, as the holder's app asks.
pub const HELD_FOR: u64 = 72;

/// One message, as a mediator handed it over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// What it is called, which is the hash of what is in it.
    pub called: String,
    /// Which relationship it came from, which is this end's own identifier.
    pub relation: String,
    /// The sealed bytes.
    pub sealed: Vec<u8>,
    /// The first epoch at which it is no longer held.
    pub until: u64,
}

/// What one mediator says is waiting.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Collection {
    /// What is in this device's mailbox, oldest first.
    pub waiting: Vec<Held>,
    /// What is at the doorbell, which belongs to the account and not to a relationship.
    pub ringing: Vec<Held>,
}

/// Tell a mediator which relationships this account has, so that each of them has a floor.
///
/// # Errors
///
/// `mediator_refused`, `mediator_not_one`, or a `node_*` word.
pub async fn carry(
    at: &Node,
    whose: &Did,
    device: &p256::SigningKey,
    relations: Vec<String>,
    epoch: u64,
) -> Result<(), Failed> {
    let answer = node::post(
        at,
        "/post",
        &asking(Errand::Carry, whose, device, relations, epoch),
    )
    .await?;
    match answer.state {
        State::Taken => Ok(()),
        State::NoSuchQuestion => Err(Failed::new("mediator_not_one")),
        _ => Err(answer.refused("mediator_refused")),
    }
}

/// Take what is waiting, without saying it arrived.
///
/// # Errors
///
/// `mediator_refused`, `mediator_not_one`, or a `node_*` word.
pub async fn collect(
    at: &Node,
    whose: &Did,
    device: &p256::SigningKey,
    epoch: u64,
) -> Result<Collection, Failed> {
    let answer = node::post(
        at,
        "/post",
        &asking(Errand::Collect, whose, device, Vec::new(), epoch),
    )
    .await?;
    match answer.state {
        State::Here => Ok(read(answer.map())),
        State::NoSuchQuestion => Err(Failed::new("mediator_not_one")),
        _ => Err(answer.refused("mediator_refused")),
    }
}

/// Say those messages arrived, so that this mediator stops holding them.
///
/// # Errors
///
/// `mediator_refused`, `mediator_not_one`, or a `node_*` word.
pub async fn confirm(
    at: &Node,
    whose: &Did,
    device: &p256::SigningKey,
    names: Vec<String>,
    epoch: u64,
) -> Result<(), Failed> {
    let answer = node::post(
        at,
        "/post",
        &asking(Errand::Confirm, whose, device, names, epoch),
    )
    .await?;
    match answer.state {
        State::Taken => Ok(()),
        State::NoSuchQuestion => Err(Failed::new("mediator_not_one")),
        _ => Err(answer.refused("mediator_refused")),
    }
}

/// Hand a sealed message to somebody's mediator, addressed to their own peer identifier.
///
/// Nobody is asked who this is from: the address is the authorisation.
///
/// # Errors
///
/// `mediator_refused`, `mediator_not_one`, or a `node_*` word.
pub async fn deliver(at: &Node, to: &str, sealed: &[u8], held_for: u64) -> Result<(), Failed> {
    let envelope = Value::Map(BTreeMap::from([
        (letter::RELATION, Value::Text(to.to_owned())),
        (letter::SEALED, Value::Bytes(sealed.to_vec())),
        (letter::FOR, Value::Uint(held_for)),
    ]))
    .to_bytes();
    let answer = node::post(at, &format!("/post/{to}"), &envelope).await?;
    match answer.state {
        State::Taken => Ok(()),
        State::NoSuchQuestion => Err(Failed::new("mediator_not_one")),
        _ => Err(answer.refused("mediator_refused")),
    }
}

/// One asking, signed by the device, as the node reads it.
fn asking(
    errand: Errand,
    whose: &Did,
    device: &p256::SigningKey,
    names: Vec<String>,
    epoch: u64,
) -> Vec<u8> {
    Asking {
        errand,
        whose: whose.clone(),
        device: Vec::new(),
        at: Epoch::new(epoch),
        names,
        signed: Vec::new(),
    }
    .signed_by(device)
    .to_bytes()
}

/// What a mediator handed over, with anything unreadable left out rather than refused.
fn read(payload: Option<&BTreeMap<u64, Value>>) -> Collection {
    let Some(fields) = payload else {
        return Collection::default();
    };
    Collection {
        waiting: messages(fields.get(&collected::WAITING)),
        ringing: messages(fields.get(&collected::RINGING)),
    }
}

/// A list of messages, with anything unreadable left out.
fn messages(value: Option<&Value>) -> Vec<Held> {
    let Some(Value::Array(listed)) = value else {
        return Vec::new();
    };
    listed
        .iter()
        .filter_map(|one| {
            let Value::Map(fields) = one else {
                return None;
            };
            let Some(Value::Text(called)) = fields.get(&message::CALLED) else {
                return None;
            };
            let Some(Value::Text(relation)) = fields.get(&message::RELATION) else {
                return None;
            };
            let Some(Value::Bytes(sealed)) = fields.get(&message::SEALED) else {
                return None;
            };
            let until = match fields.get(&message::UNTIL) {
                Some(Value::Uint(until)) => *until,
                _ => 0,
            };
            Some(Held {
                called: called.clone(),
                relation: relation.clone(),
                sealed: sealed.clone(),
                until,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{collected, message, read};
    use almena_format::cbor::Value;
    use std::collections::BTreeMap;

    #[test]
    fn one_message_a_mediator_garbled_does_not_cost_the_rest_of_the_post() {
        let good = Value::Map(BTreeMap::from([
            (message::CALLED, Value::Text("zOne".to_owned())),
            (message::RELATION, Value::Text("did:peer:2.mine".to_owned())),
            (message::SEALED, Value::Bytes(vec![1, 2, 3])),
            (message::UNTIL, Value::Uint(40)),
        ]));
        let collection = read(Some(&BTreeMap::from([(
            collected::WAITING,
            Value::Array(vec![Value::Uint(9), good, Value::Text("no".to_owned())]),
        )])));
        assert_eq!(collection.waiting.len(), 1);
        assert_eq!(collection.waiting[0].called, "zOne");
        assert_eq!(collection.waiting[0].until, 40);
        assert!(collection.ringing.is_empty());
    }

    #[test]
    fn an_answer_with_nothing_in_it_is_an_empty_mailbox() {
        let nothing = read(None);
        assert!(nothing.waiting.is_empty() && nothing.ringing.is_empty());
    }
}
