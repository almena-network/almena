//! Which act an operation is, as the number that travels in every log entry.
//!
//! **The number is unique across the whole protocol, and it has to be.** A log entry carries
//! `tipo` and does *not* carry what class of object it is about, so whoever reads one has to know
//! what it is without resolving anything. Two different acts sharing a number would be two acts a
//! node running an older version could not tell apart.
//!
//! **Zero is no act at all**, so that a field of zeroes is never a valid operation.
//!
//! `create` and `checkpoint` have a number of their own on every object even though they do the
//! same thing: their payloads and their validation rules differ, and sharing a number would force
//! a reader to resolve the object just to learn what it was reading.
//!
//! **A number is never reassigned.** An act that gets dropped leaves its number burnt: nothing
//! already signed may ever be reinterpreted.

/// The kind of act an operation performs.
///
/// It is deliberately not an enum. A node stores and propagates every operation whether it
/// understands it or not, so a kind it has never heard of has to be a value it can hold, compare
/// and pass on — an enum would make an unknown act unrepresentable, which is the one thing this
/// type must not do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Kind(u64);

impl Kind {
    // Holder
    /// `create` on a holder.
    pub const HOLDER_CREATE: Kind = Kind(1);
    /// `add_device` on a holder.
    pub const HOLDER_ADD_DEVICE: Kind = Kind(2);
    /// `remove_device` on a holder.
    pub const HOLDER_REMOVE_DEVICE: Kind = Kind(3);
    /// `rotate` on a holder.
    pub const HOLDER_ROTATE: Kind = Kind(4);
    /// `recover` on a holder.
    pub const HOLDER_RECOVER: Kind = Kind(5);
    /// `freeze` on a holder.
    pub const HOLDER_FREEZE: Kind = Kind(6);
    /// `unfreeze` on a holder.
    pub const HOLDER_UNFREEZE: Kind = Kind(7);
    /// `set_guardians` on a holder.
    pub const HOLDER_SET_GUARDIANS: Kind = Kind(8);
    /// `cancel` on a holder.
    pub const HOLDER_CANCEL: Kind = Kind(9);
    /// `checkpoint` on a holder.
    pub const HOLDER_CHECKPOINT: Kind = Kind(10);

    // Entity
    /// `create` on an entity.
    pub const ENTITY_CREATE: Kind = Kind(11);
    /// `add_owner` on an entity.
    pub const ENTITY_ADD_OWNER: Kind = Kind(12);
    /// `remove_owner` on an entity.
    pub const ENTITY_REMOVE_OWNER: Kind = Kind(13);
    /// `add_manager` on an entity.
    pub const ENTITY_ADD_MANAGER: Kind = Kind(14);
    /// `remove_manager` on an entity.
    pub const ENTITY_REMOVE_MANAGER: Kind = Kind(15);
    /// `set_threshold` on an entity.
    pub const ENTITY_SET_THRESHOLD: Kind = Kind(16);
    /// `rotate_key` on an entity.
    pub const ENTITY_ROTATE_KEY: Kind = Kind(17);
    /// `add_domain` on an entity.
    pub const ENTITY_ADD_DOMAIN: Kind = Kind(18);
    /// `remove_domain` on an entity.
    pub const ENTITY_REMOVE_DOMAIN: Kind = Kind(19);
    /// `set_alias` on an entity.
    pub const ENTITY_SET_ALIAS: Kind = Kind(20);
    /// `continuity` on an entity.
    pub const ENTITY_CONTINUITY: Kind = Kind(21);
    /// `veto` on an entity.
    pub const ENTITY_VETO: Kind = Kind(22);
    /// `close` on an entity.
    pub const ENTITY_CLOSE: Kind = Kind(23);
    /// `checkpoint` on an entity.
    pub const ENTITY_CHECKPOINT: Kind = Kind(24);

    // Issuer or verifier
    /// `create` on an issuer or verifier.
    pub const ISSUER_CREATE: Kind = Kind(25);
    /// `set_config` on an issuer or verifier.
    pub const ISSUER_SET_CONFIG: Kind = Kind(26);
    /// `set_issuance_key` on an issuer or verifier.
    pub const ISSUER_SET_ISSUANCE_KEY: Kind = Kind(27);
    /// `rotate_key` on an issuer or verifier.
    pub const ISSUER_ROTATE_KEY: Kind = Kind(28);
    /// `close` on an issuer or verifier.
    pub const ISSUER_CLOSE: Kind = Kind(29);

