//! The credential format: what an issuer signs, what a holder shows, and what verifying concludes.
//!
//! # SD-JWT VC, in ES256, from the first day
//!
//! `SPECS.md §9.1`. It is where the European ecosystem is going, and it gives selective disclosure
//! without exotic cryptography: every attribute is replaced by a commitment, the issuer signs the
//! set, and the holder reveals value and salt only for what they choose to show.
//!
//! **The credential lives in the P-256 plane entire** — issuance, key binding and presentation —
//! because ES256 is what the EUDI ARF and ISO 18013-5 require, and those are the sources
//! `SPECS.md §9.4` leans on.
//!
//! # Three fields that selective disclosure may not reach
//!
//! The proof type, the issuer identification method and whether the credential is revocable. Each
//! is mandatory, each is outside the commitments, and each is read from a **closed vocabulary** —
//! for one reason, said three times:
//!
//! - **A proof type that could be hidden would not be a mark.** A verifier that did not find one
//!   would not fail closed; it would *assume* the type it knows, which is exactly the implicit
//!   assumption the field exists to forbid.
//! - **The identification method is read before anything has been verified** — it decides *what to
//!   verify with*. So the verifier takes it from **its own list** and never from what the
//!   credential proposes, which is the algorithm-confusion mistake with a new coat on.
//! - **Non-revocability has to be an explicit signed claim**, never an absent field, or an attacker
//!   presents a revoked credential by leaving the mechanism out.
//!
//! # And the credential's own identifier is hideable
//!
//! It is a disclosure like any attribute (`SPECS.md §9.1`). An identifier that always travelled
//! would correlate two presentations exactly as an attribute would, and hiding the attributes while
//! leaving the name in place would be hiding nothing.
//!
//! # What this crate does not do
//!
//! It does not decide whether an issuer is to be trusted, and it does not fetch anything. Resolving
//! the issuer and reading a status list are the caller's, behind interfaces, so that this crate can
//! be given a credential and a set of facts and asked only *does this hold up*.

pub mod base64url;
pub mod disclosure;
pub mod issue;
pub mod present;
pub mod verify;

use almena_time::Epoch;

/// Which draft of SD-JWT VC this build writes, fixed and copied rather than tracked.
///
/// **The same discipline the attribute core follows** (`SPECS.md §9.4`): the version is pinned so
/// that a credential issued today goes on meaning what it meant when the draft moves. Changing it
/// is a decision somebody takes, not something that happens.
pub const MEDIA_TYPE: &str = "dc+sd-jwt";

/// What a key-binding JWT calls itself.
pub const BINDING_TYPE: &str = "kb+jwt";

/// The one signature algorithm a credential is written in.
pub const ALGORITHM: &str = "ES256";

/// The hash the commitments are taken with, named inside the credential so nobody has to assume.
pub const DIGEST_NAME: &str = "sha-256";

/// How a claim is proved.
///
/// **Closed** (`SPECS.md §9.1`). A value this build does not know stops the reader rather than
/// being read as the nearest thing — *a proof slightly stronger than the one I know* is exactly the
/// reading that would be dangerous.
///
/// One member today, and the field exists for the second: a predicate is satisfied by a derived
/// attribute the issuer computed at issuance, and zero knowledge is a reserved hole (`SPECS.md §18`)
/// rather than a plan. When it arrives, a template does not change — this field does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proof {
    /// The value travels with its salt, and whoever receives it recomputes the commitment.
    Disclosure,
}

impl Proof {
    /// The proof a name names, if it is one this build knows.
    #[must_use]
    pub fn of(name: &str) -> Option<Self> {
        match name {
            "disclosure" => Some(Self::Disclosure),
            _ => None,
        }
    }

    /// The name it travels as.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Disclosure => "disclosure",
        }
    }
}

/// How the issuer is to be identified.
///
/// **Closed, and the verifier accepts only the ones on its own list** (`SPECS.md §9.1`). This field
/// is read before anything has been verified, so letting the credential choose it would be letting
/// what is not yet verified decide what verifies it.
///
/// One member today. The interface is here because `SPECS.md §9.1` asks that interoperability with
/// X.509 and state trust lists stay open, and an interface added later is a migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Method {
    /// Resolve the issuer from the Almena record, and read what has been said about it there.
    Almena,
}

impl Method {
    /// The method a name names, if it is one this build knows.
    #[must_use]
    pub fn of(name: &str) -> Option<Self> {
        match name {
            "did:almena" => Some(Self::Almena),
            _ => None,
        }
    }

    /// The name it travels as.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Almena => "did:almena",
        }
    }
}

/// Whether a credential can be revoked, and where its bit is.
///
/// **Always present, in both directions** (`SPECS.md §10.1`). An absent field would let an attacker
/// present a revoked credential by leaving the mechanism out, so *not revocable* is something the
/// issuer signs rather than something a reader concludes from silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// It can be revoked, and this is the list and the place in it.
    Revocable {
        /// The status list object, by its identifier.
        list: String,
        /// Which bit of it is this credential's.
        ///
        /// **Assigned at random within a sparse space** (`SPECS.md §10.2`): a sequential index
        /// would reveal how long somebody has been a customer, which is an attribute the holder
        /// never chose to disclose and which travels in every presentation.
        index: u64,
    },
    /// It cannot be revoked, and the issuer says so rather than leaving it out.
    ///
    /// A short-lived credential, or one about a fact that cannot stop being true.
    NotRevocable,
}

/// What every credential carries outside its commitments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct About {
    /// Who issued it: the issuer element, by its identifier.
    pub issuer: String,
    /// Which template version it was issued against, by the hash of the act that published it.
    ///
    /// **By hash and never by name** (`SPECS.md §9.4`): a credential naming a template by anything
    /// a later act could change would be one whose meaning moves under it.
    pub template: String,
    /// The epoch it was issued in.
    pub issued: Epoch,
    /// The epoch it stops being valid in.
    ///
    /// **Every credential has one** (`SPECS.md §10.1`). Expiry bounds the long-term damage and
    /// revocation bounds the short-term; they are complementary, and neither replaces the other.
    pub expires: Epoch,
    /// How its claims are proved.
    pub proof: Proof,
    /// How the issuer is to be identified.
    pub method: Method,
    /// Whether it can be revoked, and where.
    pub status: Status,
}

/// Where each part of the payload sits, by the name JSON gives it.
pub mod claim {
    /// Who issued it.
    pub const ISSUER: &str = "iss";
    /// Which template version, which is what this credential is the shape of.
    pub const TEMPLATE: &str = "vct";
    /// When it was issued.
    pub const ISSUED: &str = "iat";
    /// When it stops being valid.
    pub const EXPIRES: &str = "exp";
    /// The key the holder proves possession of.
    pub const CONFIRMATION: &str = "cnf";
    /// The commitments.
    pub const COMMITMENTS: &str = "_sd";
    /// Which hash the commitments were taken with.
    pub const DIGEST: &str = "_sd_alg";
    /// How the claims are proved.
    pub const PROOF: &str = "proof";
    /// How the issuer is to be identified.
    pub const METHOD: &str = "issuer_method";
    /// Whether it is revocable, and where its bit is.
    pub const STATUS: &str = "status";
    /// The credential's own identifier, which travels as a disclosure and never in the clear.
    pub const IDENTIFIER: &str = "jti";
}
