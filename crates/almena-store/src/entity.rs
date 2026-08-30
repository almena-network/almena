//! An organisation, and what its history says governs it now.
//!
//! **An entity is not a holder** (`SPECS.md §2.2`). It has no seed, no guardians and no words
//! behind it: what keeps it alive is its owners and the threshold they signed themselves into. So
//! everything a holder gets from *one key that is the last resort* has to come from somewhere else
//! here, and the somewhere else is that **several people have to agree**.
//!
//! # Counted signatures, and why the register is not sealed
//!
//! Every operation on an entity is signed by its owners **one at a time**, and this node counts how
//! many there are against the set of owners standing at that moment (`SPECS.md §8.5`). It is not a
//! threshold signature, and the reason is not convenience:
//!
//! - **A cryptographic sharing has one `k`.** An entity that shared at two-of-five has two
//!   fragments that sign *anything* — the signature cannot tell a routine act from one that
//!   replaces every owner. Holding the three classes of `SPECS.md §8.2` apart would take three
//!   keys, three ceremonies and three things to lose. Counted, each act asks for the number that
//!   belongs to it and this node checks it.
//! - **It writes down who signed what**, which is the management history `SPECS.md §8.6` asks for
//!   and which a single collapsed signature cannot produce.
//! - **An entity that never seals anything outward never has a fragment to lose.**
//!
//! # An owner is a root identifier, not a key
//!
//! The set lists **people**, so an owner who rotates, recovers with guardians or changes phone does
//! not lose their place (`SPECS.md §8.5`). This node resolves each owner's own chain and asks it
//! which keys speak for them today. Binding a key instead would make every rotation of one person
//! a governance operation in every entity they belong to.
//!
//! # Losing quorum is an anticipated state, not a bricked one
//!
//! An entity whose owners fall below its threshold cannot govern itself — it cannot close, appoint
//! or reconfigure (`SPECS.md §12.3`). That is a known hole with a known way out: **emergency
//! continuity**, which does one thing only, needs one surviving owner, waits sixty days in public,
//! and any other surviving owner can veto (`SPECS.md §8.3`). Because the way out exists, this
//! module does not refuse the configurations that lead there — refusing them would be deciding for
//! an organisation about its own governance, which is `SPECS.md §7.1`'s to decide and not a node's.

use std::collections::{BTreeMap, BTreeSet};

use almena_format::cbor::Value;
use almena_format::identifier::Did;
use almena_format::operation::Operation;
use almena_suite::{ed25519, p256};
use almena_time::Epoch;

use crate::chain::Refused;
use crate::kind::Kind;

/// How long emergency continuity waits before it lands.
///
/// Sixty days, published (`SPECS.md §8.3`). Long enough that an owner on holiday finds out, which
/// is the whole of what the wait is for: replenishing owners **concedes** trust, and concessions
/// wait (`SPECS.md §1.8`).
pub const CONTINUITY_WAITS: almena_time::parameter::Parameter =
    almena_time::deadline::EMERGENCY_CONTINUITY;

/// How long an alias is held before anybody else may take it.
///
/// Ninety days (`SPECS.md §7.5`), and the same span whether the seal was lost or the domain stopped
/// validating — **because the harm is the same either way**: somebody arriving looking for one party
/// and finding another. Three months is what it takes for a name people had in their heads to cool.
pub const ALIAS_QUARANTINE: almena_time::parameter::Parameter =
    almena_time::deadline::ALIAS_QUARANTINE;

/// How long a verified domain stands before it has to prove itself again.
///
/// Thirty days (`SPECS.md §7.4`). It bounds how long a domain that has already changed hands can
/// keep saying it belongs here, and it is frequent enough that a passing DNS failure is
/// distinguishable from an abandonment.
pub const DOMAIN_STANDS: almena_time::parameter::Parameter =
    almena_time::deadline::DOMAIN_REVALIDATION;

/// Where each part of an entity operation sits.
///
/// **Odd is critical** (`SPECS.md §4.8`, rule 4): a reader that skipped one of these could not
/// claim to have applied the operation. Every field an entity act carries changes who governs it or
/// what it is, with one exception — which is even, and says so.
pub mod field {
    /// The entity's own key: the one it is created with and the one a rotation replaces.
    pub const KEY: u64 = 1;
    /// Which domain shows on a consent screen.
    ///
    /// **The one even field here.** A reader that passes over it holds every domain the entity has
    /// verified and does not know which one to put first — a worse screen, and not a wrong claim.
    pub const PRINCIPAL: u64 = 2;
    /// The identifier an owner or manager act is about.
    pub const WHO: u64 = 3;
    /// The threshold for acts that are reversible and of low consequence.
    pub const ROUTINE: u64 = 5;
    /// The threshold for sealing outward and for authorising an issuer's issuance key.
    pub const SEALING: u64 = 7;
    /// The threshold for changing who governs.
    pub const GOVERNANCE: u64 = 9;
    /// The domain an act is about.
    pub const DOMAIN: u64 = 11;
    /// Whether closing also revokes what the entity has outstanding.
    pub const REVOKING: u64 = 13;
    /// The alias being claimed, which is a label of a domain the entity has proved.
    pub const ALIAS: u64 = 15;
    /// An address to reach the organisation at, under a domain it has proved.
    pub const EMAIL: u64 = 17;
}

/// Which class of threshold an act is counted against.
///
/// **Three, declared from the start even when all three carry the same number today**
/// (`SPECS.md §8.2`). Raising one later is then a change of configuration and not of the schema,
/// which is the pattern `SPECS.md §4.8` and `§18` follow everywhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    /// Reversible, and of low consequence.
    Routine,
    /// Sealing outward, and authorising the key an issuer emits with.
    Sealing,
    /// Changing who governs, what the threshold is, or whether the entity goes on existing.
    Governance,
}

