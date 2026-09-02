//! Reading the record through a node, and believing none of it for having arrived.
//!
//! A node serves materials, never a finished state: the name of an object's newest act, and each
//! act in the bytes its author signed. What this does with them is what the holder's app does —
//! walk backwards from the head, one act at a time, each checked against the name it was asked
//! by, until the creation, which names itself. A summary is passed over on the way rather than
//! believed, because nothing checks a summary at admission.
//!
//! Then each chain is folded with the same readers the node folds it with, so that what the
//! partner calls a template, an issuer or a status list is what the node calls one.

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name, Network};
use almena_format::operation::Operation;
use almena_status::list::List;
use almena_store::element::Element;
use almena_store::kind::Kind;
use almena_store::status::StatusList;
use almena_store::template::{Template, Version};
use almena_time::Clock;

use crate::answer::{Answer, State};
use crate::failed::Failed;
use crate::node::{self, Node};

/// Which network a node is on, as it says itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    /// The act that opened it, which every identifier on it is written beside.
    pub name: Name,
    /// The instant epoch zero began, in seconds since the Unix epoch.
    pub began: u64,
    /// Development or production.
    pub which: Network,
    /// The epoch the node answered in.
    pub epoch: u64,
}

impl Opened {
    /// This network's clock.
    ///
    /// # Errors
    ///
    /// `network_no_clock` where the instant is one no calendar holds.
    pub fn clock(&self) -> Result<Clock, Failed> {
        Clock::from_unix(self.began).ok_or_else(|| Failed::new("network_no_clock"))
    }
}

/// Which network the node is on, and what epoch it is there.
///
/// # Errors
///
/// `network_not_answered`, or a `node_*` word.
pub async fn network(at: &Node) -> Result<Opened, Failed> {
    let answer = node::get(at, "/network").await?;
    let fields = answer
        .map()
        .filter(|_| answer.state == State::Here)
        .ok_or_else(|| answer.refused("network_not_answered"))?;
    let (Some(Value::Text(name)), Some(Value::Uint(began)), Some(Value::Uint(which))) =
        (fields.get(&1), fields.get(&2), fields.get(&3))
    else {
        return Err(Failed::new("network_not_answered"));
    };
    Ok(Opened {
        name: Name::parse(name).map_err(|_| Failed::new("network_not_answered"))?,
        began: *began,
        which: match which {
            1 => Network::Development,
            2 => Network::Production,
            _ => return Err(Failed::new("network_not_answered")),
        },
        epoch: answer.epoch,
    })
}

/// The name of an object's newest act, or nothing where the node has never seen it.
///
/// # Errors
///
/// `object_not_resolved` naming the state for a fork or an unreadable history, or a `node_*` word.
pub async fn head_of(at: &Node, object: &Name) -> Result<Option<(Name, u64)>, Failed> {
    let answer = node::get(at, &format!("/object/{}", object.as_str())).await?;
    match answer.state {
        State::Here => {
            let head = answer
                .text()
                .and_then(|text| Name::parse(text).ok())
                .ok_or_else(|| Failed::new("object_not_resolved"))?;
            Ok(Some((head, answer.epoch)))
        }
        State::DoesNotExist => Ok(None),
        _ => Err(answer.refused("object_not_resolved")),
    }
}

/// One act, by the name it is called, checked to be the act that was asked for.
///
/// # Errors
///
/// `act_not_served`, `act_not_the_one_asked_for`, or a `node_*` word.
pub async fn act(at: &Node, name: &Name) -> Result<Operation, Failed> {
    let answer = node::get(at, &format!("/act/{}", name.as_str())).await?;
    let bytes = answer
        .bytes()
        .filter(|_| answer.state == State::Here)
        .ok_or_else(|| answer.refused("act_not_served"))?;
    let operation = read(bytes).ok_or_else(|| Failed::new("act_not_served"))?;
    // Named by what it says and not by how it was signed, which is the only reading under which
    // one act has one name — and what stops a node handing over something else under this name.
    if operation.called() != *name {
        return Err(Failed::new("act_not_the_one_asked_for"));
    }
    Ok(operation)
}

/// An act, out of the bytes its author signed.
fn read(bytes: &[u8]) -> Option<Operation> {
    almena_format::operation::read(&almena_format::cbor::read(bytes).ok()?)
}

/// The whole chain of an object, oldest first, walked back from the head act by act.
///
/// # Errors
///
/// `object_does_not_exist`, `chain_does_not_name_itself`, `chain_not_that_object`, or what
/// fetching one act fails with.
pub async fn chain_of(at: &Node, object: &Did) -> Result<Vec<Operation>, Failed> {
    let (head, _) = head_of(at, object.name())
        .await?
        .ok_or_else(|| Failed::new("object_does_not_exist"))?;
    let mut chain = Vec::new();
    let mut next = Some(head);
    while let Some(name) = next {
        let operation = act(at, &name).await?;
        if operation.object != *object {
            return Err(Failed::new("chain_not_that_object"));
        }
        next = operation.previous.clone();
        chain.push(operation);
    }
    chain.reverse();
    // The creation is the one act nobody can forge: its name is the hash of its own bytes and the
    // object's identifier is that name.
    if !chain.first().is_some_and(Operation::names_itself) {
        return Err(Failed::new("chain_does_not_name_itself"));
    }
    Ok(chain)
}

