//! The mailbox, for the nodes that run one.
//!
//! **A node that does not offer this has no such route**, and says exactly that. The gate is the
//! node's own announcement — the same act everybody else reads to decide whether to send anybody
//! here — so what a node advertises and what it does are one fact and not two.
//!
//! # It is not part of the record, and nothing here pretends otherwise
//!
//! Nothing delivered here is replicated, hashed into a root, or seen by another node. A mediator
//! is a locker, not a register: what it holds it holds for one account until that account takes it,
//! and moving to another mediator costs the sender an address and nobody a history
//! (`SPECS.md §6.2`). The epoch and root still ride on every answer, because an answer that does
//! not say what it was computed against cannot be compared with another — but they say when this
//! node answered, never that the post is in the record.
//!
//! # Two doors, and only one of them asks who is there
//!
//! **Delivering does not.** A sender is a stranger holding an address, and the address is the
//! authorisation: a relationship's peer identifier is unpublished and unenumerable
//! (`SPECS.md §3.3`), so somebody writing to one is somebody who was given it. Anything addressed
//! elsewhere reaches the doorbell, which is the one thing that reaches a person with no
//! relationship to them (`SPECS.md §6.5`), and has a ceiling of its own so that it cannot be used
//! to fill a mailbox.
//!
//! **Taking does.** Post is one person's, and who may take it is exactly the devices that account's
//! own chain says operate it — which this node already holds, because it replayed that chain like
//! every other. So removing a device removes its mailbox key in the same act, and there is no
//! second register here to drift from the first.

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_mailbox::account::{Refused, TurnedAway};
use almena_mailbox::asking::{Asking, Errand, Not};
use almena_mailbox::held::Held;
use almena_mailbox::mediator::Collection;
use almena_node::Node;
use almena_time::{Epoch, Epochs};

use crate::{Said, State, raw};

/// What a delivery is made of.
///
/// All three critical. A build that passed over the relationship would put somebody's post in the
/// wrong quota; over the bytes, would hold an empty envelope; over the deadline, would hold it for
/// as long as it liked. None of those is a smaller version of delivering — each is a different act.
mod letter {
    /// Which relationship it comes from: the recipient's peer identifier for it.
    pub const RELATION: u64 = 1;
    /// The message itself, sealed between its two ends.
    pub const SEALED: u64 = 3;
    /// How many epochs the sender asks it be held for.
    pub const FOR: u64 = 5;
}

/// What one message looks like on the way back out.
mod message {
    /// Its name, which is the hash of its sealed bytes.
    pub const CALLED: u64 = 1;
    /// Which relationship it came from.
    pub const RELATION: u64 = 3;
    /// The sealed bytes, exactly as the sender handed them over.
    pub const SEALED: u64 = 5;
    /// The first epoch at which it is no longer held.
    pub const UNTIL: u64 = 7;
}

/// What a collection hands over.
mod collected {
    /// What is waiting in that device's mailbox, oldest first.
    pub const WAITING: u64 = 1;
    /// What is waiting at the doorbell, which belongs to the account.
    pub const RINGING: u64 = 3;
    /// Since when this account has had post turned away, and how much.
    pub const TURNED_AWAY: u64 = 5;
    /// Whether the mailbox had gone quiet long enough to be emptied.
    pub const WAS_INACTIVE: u64 = 7;
}

/// What being turned away is made of.
mod away {
    /// The epoch the first one was refused in.
    pub const SINCE: u64 = 1;
    /// How many have been refused since.
    pub const COUNT: u64 = 3;
}

/// Where a delivery landed.
mod landed {
    /// The name it was given, which is the hash of what was sent.
    pub const CALLED: u64 = 1;
    /// Whether it went to a mailbox or to the doorbell.
    pub const WHERE: u64 = 3;
    /// It went into the mailboxes of a relationship this account has.
    pub const MAILBOX: u64 = 1;
    /// It rang the doorbell, which is what somebody with no relationship reaches.
    pub const DOORBELL: u64 = 2;
}