/// Which class each act belongs to.
///
/// `SPECS.md §8.2` names the three classes and gives examples rather than an exhaustive table, so
/// two of these are this build's reading and are argued here rather than left to be inferred:
///
/// - **A domain is governance.** `SPECS.md §7.4` says adding one takes the threshold *because* it
///   is a public signal of trust and a compromised owner could lend legitimacy to a domain under
///   their control. That argument is the governance argument, so the class follows it.
/// - **Appointing a manager is routine.** A manager operates and cannot alter who operates
///   (`SPECS.md §8.1`), so a compromised one cannot escalate — which is exactly what makes the act
///   reversible and of low consequence.
#[must_use]
pub const fn class(kind: Kind) -> Option<Class> {
    Some(match kind {
        Kind::ENTITY_ADD_MANAGER | Kind::ENTITY_REMOVE_MANAGER | Kind::ENTITY_CHECKPOINT => {
            Class::Routine
        }
        Kind::ENTITY_ADD_OWNER
        | Kind::ENTITY_REMOVE_OWNER
        | Kind::ENTITY_SET_THRESHOLD
        | Kind::ENTITY_ROTATE_KEY
        | Kind::ENTITY_ADD_DOMAIN
        | Kind::ENTITY_REMOVE_DOMAIN
        | Kind::ENTITY_SET_ALIAS
        | Kind::ENTITY_CLOSE => Class::Governance,
        // One surviving owner, because with small sets demanding two defeats its purpose — and if
        // the governance threshold were reachable this act would not be needed (`SPECS.md §8.3`).
        Kind::ENTITY_CONTINUITY | Kind::ENTITY_VETO => Class::Routine,
        _ => return None,
    })
}

/// The three thresholds, as the entity declared them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thresholds {
    /// How many owners a routine act needs.
    pub routine: u64,
    /// How many a sealing act needs.
    pub sealing: u64,
    /// How many an act that changes who governs needs.
    pub governance: u64,
}

impl Thresholds {
    /// How many owners an act of that class needs.
    #[must_use]
    pub const fn of(&self, class: Class) -> u64 {
        match class {
            Class::Routine => self.routine,
            Class::Sealing => self.sealing,
            Class::Governance => self.governance,
        }
    }
}

/// One domain the entity has proved it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Domain {
    /// When it was last proved.
    ///
    /// **Not when it was added.** A domain proved a year ago and never checked since is a domain
    /// that may have changed hands, and the whole point of revalidating is that the record says
    /// when somebody last looked.
    pub proved: Epoch,
    /// Whether it is the one a consent screen shows first.
    pub principal: bool,
}

impl Domain {
    /// Whether it is still standing, or is due to prove itself again.
    #[must_use]
    pub fn stands(&self, at: Epoch) -> bool {
        self.proved
            .plus(DOMAIN_STANDS.epochs(self.proved))
            .is_none_or(|until| at.number() < until.number())
    }
}

/// A name an entity claims, derived from a domain it has proved.
///
/// # Three states, and the last one is not the entity's to give
///
/// *Claimed* is the entity saying so. *Signed* is that saying having reached the entity's own
/// threshold — which, for anything in the record, is the same moment: an act that got in got in
/// because enough owners signed it. **Accepted is the network having seen it**: it appears in roots
/// signed by several nodes, which is `SPECS.md §4.4`'s finality policy applied exactly as written —
/// an alias is a **concession**, a name that carries somebody's reputation, and concessions wait.
///
/// **Until it is accepted it is neither shown nor used** (`SPECS.md §7.5`). So this state carries
/// the claim and the act that made it, and whether it is accepted is a question about firmness,
/// answered where firmness is and never stored here as though an entity could assert it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    /// The name itself, in the one form it is compared in.
    pub name: String,
    /// The domain it derives from, which the entity had already proved.
    pub from: String,
    /// The act that claimed it, which is what a reader checks for firmness.
    pub claimed_by: almena_format::identifier::Name,
    /// The epoch it was claimed in.
    pub since: Epoch,
}

/// A name nobody may take yet, and the moment that ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cooling {
    /// The name.
    pub name: String,
    /// The first epoch at which somebody else may claim it.
    pub until: Epoch,
}

/// Owners being put back to recover a quorum, and when that lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Continuity {
    /// Who is being put back.
    pub owners: BTreeSet<Did>,
    /// The first epoch at which it takes effect.
    pub due: Epoch,
    /// Which act it is, so that a veto can name it.
    pub act: almena_format::identifier::Name,
}

/// An organisation, as its chain says it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    /// Who signs for it. Root identifiers, never keys.
    pub owners: BTreeSet<Did>,
    /// Who operates it without being able to alter who operates it.
    pub managers: BTreeSet<Did>,
    /// What each class of act costs, in owners.
    pub thresholds: Thresholds,
    /// The key it holds a channel with, which a rotation replaces.
    pub key: [u8; ed25519::PUBLIC_KEY_WIDTH],
    /// The domains it has proved it holds.
    pub domains: BTreeMap<String, Domain>,
    /// When it was closed, if it was.
    ///
    /// **A state and not a deletion** (`SPECS.md §12.1`). Credentials point at it and a verifier
    /// has to be able to resolve it, so its history stays and the date is published.
    pub closed: Option<Epoch>,
    /// Whether closing also revoked what it had outstanding, which was the closer's to choose.
    pub revoked_on_closing: bool,
    /// A replenishment of owners in flight, waiting out its sixty days.
    pub continuity: Option<Continuity>,
    /// The name it claims, once it has claimed one.
    pub alias: Option<Alias>,
    /// A name it gave up or lost, which nobody else may take until it has cooled.
    pub cooling: Option<Cooling>,
    /// An address to reach it at, under a domain it has proved.
    ///
    /// **Only an organisation has one, and there is nowhere to put one on a person** — which is the
    /// enforcement rather than a rule anybody has to remember (`SPECS.md §7.6`). A root identifier
    /// is public and replicated by open nodes, so publishing addresses would turn the census into a
    /// downloadable list of real people and take away every pseudonymisation argument there is.
    pub email: Option<String>,
    /// The epoch of its last act.
    ///
    /// **Published rather than judged** (`SPECS.md §12.3`). There is no way to tell from inside a
    /// frozen entity from one that has had nothing to do, so no label is put on it: the date goes
    /// where the decision is taken, and whoever is about to rely on it decides.
    pub acted: Epoch,
}

