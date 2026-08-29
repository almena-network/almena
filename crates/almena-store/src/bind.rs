//! Who put a node on the network, said by both of them.
//!
//! Whoever sustains the network earns the right to write on it, and that has to attach to somebody:
//! a node nobody claimed is a machine, and a machine cannot be credited. So a node and whoever
//! contributed it say so together, in the node's own chain, where anybody can read it.
//!
//! # Both sides, and only one of them in `firmas`
//!
//! Approving a challenge proves somebody holds their own key. It does not prove they hold the node,
//! and the node saying it alone does not prove anybody agreed — so the act needs both, or it binds
//! nothing.
//!
//! They do not both go in the signature list. What an act carries there has to say it is the
//! object's own: without that rule, anybody who saw an act go past could rewrite whose signature it
//! claimed to be and split the chain for ever. So **the node's signature goes where signatures go,
//! because it is the node's chain — and the claimant's approval travels inside the act**, with the
//! name it belongs to beside it. Anybody checks it by resolving that name, which is where the
//! answer to *which key speaks for them* lives.
//!
//! # The challenge is one use and short-lived, and neither is enforced by the record
//!
//! The node shows a challenge; whoever is claiming it approves that exact challenge with a key
//! their own chain authorises. It carries the node it is for, so an approval cannot be lifted onto
//! a different machine, and a moment it stops being good, so one that ends up in a screenshot, a
//! support bundle or the node's own log does not bind somebody's machine a year later.
//!
//! **One use is kept by the node that issued it and by nothing else**, and that is the honest place
//! for it: the record cannot tell a challenge used twice from one used once, because it never saw
//! it the first time. What the record does hold is the act, and the same act arriving twice is one
//! act — so a replay changes nothing.
//!
//! # What this does not do
//!
//! It binds. Nothing is earned by it yet: what a bound node's service is worth is a figure nobody
//! computes, so today this says who would be credited and credits nobody.

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Name};
use almena_format::operation::{Operation, Signed};
use almena_suite::ed25519;
use almena_time::Epoch;

use crate::kind::Kind;

/// Where the act carries who is claiming the node.
///
/// Odd: a reader that passed over it would read a binding that bound the node to nobody, which is
/// the one thing the act cannot mean.
const CLAIMANT: u64 = 1;

/// Where the act carries their approval of the challenge.
///
/// Odd for the same reason. Without it the act is the node saying somebody agreed, which is exactly
/// what the two-sided rule exists to refuse.
const APPROVAL: u64 = 3;

/// Where the act carries the nonce the challenge was shown with.
///
/// Odd. It is what makes these bytes signed nowhere else, so a reader that skipped it would accept
/// an approval given for something else as an approval of this.
const NONCE: u64 = 5;

/// Where the act carries the moment the challenge stopped being good.
///
/// Odd: a reader that passed over it would take an approval given long ago and long expired as one
/// given now, which is the whole of what a short life is for.
const UNTIL: u64 = 7;

/// A challenge a node shows to whoever it is asking to claim it.
///
/// **It never goes in the record.** What is written down is the act that came of it; the challenge
/// itself is a thing shown on a screen and gone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Challenge {
    /// The node being claimed. An approval cannot be lifted onto a different machine.
    pub node: Did,
    /// Something this challenge alone carries, so that an approval of it approves nothing else.
    pub nonce: [u8; 32],
    /// The moment it stops being good.
    pub until: Epoch,
}

