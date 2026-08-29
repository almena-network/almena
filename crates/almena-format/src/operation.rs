//! The operation: the whole act, signed.
//!
//! ```text
//! objeto, prev, tipo, version
//! emitida   the author's declared timestamp
//! payload   the fields of this type, with the criticality mark
//! firmas    one or more, each with the key that produced it
//! ```
//!
//! # Three sets of bytes, and the difference between them is the whole design
//!
//! | | What it is | Why |
//! |---|---|---|
//! | [`Operation::to_bytes`] | Everything | What travels and what is stored |
//! | [`Operation::signing_bytes`] | Everything **but `firmas`** | A signature cannot cover itself |
//! | [`Operation::naming_bytes`] | Everything but `firmas` **and `objeto`** | The name cannot refer to itself, and must not depend on how it was signed |
//!
//! The last one **has to be written down or it is worthless**, because two honest implementations
//! would otherwise compute different identifiers for the same operation.
//! And `firmas` comes out whole rather than one signature at a time: if the name depended on how
//! many signed, or in what order, one operation would have several names.

use crate::cbor::Value;
use crate::field::{self, Unintelligible, Vocabulary};
use crate::identifier::{Did, Name, Network};
use almena_time::Epoch;
use std::collections::BTreeMap;

/// Where each part of an operation sits in the map.
mod key {
    /// The object whose chain this advances.
    pub const OBJECT: u64 = 1;
    /// The hash of that object's previous operation, or null if this is the first.
    pub const PREVIOUS: u64 = 2;
    /// Which kind of operation this is.
    pub const KIND: u64 = 3;
    /// Which version of that kind.
    pub const VERSION: u64 = 4;
    /// The epoch its author declared.
    pub const ISSUED: u64 = 5;
    /// The fields of this kind.
    pub const PAYLOAD: u64 = 6;
    /// Who signed, and with what.
    pub const SIGNATURES: u64 = 7;
}

/// One signature on an operation: whose it is, which of their keys made it, and the signature.
///
/// **Both halves are needed and neither replaces the other.** The DID says *whose* signature this
/// is — for an entity operation an owner's root DID, never one of their keys, so that rotating or
/// recovering does not cost an owner their place. The key says *which* key produced it, which the
/// DID cannot: a holder governs their account with one key and operates it with another, and the
/// whole safety of that separation is that an operation says which of the two signed it.
///
/// The key is 32 bytes on the curve the control key and node keys use, and 33 on the one device
/// and issuance keys use. Nothing has to measure it to find out which: the kind of operation says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signed {
    /// Whose signature this is.
    pub by: Did,
    /// The public key that produced it, as it appears in the object's state.
    pub key: Vec<u8>,
    /// The signature itself. Sixty-four bytes on both of [`almena_suite`]'s curves.
    pub signature: [u8; 64],
}

/// An act on one object's chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    /// The object whose chain this advances.
    pub object: Did,
    /// The hash of that object's previous operation. [`None`] on the first, written as null.
    pub previous: Option<Name>,
    /// Which kind of operation this is.
    pub kind: u64,
    /// Which version of that kind — fixed on writing and never reinterpreted.
    pub version: u64,
    /// The epoch its author declared. It orders nothing: position in the log does that.
    pub issued: Epoch,
    /// The fields of this kind, each carrying its own criticality in its number.
    pub payload: BTreeMap<u64, Value>,
    /// **A list, because entity operations carry k of them.** An object with a single controller
    /// has a list of one — *the key the previous state authorised*.
    pub signatures: Vec<Signed>,
}