impl Entity {
    /// The entity once everything due by that moment has taken effect.
    ///
    /// The record holds the asking and the effect trails it, exactly as a holder's does: two
    /// readers asking about one moment get one answer.
    #[must_use]
    pub fn come_due(&self, at: Epoch) -> Self {
        let mut settled = self.clone();
        if let Some(continuity) = &self.continuity
            && at.number() >= continuity.due.number()
        {
            settled.owners.extend(continuity.owners.iter().cloned());
            settled.continuity = None;
        }
        settled
    }

    /// Whether it can still govern itself, or has lost its quorum.
    ///
    /// **What emergency continuity exists for** (`SPECS.md §8.3`, `§12.3`). Not an error and not
    /// rare: with small sets it is the ordinary case rather than the exception.
    #[must_use]
    pub fn has_quorum(&self) -> bool {
        u64::try_from(self.owners.len()).unwrap_or(u64::MAX) >= self.thresholds.governance
    }
}

/// The keys the record says speak for each owner, at the moment an act was written.
///
/// Resolved by whoever holds the record and handed in, never taken from the act: which keys speak
/// for a person is their own chain's answer, and an act that carried it would be an act deciding
/// who signed it.
pub type Speaking = BTreeMap<Did, BTreeSet<Vec<u8>>>;

/// The owners who actually signed this operation.
///
/// **Distinct owners and not signatures.** One owner with three devices is one vote
/// (`SPECS.md §8.2`): raising the threshold is for demanding more *people*, which is a different
/// thing and is what it exists for. A signature naming an owner but made with a key that owner's
/// chain does not authorise counts as nobody.
#[must_use]
pub fn counted(operation: &Operation, speaking: &Speaking) -> BTreeSet<Did> {
    let mut signed = BTreeSet::new();
    for signature in &operation.signatures {
        let Some(keys) = speaking.get(&signature.by) else {
            continue;
        };
        if !keys.contains(&signature.key) {
            continue;
        }
        if verified(operation, &signature.key, &signature.signature) {
            signed.insert(signature.by.clone());
        }
    }
    signed
}

/// Whether that key made that signature over this operation.
fn verified(operation: &Operation, key: &[u8], signature: &[u8; 64]) -> bool {
    let Ok(key) = <[u8; p256::PUBLIC_KEY_WIDTH]>::try_from(key) else {
        return false;
    };
    let (Ok(verifying), Ok(made)) = (
        p256::VerifyingKey::from_bytes(key),
        p256::Signature::from_bytes(*signature),
    ) else {
        return false;
    };
    verifying.verify(&operation.signing_bytes(), &made).is_ok()
}

/// The fields an entity act may carry that this build has a meaning for.
#[must_use]
pub fn vocabulary() -> almena_format::field::Vocabulary<'static> {
    use almena_format::field::Field;
    const FIELDS: &[Field] = &[
        Field::new(field::KEY),
        Field::new(field::WHO),
        Field::new(field::ROUTINE),
        Field::new(field::SEALING),
        Field::new(field::GOVERNANCE),
        Field::new(field::DOMAIN),
        Field::new(field::REVOKING),
        Field::new(field::ALIAS),
        Field::new(field::EMAIL),
        Field::new(crate::resolution::FIELD),
    ];
    almena_format::field::Vocabulary::of(FIELDS)
}

/// An entity, as the act that created it made it.
///
/// **The creating owner is named in the act and checked against the record**, because an entity has
/// no key of its own to be self-signed by the way a holder's account does: what an entity's key is
/// for is a channel, not governance. So the one thing that can authorise a creation is a person,
/// and the person has to already exist.
///
/// # Errors
///
/// [`Refused`], and `NotAuthorised` where the named owner did not sign it with a key their own
/// chain authorises.
pub fn born(operation: &Operation, speaking: &Speaking, at: Epoch) -> Result<Entity, Refused> {
    let key = fixed(operation, field::KEY)?;
    let owner = named(operation, field::WHO)?;
    let thresholds = asked(operation)?;

    // One owner, one signature, and the signature has to be theirs. Everything after this counts
    // the same way; this is only the first count, against a set of one.
    if !counted(operation, speaking).contains(&owner) {
        return Err(Refused::NotAuthorised);
    }

    Ok(Entity {
        owners: BTreeSet::from([owner]),
        managers: BTreeSet::new(),
        thresholds,
        key,
        domains: BTreeMap::new(),
        closed: None,
        revoked_on_closing: false,
        continuity: None,
        alias: None,
        cooling: None,
        email: None,
        acted: at,
    })
}

/// An organisation as it stands with one key and nobody named yet.
///
/// **What Almena Government is the moment the genesis creates it** (`SPECS.md §7.1`, `§7.9`): it
/// uses the same mechanism as any other organisation — owners and a threshold per class of act —
/// and on the day a network opens it has neither, because there is nobody yet to be one. The three
/// classes are declared from the start at one owner each, which is what `SPECS.md §8.2` asks of
/// every organisation: raising one later is then a change of configuration and not of shape.
///
/// It is not [`born`] because nobody vouches for it. `born` counts the signature of the owner an
/// act names, and the act that opens a network has no earlier owner to count — that is the whole of
/// what makes it a bootstrap.
#[must_use]
pub fn alone(key: [u8; ed25519::PUBLIC_KEY_WIDTH], at: Epoch) -> Entity {
    Entity {
        owners: BTreeSet::new(),
        managers: BTreeSet::new(),
        thresholds: Thresholds {
            routine: 1,
            sealing: 1,
            governance: 1,
        },
        key,
        domains: BTreeMap::new(),
        closed: None,
        revoked_on_closing: false,
        continuity: None,
        alias: None,
        cooling: None,
        email: None,
        acted: at,
    }
}

