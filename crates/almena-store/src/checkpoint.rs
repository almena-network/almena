//! A summary of an object's state that falls over if it lies.
//!
//! An object's state is not written anywhere: it is worked out by replaying its chain from the
//! start. That is fine for an account with a dozen acts and ruinous for a network where everybody
//! arriving redoes everybody's history. A summary is the state so far, signed inside the object's
//! own chain, so that whoever comes later takes **the summary and what came after it**.
//!
//! # Signing it stops anybody else forging it, and does not stop the object lying
//!
//! Only the object can sign its own summary, so no node, competitor or platform can make one up.
//! But an object — or somebody holding one of its keys — can sign a summary that is not true, and a
//! routine signature would then be making claims about governance to everybody who arrives
//! afterwards.
//!
//! **There are two ways for a summary to be untrue, and they are caught by different things.**
//!
//! # Hiding an act: caught by the log, for nothing
//!
//! > **Every part of the state carries the hash of the act that last set it.** *The devices are
//! > these, and `h1` set them.*
//!
//! The log carries, for every entry, which object it is about and what kind of act it is — and
//! every node holds the log. So anybody can look at that object's acts and ask whether some act
//! that governs this part came *after* the one cited and *before* the summary. No history, nobody
//! asked, and no trust in whoever served it.
//!
//! # Making a value up: caught by the acts that set it, cheaply
//!
//! Citing an act does not pin what the state **is**. A summary can name the right last act and
//! still state a value nothing ever produced — swap a device for one nobody added, drop an owner
//! who was never removed — and no amount of looking at the log alone will show it, because the log
//! says which acts happened and not what they said.
//!
//! What settles it is the acts themselves: **the value has to be what the acts that govern it
//! produce.** That costs fetching them, and it stays cheap for the reason a summary exists at all —
//! a chain is mostly acts that say nothing about any one part of the state, and none of those is
//! fetched. A person's account has a handful of acts that touch its devices in a lifetime.
//!
//! **A summary is worth exactly as much as whichever of the two checks was run**, and saying which
//! is part of the answer here rather than something a caller has to remember.
//!
//! # What governs what is protocol, not convention
//!
//! Which kinds of act govern which parts of the state comes from the table of operations and is
//! versioned with it. Guessing it here would make a summary fall over or stand up depending on who
//! was reading — which is also why an act of a kind this build has never heard of is answered with
//! *cannot say* and never with *stands*: an act nobody can read might be exactly the one that
//! governs the part being claimed.

use std::collections::{BTreeMap, BTreeSet};

use almena_format::cbor::Value;
use almena_format::entry::Entry;
use almena_format::identifier::Name;
use almena_format::operation::Operation;
use almena_time::Epoch;

use crate::kind::Kind;

/// Where an operation carries a summary of the state it leaves behind.
///
/// **Even, so it may be ignored** — and that is the honest mark rather than a convenience. Critical
/// means *if you cannot read this you cannot claim to have applied the act*, and that is false
/// here: a reader that skips the summary replays the chain and lands in exactly the state the
/// summary described, because a summary restates what the chain already produces. It saves reading
/// and changes no meaning.
///
/// Marking it critical would also be dangerous rather than merely wrong. A summary rides on ordinary
/// acts, so the day its shape has to grow, every node one version behind would declare unresolvable
/// every object that had ever written enough to owe one — turning *some objects are behind* into
/// *the busiest half of the network goes dark*.
///
/// The number is above the boundary where a field means the same thing whatever kind carries it,
/// because this one does: a summary of the resulting state is the same idea on every act there is.
pub const FIELD: u64 = almena_format::field::COMMON;

/// A part of an object's state, what it is, and which act last set it.
///
/// **Both halves, because each catches a different lie.** The hash is what the log can check for
/// nothing; the value is what the summary is actually for, and it is only worth anything once it
/// has been held against the acts that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// Which part of the state this is about.
    pub about: Governs,
    /// What that part is, as this summary states it.
    pub stated: Stated,
    /// The act that last set it, as this summary claims.
    pub set_by: Name,
}

/// What a part of an object's state is.
///
/// One shape per part, fixed by the part and never by what arrived. Thresholds, aliases and domains
/// arrive with entities, and standing in for them now would be inventing a format for something
/// nobody has designed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stated {
    /// One key.
    Key(Vec<u8>),
    /// A set of keys, which is a set and not a list: the order they were added in is not part of
    /// what the state **is**, and letting it into the bytes would give one state two summaries.
    Keys(BTreeSet<Vec<u8>>),
    /// Whether something is so.
    Flag(bool),
    /// A set of acts, by name — what the words have asked for that has not landed.
    ///
    /// A set for the reason the keys are one: the order they entered is the chain's business, and
    /// letting it into the bytes would give one queue two summaries.
    Names(BTreeSet<Name>),
    /// A set of numbers from a closed list — what a node offers.
    Numbers(BTreeSet<u64>),
    /// A set of addresses — where a node says it can be reached.
    Addresses(BTreeSet<String>),
    /// A moment, or none yet — when a node closed.
    Moment(Option<Epoch>),
}

/// A part of an object's state that some kinds of act govern.
///
/// **Not every field of every object** — only the ones a summary may claim, which is the same list
/// as the ones an act can change. What an entity's summary may claim arrives with entities.
///
/// Two objects have parts: an account and a node. They share nothing, and a summary is measured
/// against the parts of **its own** object — asked of a node, the control key is the wrong question
/// rather than a missing answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Governs {
    /// The key that controls an account.
    Control,
    /// The devices an account may act through.
    Devices,
    /// Whether the account is stopped.
    ///
    /// **A part, so that a summary cannot describe a stopped account as moving.** Without it a
    /// summary written over a freeze would let a reader bootstrap the account somebody stopped —
    /// because their phone was in a stranger's pocket — as though nothing had happened.
    Frozen,
    /// What the words have asked for alone that has not yet landed, by the act that asked.
    ///
    /// **A part, so that a summary cannot carry an asking out of sight.** What the control key
    /// signs alone waits a window in which every device can say no; a summary that had nowhere to
    /// say what is waiting would let a thief of the words summarise their own asking away, to land
    /// later on a state no reader was shown. Names and not effects: whoever needs to know what an
    /// asking does fetches the act, which the name is for.
    Pending,
    /// The key a node signs its roots with.
    NodeKey,
    /// What a node says it is running.
    Offers,
    /// Where a node says it can be reached.
    Reachable,
    /// When a node stopped counting, if it has.
    Closed,
}

/// Which object a part belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// A person's account.
    Holder,
    /// A node's own chain.
    Node,
}

impl Governs {
    /// The parts of an account.
    pub const HOLDER: [Self; 4] = [Self::Control, Self::Devices, Self::Frozen, Self::Pending];

    /// The parts of a node.
    pub const NODE: [Self; 4] = [Self::NodeKey, Self::Offers, Self::Reachable, Self::Closed];

    /// Every part of any state, in the order they are numbered.
    ///
    /// **A summary is measured against what the state has, not against what it chose to mention.**
    /// One that declares the devices and says nothing about the control key is not a smaller
    /// summary — it is one whose silence would be read as *unchanged* by everybody who arrives.
    /// Which of these a given object has is [`Self::of`].
    pub const ALL: [Self; 8] = [
        Self::Control,
        Self::Devices,
        Self::Frozen,
        Self::Pending,
        Self::NodeKey,
        Self::Offers,
        Self::Reachable,
        Self::Closed,
    ];

    /// The parts an object has, from the act that created it.
    ///
    /// The creating act is the one thing every reader of a branch holds without fetching: it is
    /// the first entry, and its kind says what class of object this is. [`None`] for a class whose
    /// state has no parts a summary may claim.
    #[must_use]
    pub fn of(creation: Kind) -> Option<&'static [Self]> {
        match creation {
            Kind::HOLDER_CREATE => Some(&Self::HOLDER),
            Kind::NODE_ANNOUNCE => Some(&Self::NODE),
            _ => None,
        }
    }

    /// Which object this part belongs to.
    const fn class(self) -> Class {
        match self {
            Self::Control | Self::Devices | Self::Frozen | Self::Pending => Class::Holder,
            Self::NodeKey | Self::Offers | Self::Reachable | Self::Closed => Class::Node,
        }
    }

    /// How it travels.
    ///
    /// Numbered, and never renumbered, for the reason acts are: a number that changed meaning would
    /// make an old summary say something its author never said. Zero is no part of any state.
    #[must_use]
    pub const fn number(self) -> u64 {
        match self {
            Self::Control => 1,
            Self::Devices => 2,
            Self::Frozen => 3,
            Self::Pending => 4,
            Self::NodeKey => 5,
            Self::Offers => 6,
            Self::Reachable => 7,
            Self::Closed => 8,
        }
    }

    /// The part of the state a number names, if this build knows it.
    #[must_use]
    pub const fn new(number: u64) -> Option<Self> {
        match number {
            1 => Some(Self::Control),
            2 => Some(Self::Devices),
            3 => Some(Self::Frozen),
            4 => Some(Self::Pending),
            5 => Some(Self::NodeKey),
            6 => Some(Self::Offers),
            7 => Some(Self::Reachable),
            8 => Some(Self::Closed),
            _ => None,
        }
    }

    /// The kinds of act that set this part of the state.
    ///
    /// From the table of operations, which is what makes a summary fall over or stand up the same
    /// way for everybody reading it.
    #[must_use]
    pub fn set_by(self) -> &'static [Kind] {
        match self {
            // Creating an account establishes its control key; rotating and recovering replace
            // it. A cancellation belongs here because what the control key asks alone only lands
            // after a wait, and a cancellation is a device striking one of those askings out — a
            // summary that hid one would describe a rotation as coming that is never coming.
            Self::Control => &[
                Kind::HOLDER_CREATE,
                Kind::HOLDER_ROTATE,
                Kind::HOLDER_RECOVER,
                Kind::HOLDER_CANCEL,
            ],
            // Creating an account settles that it has none yet, which is a thing a summary may
            // claim. Recovering belongs here as much as under the control key: it empties the set
            // and enrols the device that asked, so a summary citing an earlier `add_device` would
            // otherwise stand while describing devices that were wiped. Rotating is here for the
            // reading rather than the value: whether an act was the words asking — and so whether
            // its effect waited — is judged against the control key of its moment, and without the
            // rotations nobody can say which key that was. And a cancellation, for the same reason
            // as above.
            Self::Devices => &[
                Kind::HOLDER_CREATE,
                Kind::HOLDER_ADD_DEVICE,
                Kind::HOLDER_REMOVE_DEVICE,
                Kind::HOLDER_ROTATE,
                Kind::HOLDER_RECOVER,
                Kind::HOLDER_CANCEL,
            ],
            // Creating settles that the account is moving. Freezing stops it at once; thawing is
            // the words asking, so it waits, and a cancellation may strike the thaw out. Recovering
            // leaves the account thawed.
            Self::Frozen => &[
                Kind::HOLDER_CREATE,
                Kind::HOLDER_FREEZE,
                Kind::HOLDER_UNFREEZE,
                Kind::HOLDER_RECOVER,
                Kind::HOLDER_CANCEL,
            ],
            // Everything the words may ask for alone enters the queue, and a cancellation leaves
            // it. Creating settles that nothing is waiting yet. Every asking kind is here whoever
            // signed it — a device's addition lands at once and leaves the queue as it found it,
            // and citing it as the act that last settled the queue is still citing the truth.
            Self::Pending => &[
                Kind::HOLDER_CREATE,
                Kind::HOLDER_ADD_DEVICE,
                Kind::HOLDER_REMOVE_DEVICE,
                Kind::HOLDER_ROTATE,
                Kind::HOLDER_UNFREEZE,
                Kind::HOLDER_SET_GUARDIANS,
                Kind::HOLDER_RECOVER,
                Kind::HOLDER_CANCEL,
            ],
            // A node's first announcement names it and fixes its key; every announcement after
            // says again what it offers and where it is. The key never changes and the later
            // announcements are cited for it all the same: a summary cites the latest act that
            // governs each part, and a re-announcement governs the key by leaving it alone.
            Self::NodeKey | Self::Offers | Self::Reachable => &[Kind::NODE_ANNOUNCE],
            // Announcing settles that the node is counting; closing settles when it stopped, and
            // announcing again afterwards does not reopen it.
            Self::Closed => &[Kind::NODE_ANNOUNCE, Kind::NODE_CLOSE],
        }
    }
}