/// Take a status list's bytes, so this node can serve them to whoever asks.
///
/// **Nothing is understood here** (`SPECS.md §4.8`, `§10.2`). A node hashes what it was handed and
/// keeps it if the record names that hash as some list's current version. It never reads a
/// bitstring: replicating one is holding bytes, and a change to that format is a change to issuers
/// and verifiers and to nobody else.
///
/// Anybody may hand these over and there is nothing to authenticate: what decides whether the bytes
/// are kept is the record, which is public — and a node that took anything else would be one
/// anybody can fill with rubbish under the name of a service.
pub fn kept(node: &mut Node, bytes: &[u8], now: Epoch) -> Said {
    let root = node.root_now();
    match node.keep_list(bytes.to_vec(), now) {
        Ok(list) => raw(
            now,
            root,
            State::Taken,
            Some(Value::Text(list.as_str().to_owned())),
            None,
        ),
        Err(why) => raw(
            now,
            root,
            State::NotTaken,
            None,
            Some(match why {
                almena_node::NotKept::NotNamed => not_kept::NOT_NAMED,
                almena_node::NotKept::WindowPast => not_kept::WINDOW_PAST,
            }),
        ),
    }
}

/// Why a node did not keep a status list, as a number.
///
/// **Its own numbering, and deliberately not the one an act's refusals use.** These answer a
/// different question, and folding them into one vocabulary would let a reader that knew one set
/// read a number from the other and believe it had understood.
pub mod not_kept {
    /// No list in the record names those bytes as its current version.
    pub const NOT_NAMED: u64 = 1;
    /// The window it covers has passed, so nothing it says is about a credential still alive.
    pub const WINDOW_PAST: u64 = 2;
}

/// Why the post would not take something, as a number.
///
/// **One vocabulary for both kinds of no**, because a sender and a device both need to know which
/// one they got and neither benefits from two numbering schemes. Written out one by one rather
/// than cast from the types they name, so that reordering a variant somewhere else cannot silently
/// renumber what a node says.
#[must_use]
pub fn why(refused: Refused) -> u64 {
    match refused {
        Refused::TooLarge => 1,
        Refused::RelationFull => 2,
        Refused::AccountFull => 3,
        Refused::DoorbellFull => 4,
        Refused::NoSuchMailbox => 5,
        Refused::Inactive => 6,
        Refused::NoSuchRelation => 7,
    }
}

/// The same, for an asking that did not hold up.
#[must_use]
pub fn why_not(not: Not) -> u64 {
    match not {
        Not::Unreadable => 8,
        Not::OutOfTime => 9,
        Not::NotThatDevice => 10,
    }
}

/// This node cannot say what that account is, so it cannot say whose device that is either.
///
/// Its own ignorance, said as its own: never *no devices*, which would turn not knowing into a
/// lockout, and never a rule the account broke.
const CANNOT_SAY_WHOSE: u64 = 11;

/// Hand a message to somebody's mediator.
///
/// **Nobody is asked who they are.** Whoever holds a relationship's address may write to it, and
/// whoever does not reaches the doorbell instead — with its own small ceiling, so that ringing it
/// cannot be a way to fill a mailbox.
#[must_use]
pub fn deliver(node: &mut Node, to: &str, envelope: &[u8], now: Epoch) -> Said {
    let root = node.root_now();
    let Some(post) = node.post() else {
        return raw(now, root, State::NoSuchQuestion, None, None);
    };
    let Some(message) = read(envelope, now) else {
        return raw(now, root, State::Malformed, None, None);
    };
    let called = message.called.clone();

    // **A sender addresses a relationship, not an account** (`SPECS.md §6.5`). That is what lets a
    // relationship exist without either end learning the other's root identifier: what the sender
    // holds is an address, and this is the only party that has to know which of its customers
    // answers to it. A root identifier resolves too, and that is the doorbell.
    let Some(whose) = post.addressed(to).cloned() else {
        return raw(
            now,
            root,
            State::NotTaken,
            None,
            Some(why(Refused::NoSuchMailbox)),
        );
    };

    // Where it lands is the account's to decide and not the sender's: a relationship this account
    // has goes to its mailboxes, and everything else rings. A sender who thought they were
    // delivering is told which of the two happened rather than left to assume.
    let known = post.knows(&whose, &message.relation);
    let outcome = if known {
        post.deliver(&whose, &message, now)
            .map(|()| landed::MAILBOX)
    } else {
        post.ring(&whose, message, now).map(|()| landed::DOORBELL)
    };

    match outcome {
        Ok(went) => raw(
            now,
            root,
            State::Taken,
            Some(Value::Map(BTreeMap::from([
                (landed::CALLED, Value::Text(called.as_str().to_owned())),
                (landed::WHERE, Value::Uint(went)),
            ]))),
            None,
        ),
        Err(refused) => raw(now, root, State::NotTaken, None, Some(why(refused))),
    }
}