/// What an act does to an entity, once it has been established that enough owners signed it.
///
/// # Errors
///
/// [`Refused`].
pub fn does(operation: &Operation, entity: &Entity, kind: Kind) -> Result<Entity, Refused> {
    let mut next = entity.come_due(operation.issued);
    next.acted = operation.issued;

    match kind {
        Kind::ENTITY_ADD_OWNER => {
            next.owners.insert(named(operation, field::WHO)?);
        }
        Kind::ENTITY_REMOVE_OWNER => {
            // **Removed even when it takes the set under its own threshold.** Refusing would be a
            // node deciding an organisation's governance for it; losing quorum is an anticipated
            // state with a way out (`SPECS.md §8.3`), and the way out is why this can be allowed.
            next.owners.remove(&named(operation, field::WHO)?);
        }
        Kind::ENTITY_ADD_MANAGER => {
            next.managers.insert(named(operation, field::WHO)?);
        }
        Kind::ENTITY_REMOVE_MANAGER => {
            next.managers.remove(&named(operation, field::WHO)?);
        }
        Kind::ENTITY_SET_THRESHOLD => next.thresholds = asked(operation)?,
        Kind::ENTITY_ROTATE_KEY => {
            next.key = fixed(operation, field::KEY)?;
            // An organisation may say where to write to it in the same breath as anything else,
            // which is why this rides here rather than having an act of its own — and it is held to
            // the domain it proved whichever act carries it.
            reachable(operation, &mut next)?;
        }
        Kind::ENTITY_ADD_DOMAIN => proved(operation, &mut next)?,
        Kind::ENTITY_SET_ALIAS => claimed(operation, &mut next)?,
        Kind::ENTITY_REMOVE_DOMAIN => given_up(operation, &mut next)?,
        Kind::ENTITY_CLOSE => {
            next.closed = Some(operation.issued);
            next.revoked_on_closing = matches!(
                operation.payload.get(&field::REVOKING),
                Some(Value::Uint(1))
            );
        }
        Kind::ENTITY_CONTINUITY => next.continuity = Some(replenishing(operation, entity)?),
        Kind::ENTITY_VETO => {
            // Any surviving owner, during the window. Saying no needs nobody's agreement, which is
            // the same asymmetry `SPECS.md §11.12` gives a device against the words.
            if next.continuity.is_none() {
                return Err(Refused::NotAuthorised);
            }
            next.continuity = None;
        }
        // A summary restates what the chain already produces and concedes nothing.
        Kind::ENTITY_CHECKPOINT => {}
        _ => return Err(Refused::Malformed),
    }
    Ok(next)
}

/// Owners being put back, and when that lands.
///
/// **It does one thing: replenish owners until there is a quorum again.** Nothing else, which is
/// what makes a threshold of one safe enough to have at all (`SPECS.md §8.3`) — and why an entity
/// that can still govern itself has no business using it.
fn replenishing(operation: &Operation, entity: &Entity) -> Result<Continuity, Refused> {
    if entity.has_quorum() {
        return Err(Refused::NotAuthorised);
    }
    Ok(Continuity {
        owners: BTreeSet::from([named(operation, field::WHO)?]),
        due: operation
            .issued
            .plus(CONTINUITY_WAITS.epochs(operation.issued))
            .ok_or(Refused::Malformed)?,
        act: operation.called(),
    })
}

/// Give a domain up, and with it whatever hung from it.
///
/// **Giving up the domain costs the name it derived from** (`SPECS.md §7.5`), and the name then
/// cools before anybody else may have it — so that nobody inherits the reputation of whoever left.
fn given_up(operation: &Operation, next: &mut Entity) -> Result<(), Refused> {
    let domain = domain(operation)?;
    next.domains.remove(&domain);
    if next
        .alias
        .as_ref()
        .is_some_and(|alias| alias.from == domain)
    {
        cool(next, operation.issued);
    }
    Ok(())
}

/// Take an address to reach the organisation at, under a domain it has proved.
///
/// **Consistent with the verified domain, or it is not taken** (`SPECS.md §7.6`). An address under
/// a domain nobody proved would be a way to look official without having shown anything, and a
/// domain is the only thing here that has been proved in both directions.
///
/// Absent leaves what is there, which is not the same as clearing it: an organisation that changed
/// its key has said nothing about where to write to it.
fn reachable(operation: &Operation, next: &mut Entity) -> Result<(), Refused> {
    let Some(Value::Text(email)) = operation.payload.get(&field::EMAIL) else {
        return Ok(());
    };
    if email.is_empty() {
        next.email = None;
        return Ok(());
    }
    let Some((_, under)) = email.split_once('@') else {
        return Err(Refused::Malformed);
    };
    let under = under.to_lowercase();
    if !next
        .domains
        .keys()
        .any(|domain| *domain == under || under.ends_with(&format!(".{domain}")))
    {
        return Err(Refused::NotAuthorised);
    }
    next.email = Some(email.to_lowercase());
    Ok(())
}