/// Where a summary sits: the act carrying it, and the branch that act is on.
///
/// **The branch and not the object.** A node writes acts down as it hears of them, so two honest
/// nodes hold one chain in two orders — and a check that read arrival order would give two honest
/// nodes two answers about one summary. Following each act's own account of what it comes after is
/// the only order everybody agrees on, and where a chain has split it picks out one side instead of
/// interleaving both.
#[derive(Debug, Clone, Copy)]
pub struct Placed<'a> {
    /// The act that carries the summary. It claims the state **as of this act**, so what governs
    /// the state after it is later history and not something the summary hid.
    pub carrier: &'a Name,
    /// The branch that act sits on, oldest first.
    pub branch: &'a [&'a Entry],
}

/// What a claim turned out to be worth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing governing it was hidden, and the acts that govern it produce what it says.
    ///
    /// How much that is worth depends on which checks were run: [`left_out`] alone says only that
    /// nothing was hidden.
    Stands,
    /// It hides an act that governs this part of the state, which is that act.
    LeftOut(Name),
    /// The acts that govern it do not produce what it says.
    Fabricated,
    /// This summary claims nothing about a part of the state that has one.
    Missing,
    /// This build cannot say, and that act is why.
    ///
    /// An act of a kind it has never heard of — which might be exactly the one that governs this —
    /// or one the claim depends on that whoever is checking does not hold. **Never mistaken for
    /// standing**: serving a state from before an act nobody understood is the one thing a node may
    /// not do, and it is no better done from the reading side.
    CannotSay(Name),
}

/// One branch of an object's chain, oldest first.
///
/// Walked backwards from `head` along each act's own account of what it follows, so the order is
/// the one every reader arrives at rather than the one this node happened to hear things in.
#[must_use]
pub fn branch<'a>(entries: &[&'a Entry], head: &Name) -> Vec<&'a Entry> {
    let by_hash: BTreeMap<&Name, &'a Entry> =
        entries.iter().map(|entry| (&entry.hash, *entry)).collect();

    let mut walked = Vec::new();
    let mut at = Some(head.clone());
    // Bounded by what is held, so that entries someone arranged into a ring cannot spin here.
    while let Some(hash) = at.take() {
        if walked.len() > entries.len() {
            break;
        }
        let Some(entry) = by_hash.get(&hash) else {
            break;
        };
        walked.push(*entry);
        at = entry.previous.clone();
    }
    walked.reverse();
    walked
}

/// Whether a claim hides an act that governs it — answered from the log alone.
///
/// The free half of the check: it needs what every node holds and nothing that has to be fetched.
/// [`Verdict::Stands`] here means only that nothing was hidden, which is worth having and is not
/// the same as the value being true — for that, see [`produces`].
#[must_use]
pub fn left_out(claim: &Claim, placed: Placed<'_>) -> Verdict {
    let Some(window) = up_to(placed) else {
        return Verdict::CannotSay(placed.carrier.clone());
    };
    // Which object this is comes from the act that created it, which is the first entry of the
    // branch and the one thing every reader holds. A class with no parts is one whose summary
    // nobody can check, and a claim about a part the object does not have is the wrong question.
    let Some(class) = class_of(window) else {
        return Verdict::CannotSay(placed.carrier.clone());
    };
    if claim.about.class() != class {
        return Verdict::CannotSay(placed.carrier.clone());
    }
    // An act this build cannot apply, anywhere on the branch up to the summary, makes the whole
    // object opaque — admission stops resolving it and never starts again, so there is no state for
    // a summary to be right or wrong about. Caught across the whole window and not only after the
    // cited act, because opacity is not undone by whatever comes later.
    //
    // **By kind and by field, from the log alone.** An act whose payload carries a critical field
    // this build has no meaning for makes the object opaque exactly as an unknown kind does — see
    // `chain::holder_vocabulary` — and the entry lists the odd numbers its payload carries so that
    // this can be seen without fetching the act. Fetching a whole chain to find out would be paying
    // the cost a summary exists to avoid.
    if let Some(beyond) = window.iter().find(|entry| unreadable(entry, class)) {
        return Verdict::CannotSay(beyond.hash.clone());
    }
    let governs = claim.about.set_by();

    // Everything after the act it cites. Two ways of citing nothing, and they come to the same
    // thing: an act that is not on this branch at all, and one that is but does not govern this part
    // of the state. Neither ever set what the summary says it set, so everything that governs it is
    // later than nothing.
    let after = window
        .iter()
        .position(|entry| {
            entry.hash == claim.set_by
                && Kind::new(entry.kind).is_some_and(|kind| governs.contains(&kind))
        })
        .map_or(0, |at| at + 1);
    let later = &window[after.min(window.len())..];

    // A definite answer is preferred to *cannot say*: an act it hid is a fact about this summary,
    // while an act of a kind nobody can read is a fact about this build.
    match later
        .iter()
        .find(|entry| Kind::new(entry.kind).is_some_and(|kind| governs.contains(&kind)))
    {
        Some(hidden) => Verdict::LeftOut(hidden.hash.clone()),
        // Nothing hidden, and no kind on the branch this build cannot apply. **Which is all the
        // log can say**: it does not carry payloads, so *stands* here means *nothing in the log
        // contradicts it* and never *this account resolves*.
        None => Verdict::Stands,
    }
}

/// The acts a claim's value has to be held against.
///
/// **Only the ones that govern it**, which is what keeps checking a summary cheap: a chain is
/// mostly acts that say nothing about any one part of the state, and none of those is fetched.
#[must_use]
pub fn needs(claim: &Claim, placed: Placed<'_>) -> Vec<Name> {
    let governs = claim.about.set_by();
    up_to(placed)
        .unwrap_or_default()
        .iter()
        .filter(|entry| Kind::new(entry.kind).is_some_and(|kind| governs.contains(&kind)))
        .map(|entry| entry.hash.clone())
        .collect()
}

/// Whether the acts that govern this part of the state produce what the claim says.
///
/// `acts` are what [`needs`] asked for, oldest first, and `at` is the moment of the act carrying
/// the summary — a summary claims the state **as of itself**, and what the control key asked for
/// alone only counts once its wait has run out by then. **This is the half a hash cannot cover**:
/// a summary that cites the right act and states a value nothing ever produced passes every check
/// the log alone can make.
#[must_use]
pub fn produces(claim: &Claim, acts: &[&Operation], at: Epoch) -> Verdict {
    match claim.about.class() {
        Class::Holder => produces_on_a_holder(claim, acts, at),
        Class::Node => produces_on_a_node(claim, acts),
    }
}

/// [`produces`], for a part of an account.
fn produces_on_a_holder(claim: &Claim, acts: &[&Operation], at: Epoch) -> Verdict {
    let folded = match replayed(acts, at) {
        Ok(folded) => folded,
        Err(verdict) => return verdict,
    };
    match (&claim.about, &claim.stated) {
        (Governs::Control, Stated::Key(said)) if folded.control.as_ref() == Some(said) => {
            Verdict::Stands
        }
        (Governs::Devices, Stated::Keys(said)) if *said == folded.devices => Verdict::Stands,
        (Governs::Frozen, Stated::Flag(said)) if *said == folded.frozen => Verdict::Stands,
        // What is still waiting at the summary's own moment: everything asked and not yet due,
        // and not struck out. Names, compared as a set.
        (Governs::Pending, Stated::Names(said)) if *said == folded.pending() => Verdict::Stands,
        _ => Verdict::Fabricated,
    }
}

/// [`produces`], for a part of a node.
///
/// Nothing a node's chain says waits, so there is no moment to judge it at: what the announcements
/// and the closing say is what the node is.
fn produces_on_a_node(claim: &Claim, acts: &[&Operation]) -> Verdict {
    let folded = match replayed_node(acts) {
        Ok(folded) => folded,
        Err(verdict) => return verdict,
    };
    match (&claim.about, &claim.stated) {
        (Governs::NodeKey, Stated::Key(said)) if folded.key.as_ref() == Some(said) => {
            Verdict::Stands
        }
        (Governs::Offers, Stated::Numbers(said)) if *said == folded.offers => Verdict::Stands,
        (Governs::Reachable, Stated::Addresses(said)) if *said == folded.reachable => {
            Verdict::Stands
        }
        (Governs::Closed, Stated::Moment(said)) if *said == folded.closed => Verdict::Stands,
        _ => Verdict::Fabricated,
    }
}

/// Both halves, which is what a summary standing up actually means.
#[must_use]
pub fn holds_up(claim: &Claim, placed: Placed<'_>, acts: &[&Operation], at: Epoch) -> Verdict {
    match left_out(claim, placed) {
        Verdict::Stands => produces(claim, acts, at),
        other => other,
    }
}

/// Everything wrong with a summary, including what it did not say.
///
/// **All of them, not the first.** A summary that hid two things is a different thing from one that
/// hid one, and whoever is looking at it should see the whole of what is wrong with it.
///
/// `acts` is everything [`needs`] asked for across all the claims; anything missing from it turns
/// that claim into *cannot say* rather than into a pass.
#[must_use]
pub fn falls_over(
    claims: &[Claim],
    placed: Placed<'_>,
    acts: &[&Operation],
    at: Epoch,
) -> Vec<(Governs, Verdict)> {
    let mut wrong = Vec::new();
    // Measured against the parts **this** object has, read off the act that created it. A branch
    // that does not start with a creation this build knows the parts of is one whose summary
    // cannot be checked, and every claim on it is answered so rather than passed.
    let Some(expected) = up_to(placed).and_then(class_of).map(|class| match class {
        Class::Holder => &Governs::HOLDER[..],
        Class::Node => &Governs::NODE[..],
    }) else {
        return claims
            .iter()
            .map(|claim| (claim.about, Verdict::CannotSay(placed.carrier.clone())))
            .collect();
    };
    for &about in expected {
        let Some(claim) = claims.iter().find(|claim| claim.about == about) else {
            wrong.push((about, Verdict::Missing));
            continue;
        };
        let wanted = needs(claim, placed);
        let mut had = Vec::new();
        let mut absent = None;
        for hash in &wanted {
            match acts.iter().find(|act| act.called() == *hash) {
                Some(act) => had.push(*act),
                None => absent = Some(hash.clone()),
            }
        }
        let verdict = match absent {
            Some(hash) => match left_out(claim, placed) {
                Verdict::Stands => Verdict::CannotSay(hash),
                other => other,
            },
            None => holds_up(claim, placed, &had, at),
        };
        if verdict != Verdict::Stands {
            wrong.push((about, verdict));
        }
    }
    wrong
}