/// What an account's own device asks of its mediator.
///
/// Declaring, taking and confirming, each proved by a key the account's chain authorises. A node
/// that cannot say what the account is says so, rather than answering as if it had looked and found
/// nobody.
#[must_use]
pub fn asked(node: &mut Node, asking: &[u8], now: Epoch) -> Said {
    let root = node.root_now();
    if !node.carries_post() {
        return raw(now, root, State::NoSuchQuestion, None, None);
    }
    let Ok(asking) = Asking::read(asking) else {
        return raw(now, root, State::Malformed, None, None);
    };
    let Some(devices) = node.devices_on(&asking.whose, now) else {
        return raw(now, root, State::NotTaken, None, Some(CANNOT_SAY_WHOSE));
    };
    if let Err(not) = asking.holds(&devices, now) {
        return raw(now, root, State::NotTaken, None, Some(why_not(not)));
    }

    // Checked above, and `carries_post` is what it is checked against.
    let Some(post) = node.post() else {
        return raw(now, root, State::NoSuchQuestion, None, None);
    };
    match asking.errand {
        Errand::Carry => {
            // **The devices come from the chain and the relationships from the asking**, because
            // only one of the two is the account's to declare here. A mediator that took the device
            // list from whoever was talking to it would be a mediator where saying so made it so.
            post.carry(&asking.whose, devices, asking.names.clone(), now);
            raw(now, root, State::Taken, None, None)
        }
        Errand::Collect => match post.collect(&asking.whose, &asking.device, now) {
            Some(collection) => raw(now, root, State::Here, Some(handed(&collection)), None),
            None => raw(
                now,
                root,
                State::NotTaken,
                None,
                Some(why(Refused::NoSuchMailbox)),
            ),
        },
        Errand::Confirm => {
            post.confirm(&asking.whose, &asking.device, &asking.named(), now);
            raw(now, root, State::Taken, None, None)
        }
        // **One endpoint, opaque, and only for a device this account has** (`SPECS.md §6.3`). What
        // is held is somewhere to deliver a signal to; how the signal reaches a telephone — a relay
        // translating a handle, somebody's own push distributor — is not this node's business, and
        // not knowing is what stops the notification path becoming a dependency.
        //
        // Naming no endpoint clears it, which is a device saying *stop waking me*.
        Errand::Wake => {
            let endpoint = asking.names.first().map_or("", String::as_str);
            match post.wakes_at(&asking.whose, &asking.device, endpoint) {
                true => raw(now, root, State::Taken, None, None),
                false => raw(
                    now,
                    root,
                    State::NotTaken,
                    None,
                    Some(why(Refused::NoSuchMailbox)),
                ),
            }
        }
    }
}

/// One delivery, read from the bytes a sender sent.
fn read(envelope: &[u8], now: Epoch) -> Option<Held> {
    let Ok(Value::Map(fields)) = almena_format::cbor::read(envelope) else {
        return None;
    };
    let Some(Value::Text(relation)) = fields.get(&letter::RELATION) else {
        return None;
    };
    let Some(Value::Bytes(sealed)) = fields.get(&letter::SEALED) else {
        return None;
    };
    let Some(Value::Uint(held_for)) = fields.get(&letter::FOR) else {
        return None;
    };
    Some(Held::new(
        relation.clone(),
        sealed.clone(),
        Epochs(*held_for),
        now,
    ))
}

/// What a collection looks like on the way out.
fn handed(collection: &Collection) -> Value {
    let mut fields = BTreeMap::from([
        (collected::WAITING, listed(&collection.waiting)),
        (collected::RINGING, listed(&collection.ringing)),
    ]);
    if let Some(TurnedAway { since, count }) = collection.turned_away {
        fields.insert(
            collected::TURNED_AWAY,
            Value::Map(BTreeMap::from([
                (away::SINCE, Value::Uint(since.number())),
                (away::COUNT, Value::Uint(count)),
            ])),
        );
    }
    if collection.was_inactive {
        // Present only when it happened, so that a build reading this cannot mistake a zero it
        // invented for a node saying no.
        fields.insert(collected::WAS_INACTIVE, Value::Uint(1));
    }
    Value::Map(fields)
}

/// A list of messages, oldest first, in the bytes their senders handed over.
fn listed(messages: &[Held]) -> Value {
    Value::Array(
        messages
            .iter()
            .map(|held| {
                Value::Map(BTreeMap::from([
                    (
                        message::CALLED,
                        Value::Text(held.called.as_str().to_owned()),
                    ),
                    (message::RELATION, Value::Text(held.relation.clone())),
                    (message::SEALED, Value::Bytes(held.sealed.clone())),
                    (message::UNTIL, Value::Uint(held.until.number())),
                ]))
            })
            .collect(),
    )
}