/// Take a name the entity claims, derived from a domain it has already proved.
///
/// # What is checked here, and what is not
///
/// - **It derives from a domain this entity holds** (`SPECS.md §7.5`). A free name would be a name
///   anybody could pick; a domain is a claim somebody already proved in both directions.
/// - **It is written the one way it is compared.** Internationalised domains admit homoglyphs that
///   render identically, so the character set is restricted and the form is refused rather than
///   repaired: a rule that quietly rewrote what was signed would make what was signed and what was
///   stored two different things.
/// - **A name still cooling is not free.** Ninety days after somebody gave one up or lost it, and
///   not before.
///
/// What is **not** checked here is the seal, and that is not an omission — whether this entity is
/// certified is a fact about a different object, so it is asked where the whole record is.
fn claimed(operation: &Operation, next: &mut Entity) -> Result<(), Refused> {
    let Some(Value::Text(name)) = operation.payload.get(&field::ALIAS) else {
        return Err(Refused::Malformed);
    };
    let name = readable(name).ok_or(Refused::Malformed)?;

    // The domain it derives from: the one whose first label this name is, and which this entity has
    // proved. Anything else is a name chosen freely, which is what deriving exists to prevent.
    let from = next
        .domains
        .keys()
        .find(|domain| domain.split('.').next() == Some(name.as_str()))
        .ok_or(Refused::NotAuthorised)?
        .clone();

    // Its own cooling name is its own to take back; somebody else's is not, and that is settled
    // where the whole record is.
    if next.cooling.as_ref().is_some_and(|cooling| {
        cooling.name == name && operation.issued.number() < cooling.until.number()
    }) {
        next.cooling = None;
    }

    next.alias = Some(Alias {
        name,
        from,
        claimed_by: operation.called(),
        since: operation.issued,
    });
    Ok(())
}

/// Put the name this entity holds into quarantine, and let go of it.
fn cool(next: &mut Entity, at: Epoch) {
    if let Some(alias) = next.alias.take() {
        next.cooling = Some(Cooling {
            name: alias.name,
            until: at
                .plus(ALIAS_QUARANTINE.epochs(at))
                .unwrap_or(Epoch::new(u64::MAX)),
        });
    }
}

/// A name in the one form it is compared in, or nothing.
///
/// **Restricted and refused rather than repaired.** Internationalised domains admit homoglyphs that
/// render identically (`SPECS.md §7.5`), so what is admitted is lower-case ASCII letters, digits
/// and the hyphen — the set a hostname label may hold — and anything else is not this name.
fn readable(name: &str) -> Option<String> {
    const LONGEST: usize = 63;
    if name.is_empty() || name.len() > LONGEST {
        return None;
    }
    if name.starts_with('-') || name.ends_with('-') {
        return None;
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return None;
    }
    Some(name.to_owned())
}

/// The three numbers an act declares, refused if any of them is none at all.
///
/// **A threshold of nought is not a low threshold, it is no threshold** — an entity anybody could
/// sign for. One and one is where `SPECS.md §8.2` says this starts, and the act asks for the three
/// numbers explicitly rather than defaulting to ones nobody looked at.
fn asked(operation: &Operation) -> Result<Thresholds, Refused> {
    let thresholds = Thresholds {
        routine: number(operation, field::ROUTINE)?,
        sealing: number(operation, field::SEALING)?,
        governance: number(operation, field::GOVERNANCE)?,
    };
    if thresholds.routine == 0 || thresholds.sealing == 0 || thresholds.governance == 0 {
        return Err(Refused::Malformed);
    }
    Ok(thresholds)
}

/// Take a domain into the entity, and decide whether it is the one a screen shows first.
///
/// **A domain in the register belongs to one entity** (`SPECS.md §7.5`). Whether it is claimed
/// elsewhere is a question about the whole record and is settled where the whole record is; what
/// is settled here is only what this entity says about itself.
fn proved(operation: &Operation, next: &mut Entity) -> Result<(), Refused> {
    let domain = domain(operation)?;
    let principal = matches!(
        operation.payload.get(&field::PRINCIPAL),
        Some(Value::Uint(1))
    ) || next.domains.is_empty();
    if principal {
        for held in next.domains.values_mut() {
            held.principal = false;
        }
    }
    next.domains.insert(
        domain,
        Domain {
            proved: operation.issued,
            principal,
        },
    );
    Ok(())
}

/// The key an act carries at that field.
fn fixed(operation: &Operation, at: u64) -> Result<[u8; ed25519::PUBLIC_KEY_WIDTH], Refused> {
    let Some(Value::Bytes(bytes)) = operation.payload.get(&at) else {
        return Err(Refused::Malformed);
    };
    bytes.as_slice().try_into().map_err(|_| Refused::Malformed)
}

/// The identifier an act carries at that field.
fn named(operation: &Operation, at: u64) -> Result<Did, Refused> {
    let Some(Value::Text(text)) = operation.payload.get(&at) else {
        return Err(Refused::Malformed);
    };
    Did::parse(text).map_err(|_| Refused::Malformed)
}

/// The number an act carries at that field.
fn number(operation: &Operation, at: u64) -> Result<u64, Refused> {
    match operation.payload.get(&at) {
        Some(Value::Uint(number)) => Ok(*number),
        _ => Err(Refused::Malformed),
    }
}

/// The domain an act is about, in the one form it is compared in.
///
/// **Lower case, and refused rather than repaired if it is not already.** Two spellings of one
/// domain would be two entries in a map that decides who a name belongs to, and a rule that
/// silently repaired what somebody wrote is a rule under which what was signed and what was stored
/// are two different things.
fn domain(operation: &Operation) -> Result<String, Refused> {
    let Some(Value::Text(text)) = operation.payload.get(&field::DOMAIN) else {
        return Err(Refused::Malformed);
    };
    if text.is_empty() || *text != text.to_lowercase() || text.contains(char::is_whitespace) {
        return Err(Refused::Malformed);
    }
    Ok(text.clone())
}

#[cfg(test)]
mod tests {
    use super::{Class, Continuity, Entity, Speaking, Thresholds, born, class, counted, does};
    use crate::chain::Refused;
    use crate::kind::Kind;
    use almena_format::cbor::Value;
    use almena_format::identifier::{Did, Name, Network};
    use almena_format::operation::{Operation, Signed, create};
    use almena_suite::p256;
    use almena_time::Epoch;
    use std::collections::{BTreeMap, BTreeSet};