/// The value an operation carries at the summary field.
#[must_use]
pub fn declaration(claims: &[Claim]) -> Value {
    Value::Map(
        claims
            .iter()
            .map(|claim| {
                (
                    claim.about.number(),
                    Value::Array(vec![
                        stated(&claim.stated),
                        Value::Text(claim.set_by.as_str().to_owned()),
                    ]),
                )
            })
            .collect(),
    )
}

/// What an operation says about the state it leaves behind.
///
/// [`None`] when it says nothing, which most acts do. [`Unreadable`] when it claims a part of the
/// state this build has no meaning for — and then **none of it is used**, because reading three
/// claims out of four and calling the summary sound is the one wrong answer available here.
///
/// # Errors
///
/// [`Unreadable`], naming the part of the state that could not be read, or nothing when the field
/// is not shaped like a summary at all.
pub fn declared(operation: &Operation) -> Result<Option<Vec<Claim>>, Unreadable> {
    let Some(value) = operation.payload.get(&FIELD) else {
        return Ok(None);
    };
    let Value::Map(parts) = value else {
        return Err(Unreadable(None));
    };

    // **A summary that claims nothing is not a smaller summary.** Left as one, an empty field would
    // discharge the debt an object owes for ever while telling whoever arrives absolutely nothing —
    // which is worse than no field, because no field is honest about it.
    if parts.is_empty() {
        return Err(Unreadable(None));
    }

    let mut claims = Vec::new();
    for (&number, part) in parts {
        let about = Governs::new(number).ok_or(Unreadable(Some(number)))?;
        let Value::Array(pair) = part else {
            return Err(Unreadable(Some(number)));
        };
        let [what, Value::Text(set_by)] = pair.as_slice() else {
            return Err(Unreadable(Some(number)));
        };
        claims.push(Claim {
            about,
            stated: read_stated(what, about).ok_or(Unreadable(Some(number)))?,
            set_by: Name::parse(set_by).map_err(|_| Unreadable(Some(number)))?,
        });
    }
    Ok(Some(claims))
}

/// A summary this build cannot read, and which part of the state stopped it.
///
/// [`None`] when the field is not shaped like a summary at all. It is not a reason to refuse the
/// act that carried it: the field may be ignored, and an object whose summary cannot be read still
/// resolves by replaying its chain, which is what the summary was saving and not what it replaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unreadable(pub Option<u64>);

/// The branch up to and including the act carrying the summary.
fn up_to<'a>(placed: Placed<'a>) -> Option<&'a [&'a Entry]> {
    let at = placed
        .branch
        .iter()
        .position(|entry| entry.hash == *placed.carrier)?;
    placed.branch.get(..=at)
}

/// Which object a branch belongs to, from the act that created it.
fn class_of(window: &[&Entry]) -> Option<Class> {
    match Kind::new(window.first()?.kind)? {
        Kind::HOLDER_CREATE => Some(Class::Holder),
        Kind::NODE_ANNOUNCE => Some(Class::Node),
        _ => None,
    }
}

/// Whether this build cannot work the act an entry places into that object's state.
///
/// **Not the same as an unknown number.** A kind can be in this build's vocabulary and still have
/// no logic that applies it — recovery and setting guardians are named but not yet built, and
/// admission turns an account carrying one **opaque**: it stops resolving it, because a state it
/// could not compute is one it must not serve. The reading side has to draw the same line, or it
/// would vouch for a state the writing side gave up on. So this is the object's appliable kinds as
/// a closed list, and anything else — a newer number, a named-but-unbuilt kind, a kind that belongs
/// to some other class of object — makes the object uncheckable here exactly as it makes it
/// unresolvable there.
///
/// **And the fields, by the same rule.** Admission refuses to apply an act carrying a critical
/// field it has no meaning for, and the entry lists the odd numbers the payload carries, so a
/// number outside this build's vocabulary for that class is an act it could not have applied. The
/// vocabulary is the one admission uses, or the two sides would draw two lines: an account's acts
/// are all read with the account's vocabulary, and a node's announcement with the announcement's —
/// its other acts are applied without one being asked of them, so none is asked here either.
fn unreadable(entry: &Entry, class: Class) -> bool {
    let vocabulary = match (class, Kind::new(entry.kind)) {
        (
            Class::Holder,
            Some(
                Kind::HOLDER_CREATE
                | Kind::HOLDER_ADD_DEVICE
                | Kind::HOLDER_REMOVE_DEVICE
                | Kind::HOLDER_ROTATE
                | Kind::HOLDER_FREEZE
                | Kind::HOLDER_UNFREEZE
                | Kind::HOLDER_CANCEL
                | Kind::HOLDER_CHECKPOINT,
            ),
        ) => Some(crate::chain::holder_vocabulary()),
        (Class::Node, Some(Kind::NODE_ANNOUNCE)) => Some(crate::capability::vocabulary()),
        (
            Class::Node,
            Some(Kind::NODE_BIND | Kind::NODE_UNBIND | Kind::NODE_SUMMARY | Kind::NODE_CLOSE),
        ) => None,
        _ => return true,
    };
    vocabulary.is_some_and(|vocabulary| {
        entry.critical.iter().any(|number| {
            !vocabulary
                .fields
                .contains(&almena_format::field::Field::new(*number))
        })
    })
}

/// The key an act names, which is the one field the basic acts of an account carry.
fn named(act: &Operation) -> Option<Vec<u8>> {
    match act.payload.get(&1) {
        Some(Value::Bytes(key)) => Some(key.clone()),
        _ => None,
    }
}

/// The account the governing acts produce, replayed as everybody replays them.
struct Replayed {
    /// The key that governs it, if the acts establish one.
    control: Option<Vec<u8>>,
    /// The keys that operate it.
    devices: BTreeSet<Vec<u8>>,
    /// Whether it is stopped.
    frozen: bool,
    /// What the control key has asked for that has not yet landed: which act asked, what it
    /// changes, and from when it is in force.
    waiting: Vec<(Name, Asked, Epoch)>,
}

/// What one waiting asking will change when it lands, as the replay needs to know it.
enum Asked {
    /// A device key joins the set.
    Add(Vec<u8>),
    /// One leaves it.
    Remove(Vec<u8>),
    /// The control key is replaced.
    Rotate(Vec<u8>),
    /// The account starts moving again.
    Thaw,
}

impl Replayed {
    /// Land everything due by that moment, in the order it was asked.
    fn come_due(&mut self, at: Epoch) {
        let due: Vec<(Name, Asked, Epoch)> = self
            .waiting
            .extract_if(.., |(_, _, due)| due.number() <= at.number())
            .collect();
        for (_, asked, _) in due {
            match asked {
                Asked::Add(key) => {
                    self.devices.insert(key);
                }
                Asked::Remove(key) => {
                    self.devices.remove(&key);
                }
                Asked::Rotate(key) => self.control = Some(key),
                Asked::Thaw => self.frozen = false,
            }
        }
    }

    /// What is still waiting, by the act that asked.
    fn pending(&self) -> BTreeSet<Name> {
        self.waiting
            .iter()
            .map(|(name, _, _)| name.clone())
            .collect()
    }
}

/// The node the governing acts produce, replayed as everybody replays them.
struct NodeReplayed {
    /// The key it signs with, fixed by the announcement that named it.
    key: Option<Vec<u8>>,
    /// What it says it is running, as the numbers the closed list gives them.
    offers: BTreeSet<u64>,
    /// Where it says it can be reached.
    reachable: BTreeSet<String>,
    /// When it stopped counting, once it has.
    closed: Option<Epoch>,
}

/// The acts that govern a node's parts, replayed.
///
/// **The same reading admission gives them.** An announcement is read with the announcement's
/// vocabulary and refused for a capability the list does not name; every announcement after the
/// first says again what the node offers and where it is; closing is said once, at the epoch it
/// was first said, and announcing again afterwards reopens nothing.
fn replayed_node(acts: &[&Operation]) -> Result<NodeReplayed, Verdict> {
    let mut folded = NodeReplayed {
        key: None,
        offers: BTreeSet::new(),
        reachable: BTreeSet::new(),
        closed: None,
    };
    for act in acts {
        match Kind::new(act.kind) {
            Some(Kind::NODE_ANNOUNCE) => {
                if act.understood(crate::capability::vocabulary()).is_err() {
                    return Err(Verdict::CannotSay(act.called()));
                }
                if folded.key.is_none() {
                    folded.key = match act.payload.get(&1) {
                        Some(Value::Bytes(key)) if key.len() == 32 => Some(key.clone()),
                        _ => return Err(Verdict::CannotSay(act.called())),
                    };
                }
                folded.offers = match act.payload.get(&crate::capability::OFFERS) {
                    None => BTreeSet::new(),
                    Some(Value::Array(said)) => said
                        .iter()
                        .map(|one| match one {
                            Value::Uint(number) => Some(*number),
                            _ => None,
                        })
                        .collect::<Option<_>>()
                        .ok_or_else(|| Verdict::CannotSay(act.called()))?,
                    Some(_) => return Err(Verdict::CannotSay(act.called())),
                };
                folded.reachable = match act.payload.get(&crate::capability::WHERE) {
                    None => BTreeSet::new(),
                    Some(Value::Array(said)) => said
                        .iter()
                        .map(|one| match one {
                            Value::Text(address) => Some(address.clone()),
                            _ => None,
                        })
                        .collect::<Option<_>>()
                        .ok_or_else(|| Verdict::CannotSay(act.called()))?,
                    Some(_) => return Err(Verdict::CannotSay(act.called())),
                };
            }
            Some(Kind::NODE_CLOSE) => folded.closed = Some(folded.closed.unwrap_or(act.issued)),
            _ => {}
        }
    }
    Ok(folded)
}

/// The governing acts replayed up to a moment, with the wait the control key pays.
///
/// **The same rule the record applies, applied again by whoever checks.** What the control key
/// signs alone enters at once and lands when its wait runs out, unless a device cancelled it
/// first — so what the devices *are* at any moment depends on when the asking happened, who
/// signed it, and what was struck out. A checker that replayed the acts as if everything landed
/// at once would call an honest summary a lie and a lie honest, each exactly once per wait.
fn replayed(acts: &[&Operation], at: Epoch) -> Result<Replayed, Verdict> {
    let mut folded = Replayed {
        control: None,
        devices: BTreeSet::new(),
        frozen: false,
        waiting: Vec::new(),
    };
    for act in acts {
        folded.come_due(act.issued);
        folded.takes(act)?;
    }
    folded.come_due(at);
    Ok(folded)
}