impl Challenge {
    /// The bytes whoever is claiming the node puts their name to.
    ///
    /// Canonical, so that the two sides are signing and checking the same thing rather than each
    /// building its own idea of what was agreed.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        Value::Map(BTreeMap::from([
            (CLAIMANT, Value::Text(self.node.to_string())),
            (NONCE, Value::Bytes(self.nonce.to_vec())),
            (UNTIL, Value::Uint(self.until.number())),
        ]))
        .to_bytes()
    }

    /// How it is shown to somebody, in the alphabet everything here is named in.
    ///
    /// **A challenge crosses from a machine to a person and back.** It goes on a screen, into a
    /// camera, or through a paste buffer, so it has to survive being read out loud and typed wrong
    /// — which is what base58 is for: no zero, no capital O, no capital I, no lowercase l.
    #[must_use]
    pub fn to_text(&self) -> String {
        almena_format::identifier::base58(&self.to_bytes())
    }

    /// A challenge somebody handed back, if that is what it is.
    ///
    /// [`None`] for text that is not one, or one missing any of what it takes to be checked — which
    /// is not a weaker challenge but a different thing wearing its clothes.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        let bytes = almena_format::identifier::unbase58(text)?;
        let Ok(Value::Map(fields)) = almena_format::cbor::read(&bytes) else {
            return None;
        };
        let (Some(Value::Text(node)), Some(Value::Bytes(nonce)), Some(&Value::Uint(until))) = (
            fields.get(&CLAIMANT),
            fields.get(&NONCE),
            fields.get(&UNTIL),
        ) else {
            return None;
        };
        Some(Self {
            node: Did::parse(node).ok()?,
            nonce: nonce.as_slice().try_into().ok()?,
            until: Epoch::new(until),
        })
    }

    /// Whether it is still good at that moment.
    ///
    /// Ends included: a challenge is good up to and including the moment it names.
    #[must_use]
    pub fn good_at(&self, now: Epoch) -> bool {
        now.number() <= self.until.number()
    }
}

/// What whoever is claiming a node hands back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approval {
    /// Who they are, as the record names them.
    pub claimant: Did,
    /// Their signature over the challenge, by a key their own chain authorises.
    pub signature: [u8; 64],
}

impl Approval {
    /// How it is handed back, in the alphabet everything here is named in.
    #[must_use]
    pub fn to_text(&self) -> String {
        almena_format::identifier::base58(
            &Value::Map(BTreeMap::from([
                (CLAIMANT, Value::Text(self.claimant.to_string())),
                (APPROVAL, Value::Bytes(self.signature.to_vec())),
            ]))
            .to_bytes(),
        )
    }

    /// An approval somebody handed back, if that is what it is.
    ///
    /// **Nothing here is checked.** That it reads is not that it is theirs — whose key signed it,
    /// and whether that key speaks for them, are answered against their own chain and nowhere else.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        let bytes = almena_format::identifier::unbase58(text)?;
        let Ok(Value::Map(fields)) = almena_format::cbor::read(&bytes) else {
            return None;
        };
        let (Some(Value::Text(claimant)), Some(Value::Bytes(signature))) =
            (fields.get(&CLAIMANT), fields.get(&APPROVAL))
        else {
            return None;
        };
        Some(Self {
            claimant: Did::parse(claimant).ok()?,
            signature: signature.as_slice().try_into().ok()?,
        })
    }
}

/// Say that a node was contributed by somebody, with what they both put their name to.
///
/// The node signs the act because it is the node's chain; the claimant's approval travels inside it.
#[must_use]
pub fn bind(node: &Did, head: &Name, what: &Claiming<'_>, by: &ed25519::SigningKey) -> Operation {
    let Claiming {
        challenge,
        approval,
        issued,
    } = what;
    let payload = BTreeMap::from([
        (CLAIMANT, Value::Text(approval.claimant.to_string())),
        (APPROVAL, Value::Bytes(approval.signature.to_vec())),
        (NONCE, Value::Bytes(challenge.nonce.to_vec())),
        (UNTIL, Value::Uint(challenge.until.number())),
    ]);
    signed(
        &Chain {
            node,
            head,
            issued: *issued,
        },
        Kind::NODE_BIND,
        payload,
        by,
    )
}

/// A claim being written down: what was shown, what came back, and when.
#[derive(Debug, Clone, Copy)]
pub struct Claiming<'a> {
    /// What the node showed.
    pub challenge: &'a Challenge,
    /// What came back.
    pub approval: &'a Approval,
    /// When it is being written down.
    pub issued: Epoch,
}