impl Operation {
    /// Everything: what travels between nodes and what a node stores.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut fields = self.without_signatures();
        let signatures = self.signatures.iter().map(Signed::to_value).collect();
        fields.insert(key::SIGNATURES, Value::Array(signatures));
        Value::Map(fields).to_bytes()
    }

    /// What a signature covers: everything but `firmas`.
    #[must_use]
    pub fn signing_bytes(&self) -> Vec<u8> {
        Value::Map(self.without_signatures()).to_bytes()
    }

    /// What the name is computed over: everything but `firmas` and `objeto`.
    #[must_use]
    pub fn naming_bytes(&self) -> Vec<u8> {
        let mut fields = self.without_signatures();
        fields.remove(&key::OBJECT);
        Value::Map(fields).to_bytes()
    }

    /// The name this operation gives the object it creates.
    ///
    /// Meaningful only on a creation — on any later operation it is the hash of an act that
    /// created nothing, which is why [`Self::names_itself`] is the question worth asking.
    #[must_use]
    pub fn name(&self) -> Name {
        Name::of(&self.naming_bytes())
    }

    /// What this act is called: on its object's chain, in the log, and to anybody citing it.
    ///
    /// **It does not depend on how the act was signed**, and it must not. A signature covers
    /// everything but `firmas`, so the signature bytes are outside what any signature protects —
    /// while a name computed over them would be inside what anybody can change. And they *can* be
    /// changed without a key: an ECDSA signature has two valid forms for one message, either of
    /// which verifies, so anybody who merely saw an act go past could reprint it in the other form.
    /// Named over the signatures, that would be a second act claiming one predecessor: a fork on
    /// somebody else's chain, made by somebody holding nothing and forging nothing.
    ///
    /// This is the same argument [`Self::naming_bytes`] already makes for the name of an **object**,
    /// carried to where an act is named. What travels and what is stored is still all of it — only
    /// what it is *called* leaves out the part that two honest parties can write two ways.
    #[must_use]
    pub fn called(&self) -> Name {
        Name::of(&self.signing_bytes())
    }

    /// Whether this creation's `objeto` is the name its own bytes give it.
    ///
    /// This is the whole promise made checkable: *whoever holds the creation recomputes the
    /// identifier and checks that it matches, **without asking any node***.
    #[must_use]
    pub fn names_itself(&self) -> bool {
        self.object.name() == &self.name()
    }

    /// Whether a reader with this vocabulary can apply this operation.
    ///
    /// # Errors
    ///
    /// [`Unintelligible`] when the payload carries a critical field the reader has no meaning for,
    /// or a value outside a vocabulary declared closed.
    pub fn understood(&self, vocabulary: Vocabulary<'_>) -> Result<(), Unintelligible> {
        field::understood(&self.payload, vocabulary)
    }

    /// Every field but `firmas`, which is what both of the other two byte strings start from.
    fn without_signatures(&self) -> BTreeMap<u64, Value> {
        let previous = self
            .previous
            .as_ref()
            .map_or(Value::Null, |name| Value::Text(name.as_str().to_owned()));

        BTreeMap::from([
            (key::OBJECT, Value::Text(self.object.to_string())),
            (key::PREVIOUS, previous),
            (key::KIND, Value::Uint(self.kind)),
            (key::VERSION, Value::Uint(self.version)),
            (key::ISSUED, Value::Uint(self.issued.number())),
            (key::PAYLOAD, Value::Map(self.payload.clone())),
        ])
    }
}

impl Signed {
    /// A signature as it appears inside `firmas`.
    fn to_value(&self) -> Value {
        Value::Array(vec![
            Value::Text(self.by.to_string()),
            Value::Bytes(self.key.clone()),
            Value::Bytes(self.signature.to_vec()),
        ])
    }
}

/// Read an act back from the bytes it arrived in.
///
/// **The bytes are kept, not replaced by this.** What comes back is what the act *says*; the act
/// itself is still the bytes, and anything that verifies a signature verifies against those. This
/// exists so a node can tell what it is looking at, not so it can start using a tidied copy.
///
/// [`None`] when the value is not an act: a field missing, a field of the wrong shape, or a
/// signature that is not a signer, a key and sixty-four bytes.
#[must_use]
pub fn read(value: &Value) -> Option<Operation> {
    let Value::Map(fields) = value else {
        return None;
    };

    let Value::Text(object) = fields.get(&key::OBJECT)? else {
        return None;
    };
    let previous = match fields.get(&key::PREVIOUS)? {
        Value::Null => None,
        Value::Text(name) => Some(Name::parse(name).ok()?),
        _ => return None,
    };
    let (&Value::Uint(kind), &Value::Uint(version), &Value::Uint(issued)) = (
        fields.get(&key::KIND)?,
        fields.get(&key::VERSION)?,
        fields.get(&key::ISSUED)?,
    ) else {
        return None;
    };
    let Value::Map(payload) = fields.get(&key::PAYLOAD)? else {
        return None;
    };
    let Value::Array(signatures) = fields.get(&key::SIGNATURES)? else {
        return None;
    };

    Some(Operation {
        object: Did::parse(object).ok()?,
        previous,
        kind,
        version,
        issued: Epoch::new(issued),
        payload: payload.clone(),
        signatures: signatures
            .iter()
            .map(read_signature)
            .collect::<Option<_>>()?,
    })
}