impl Replayed {
    /// Work one act in, or say why this build cannot.
    ///
    /// # Errors
    ///
    /// [`Verdict::CannotSay`], naming the act, for anything the replay cannot follow.
    fn takes(&mut self, act: &Operation) -> Result<(), Verdict> {
        // **What this build cannot claim to have applied, it does not vouch for** (`SPECS.md §4.8`,
        // rule 4). The chain's own admission stops at an act carrying a critical field it has no
        // meaning for, and a verifier that walked past one would be the third reader of the same
        // act reaching a third answer — the store declaring the object opaque while this signs off
        // on the state it computed without it.
        if act.understood(crate::chain::holder_vocabulary()).is_err() {
            return Err(Verdict::CannotSay(act.called()));
        }

        // Whether this was the words asking — and so whether its effect waited — is judged
        // against the control key of its own moment, never against the key's shape.
        let by_control = act
            .signatures
            .first()
            .is_some_and(|signature| Some(&signature.key) == self.control.as_ref());
        match Kind::new(act.kind) {
            // Only ever the first act on a branch. One anywhere else is an act the record would
            // have refused, so a branch carrying it is a branch no node built — and taking it as
            // an account being reset would be vouching for a state that exists nowhere.
            Some(Kind::HOLDER_CREATE) if self.control.is_some() => {
                return Err(Verdict::CannotSay(act.called()));
            }
            Some(Kind::HOLDER_CREATE) => {
                self.control = named(act);
                self.devices.clear();
                if self.control.is_none() {
                    return Err(Verdict::CannotSay(act.called()));
                }
            }
            Some(kind @ (Kind::HOLDER_ADD_DEVICE | Kind::HOLDER_REMOVE_DEVICE)) => {
                self.device_moves(act, kind, by_control)?;
            }
            // Only the control key rotates the control key, so a rotation always waited.
            Some(Kind::HOLDER_ROTATE) => match named(act) {
                Some(key) => self
                    .waiting
                    .push((act.called(), Asked::Rotate(key), due_from(act)?)),
                None => return Err(Verdict::CannotSay(act.called())),
            },
            Some(Kind::HOLDER_CANCEL) => self.struck_out(act)?,
            // Freezing lands at once whoever signed it: it denies, and there is nothing in a
            // denial worth stealing a wait for.
            Some(Kind::HOLDER_FREEZE) => self.frozen = true,
            // Thawing concedes back everything the freeze stopped, so it is the words asking and
            // it waits. Nobody else's thaw is ever admitted, so nobody else's is replayed.
            Some(Kind::HOLDER_UNFREEZE) if by_control => {
                self.waiting
                    .push((act.called(), Asked::Thaw, due_from(act)?));
            }
            // It empties the set and enrols the device that asked, and this build cannot say
            // which. Naming guardians is the same case: a kind this build numbers and does not
            // apply, so nothing about the queue after it can be vouched for.
            Some(Kind::HOLDER_RECOVER | Kind::HOLDER_SET_GUARDIANS) => {
                return Err(Verdict::CannotSay(act.called()));
            }
            _ => {}
        }
        Ok(())
    }
}

impl Replayed {
    /// A device joining or leaving: at once by a device's hand, after the wait by the words'.
    ///
    /// # Errors
    ///
    /// [`Verdict::CannotSay`] for an act that names no key.
    fn device_moves(
        &mut self,
        act: &Operation,
        kind: Kind,
        by_control: bool,
    ) -> Result<(), Verdict> {
        let key = named(act).ok_or_else(|| Verdict::CannotSay(act.called()))?;
        let joins = kind == Kind::HOLDER_ADD_DEVICE;
        if by_control {
            let asked = if joins {
                Asked::Add(key)
            } else {
                Asked::Remove(key)
            };
            self.waiting.push((act.called(), asked, due_from(act)?));
        } else if joins {
            self.devices.insert(key);
        } else {
            self.devices.remove(&key);
        }
        Ok(())
    }

    /// A device saying no: the named asking, struck out of what is waiting.
    ///
    /// **Struck only as admission would strike it**, or the reading side would credit a
    /// cancellation the writing side refused, and two honest nodes would compute different
    /// accounts from one chain. So the same two rules hold here: the words never cancel — a
    /// cancellation is the counterweight the *devices* hold against them — and a device may not
    /// strike the very asking that removes it. A cancellation the chain would not have taken
    /// leaves the waiting list untouched, exactly as if it had never been written.
    ///
    /// One that names nothing here struck out an asking this replay was not handed — one that
    /// governs no part of the claim being checked — and leaves the list as it found it.
    ///
    /// # Errors
    ///
    /// [`Verdict::CannotSay`] for a cancellation whose naming this build cannot read.
    fn struck_out(&mut self, act: &Operation) -> Result<(), Verdict> {
        let Some(Value::Text(struck)) = act.payload.get(&1) else {
            return Err(Verdict::CannotSay(act.called()));
        };
        let Ok(struck) = Name::parse(struck) else {
            return Err(Verdict::CannotSay(act.called()));
        };
        let signer = act.signatures.first().map(|signature| &signature.key);
        // The words never cancel: in their own hands the counterweight weighs nothing.
        if signer == self.control.as_ref() {
            return Ok(());
        }
        // A device may not strike the asking that removes it — else a stolen device would veto its
        // own expulsion for ever, on the reading side as much as the writing one.
        let self_removal = self
            .waiting
            .iter()
            .find(|(name, _, _)| *name == struck)
            .is_some_and(|(_, asked, _)| match asked {
                Asked::Remove(key) => Some(key) == signer,
                _ => false,
            });
        if self_removal {
            return Ok(());
        }
        self.waiting.retain(|(name, _, _)| *name != struck);
        Ok(())
    }
}

/// The first epoch an asking is in force, under the wait of its own moment.
fn due_from(act: &Operation) -> Result<Epoch, Verdict> {
    act.issued
        .plus(almena_time::Epochs(
            almena_time::deadline::CONTROL_KEY_WAIT.at(act.issued),
        ))
        .ok_or_else(|| Verdict::CannotSay(act.called()))
}

/// What a stated value looks like on the wire.
///
/// A flag is a number and not a CBOR boolean, because the profile every act is written in has no
/// boolean; a moment nobody has reached is null, as a first act's `prev` is.
fn stated(what: &Stated) -> Value {
    match what {
        Stated::Key(key) => Value::Bytes(key.clone()),
        Stated::Keys(keys) => Value::Array(keys.iter().cloned().map(Value::Bytes).collect()),
        Stated::Flag(flag) => Value::Uint(u64::from(*flag)),
        Stated::Names(names) => Value::Array(
            names
                .iter()
                .map(|name| Value::Text(name.as_str().to_owned()))
                .collect(),
        ),
        Stated::Numbers(numbers) => {
            Value::Array(numbers.iter().copied().map(Value::Uint).collect())
        }
        Stated::Addresses(addresses) => {
            Value::Array(addresses.iter().cloned().map(Value::Text).collect())
        }
        Stated::Moment(moment) => moment.map_or(Value::Null, |epoch| Value::Uint(epoch.number())),
    }
}