/// Hand an act to the node.
///
/// # Errors
///
/// `act_not_taken` with the rule the node named, or a `node_*` word.
pub async fn deliver(at: &Node, operation: &Operation) -> Result<Answer, Failed> {
    let answer = node::post(at, "/acts", &operation.to_bytes()).await?;
    if answer.state != State::Taken {
        return Err(answer.refused("act_not_taken"));
    }
    Ok(answer)
}

/// One version of a template, by the hash of the act that published it, with the template it is on.
///
/// The version's act names the template's identifier, and the template's chain is what says the
/// version is one of its own.
///
/// # Errors
///
/// `template_not_one`, `template_no_such_version`, or what fetching fails with.
pub async fn template_version(at: &Node, version: &Name) -> Result<(Template, Version), Failed> {
    let published = act(at, version).await?;
    let template = template(at, &published.object).await?;
    let one = template
        .versions
        .iter()
        .find(|held| held.called == *version)
        .cloned()
        .ok_or_else(|| Failed::new("template_no_such_version"))?;
    Ok((template, one))
}

/// A template, folded out of its chain with the node's own readers.
///
/// # Errors
///
/// `template_not_one`, or what fetching fails with.
pub async fn template(at: &Node, object: &Did) -> Result<Template, Failed> {
    let chain = chain_of(at, object).await?;
    let (first, rest) = chain
        .split_first()
        .ok_or_else(|| Failed::new("template_not_one"))?;
    if first.kind != Kind::TEMPLATE_PUBLISH.number() {
        return Err(Failed::new("template_not_one"));
    }
    let mut held =
        almena_store::template::born(first).map_err(|_| Failed::new("template_not_one"))?;
    for operation in rest {
        let kind = Kind::new(operation.kind).ok_or_else(|| Failed::new("template_not_one"))?;
        held = almena_store::template::does(operation, &held, kind)
            .map_err(|_| Failed::new("template_not_one"))?;
    }
    Ok(held)
}

/// An issuer element, folded out of its chain with the node's own readers.
///
/// # Errors
///
/// `issuer_not_one`, or what fetching fails with.
pub async fn element(at: &Node, object: &Did) -> Result<Element, Failed> {
    let chain = chain_of(at, object).await?;
    let (first, rest) = chain
        .split_first()
        .ok_or_else(|| Failed::new("issuer_not_one"))?;
    if first.kind != Kind::ISSUER_CREATE.number() {
        return Err(Failed::new("issuer_not_one"));
    }
    let mut held = almena_store::element::born(first).map_err(|_| Failed::new("issuer_not_one"))?;
    for operation in rest {
        let kind = Kind::new(operation.kind).ok_or_else(|| Failed::new("issuer_not_one"))?;
        held = almena_store::element::does(operation, &held, kind)
            .map_err(|_| Failed::new("issuer_not_one"))?;
    }
    Ok(held)
}

/// Whether an organisation's chain carries the act that closes it.
///
/// **Read from the acts and not from a state**, because the one question a verifier asks of an
/// organisation is this one, and a chain with the closing act on it answers it whatever else the
/// chain carries.
///
/// # Errors
///
/// What fetching fails with.
pub async fn entity_closed(at: &Node, object: &Did) -> Result<bool, Failed> {
    let chain = chain_of(at, object).await?;
    Ok(chain
        .iter()
        .any(|operation| operation.kind == Kind::ENTITY_CLOSE.number()))
}

/// A status list, folded out of its chain with the node's own readers.
///
/// # Errors
///
/// `status_list_not_one`, or what fetching fails with.
pub async fn status_list(at: &Node, object: &Did) -> Result<StatusList, Failed> {
    let chain = chain_of(at, object).await?;
    let (first, rest) = chain
        .split_first()
        .ok_or_else(|| Failed::new("status_list_not_one"))?;
    if first.kind != Kind::STATUS_LIST_PUBLISH_VERSION.number() {
        return Err(Failed::new("status_list_not_one"));
    }
    let mut held =
        almena_store::status::born(first).map_err(|_| Failed::new("status_list_not_one"))?;
    for operation in rest {
        let kind = Kind::new(operation.kind).ok_or_else(|| Failed::new("status_list_not_one"))?;
        held = almena_store::status::does(operation, &held, kind)
            .map_err(|_| Failed::new("status_list_not_one"))?;
    }
    Ok(held)
}

/// The bytes of one status list version, from a node that holds a copy, or nothing where it does not.
///
/// **Nothing is not *does not exist*.** A node without a copy of a public list says something
/// about itself, and the verifier says *could not verify* rather than anything about the issuer.
///
/// # Errors
///
/// A `node_*` word.
pub async fn list_bytes(at: &Node, version: &[u8]) -> Result<Option<List>, Failed> {
    let answer = node::get(at, &format!("/list/{}", crate::directory::hex(version))).await?;
    let Some(bytes) = answer.bytes().filter(|_| answer.state == State::Here) else {
        return Ok(None);
    };
    Ok(core::str::from_utf8(bytes)
        .ok()
        .and_then(|text| List::read(text).ok()))
}

/// Hand a status list's bytes to the node, so that it serves them.
///
/// # Errors
///
/// `list_not_kept` with the node's reason, or a `node_*` word.
pub async fn keep_list(at: &Node, list: &List) -> Result<(), Failed> {
    let answer = node::post(at, "/list", list.written().as_bytes()).await?;
    if answer.state != State::Taken {
        return Err(answer.refused("list_not_kept"));
    }
    Ok(())
}