/// One element of `firmas`.
fn read_signature(value: &Value) -> Option<Signed> {
    let Value::Array(parts) = value else {
        return None;
    };
    let [Value::Text(by), Value::Bytes(key), Value::Bytes(signature)] = parts.as_slice() else {
        return None;
    };
    Some(Signed {
        by: Did::parse(by).ok()?,
        key: key.clone(),
        signature: (*signature).as_slice().try_into().ok()?,
    })
}

/// A creation operation, whose `objeto` is the name its own bytes give it.
///
/// Built in this order because there is no other order it could be built in: the name comes from
/// the bytes, so the bytes are laid out first with a placeholder, the name computed, and the
/// object filled in. Nothing here signs — signing is the caller's, over
/// [`Operation::signing_bytes`], and this returns an operation with an empty `firmas`.
#[must_use]
pub fn create(
    network: Network,
    kind: u64,
    version: u64,
    issued: Epoch,
    payload: BTreeMap<u64, Value>,
) -> Operation {
    let placeholder = Did::new(network, Name::of(b""));
    let mut operation = Operation {
        object: placeholder,
        previous: None,
        kind,
        version,
        issued,
        payload,
        signatures: Vec::new(),
    };
    operation.object = Did::new(network, operation.name());
    operation
}

#[cfg(test)]
mod tests {
    use super::{Operation, Signed, create, key};
    use crate::cbor::{Value, read};
    use crate::field::{Field, Unintelligible, Vocabulary};
    use crate::identifier::{Did, Name, Network};
    use almena_time::Epoch;
    use std::collections::BTreeMap;

    fn payload() -> BTreeMap<u64, Value> {
        BTreeMap::from([(1, Value::Text("a device".to_owned()))])
    }

    fn creation() -> Operation {
        create(Network::Development, 1, 1, Epoch::GENESIS, payload())
    }

    #[test]
    fn a_creation_names_itself() {
        let operation = creation();
        assert!(operation.names_itself());
        assert!(operation.object.to_string().starts_with("did:almena:dev:z"));
    }

    #[test]
    fn every_form_of_the_operation_is_canonical() {
        let mut operation = creation();
        operation.signatures.push(Signed {
            by: operation.object.clone(),
            key: vec![2; 33],
            signature: [7; 64],
        });
        for bytes in [
            operation.to_bytes(),
            operation.signing_bytes(),
            operation.naming_bytes(),
        ] {
            assert_eq!(almena_cbor::canonical(&bytes), Ok(()));
            assert!(read(&bytes).is_ok());
        }
    }

    #[test]
    fn the_name_does_not_depend_on_who_signed_or_how_many() {
        // If the name depended on the signatures, one act would have several names — and P-256
        // signatures are not even reproducible, so it would have them by accident.
        let bare = creation();
        let mut signed = bare.clone();
        signed.signatures.push(Signed {
            by: bare.object.clone(),
            key: vec![2; 33],
            signature: [1; 64],
        });
        let mut twice = signed.clone();
        twice.signatures.push(Signed {
            by: bare.object.clone(),
            key: vec![2; 33],
            signature: [2; 64],
        });

        assert_eq!(bare.name(), signed.name());
        assert_eq!(bare.name(), twice.name());
        assert_eq!(bare.naming_bytes(), twice.naming_bytes());
    }

    #[test]
    fn the_three_byte_strings_differ_by_exactly_one_field_each() {
        let mut operation = creation();
        operation.signatures.push(Signed {
            by: operation.object.clone(),
            key: vec![2; 33],
            signature: [3; 64],
        });

        let whole = fields(&operation.to_bytes());
        let signing = fields(&operation.signing_bytes());
        let naming = fields(&operation.naming_bytes());

        assert!(whole.contains_key(&key::SIGNATURES));
        assert!(!signing.contains_key(&key::SIGNATURES));
        assert!(signing.contains_key(&key::OBJECT));
        assert!(!naming.contains_key(&key::OBJECT));
        assert_eq!(naming.len(), signing.len() - 1);
    }