    // Template
    /// `publish` on a template.
    pub const TEMPLATE_PUBLISH: Kind = Kind(30);
    /// `deprecate` on a template.
    pub const TEMPLATE_DEPRECATE: Kind = Kind(31);

    // Attribute
    /// `publish` on an attribute.
    pub const ATTRIBUTE_PUBLISH: Kind = Kind(32);
    /// `translate` on an attribute.
    pub const ATTRIBUTE_TRANSLATE: Kind = Kind(33);
    /// `deprecate` on an attribute.
    pub const ATTRIBUTE_DEPRECATE: Kind = Kind(34);

    // Source
    /// `admit` on a source.
    pub const SOURCE_ADMIT: Kind = Kind(35);
    /// `deprecate` on a source.
    pub const SOURCE_DEPRECATE: Kind = Kind(36);

    // Tag
    /// `add` on a tag.
    pub const TAG_ADD: Kind = Kind(37);
    /// `translate` on a tag.
    pub const TAG_TRANSLATE: Kind = Kind(38);
    /// `deprecate` on a tag.
    pub const TAG_DEPRECATE: Kind = Kind(39);

    // Status list
    /// `publish_version` on a status list.
    pub const STATUS_LIST_PUBLISH_VERSION: Kind = Kind(40);

    // Certification
    /// `issue` on a certification.
    pub const CERTIFICATION_ISSUE: Kind = Kind(41);
    /// `revoke` on a certification.
    pub const CERTIFICATION_REVOKE: Kind = Kind(42);

    // Node
    /// `announce` on a node.
    pub const NODE_ANNOUNCE: Kind = Kind(43);
    /// `bind` on a node.
    pub const NODE_BIND: Kind = Kind(44);
    /// `unbind` on a node.
    pub const NODE_UNBIND: Kind = Kind(45);
    /// `summary` on a node.
    pub const NODE_SUMMARY: Kind = Kind(46);

    // Contradiction
    /// `publish` on a contradiction.
    pub const CONTRADICTION_PUBLISH: Kind = Kind(47);

    // Governance proposal
    /// `open` on a governance proposal.
    pub const PROPOSAL_OPEN: Kind = Kind(48);
    /// `close` on a governance proposal.
    pub const PROPOSAL_CLOSE: Kind = Kind(49);

    // Vote
    /// `cast` on a vote.
    pub const VOTE_CAST: Kind = Kind(50);

    // Genesis
    /// `genesis` — the one act that opens a network. Its object is Almena Government, and its own
    /// hash is what the network is called.
    ///
    /// It has a number of its own rather than being an entity creation with extra fields, because
    /// it does three things no other act does — opens the record, fixes the instant epoch zero
    /// begins, and declares which network this is — and because recognising it by its being first
    /// would be deciding validity against a position, which is exactly what nothing here may do.
    pub const GENESIS: Kind = Kind(51);

    // Reply
    /// `publish` on a reply: what the party a decision was taken about has to say back.
    ///
    /// **An object of its own, pointing at the decision** — the same figure as a certification and a
    /// vote, and for the same reason: nobody writes in somebody else's chain. A reply appended to
    /// the certification's chain would mean the party affected could add to what Almena said, and a
    /// reply Almena had to accept would be no reply at all.
    ///
    /// `SPECS.md §7.8` calls for it and does not number it, so this number is this build's. It
    /// exists because there is no authority above Almena: appealing *to Almena* is asking it to
    /// re-read itself, and what fits the rest of the design is that **the decision and the answer
    /// are published together, and for ever** — whoever chooses their own root of trust reads both
    /// and judges.
    pub const REPLY_PUBLISH: Kind = Kind(52);
}

impl Kind {
    /// The kind a number names, or nothing for zero.
    #[must_use]
    pub const fn new(number: u64) -> Option<Self> {
        if number == 0 {
            return None;
        }
        Some(Self(number))
    }

    /// The number, which is how it travels.
    #[must_use]
    pub const fn number(self) -> u64 {
        self.0
    }

    /// Whether this build knows what the act is.
    ///
    /// An unknown kind is not an error: it is an operation from a newer version, which this node
    /// stores and passes on, and whose object it then declines to resolve rather than serving a
    /// state it cannot vouch for.
    #[must_use]
    pub fn known(self) -> bool {
        Self::ALL.contains(&self)
    }