    fn now() -> Epoch {
        Epoch::GENESIS
            .plus(almena_time::Epochs(100))
            .expect("no overflow")
    }

    fn key(seed: u8) -> p256::SigningKey {
        p256::SigningKey::from_secret([seed; 32]).expect("a valid scalar")
    }

    /// One person, named by their account.
    fn owner(seed: u8) -> Did {
        Did::new(Network::Development, Name::of(&[seed; 8]))
    }

    /// What the record says about a set of people, each with one device.
    fn speaking(people: &[(u8, u8)]) -> Speaking {
        people
            .iter()
            .map(|(who, device)| {
                (
                    owner(*who),
                    BTreeSet::from([key(*device).verifying_key().bytes().to_vec()]),
                )
            })
            .collect()
    }

    /// An act on an entity's chain, signed by the people named.
    fn act(kind: Kind, payload: BTreeMap<u64, Value>, signers: &[(u8, u8)]) -> Operation {
        let mut operation = Operation {
            object: Did::new(Network::Development, Name::of(b"an entity")),
            previous: Some(Name::of(b"whatever came before")),
            kind: kind.number(),
            version: 1,
            issued: now(),
            payload,
            signatures: Vec::new(),
        };
        sign(&mut operation, signers);
        operation
    }

    fn sign(operation: &mut Operation, signers: &[(u8, u8)]) {
        let over = operation.signing_bytes();
        for (who, device) in signers {
            operation.signatures.push(Signed {
                by: owner(*who),
                key: key(*device).verifying_key().bytes().to_vec(),
                signature: key(*device).sign(&over).bytes(),
            });
        }
    }

    /// A creation, which names its first owner and its three thresholds.
    fn creation(routine: u64, sealing: u64, governance: u64, signers: &[(u8, u8)]) -> Operation {
        let mut operation = create(
            Network::Development,
            Kind::ENTITY_CREATE.number(),
            1,
            now(),
            BTreeMap::from([
                (super::field::KEY, Value::Bytes(vec![9; 32])),
                (super::field::WHO, Value::Text(owner(1).to_string())),
                (super::field::ROUTINE, Value::Uint(routine)),
                (super::field::SEALING, Value::Uint(sealing)),
                (super::field::GOVERNANCE, Value::Uint(governance)),
            ]),
        );
        sign(&mut operation, signers);
        operation
    }

    fn an_entity(owners: &[u8], thresholds: Thresholds) -> Entity {
        Entity {
            owners: owners.iter().copied().map(owner).collect(),
            managers: BTreeSet::new(),
            thresholds,
            key: [9; 32],
            domains: BTreeMap::new(),
            closed: None,
            revoked_on_closing: false,
            continuity: None,
            alias: None,
            cooling: None,
            email: None,
            acted: now(),
        }
    }

    fn one_and_one() -> Thresholds {
        Thresholds {
            routine: 1,
            sealing: 1,
            governance: 1,
        }
    }

    #[test]
    fn an_entity_starts_at_one_owner_and_one_approver() {
        // **Forcing somebody to find a second owner to create an entity is a barrier that does not
        // pay for itself** (`SPECS.md §8.2`), and most start as one person.
        let entity = born(&creation(1, 1, 1, &[(1, 11)]), &speaking(&[(1, 11)]), now())
            .expect("its own owner signed it");
        assert_eq!(entity.owners, BTreeSet::from([owner(1)]));
        assert_eq!(entity.thresholds, one_and_one());
        assert!(entity.has_quorum());
    }

    #[test]
    fn naming_somebody_as_the_owner_is_not_being_them() {
        // The act says who its owner is; the record says which keys are theirs. Without the second,
        // anybody could create an entity in a stranger's name and hold it.
        assert_eq!(
            born(&creation(1, 1, 1, &[(1, 99)]), &speaking(&[(1, 11)]), now()),
            Err(Refused::NotAuthorised),
            "a key that owner's chain does not authorise"
        );
        assert_eq!(
            born(&creation(1, 1, 1, &[(2, 22)]), &speaking(&[(2, 22)]), now()),
            Err(Refused::NotAuthorised),
            "somebody else entirely, signing perfectly well"
        );
    }

    #[test]
    fn a_threshold_of_nought_is_not_a_low_threshold_but_none_at_all() {
        // An entity anybody could sign for. The creation asks for the three numbers explicitly
        // rather than defaulting to one nobody looked at (`SPECS.md §8.2`).
        for (routine, sealing, governance) in [(0, 1, 1), (1, 0, 1), (1, 1, 0)] {
            assert_eq!(
                born(
                    &creation(routine, sealing, governance, &[(1, 11)]),
                    &speaking(&[(1, 11)]),
                    now()
                ),
                Err(Refused::Malformed)
            );
        }
    }

    #[test]
    fn one_owner_with_three_devices_is_one_vote() {
        // **`k-de-n` counts owners, not devices** (`SPECS.md §8.2`). Raising the threshold is for
        // demanding more *people*; tolerating a lost phone is what `SPECS.md §11.11` is for.
        let mut speaking = Speaking::new();
        speaking.insert(
            owner(1),
            [11u8, 12, 13]
                .into_iter()
                .map(|seed| key(seed).verifying_key().bytes().to_vec())
                .collect(),
        );
        let signed = act(
            Kind::ENTITY_ADD_OWNER,
            BTreeMap::from([(super::field::WHO, Value::Text(owner(2).to_string()))]),
            &[(1, 11), (1, 12), (1, 13)],
        );
        assert_eq!(counted(&signed, &speaking), BTreeSet::from([owner(1)]));
    }