    fn fields(bytes: &[u8]) -> BTreeMap<u64, Value> {
        match read(bytes) {
            Ok(Value::Map(fields)) => fields,
            other => panic!("an operation is a map, got {other:?}"),
        }
    }

    #[test]
    fn changing_anything_at_all_changes_the_name() {
        let base = creation();
        let later = create(
            Network::Development,
            1,
            1,
            Epoch::GENESIS
                .plus(almena_time::Epochs(1))
                .expect("no overflow"),
            payload(),
        );
        let other_kind = create(Network::Development, 2, 1, Epoch::GENESIS, payload());
        assert_ne!(base.name(), later.name());
        assert_ne!(base.name(), other_kind.name());
    }

    #[test]
    fn the_network_does_not_change_the_name() {
        // The mark is for people; what actually separates two networks is the genesis hash inside
        // the operation and what nodes say on connecting. A name that changed with the prefix
        // would be a second, weaker claim about the same thing.
        let development = creation();
        let production = create(Network::Production, 1, 1, Epoch::GENESIS, payload());
        assert_eq!(development.name(), production.name());
    }

    #[test]
    fn a_first_operation_writes_prev_as_null_rather_than_leaving_it_out() {
        // One shape per thing: if absence could be written two ways, the same first operation
        // would have two names.
        let bytes = creation().naming_bytes();
        assert_eq!(fields(&bytes).get(&key::PREVIOUS), Some(&Value::Null));
    }

    #[test]
    fn an_unknown_critical_field_makes_it_unintelligible() {
        let fields = [Field::new(1)];
        let mut operation = creation();
        operation.payload.insert(3, Value::Uint(1));
        assert_eq!(
            operation.understood(Vocabulary::of(&fields)),
            Err(Unintelligible::Field(Field::new(3)))
        );
        // And an unknown field that may be ignored does not.
        let mut ignorable = creation();
        ignorable.payload.insert(4, Value::Uint(1));
        assert_eq!(ignorable.understood(Vocabulary::of(&fields)), Ok(()));
    }

    #[test]
    fn an_act_read_back_is_the_act_that_was_written() {
        let mut operation = creation();
        operation.signatures.push(Signed {
            by: operation.object.clone(),
            key: vec![2; 33],
            signature: [0xab; 64],
        });

        let value = read(&operation.to_bytes()).expect("canonical");
        assert_eq!(super::read(&value), Some(operation.clone()));

        // And reading it changes nothing about it: what verifies a signature is still the bytes.
        let again = super::read(&value).expect("an act");
        assert_eq!(again.to_bytes(), operation.to_bytes());
    }

    #[test]
    fn something_that_is_not_an_act_is_not_read_as_one() {
        // Each of these is canonical and none of them is an act.
        let not_a_map = Value::Uint(1);
        assert_eq!(super::read(&not_a_map), None);

        let missing_a_field =
            Value::Map(BTreeMap::from([(key::OBJECT, Value::Text("x".to_owned()))]));
        assert_eq!(super::read(&missing_a_field), None);

        let mut wrong_shape = fields(&creation().to_bytes());
        wrong_shape.insert(key::KIND, Value::Text("one".to_owned()));
        assert_eq!(super::read(&Value::Map(wrong_shape)), None);
    }

    #[test]
    fn a_signature_that_is_not_three_parts_is_not_a_signature() {
        let mut operation = creation();
        operation.signatures.push(Signed {
            by: operation.object.clone(),
            key: vec![2; 33],
            signature: [0xab; 64],
        });
        let mut fields = fields(&operation.to_bytes());
        fields.insert(
            key::SIGNATURES,
            Value::Array(vec![Value::Array(vec![Value::Uint(1)])]),
        );
        assert_eq!(super::read(&Value::Map(fields)), None);
    }

    #[test]
    fn an_object_that_does_not_name_itself_is_caught() {
        let mut lying = creation();
        lying.object = Did::new(Network::Development, Name::of(b"some other operation"));
        assert!(!lying.names_itself());
    }
}