    /// Every act this build knows, in the order they are numbered.
    pub const ALL: [Self; 52] = [
        Self::HOLDER_CREATE,
        Self::HOLDER_ADD_DEVICE,
        Self::HOLDER_REMOVE_DEVICE,
        Self::HOLDER_ROTATE,
        Self::HOLDER_RECOVER,
        Self::HOLDER_FREEZE,
        Self::HOLDER_UNFREEZE,
        Self::HOLDER_SET_GUARDIANS,
        Self::HOLDER_CANCEL,
        Self::HOLDER_CHECKPOINT,
        Self::ENTITY_CREATE,
        Self::ENTITY_ADD_OWNER,
        Self::ENTITY_REMOVE_OWNER,
        Self::ENTITY_ADD_MANAGER,
        Self::ENTITY_REMOVE_MANAGER,
        Self::ENTITY_SET_THRESHOLD,
        Self::ENTITY_ROTATE_KEY,
        Self::ENTITY_ADD_DOMAIN,
        Self::ENTITY_REMOVE_DOMAIN,
        Self::ENTITY_SET_ALIAS,
        Self::ENTITY_CONTINUITY,
        Self::ENTITY_VETO,
        Self::ENTITY_CLOSE,
        Self::ENTITY_CHECKPOINT,
        Self::ISSUER_CREATE,
        Self::ISSUER_SET_CONFIG,
        Self::ISSUER_SET_ISSUANCE_KEY,
        Self::ISSUER_ROTATE_KEY,
        Self::ISSUER_CLOSE,
        Self::TEMPLATE_PUBLISH,
        Self::TEMPLATE_DEPRECATE,
        Self::ATTRIBUTE_PUBLISH,
        Self::ATTRIBUTE_TRANSLATE,
        Self::ATTRIBUTE_DEPRECATE,
        Self::SOURCE_ADMIT,
        Self::SOURCE_DEPRECATE,
        Self::TAG_ADD,
        Self::TAG_TRANSLATE,
        Self::TAG_DEPRECATE,
        Self::STATUS_LIST_PUBLISH_VERSION,
        Self::CERTIFICATION_ISSUE,
        Self::CERTIFICATION_REVOKE,
        Self::NODE_ANNOUNCE,
        Self::NODE_BIND,
        Self::NODE_UNBIND,
        Self::NODE_SUMMARY,
        Self::CONTRADICTION_PUBLISH,
        Self::PROPOSAL_OPEN,
        Self::PROPOSAL_CLOSE,
        Self::VOTE_CAST,
        Self::GENESIS,
        Self::REPLY_PUBLISH,
    ];
}

#[cfg(test)]
mod tests {
    use super::Kind;

    #[test]
    fn zero_is_no_act() {
        assert_eq!(Kind::new(0), None);
        assert!(Kind::new(1).is_some());
    }

    #[test]
    fn the_numbers_run_from_one_with_no_gap_and_no_repeat() {
        // The table is transcribed by hand from the decision that fixed it, so this is what
        // catches a number typed twice or skipped — either of which would be an act nobody could
        // name, or two acts nobody could tell apart.
        let numbers: Vec<u64> = Kind::ALL.iter().map(|kind| kind.number()).collect();
        assert_eq!(numbers, (1..=52).collect::<Vec<u64>>());
    }

    #[test]
    fn creating_each_object_is_a_different_act() {
        // Same word, different payloads and different rules. Sharing a number would mean
        // resolving the object to find out what was being read.
        assert_ne!(Kind::HOLDER_CREATE, Kind::ENTITY_CREATE);
        assert_ne!(Kind::ENTITY_CREATE, Kind::ISSUER_CREATE);
        assert_ne!(Kind::HOLDER_CHECKPOINT, Kind::ENTITY_CHECKPOINT);
    }

    #[test]
    fn an_act_from_a_newer_version_is_a_value_and_not_an_error() {
        // The property the whole type exists for: a node replicates what it does not understand.
        let newer = Kind::new(9_999).expect("not zero");
        assert!(!newer.known());
        assert_eq!(newer.number(), 9_999);
    }

    #[test]
    fn everything_this_build_lists_is_known() {
        for kind in Kind::ALL {
            assert!(kind.known(), "{kind:?}");
        }
    }
}