    #[test]
    fn a_signature_wearing_somebody_else_s_name_counts_as_nobody() {
        // The name on a signature is not covered by the signature, so it is worth nothing on its
        // own: the key has to be one that owner's own chain authorises.
        let mut signed = act(
            Kind::ENTITY_ADD_OWNER,
            BTreeMap::from([(super::field::WHO, Value::Text(owner(3).to_string()))]),
            &[(1, 11)],
        );
        // Somebody who saw it go past relabels the signature as a second owner's.
        signed.signatures[0].by = owner(2);
        assert!(counted(&signed, &speaking(&[(1, 11), (2, 22)])).is_empty());
    }

    #[test]
    fn a_signature_that_does_not_check_counts_as_nobody() {
        let mut signed = act(
            Kind::ENTITY_ADD_OWNER,
            BTreeMap::from([(super::field::WHO, Value::Text(owner(3).to_string()))]),
            &[(1, 11), (2, 22)],
        );
        signed.signatures[0].signature[0] ^= 1;
        assert_eq!(
            counted(&signed, &speaking(&[(1, 11), (2, 22)])),
            BTreeSet::from([owner(2)]),
            "and the one beside it still counts"
        );
    }

    #[test]
    fn the_three_classes_ask_for_three_different_numbers() {
        // Which is the whole reason the register uses counted signatures rather than a threshold
        // signature: one sharing has one `k` and cannot tell a routine act from a governance one.
        assert_eq!(class(Kind::ENTITY_ADD_MANAGER), Some(Class::Routine));
        assert_eq!(class(Kind::ENTITY_ADD_OWNER), Some(Class::Governance));
        assert_eq!(class(Kind::ENTITY_ADD_DOMAIN), Some(Class::Governance));
        assert_eq!(class(Kind::HOLDER_ADD_DEVICE), None);

        let thresholds = Thresholds {
            routine: 1,
            sealing: 2,
            governance: 3,
        };
        assert_eq!(thresholds.of(Class::Routine), 1);
        assert_eq!(thresholds.of(Class::Sealing), 2);
        assert_eq!(thresholds.of(Class::Governance), 3);
    }

    #[test]
    fn an_owner_can_be_removed_even_when_it_costs_the_entity_its_quorum() {
        // **Refusing would be a node deciding an organisation's governance for it.** Losing quorum
        // is an anticipated state with a way out (`SPECS.md §8.3`), and the way out is what makes
        // allowing this safe rather than reckless.
        let entity = an_entity(
            &[1, 2],
            Thresholds {
                governance: 2,
                ..one_and_one()
            },
        );
        let after = does(
            &act(
                Kind::ENTITY_REMOVE_OWNER,
                BTreeMap::from([(super::field::WHO, Value::Text(owner(2).to_string()))]),
                &[(1, 11), (2, 22)],
            ),
            &entity,
            Kind::ENTITY_REMOVE_OWNER,
        )
        .expect("both signed");
        assert_eq!(after.owners, BTreeSet::from([owner(1)]));
        assert!(!after.has_quorum(), "and it says so rather than pretending");
    }

    #[test]
    fn continuity_puts_owners_back_and_waits_sixty_days_in_public() {
        let entity = an_entity(
            &[1],
            Thresholds {
                governance: 2,
                ..one_and_one()
            },
        );
        assert!(!entity.has_quorum());

        let asked = does(
            &act(
                Kind::ENTITY_CONTINUITY,
                BTreeMap::from([(super::field::WHO, Value::Text(owner(3).to_string()))]),
                &[(1, 11)],
            ),
            &entity,
            Kind::ENTITY_CONTINUITY,
        )
        .expect("one surviving owner is the threshold");

        assert_eq!(asked.owners, BTreeSet::from([owner(1)]), "not yet");
        let due = asked.continuity.as_ref().expect("waiting").due;
        assert_eq!(due.number(), now().number() + super::CONTINUITY_WAITS.now());
        assert_eq!(
            asked.come_due(due).owners,
            BTreeSet::from([owner(1), owner(3)])
        );
    }

    #[test]
    fn continuity_is_only_for_an_entity_that_has_lost_its_quorum() {
        // It does one thing, and that is what makes a threshold of one safe enough to have. An
        // entity that can still govern itself has the governance threshold and does not need this.
        let entity = an_entity(&[1, 2], one_and_one());
        assert_eq!(
            does(
                &act(
                    Kind::ENTITY_CONTINUITY,
                    BTreeMap::from([(super::field::WHO, Value::Text(owner(3).to_string()))]),
                    &[(1, 11)],
                ),
                &entity,
                Kind::ENTITY_CONTINUITY,
            ),
            Err(Refused::NotAuthorised)
        );
    }

    #[test]
    fn any_surviving_owner_stops_a_continuity_while_it_waits() {
        // Saying no needs nobody's agreement, which is the same asymmetry a device has against the
        // words in `SPECS.md §11.12`.
        let mut entity = an_entity(
            &[1, 2],
            Thresholds {
                governance: 3,
                ..one_and_one()
            },
        );
        entity.continuity = Some(Continuity {
            owners: BTreeSet::from([owner(9)]),
            due: Epoch::new(now().number() + 1_000),
            act: Name::of(b"the asking"),
        });

        let after = does(
            &act(Kind::ENTITY_VETO, BTreeMap::new(), &[(2, 22)]),
            &entity,
            Kind::ENTITY_VETO,
        )
        .expect("one owner");
        assert!(after.continuity.is_none());
        assert_eq!(
            after.come_due(Epoch::new(now().number() + 2_000)).owners,
            BTreeSet::from([owner(1), owner(2)]),
            "and what it would have added never lands"
        );
    }