/// A stated value read back, in the shape that part of the state has.
///
/// The shape comes from which part of the state it is, never from what arrived: taking a list where
/// a key belongs because a list turned up would be letting the sender pick how it is read.
fn read_stated(value: &Value, about: Governs) -> Option<Stated> {
    match (about, value) {
        (Governs::Control | Governs::NodeKey, Value::Bytes(key)) => Some(Stated::Key(key.clone())),
        (Governs::Devices, Value::Array(keys)) => keys
            .iter()
            .map(|key| match key {
                Value::Bytes(key) => Some(key.clone()),
                _ => None,
            })
            .collect::<Option<BTreeSet<Vec<u8>>>>()
            .map(Stated::Keys),
        (Governs::Frozen, Value::Uint(0)) => Some(Stated::Flag(false)),
        (Governs::Frozen, Value::Uint(1)) => Some(Stated::Flag(true)),
        (Governs::Pending, Value::Array(names)) => names
            .iter()
            .map(|name| match name {
                Value::Text(name) => Name::parse(name).ok(),
                _ => None,
            })
            .collect::<Option<BTreeSet<Name>>>()
            .map(Stated::Names),
        (Governs::Offers, Value::Array(numbers)) => numbers
            .iter()
            .map(|number| match number {
                Value::Uint(number) => Some(*number),
                _ => None,
            })
            .collect::<Option<BTreeSet<u64>>>()
            .map(Stated::Numbers),
        (Governs::Reachable, Value::Array(addresses)) => addresses
            .iter()
            .map(|address| match address {
                Value::Text(address) => Some(address.clone()),
                _ => None,
            })
            .collect::<Option<BTreeSet<String>>>()
            .map(Stated::Addresses),
        (Governs::Closed, Value::Null) => Some(Stated::Moment(None)),
        (Governs::Closed, Value::Uint(epoch)) => Some(Stated::Moment(Some(Epoch::new(*epoch)))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Claim, Governs, Placed, Stated, Verdict, branch, declaration, declared, falls_over,
        holds_up, left_out, needs, produces,
    };
    use almena_format::cbor::Value;
    use almena_format::entry::Entry;
    use almena_format::identifier::{Name, Network};
    use almena_format::operation::{Operation, create};
    use almena_time::Epoch;
    use std::collections::{BTreeMap, BTreeSet};

    /// An account's chain: what happened, in the order it happened.
    struct Chain {
        acts: Vec<Operation>,
        entries: Vec<Entry>,
    }

    impl Chain {
        /// A new account, with nothing on it.
        fn new() -> Self {
            let creation = create(
                Network::Development,
                crate::kind::Kind::HOLDER_CREATE.number(),
                1,
                Epoch::GENESIS,
                BTreeMap::from([(1, Value::Bytes(vec![1; 32]))]),
            );
            let entry = Entry::of(&creation, 0, None);
            Self {
                acts: vec![creation],
                entries: vec![entry],
            }
        }

        /// One more act on it, following the last.
        fn then(&mut self, kind: crate::kind::Kind, key: &[u8]) -> &mut Self {
            let at = self.entries.len() as u64;
            let act = Operation {
                object: self.acts[0].object.clone(),
                previous: Some(self.entries[self.entries.len() - 1].hash.clone()),
                kind: kind.number(),
                version: 1,
                issued: Epoch::GENESIS,
                payload: BTreeMap::from([(1, Value::Bytes(key.to_vec()))]),
                signatures: Vec::new(),
            };
            self.entries.push(Entry::of(&act, at, None));
            self.acts.push(act);
            self
        }

        /// One more act, at a chosen moment, wearing a signature that names its key.
        ///
        /// The signature is dress rather than proof — nothing here verifies it, because
        /// admission already did — but **whose key it names decides whether the act was the
        /// words asking**, and that is what the replay classifies by.
        fn then_by(
            &mut self,
            kind: crate::kind::Kind,
            payload: Value,
            by: &[u8],
            at: Epoch,
        ) -> &mut Self {
            let position = self.entries.len() as u64;
            let act = Operation {
                object: self.acts[0].object.clone(),
                previous: Some(self.entries[self.entries.len() - 1].hash.clone()),
                kind: kind.number(),
                version: 1,
                issued: at,
                payload: BTreeMap::from([(1, payload)]),
                signatures: vec![almena_format::operation::Signed {
                    by: self.acts[0].object.clone(),
                    key: by.to_vec(),
                    signature: [0; 64],
                }],
            };
            self.entries.push(Entry::of(&act, position, None));
            self.acts.push(act);
            self
        }

        /// A node's chain: the announcement that names it, carrying its key.
        fn node() -> Self {
            let announced = create(
                Network::Development,
                crate::kind::Kind::NODE_ANNOUNCE.number(),
                1,
                Epoch::GENESIS,
                BTreeMap::from([(1, Value::Bytes(vec![5; 32]))]),
            );
            let entry = Entry::of(&announced, 0, None);
            Self {
                acts: vec![announced],
                entries: vec![entry],
            }
        }

        /// One more act, carrying whatever payload it is given, signed by that key at that moment.
        fn then_carrying(
            &mut self,
            kind: crate::kind::Kind,
            payload: BTreeMap<u64, Value>,
            by: &[u8],
            at: Epoch,
        ) -> &mut Self {
            let position = self.entries.len() as u64;
            let act = Operation {
                object: self.acts[0].object.clone(),
                previous: Some(self.entries[self.entries.len() - 1].hash.clone()),
                kind: kind.number(),
                version: 1,
                issued: at,
                payload,
                signatures: vec![almena_format::operation::Signed {
                    by: self.acts[0].object.clone(),
                    key: by.to_vec(),
                    signature: [0; 64],
                }],
            };
            self.entries.push(Entry::of(&act, position, None));
            self.acts.push(act);
            self
        }

        fn held(&self) -> Vec<&Entry> {
            self.entries.iter().collect()
        }

        fn hash(&self, at: usize) -> Name {
            self.entries[at].hash.clone()
        }

        fn head(&self) -> Name {
            self.hash(self.entries.len() - 1)
        }

        /// The acts a claim needs, fetched as whoever is checking would fetch them.
        fn acts_for(&self, claim: &Claim, carrier: &Name, branch: &[&Entry]) -> Vec<&Operation> {
            let wanted = needs(claim, Placed { carrier, branch });
            self.acts
                .iter()
                .filter(|act| wanted.contains(&act.called()))
                .collect()
        }
    }

    fn devices(keys: &[&[u8]]) -> Stated {
        Stated::Keys(keys.iter().map(|key| key.to_vec()).collect::<BTreeSet<_>>())
    }

    #[test]
    fn a_summary_that_hides_nothing_and_says_what_happened_stands() {
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);

        let held = chain.held();
        let walked = branch(&held, &chain.head());
        let carrier = chain.head();
        let claim = Claim {
            about: Governs::Devices,
            stated: devices(&[&[7; 33]]),
            set_by: chain.hash(1),
        };
        let acts = chain.acts_for(&claim, &carrier, &walked);

        assert_eq!(
            holds_up(
                &claim,
                Placed {
                    carrier: &carrier,
                    branch: &walked
                },
                &acts,
                Epoch::GENESIS
            ),
            Verdict::Stands
        );
    }

    #[test]
    fn a_summary_that_hides_a_later_act_falls_over() {
        // **The lie the log alone catches.** A routine signature would otherwise make claims about
        // governance to everybody arriving afterwards.
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[8; 33]);

        let held = chain.held();
        let walked = branch(&held, &chain.head());
        let carrier = chain.head();

        assert_eq!(
            left_out(
                &Claim {
                    about: Governs::Devices,
                    stated: devices(&[&[7; 33]]),
                    set_by: chain.hash(1),
                },
                Placed {
                    carrier: &carrier,
                    branch: &walked
                }
            ),
            Verdict::LeftOut(chain.hash(2)),
            "and it says which act it left out"
        );
    }

    #[test]
    fn a_summary_that_makes_a_value_up_falls_over_even_though_it_hides_nothing() {
        // **The lie a hash cannot catch, and the reason the value has to be held against the acts.**
        // The cited act is the right one, nothing governing came after it, and the value is one
        // nothing ever produced: a device nobody added, and the real one gone.
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);

        let held = chain.held();
        let walked = branch(&held, &chain.head());
        let carrier = chain.head();
        let lying = Claim {
            about: Governs::Devices,
            stated: devices(&[&[99; 33]]),
            set_by: chain.hash(1),
        };
        let placed = Placed {
            carrier: &carrier,
            branch: &walked,
        };

        assert_eq!(
            left_out(&lying, placed),
            Verdict::Stands,
            "the log alone has nothing to say against it"
        );
        assert_eq!(
            produces(
                &lying,
                &chain.acts_for(&lying, &carrier, &walked),
                Epoch::GENESIS
            ),
            Verdict::Fabricated,
            "and the acts that govern it settle it"
        );
    }

    #[test]
    fn a_summary_that_quietly_drops_a_device_falls_over() {
        // The same lie by omission of a value rather than of an act: both devices were added, one
        // was never removed, and the summary leaves it out. Nothing governing came later.
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[8; 33]);

        let held = chain.held();
        let walked = branch(&held, &chain.head());
        let carrier = chain.head();
        let claim = Claim {
            about: Governs::Devices,
            stated: devices(&[&[8; 33]]),
            set_by: chain.hash(2),
        };

        assert_eq!(
            left_out(
                &claim,
                Placed {
                    carrier: &carrier,
                    branch: &walked
                }
            ),
            Verdict::Stands
        );
        assert_eq!(
            produces(
                &claim,
                &chain.acts_for(&claim, &carrier, &walked),
                Epoch::GENESIS
            ),
            Verdict::Fabricated
        );
    }

    #[test]
    fn a_device_that_was_removed_is_not_one_a_summary_may_claim() {
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[8; 33]);
        chain.then(crate::kind::Kind::HOLDER_REMOVE_DEVICE, &[7; 33]);

        let held = chain.held();
        let walked = branch(&held, &chain.head());
        let carrier = chain.head();
        let claim = Claim {
            about: Governs::Devices,
            stated: devices(&[&[7; 33], &[8; 33]]),
            set_by: chain.hash(3),
        };

        assert_eq!(
            produces(
                &claim,
                &chain.acts_for(&claim, &carrier, &walked),
                Epoch::GENESIS
            ),
            Verdict::Fabricated,
            "the acts say it left, whatever the summary says"
        );
    }

    #[test]
    fn what_governs_something_else_does_not_make_a_summary_fall_over() {
        // A device added after the control key was set says nothing about the control key. Reading
        // it as though it did would make honest summaries fall over.
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);

        let held = chain.held();
        let walked = branch(&held, &chain.head());
        let carrier = chain.head();
        let claim = Claim {
            about: Governs::Control,
            stated: Stated::Key(vec![1; 32]),
            set_by: chain.hash(0),
        };

        assert_eq!(
            holds_up(
                &claim,
                Placed {
                    carrier: &carrier,
                    branch: &walked
                },
                &chain.acts_for(&claim, &carrier, &walked),
                Epoch::GENESIS
            ),
            Verdict::Stands
        );
    }

    #[test]
    fn rotating_makes_a_claim_about_the_control_key_fall_over() {
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ROTATE, &[2; 32]);

        let held = chain.held();
        let walked = branch(&held, &chain.head());
        let carrier = chain.head();

        assert_eq!(
            left_out(
                &Claim {
                    about: Governs::Control,
                    stated: Stated::Key(vec![1; 32]),
                    set_by: chain.hash(0),
                },
                Placed {
                    carrier: &carrier,
                    branch: &walked
                }
            ),
            Verdict::LeftOut(chain.hash(1))
        );
    }

    #[test]
    fn an_act_this_build_cannot_read_is_answered_with_cannot_say() {
        // **Never mistaken for standing.** An act nobody can read might be exactly the one that
        // governs what is being claimed, and it is the case an attacker can arrange.
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);
        let newer = crate::kind::Kind::new(9_999).expect("not zero");
        chain.then(newer, &[3; 33]);

        let held = chain.held();
        let walked = branch(&held, &chain.head());
        let carrier = chain.head();

        assert_eq!(
            left_out(
                &Claim {
                    about: Governs::Devices,
                    stated: devices(&[&[7; 33]]),
                    set_by: chain.hash(1),
                },
                Placed {
                    carrier: &carrier,
                    branch: &walked
                }
            ),
            Verdict::CannotSay(chain.hash(2))
        );
    }

    #[test]
    fn what_came_after_the_summary_is_later_history_and_not_something_it_hid() {
        // A summary claims the state as of the act that carries it. An act after that one is what
        // whoever arrives applies on top — reading it as concealment would make every summary on a
        // living object fall over the moment it did anything else.
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);
        let carrier = chain.head();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[8; 33]);

        let held = chain.held();
        let walked = branch(&held, &chain.head());

        assert_eq!(
            left_out(
                &Claim {
                    about: Governs::Devices,
                    stated: devices(&[&[7; 33]]),
                    set_by: chain.hash(1),
                },
                Placed {
                    carrier: &carrier,
                    branch: &walked
                }
            ),
            Verdict::Stands
        );
    }

    #[test]
    fn a_claim_citing_something_not_on_this_branch_has_cited_nothing() {
        // Otherwise a summary could point at an act nobody can find and be treated as current.
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);

        let held = chain.held();
        let walked = branch(&held, &chain.head());
        let carrier = chain.head();

        assert_eq!(
            left_out(
                &Claim {
                    about: Governs::Devices,
                    stated: devices(&[&[7; 33]]),
                    set_by: Name::of(b"an act of somebody else's"),
                },
                Placed {
                    carrier: &carrier,
                    branch: &walked
                }
            ),
            Verdict::LeftOut(chain.hash(0)),
            "everything that governs it is later than nothing"
        );
    }

    #[test]
    fn a_summary_that_says_nothing_about_a_part_of_the_state_falls_over() {
        // Completeness is measured against what the state has, not against what the summary chose
        // to mention: silence about the control key would be read as *unchanged* by everybody.
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);

        let held = chain.held();
        let walked = branch(&held, &chain.head());
        let carrier = chain.head();
        let only_devices = [Claim {
            about: Governs::Devices,
            stated: devices(&[&[7; 33]]),
            set_by: chain.hash(1),
        }];
        let acts: Vec<&Operation> = chain.acts.iter().collect();

        let fell = falls_over(
            &only_devices,
            Placed {
                carrier: &carrier,
                branch: &walked,
            },
            &acts,
            Epoch::GENESIS,
        );
        assert_eq!(
            fell,
            vec![
                (Governs::Control, Verdict::Missing),
                (Governs::Frozen, Verdict::Missing),
                (Governs::Pending, Verdict::Missing)
            ]
        );
    }

    #[test]
    fn a_summary_that_accounts_for_everything_does_not_fall_over() {
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);

        let held = chain.held();
        let walked = branch(&held, &chain.head());
        let carrier = chain.head();
        let whole = [
            Claim {
                about: Governs::Control,
                stated: Stated::Key(vec![1; 32]),
                set_by: chain.hash(0),
            },
            Claim {
                about: Governs::Devices,
                stated: devices(&[&[7; 33]]),
                set_by: chain.hash(1),
            },
            Claim {
                about: Governs::Frozen,
                stated: Stated::Flag(false),
                set_by: chain.hash(0),
            },
            Claim {
                about: Governs::Pending,
                stated: Stated::Names(BTreeSet::new()),
                set_by: chain.hash(1),
            },
        ];
        let acts: Vec<&Operation> = chain.acts.iter().collect();

        assert!(
            falls_over(
                &whole,
                Placed {
                    carrier: &carrier,
                    branch: &walked
                },
                &acts,
                Epoch::GENESIS
            )
            .is_empty(),
            "citing the latest act that governs each part, and saying what it produced"
        );
    }

    #[test]
    fn an_act_the_checker_does_not_hold_is_cannot_say_rather_than_a_pass() {
        // History is spread and its availability is measured rather than promised. Not having an
        // act is a thing to say, never a reason to conclude the summary is sound.
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);

        let held = chain.held();
        let walked = branch(&held, &chain.head());
        let carrier = chain.head();
        let whole = [
            Claim {
                about: Governs::Control,
                stated: Stated::Key(vec![1; 32]),
                set_by: chain.hash(0),
            },
            Claim {
                about: Governs::Devices,
                stated: devices(&[&[7; 33]]),
                set_by: chain.hash(1),
            },
            Claim {
                about: Governs::Frozen,
                stated: Stated::Flag(false),
                set_by: chain.hash(0),
            },
            Claim {
                about: Governs::Pending,
                stated: Stated::Names(BTreeSet::new()),
                set_by: chain.hash(1),
            },
        ];

        let fell = falls_over(
            &whole,
            Placed {
                carrier: &carrier,
                branch: &walked,
            },
            &[],
            Epoch::GENESIS,
        );
        assert_eq!(fell.len(), 4, "{fell:?}");
        assert!(
            fell.iter()
                .all(|(_, verdict)| matches!(verdict, Verdict::CannotSay(_)))
        );
    }

    #[test]
    fn the_order_a_node_heard_acts_in_changes_nothing() {
        // **Two honest nodes must give one answer.** A node writes acts down as it hears of them,
        // so reading arrival order would let the same summary stand on one node and fall on another.
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[8; 33]);

        let head = chain.head();
        let forwards = branch(&chain.held(), &head);
        let mut shuffled = chain.held();
        shuffled.reverse();
        let backwards = branch(&shuffled, &head);

        assert_eq!(forwards, backwards);
    }

    #[test]
    fn a_summary_survives_the_wire() {
        let claims = vec![
            Claim {
                about: Governs::Control,
                stated: Stated::Key(vec![1; 32]),
                set_by: Name::of(b"the act that set it"),
            },
            Claim {
                about: Governs::Devices,
                stated: devices(&[&[7; 33], &[8; 33]]),
                set_by: Name::of(b"the act that added the second"),
            },
        ];
        let mut carrier = create(
            Network::Development,
            crate::kind::Kind::HOLDER_ADD_DEVICE.number(),
            1,
            Epoch::GENESIS,
            BTreeMap::from([(1, Value::Bytes(vec![8; 33]))]),
        );
        carrier.payload.insert(super::FIELD, declaration(&claims));

        assert_eq!(declared(&carrier), Ok(Some(claims)));
    }

    #[test]
    fn most_acts_say_nothing_about_the_state_and_that_is_not_an_error() {
        let plain = create(
            Network::Development,
            crate::kind::Kind::HOLDER_ADD_DEVICE.number(),
            1,
            Epoch::GENESIS,
            BTreeMap::from([(1, Value::Bytes(vec![8; 33]))]),
        );
        assert_eq!(declared(&plain), Ok(None));
    }

    #[test]
    fn a_summary_naming_a_part_of_the_state_this_build_does_not_know_is_used_for_nothing() {
        // All or nothing. Reading three claims out of four and calling the summary sound is the one
        // wrong answer available here.
        let mut carrier = create(
            Network::Development,
            crate::kind::Kind::HOLDER_ADD_DEVICE.number(),
            1,
            Epoch::GENESIS,
            BTreeMap::from([(1, Value::Bytes(vec![8; 33]))]),
        );
        carrier.payload.insert(
            super::FIELD,
            Value::Map(BTreeMap::from([(
                77,
                Value::Array(vec![
                    Value::Bytes(vec![1; 32]),
                    Value::Text(Name::of(b"whatever").as_str().to_owned()),
                ]),
            )])),
        );

        assert_eq!(declared(&carrier), Err(super::Unreadable(Some(77))));
    }

    #[test]
    fn the_field_a_summary_travels_in_may_be_ignored() {
        // Critical would mean *you cannot claim to have applied this act without reading it*, and
        // that is false: a reader that skips it replays the chain and lands in the same state.
        let field = almena_format::field::Field::new(super::FIELD);
        assert!(!field.is_critical());
        assert!(
            field.is_common(),
            "and it means the same thing whatever act carries it"
        );
    }

    #[test]
    fn a_summary_that_claims_nothing_is_not_a_summary() {
        // Left as one, it would discharge the debt an object owes for ever while telling whoever
        // arrives absolutely nothing — worse than no field, because no field is honest about it.
        let mut carrier = create(
            Network::Development,
            crate::kind::Kind::HOLDER_CHECKPOINT.number(),
            1,
            Epoch::GENESIS,
            BTreeMap::new(),
        );
        carrier
            .payload
            .insert(super::FIELD, Value::Map(BTreeMap::new()));

        assert_eq!(declared(&carrier), Err(super::Unreadable(None)));
    }

    #[test]
    fn citing_an_act_that_governs_nothing_is_citing_nothing() {
        // A summary could otherwise point at any act at all — the one carrying it, say — and empty
        // the window it was going to be checked against.
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);
        chain.then(crate::kind::Kind::HOLDER_CHECKPOINT, &[]);

        let held = chain.held();
        let walked = branch(&held, &chain.head());
        let carrier = chain.head();

        assert_eq!(
            left_out(
                &Claim {
                    about: Governs::Devices,
                    stated: devices(&[&[7; 33]]),
                    set_by: chain.hash(2),
                },
                Placed {
                    carrier: &carrier,
                    branch: &walked
                }
            ),
            Verdict::LeftOut(chain.hash(0)),
            "nothing that governs the devices is later than nothing"
        );
    }

    #[test]
    fn what_the_words_asked_counts_only_once_its_wait_is_out() {
        // The checker replays the same wait the record applied. Judged as of the carrying act's
        // moment: before the wait is out the asking counts for nothing, after it, for the device.
        let control = vec![1u8; 32];
        let planted = vec![7u8; 33];
        let mut chain = Chain::new();
        chain.then_by(
            crate::kind::Kind::HOLDER_ADD_DEVICE,
            Value::Bytes(planted.clone()),
            &control,
            Epoch::new(10),
        );
        let acts: Vec<&Operation> = chain.acts.iter().collect();

        let waiting_still = Claim {
            about: Governs::Devices,
            stated: Stated::Keys(BTreeSet::new()),
            set_by: chain.hash(1),
        };
        assert_eq!(
            produces(&waiting_still, &acts, Epoch::new(11)),
            Verdict::Stands,
            "during the wait, an account with nothing on it is the honest claim"
        );
        assert_eq!(
            produces(&waiting_still, &acts, Epoch::new(82)),
            Verdict::Fabricated,
            "and once the wait is out, the same claim hides a device"
        );

        let landed = Claim {
            about: Governs::Devices,
            stated: Stated::Keys(BTreeSet::from([planted])),
            set_by: chain.hash(1),
        };
        assert_eq!(
            produces(&landed, &acts, Epoch::new(11)),
            Verdict::Fabricated,
            "claiming the device before its wait is out is jumping the wait"
        );
        assert_eq!(produces(&landed, &acts, Epoch::new(82)), Verdict::Stands);
    }

    #[test]
    fn a_struck_out_asking_never_counts_however_long_anybody_waits() {
        let control = vec![1u8; 32];
        let planted = vec![7u8; 33];
        let device = vec![9u8; 33];
        let mut chain = Chain::new();
        // A device joins by a device's hand — immediate — and then the words plant a key, and
        // the device strikes the asking out inside the window.
        chain.then_by(
            crate::kind::Kind::HOLDER_ADD_DEVICE,
            Value::Bytes(device.clone()),
            &device,
            Epoch::new(1),
        );
        chain.then_by(
            crate::kind::Kind::HOLDER_ADD_DEVICE,
            Value::Bytes(planted.clone()),
            &control,
            Epoch::new(10),
        );
        let asking = chain.hash(2);
        chain.then_by(
            crate::kind::Kind::HOLDER_CANCEL,
            Value::Text(asking.as_str().to_owned()),
            &device,
            Epoch::new(11),
        );
        let acts: Vec<&Operation> = chain.acts.iter().collect();

        let honest = Claim {
            about: Governs::Devices,
            stated: Stated::Keys(BTreeSet::from([device])),
            set_by: chain.hash(3),
        };
        assert_eq!(
            produces(&honest, &acts, Epoch::new(10_000)),
            Verdict::Stands,
            "the struck-out key never arrives"
        );

        let with_planted = Claim {
            about: Governs::Devices,
            stated: Stated::Keys(BTreeSet::from([vec![9u8; 33], planted])),
            set_by: chain.hash(3),
        };
        assert_eq!(
            produces(&with_planted, &acts, Epoch::new(10_000)),
            Verdict::Fabricated
        );
    }

    #[test]
    fn the_replay_ignores_a_cancel_the_words_signed() {
        // The reading side credits a cancellation only where the writing side took it. The words
        // never cancel, so their attempt strikes nothing and the asking still lands.
        let control = vec![1u8; 32];
        let device = vec![9u8; 33];
        let planted = vec![7u8; 33];
        let mut chain = Chain::new();
        chain.then_by(
            crate::kind::Kind::HOLDER_ADD_DEVICE,
            Value::Bytes(device.clone()),
            &device,
            Epoch::new(1),
        );
        chain.then_by(
            crate::kind::Kind::HOLDER_ADD_DEVICE,
            Value::Bytes(planted.clone()),
            &control,
            Epoch::new(10),
        );
        let asking = chain.hash(2);
        chain.then_by(
            crate::kind::Kind::HOLDER_CANCEL,
            Value::Text(asking.as_str().to_owned()),
            &control,
            Epoch::new(11),
        );
        let refs: Vec<&Operation> = chain.acts.iter().collect();
        let claim = Claim {
            about: Governs::Devices,
            stated: Stated::Keys(BTreeSet::from([device, planted])),
            set_by: chain.hash(2),
        };
        assert_eq!(
            produces(&claim, &refs, Epoch::new(10_000)),
            Verdict::Stands,
            "the words' cancel struck nothing, so the planted key lands"
        );
    }

    #[test]
    fn the_replay_ignores_a_device_cancelling_its_own_removal() {
        // A device may not strike the asking that removes it — on the reading side as much as the
        // writing one, or a stolen device would veto its own expulsion at bootstrap time.
        let control = vec![1u8; 32];
        let device = vec![9u8; 33];
        let mut chain = Chain::new();
        chain.then_by(
            crate::kind::Kind::HOLDER_ADD_DEVICE,
            Value::Bytes(device.clone()),
            &device,
            Epoch::new(1),
        );
        chain.then_by(
            crate::kind::Kind::HOLDER_REMOVE_DEVICE,
            Value::Bytes(device.clone()),
            &control,
            Epoch::new(10),
        );
        let removal = chain.hash(2);
        chain.then_by(
            crate::kind::Kind::HOLDER_CANCEL,
            Value::Text(removal.as_str().to_owned()),
            &device,
            Epoch::new(11),
        );
        let refs: Vec<&Operation> = chain.acts.iter().collect();
        let gone = Claim {
            about: Governs::Devices,
            stated: Stated::Keys(BTreeSet::new()),
            set_by: chain.hash(2),
        };
        assert_eq!(
            produces(&gone, &refs, Epoch::new(10_000)),
            Verdict::Stands,
            "the device could not veto its own removal, so the removal lands"
        );
    }

    #[test]
    fn a_rotation_lands_late_and_the_replay_knows_whose_hand_signed_after_it() {
        // Once a rotation lands, the old key's askings are nobody's askings — classification
        // follows the control key of each act's own moment.
        let old_control = vec![1u8; 32];
        let new_control = vec![2u8; 32];
        let planted = vec![7u8; 33];
        let mut chain = Chain::new();
        chain.then_by(
            crate::kind::Kind::HOLDER_ROTATE,
            Value::Bytes(new_control.clone()),
            &old_control,
            Epoch::new(10),
        );
        // At epoch 90 the rotation has landed; the old key planting now is a stranger's act —
        // but the replay is not admission, and a stranger's act would never have been admitted.
        // What matters here: an asking by the NEW key after landing waits like the words it now is.
        chain.then_by(
            crate::kind::Kind::HOLDER_ADD_DEVICE,
            Value::Bytes(planted.clone()),
            &new_control,
            Epoch::new(90),
        );
        let acts: Vec<&Operation> = chain.acts.iter().collect();

        let control_now = Claim {
            about: Governs::Control,
            stated: Stated::Key(new_control),
            set_by: chain.hash(1),
        };
        assert_eq!(
            produces(&control_now, &acts, Epoch::new(90)),
            Verdict::Stands
        );
        assert_eq!(
            // A carrier from before the rotation landed holds a shorter branch, and against
            // it the old key is still the account's.
            produces(&control_now, &acts[..2], Epoch::new(11)),
            Verdict::Fabricated,
            "before the rotation lands, the old key is still the account's"
        );

        let devices_wait = Claim {
            about: Governs::Devices,
            stated: Stated::Keys(BTreeSet::new()),
            set_by: chain.hash(2),
        };
        assert_eq!(
            produces(&devices_wait, &acts, Epoch::new(91)),
            Verdict::Stands,
            "the new words' asking waits like any words' asking"
        );
        assert_eq!(
            produces(&devices_wait, &acts, Epoch::new(162)),
            Verdict::Fabricated
        );
    }

    #[test]
    fn a_known_but_unbuilt_act_makes_a_summary_uncheckable_not_ignorable() {
        // The divergence this closes: admission turns an account carrying a named-but-unbuilt act
        // (setting guardians, say) opaque, and the reading side must not quietly skip it and vouch
        // for a state admission gave up on. Both sides land on *cannot say*.
        let device = vec![9u8; 33];
        let mut chain = Chain::new();
        chain.then_by(
            crate::kind::Kind::HOLDER_ADD_DEVICE,
            Value::Bytes(device.clone()),
            &[1u8; 32],
            Epoch::new(1),
        );
        // A kind this build knows the number of but has no logic for.
        chain.then_by(
            crate::kind::Kind::HOLDER_SET_GUARDIANS,
            Value::Bytes(vec![7u8; 33]),
            &[1u8; 32],
            Epoch::new(2),
        );
        // A summary carried after it, claiming a concrete device set.
        chain.then_by(
            crate::kind::Kind::HOLDER_CHECKPOINT,
            Value::Bytes(vec![]),
            &device,
            Epoch::new(3),
        );

        let held = chain.held();
        let carrier = chain.head();
        let walked = branch(&held, &carrier);
        let claim = Claim {
            about: Governs::Devices,
            stated: Stated::Keys(BTreeSet::from([device])),
            set_by: chain.hash(1),
        };
        assert!(
            matches!(
                left_out(
                    &claim,
                    Placed {
                        carrier: &carrier,
                        branch: &walked
                    }
                ),
                Verdict::CannotSay(_)
            ),
            "the branch carries an act this build cannot apply, so nothing about it can be vouched"
        );
    }

    /// A part of the state, as a summary states it and cites it.
    fn claim(about: Governs, stated: Stated, set_by: Name) -> Claim {
        Claim {
            about,
            stated,
            set_by,
        }
    }

    impl Chain {
        /// The branch to the head, as a reader walks it.
        fn walked(&self) -> Vec<&Entry> {
            branch(&self.held(), &self.head())
        }

        /// Every act, as a reader that fetched them all would hold them.
        fn every_act(&self) -> Vec<&Operation> {
            self.acts.iter().collect()
        }

        /// A summary act on the end of the chain, signed by that key at that moment.
        fn summarised_by(&mut self, by: &[u8], at: Epoch) -> &mut Self {
            self.then_by(
                crate::kind::Kind::HOLDER_CHECKPOINT,
                Value::Bytes(vec![]),
                by,
                at,
            )
        }
    }

    /// An account with one device, on which the words have asked for a second — which waits —
    /// and a summary written inside the window. Returns the chain and the asking's name.
    fn an_asking_in_flight() -> (Chain, Name) {
        let control = vec![1u8; 32];
        let device = vec![9u8; 33];
        let mut chain = Chain::new();
        chain.then_by(
            crate::kind::Kind::HOLDER_ADD_DEVICE,
            Value::Bytes(device.clone()),
            &device,
            Epoch::new(1),
        );
        chain.then_by(
            crate::kind::Kind::HOLDER_ADD_DEVICE,
            Value::Bytes(vec![7u8; 33]),
            &control,
            Epoch::new(10),
        );
        let asking = chain.hash(2);
        chain.summarised_by(&device, Epoch::new(11));
        (chain, asking)
    }

    #[test]
    fn a_summary_that_omits_an_asking_in_flight_falls_over() {
        // **The attack a summary used to be refused for**, now caught by the summary itself. The
        // words ask for a device, which waits; a summary written inside the window that says
        // nothing is waiting would carry the asking out of every reader's sight, to land later on
        // a state nobody was shown. So what is waiting is a part of the state, cited like any
        // other, and a summary that hides it falls over on both checks the log allows.
        let (chain, asking) = an_asking_in_flight();
        let walked = chain.walked();
        let carrier = chain.head();
        let placed = Placed {
            carrier: &carrier,
            branch: &walked,
        };

        let hiding = claim(
            Governs::Pending,
            Stated::Names(BTreeSet::new()),
            asking.clone(),
        );
        assert_eq!(
            produces(&hiding, &chain.every_act(), Epoch::new(11)),
            Verdict::Fabricated,
            "the asking is in flight at the summary's moment, and the summary says nothing is"
        );

        let citing_before_it = claim(
            Governs::Pending,
            Stated::Names(BTreeSet::new()),
            chain.hash(1),
        );
        assert_eq!(
            left_out(&citing_before_it, placed),
            Verdict::LeftOut(asking),
            "and citing the act before the asking hides the asking"
        );
    }

    #[test]
    fn a_summary_that_names_the_asking_in_flight_stands_until_it_lands() {
        let (chain, asking) = an_asking_in_flight();
        let walked = chain.walked();
        let carrier = chain.head();
        let placed = Placed {
            carrier: &carrier,
            branch: &walked,
        };
        let honest = claim(
            Governs::Pending,
            Stated::Names(BTreeSet::from([asking.clone()])),
            asking,
        );
        assert_eq!(
            holds_up(&honest, placed, &chain.every_act(), Epoch::new(11)),
            Verdict::Stands
        );
        assert_eq!(
            produces(&honest, &chain.every_act(), Epoch::new(82)),
            Verdict::Fabricated,
            "once it has landed it is no longer waiting, and saying it is would be as false"
        );
    }

    #[test]
    fn a_summary_that_says_not_frozen_on_a_frozen_account_falls_over() {
        // **The other half of the same attack.** A summary that could describe a stopped account
        // as moving would let a reader bootstrap the account somebody froze because their phone
        // was in a stranger's pocket as though nothing had happened.
        let control = vec![1u8; 32];
        let mut chain = Chain::new();
        chain.then_by(
            crate::kind::Kind::HOLDER_FREEZE,
            Value::Bytes(vec![]),
            &control,
            Epoch::new(5),
        );
        chain.summarised_by(&control, Epoch::new(6));
        let walked = chain.walked();
        let carrier = chain.head();
        let placed = Placed {
            carrier: &carrier,
            branch: &walked,
        };
        let acts = chain.every_act();

        let thawed = claim(Governs::Frozen, Stated::Flag(false), chain.hash(1));
        assert_eq!(produces(&thawed, &acts, Epoch::new(6)), Verdict::Fabricated);

        let hiding_the_freeze = claim(Governs::Frozen, Stated::Flag(false), chain.hash(0));
        assert_eq!(
            left_out(&hiding_the_freeze, placed),
            Verdict::LeftOut(chain.hash(1))
        );

        let stopped = claim(Governs::Frozen, Stated::Flag(true), chain.hash(1));
        assert_eq!(
            holds_up(&stopped, placed, &acts, Epoch::new(6)),
            Verdict::Stands
        );
    }

    #[test]
    fn a_thaw_the_words_asked_for_waits_and_the_summary_says_so_until_it_lands() {
        // Thawing concedes back everything the freeze stopped, so it waits like anything else the
        // words ask alone — and while it waits the account is still frozen and the asking is in
        // flight, which is what a summary written in the window has to say.
        let control = vec![1u8; 32];
        let mut chain = Chain::new();
        chain.then_by(
            crate::kind::Kind::HOLDER_FREEZE,
            Value::Bytes(vec![]),
            &control,
            Epoch::new(5),
        );
        chain.then_by(
            crate::kind::Kind::HOLDER_UNFREEZE,
            Value::Bytes(vec![]),
            &control,
            Epoch::new(10),
        );
        let thawing = chain.hash(2);
        let acts = chain.every_act();

        let still_frozen = claim(Governs::Frozen, Stated::Flag(true), thawing.clone());
        let thaw_waiting = claim(
            Governs::Pending,
            Stated::Names(BTreeSet::from([thawing.clone()])),
            thawing.clone(),
        );
        assert_eq!(
            produces(&still_frozen, &acts, Epoch::new(11)),
            Verdict::Stands
        );
        assert_eq!(
            produces(&thaw_waiting, &acts, Epoch::new(11)),
            Verdict::Stands
        );

        let moving = claim(Governs::Frozen, Stated::Flag(false), thawing.clone());
        let nothing_waiting = claim(Governs::Pending, Stated::Names(BTreeSet::new()), thawing);
        assert_eq!(produces(&moving, &acts, Epoch::new(82)), Verdict::Stands);
        assert_eq!(
            produces(&nothing_waiting, &acts, Epoch::new(82)),
            Verdict::Stands
        );
        assert_eq!(
            produces(&moving, &acts, Epoch::new(11)),
            Verdict::Fabricated,
            "claiming the thaw before its wait is out is jumping the wait"
        );
    }

    /// An account with one device, and the four claims that describe it truthfully.
    fn an_account_with_one_device() -> (Chain, [Claim; 4]) {
        let mut chain = Chain::new();
        chain.then(crate::kind::Kind::HOLDER_ADD_DEVICE, &[7; 33]);
        let whole = [
            claim(Governs::Control, Stated::Key(vec![1; 32]), chain.hash(0)),
            claim(Governs::Devices, devices(&[&[7; 33]]), chain.hash(1)),
            claim(Governs::Frozen, Stated::Flag(false), chain.hash(0)),
            claim(
                Governs::Pending,
                Stated::Names(BTreeSet::new()),
                chain.hash(1),
            ),
        ];
        (chain, whole)
    }

    #[test]
    fn a_summary_that_says_nothing_about_freezing_or_what_is_waiting_falls_over() {
        // Completeness is measured against what the state has. An account has a freeze and a
        // queue whether or not a summary chose to mention them, and silence about either would be
        // read as *moving* and *nothing waiting* by everybody who arrives.
        let (chain, whole) = an_account_with_one_device();
        let walked = chain.walked();
        let carrier = chain.head();
        let placed = Placed {
            carrier: &carrier,
            branch: &walked,
        };
        let acts = chain.every_act();

        let fell = falls_over(&whole[..2], placed, &acts, Epoch::GENESIS);
        assert_eq!(
            fell,
            vec![
                (Governs::Frozen, Verdict::Missing),
                (Governs::Pending, Verdict::Missing)
            ]
        );
        assert!(falls_over(&whole, placed, &acts, Epoch::GENESIS).is_empty());
    }

    /// An account whose first device arrived carrying one more field, at that number.
    fn a_device_added_carrying(number: u64) -> Chain {
        let device = vec![9u8; 33];
        let mut chain = Chain::new();
        chain.then_carrying(
            crate::kind::Kind::HOLDER_ADD_DEVICE,
            BTreeMap::from([
                (1, Value::Bytes(device.clone())),
                (number, Value::Text("something newer".to_owned())),
            ]),
            &device,
            Epoch::new(1),
        );
        chain.summarised_by(&device, Epoch::new(2));
        chain
    }

    #[test]
    fn an_act_carrying_a_critical_field_this_build_cannot_read_makes_a_summary_uncheckable() {
        // **What the log could not say before it carried the numbers.** The act's kind is one this
        // build applies, so a check by kind alone would vouch across it — while admission, holding
        // the act, declares the account opaque at the same field. The entry now lists the odd
        // numbers the payload carries, and the reading side draws the same line as the writing side
        // without fetching anything.
        let chain = a_device_added_carrying(77);
        let walked = chain.walked();
        let carrier = chain.head();
        let control = claim(Governs::Control, Stated::Key(vec![1; 32]), chain.hash(0));
        assert_eq!(
            left_out(
                &control,
                Placed {
                    carrier: &carrier,
                    branch: &walked
                }
            ),
            Verdict::CannotSay(chain.hash(1)),
            "even for a part that act does not govern: opacity is about the account"
        );
    }

    #[test]
    fn an_act_carrying_an_even_field_this_build_cannot_read_is_passed_over_here_too() {
        // On the reading side as on the writing: an even number is one a reader may skip and
        // still claim to have applied the act, which is what makes an extension an addition.
        let chain = a_device_added_carrying(78);
        let walked = chain.walked();
        let carrier = chain.head();
        let control = claim(Governs::Control, Stated::Key(vec![1; 32]), chain.hash(0));
        assert_eq!(
            left_out(
                &control,
                Placed {
                    carrier: &carrier,
                    branch: &walked
                }
            ),
            Verdict::Stands
        );
    }

    /// A node saying again what it offers and where it is.
    fn announcing_again(offers: &[u64], reachable: &[&str]) -> BTreeMap<u64, Value> {
        BTreeMap::from([
            (
                crate::capability::OFFERS,
                Value::Array(offers.iter().copied().map(Value::Uint).collect()),
            ),
            (crate::capability::SPEAKS, Value::Uint(1)),
            (
                crate::capability::WHERE,
                Value::Array(
                    reachable
                        .iter()
                        .map(|address| Value::Text((*address).to_owned()))
                        .collect(),
                ),
            ),
        ])
    }

    const ONE: &str = "/ip4/10.0.0.1/tcp/4001";
    const TWO: &str = "/ip4/10.0.0.2/tcp/4001";

    /// A node that announced twice — more the second time — with a daily summary after each.
    fn a_node_that_announced_twice() -> Chain {
        let key = vec![5u8; 32];
        let mut chain = Chain::node();
        chain.then_carrying(
            crate::kind::Kind::NODE_ANNOUNCE,
            announcing_again(&[1, 3], &[ONE]),
            &key,
            Epoch::new(1),
        );
        chain.then_carrying(
            crate::kind::Kind::NODE_SUMMARY,
            BTreeMap::new(),
            &key,
            Epoch::new(24),
        );
        chain.then_carrying(
            crate::kind::Kind::NODE_ANNOUNCE,
            announcing_again(&[1, 2, 3], &[ONE, TWO]),
            &key,
            Epoch::new(30),
        );
        chain.then_carrying(
            crate::kind::Kind::NODE_SUMMARY,
            BTreeMap::new(),
            &key,
            Epoch::new(48),
        );
        chain
    }

    /// The four claims that describe that node truthfully, as of its last act.
    fn what_the_node_is(chain: &Chain) -> [Claim; 4] {
        let latest = chain.hash(3);
        [
            claim(Governs::NodeKey, Stated::Key(vec![5; 32]), latest.clone()),
            claim(
                Governs::Offers,
                Stated::Numbers(BTreeSet::from([1, 2, 3])),
                latest.clone(),
            ),
            claim(
                Governs::Reachable,
                Stated::Addresses(BTreeSet::from([ONE.to_owned(), TWO.to_owned()])),
                latest.clone(),
            ),
            claim(Governs::Closed, Stated::Moment(None), latest),
        ]
    }

    #[test]
    fn a_node_s_own_chain_can_be_summarised() {
        // A node's chain grows by one daily summary for as long as it runs, and until it had parts
        // a summary could claim it owed one it could never pay. What a node **is** — its key, what
        // it offers, where it says it is, whether it has closed — is set by its announcements and
        // its closing, and is checked exactly as an account's parts are.
        let chain = a_node_that_announced_twice();
        let walked = chain.walked();
        let carrier = chain.head();
        let placed = Placed {
            carrier: &carrier,
            branch: &walked,
        };
        let honest = what_the_node_is(&chain);
        assert!(
            falls_over(&honest, placed, &chain.every_act(), Epoch::new(48)).is_empty(),
            "the latest announcement set every part, and it says what the acts say"
        );

        // A summary that says nothing about closing is not a smaller summary of a node.
        let fell = falls_over(&honest[..3], placed, &chain.every_act(), Epoch::new(48));
        assert_eq!(fell, vec![(Governs::Closed, Verdict::Missing)]);
    }

    #[test]
    fn a_node_summary_that_hides_an_announcement_or_inflates_one_falls_over() {
        let chain = a_node_that_announced_twice();
        let walked = chain.walked();
        let carrier = chain.head();
        let placed = Placed {
            carrier: &carrier,
            branch: &walked,
        };

        let stale = claim(
            Governs::Offers,
            Stated::Numbers(BTreeSet::from([1, 3])),
            chain.hash(1),
        );
        assert_eq!(
            left_out(&stale, placed),
            Verdict::LeftOut(chain.hash(3)),
            "citing the first announcement hides the second"
        );

        let inflated = claim(
            Governs::Offers,
            Stated::Numbers(BTreeSet::from([1, 2, 3, 4])),
            chain.hash(3),
        );
        assert_eq!(
            produces(&inflated, &chain.every_act(), Epoch::new(48)),
            Verdict::Fabricated,
            "and a capability nobody announced is not one"
        );
    }

    #[test]
    fn closing_a_node_is_a_part_of_what_it_is_and_the_moment_does_not_move() {
        let key = vec![5u8; 32];
        let mut chain = Chain::node();
        chain.then_carrying(
            crate::kind::Kind::NODE_CLOSE,
            BTreeMap::new(),
            &key,
            Epoch::new(40),
        );
        chain.then_carrying(
            crate::kind::Kind::NODE_SUMMARY,
            BTreeMap::new(),
            &key,
            Epoch::new(48),
        );
        let walked = chain.walked();
        let carrier = chain.head();
        let placed = Placed {
            carrier: &carrier,
            branch: &walked,
        };
        let acts = chain.every_act();

        let open = claim(Governs::Closed, Stated::Moment(None), chain.hash(0));
        assert_eq!(left_out(&open, placed), Verdict::LeftOut(chain.hash(1)));

        let closed = claim(
            Governs::Closed,
            Stated::Moment(Some(Epoch::new(40))),
            chain.hash(1),
        );
        assert_eq!(
            holds_up(&closed, placed, &acts, Epoch::new(48)),
            Verdict::Stands
        );

        let moved = claim(
            Governs::Closed,
            Stated::Moment(Some(Epoch::new(41))),
            chain.hash(1),
        );
        assert_eq!(produces(&moved, &acts, Epoch::new(48)), Verdict::Fabricated);
    }

    #[test]
    fn every_shape_a_part_can_take_survives_the_wire() {
        let cited = Name::of(b"the act that set it");
        let claims = vec![
            claim(Governs::Frozen, Stated::Flag(true), cited.clone()),
            claim(
                Governs::Pending,
                Stated::Names(BTreeSet::from([Name::of(b"an asking")])),
                cited.clone(),
            ),
            claim(
                Governs::Offers,
                Stated::Numbers(BTreeSet::from([1, 2])),
                cited.clone(),
            ),
            claim(
                Governs::Reachable,
                Stated::Addresses(BTreeSet::from([ONE.to_owned()])),
                cited.clone(),
            ),
            claim(
                Governs::Closed,
                Stated::Moment(Some(Epoch::new(40))),
                cited.clone(),
            ),
        ];
        let mut carrier = create(
            Network::Development,
            crate::kind::Kind::NODE_SUMMARY.number(),
            1,
            Epoch::GENESIS,
            BTreeMap::new(),
        );
        carrier.payload.insert(super::FIELD, declaration(&claims));
        assert_eq!(declared(&carrier), Ok(Some(claims)));

        // And a value in the wrong shape for its part is refused rather than read as something.
        let wrong_shape =
            Value::Array(vec![Value::Uint(2), Value::Text(cited.as_str().to_owned())]);
        carrier.payload.insert(
            super::FIELD,
            Value::Map(BTreeMap::from([(Governs::Frozen.number(), wrong_shape)])),
        );
        assert_eq!(
            declared(&carrier),
            Err(super::Unreadable(Some(Governs::Frozen.number())))
        );
    }
}