/// Say that a node is no longer contributed by anybody.
///
/// **The node alone.** Whoever claimed it agreed to be credited for what it served, and letting go
/// of that costs them nothing they can be held to — so nobody has to be asked. Credit stops
/// accruing from here and never in arrears: what was served was served.
#[must_use]
pub fn unbind(node: &Did, head: &Name, at: Epoch, by: &ed25519::SigningKey) -> Operation {
    signed(
        &Chain {
            node,
            head,
            issued: at,
        },
        Kind::NODE_UNBIND,
        BTreeMap::new(),
        by,
    )
}

/// Who an act says contributed the node, if it says so at all and says it whole.
///
/// [`None`] for anything that is not a binding, and for one missing any of what it takes to be
/// checked. **A binding that cannot be checked is not a weaker binding** — it is the node's word
/// about somebody who never agreed, which is the thing the two-sided rule refuses.
#[must_use]
pub fn claimed(operation: &Operation) -> Option<(Approval, Challenge)> {
    if Kind::new(operation.kind) != Some(Kind::NODE_BIND) {
        return None;
    }
    let (
        Some(Value::Text(claimant)),
        Some(Value::Bytes(approval)),
        Some(Value::Bytes(nonce)),
        Some(&Value::Uint(until)),
    ) = (
        operation.payload.get(&CLAIMANT),
        operation.payload.get(&APPROVAL),
        operation.payload.get(&NONCE),
        operation.payload.get(&UNTIL),
    )
    else {
        return None;
    };

    Some((
        Approval {
            claimant: Did::parse(claimant).ok()?,
            signature: approval.as_slice().try_into().ok()?,
        },
        Challenge {
            node: operation.object.clone(),
            nonce: nonce.as_slice().try_into().ok()?,
            until: Epoch::new(until),
        },
    ))
}

/// Whether the approval inside a binding really is that claimant's, by a key they authorise.
///
/// `speaks_for_them` is the key the claimant's own chain authorises, resolved from the record —
/// **never taken from the act being checked**, which is the thing under suspicion.
///
/// **It is the control key and not a device.** Claiming a node writes somebody's name into a record
/// that does not forget, beside a machine that has an address; it is not routine, and the words
/// behind the control key are what a person falls back on for the things that are not. The flow
/// described for people has a device scanning a code, and a device signs on the other curve —
/// taking one would mean measuring a key to guess which curve made it, which is the coincidence
/// this design refuses to turn into a rule. Adding it is a thing to add, not a thing to allow by
/// accident.
///
/// It also checks the act was written while the challenge was still good: an approval that ended up
/// in a screenshot or a support bundle must not bind somebody's machine a year later.
#[must_use]
pub fn agreed(operation: &Operation, speaks_for_them: &[u8; ed25519::PUBLIC_KEY_WIDTH]) -> bool {
    let Some((approval, challenge)) = claimed(operation) else {
        return false;
    };
    if !challenge.good_at(operation.issued) {
        return false;
    }
    let Ok(verifying) = ed25519::VerifyingKey::from_bytes(*speaks_for_them) else {
        return false;
    };
    verifying
        .verify(
            &challenge.to_bytes(),
            &ed25519::Signature::from_bytes(approval.signature),
        )
        .is_ok()
}

/// Where on a node's chain an act is being added, and when.
#[derive(Debug, Clone, Copy)]
struct Chain<'a> {
    /// Whose chain it is.
    node: &'a Did,
    /// What it comes after.
    head: &'a Name,
    /// When it is being written.
    issued: Epoch,
}

/// An act on a node's own chain, signed by the node.
fn signed(
    on: &Chain<'_>,
    kind: Kind,
    payload: BTreeMap<u64, Value>,
    by: &ed25519::SigningKey,
) -> Operation {
    let Chain { node, head, issued } = on;
    let mut operation = Operation {
        object: (*node).clone(),
        previous: Some((*head).clone()),
        kind: kind.number(),
        version: 1,
        issued: *issued,
        payload,
        signatures: Vec::new(),
    };
    let signature = by.sign(&operation.signing_bytes());
    operation.signatures.push(Signed {
        by: (*node).clone(),
        key: by.verifying_key().bytes().to_vec(),
        signature: signature.bytes(),
    });
    operation
}