    #[test]
    fn a_domain_stands_for_thirty_days_and_then_owes_a_proof() {
        // A month bounds how long a domain that already changed hands can keep saying it belongs
        // here, and is frequent enough to tell a passing DNS failure from an abandonment.
        let after = does(
            &act(
                Kind::ENTITY_ADD_DOMAIN,
                BTreeMap::from([(
                    super::field::DOMAIN,
                    Value::Text("almena.network".to_owned()),
                )]),
                &[(1, 11)],
            ),
            &an_entity(&[1], one_and_one()),
            Kind::ENTITY_ADD_DOMAIN,
        )
        .expect("signed");

        let domain = after.domains.get("almena.network").expect("added");
        assert!(domain.principal, "the first one is the one a screen shows");
        assert!(domain.stands(now()));
        assert!(!domain.stands(Epoch::new(now().number() + super::DOMAIN_STANDS.now())));
    }

    #[test]
    fn a_domain_written_two_ways_would_be_two_domains_so_only_one_way_is_taken() {
        // The map decides who a name belongs to. Silently repairing what somebody wrote would make
        // what was signed and what was stored two different things.
        for written in ["Almena.Network", "", "almena network"] {
            assert_eq!(
                does(
                    &act(
                        Kind::ENTITY_ADD_DOMAIN,
                        BTreeMap::from([(super::field::DOMAIN, Value::Text(written.to_owned()))]),
                        &[(1, 11)],
                    ),
                    &an_entity(&[1], one_and_one()),
                    Kind::ENTITY_ADD_DOMAIN,
                ),
                Err(Refused::Malformed),
                "{written}"
            );
        }
    }

    #[test]
    fn closing_is_a_state_with_a_date_and_never_a_deletion() {
        // Credentials point at an entity and a verifier has to be able to resolve it
        // (`SPECS.md §12.1`), so its history stays and what changes is what it says about itself.
        let after = does(
            &act(
                Kind::ENTITY_CLOSE,
                BTreeMap::from([(super::field::REVOKING, Value::Uint(1))]),
                &[(1, 11)],
            ),
            &an_entity(&[1], one_and_one()),
            Kind::ENTITY_CLOSE,
        )
        .expect("signed");
        assert_eq!(after.closed, Some(now()));
        assert!(after.revoked_on_closing, "which was the closer's to choose");
        assert_eq!(
            after.owners,
            BTreeSet::from([owner(1)]),
            "and it is all still there"
        );
    }

    #[test]
    fn an_address_has_to_be_under_a_domain_the_organisation_proved() {
        // **Consistent with the verified domain, or it is not taken** (`SPECS.md §7.6`). One under
        // a domain nobody proved would be a way to look official without having shown anything.
        let mut entity = an_entity(&[1], one_and_one());
        entity.domains.insert(
            "panaderia.example".to_owned(),
            super::Domain {
                proved: now(),
                principal: true,
            },
        );

        let setting = |written: &str| {
            does(
                &act(
                    Kind::ENTITY_ROTATE_KEY,
                    BTreeMap::from([
                        (super::field::KEY, Value::Bytes(vec![4; 32])),
                        (super::field::EMAIL, Value::Text(written.to_owned())),
                    ]),
                    &[(1, 11)],
                ),
                &entity,
                Kind::ENTITY_ROTATE_KEY,
            )
        };

        assert_eq!(
            setting("hola@panaderia.example").map(|after| after.email),
            Ok(Some("hola@panaderia.example".to_owned()))
        );
        assert_eq!(
            setting("Hola@Panaderia.Example").map(|after| after.email),
            Ok(Some("hola@panaderia.example".to_owned())),
            "and compared in one form, because two spellings would be two addresses"
        );
        assert_eq!(
            setting("hola@correo.panaderia.example").map(|after| after.email),
            Ok(Some("hola@correo.panaderia.example".to_owned())),
            "under it counts as under it"
        );
        assert_eq!(setting("hola@somewhere.else"), Err(Refused::NotAuthorised));
        assert_eq!(setting("not an address at all"), Err(Refused::Malformed));
    }

    #[test]
    fn an_act_that_says_nothing_about_an_address_has_said_nothing_about_it() {
        // Absent leaves what is there; empty clears it. An organisation that changed its key has
        // not said anything about where to write to it.
        let mut entity = an_entity(&[1], one_and_one());
        entity.email = Some("hola@panaderia.example".to_owned());
        entity.domains.insert(
            "panaderia.example".to_owned(),
            super::Domain {
                proved: now(),
                principal: true,
            },
        );

        let after = does(
            &act(
                Kind::ENTITY_ROTATE_KEY,
                BTreeMap::from([(super::field::KEY, Value::Bytes(vec![4; 32]))]),
                &[(1, 11)],
            ),
            &entity,
            Kind::ENTITY_ROTATE_KEY,
        )
        .expect("signed");
        assert_eq!(after.email.as_deref(), Some("hola@panaderia.example"));

        let cleared = does(
            &act(
                Kind::ENTITY_ROTATE_KEY,
                BTreeMap::from([
                    (super::field::KEY, Value::Bytes(vec![4; 32])),
                    (super::field::EMAIL, Value::Text(String::new())),
                ]),
                &[(1, 11)],
            ),
            &entity,
            Kind::ENTITY_ROTATE_KEY,
        )
        .expect("signed");
        assert_eq!(cleared.email, None);
    }

    #[test]
    fn what_it_last_did_is_published_rather_than_judged() {
        // There is no way to tell a frozen entity from one with nothing to do, so no label is put
        // on it: the date goes where the decision is taken (`SPECS.md §12.3`).
        let entity = an_entity(&[1], one_and_one());
        let later = Epoch::new(now().number() + 500);
        let mut signed = act(
            Kind::ENTITY_ADD_MANAGER,
            BTreeMap::from([(super::field::WHO, Value::Text(owner(4).to_string()))]),
            &[],
        );
        signed.issued = later;
        sign(&mut signed, &[(1, 11)]);

        let after = does(&signed, &entity, Kind::ENTITY_ADD_MANAGER).expect("signed");
        assert_eq!(after.acted, later);
        assert_eq!(after.managers, BTreeSet::from([owner(4)]));
    }
}
