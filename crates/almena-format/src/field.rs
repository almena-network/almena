//! What a reader has to understand before it may claim to have applied an operation.
//!
//! *Critical* means **"if you do not understand this, you cannot claim to have applied this
//! operation."** Without a mark, adding a field to a known type is a silent disaster: an old node
//! parses the operation, ignores the field it does not know, and concludes the opposite of what it
//! said — a certification with a scope read as a certification without limits.
//!
//! # The mark is the number
//!
//! **An odd field number is critical; an even one may be ignored.** This is CoAP's rule for its
//! options (RFC 7252, section 5.4.1), and it is picked over X.509's per-extension boolean for a
//! reason that matters more here than there: a boolean is a thing that can be stripped. Whoever
//! removes it turns a critical field into an ignorable one and the operation still parses. When
//! the mark *is* the field's name, there is nothing to strip — changing the parity changes which
//! field it is, and an old node then does not recognise it at all.
//!
//! It also costs nothing. The log entry is the universal and therefore expensive part, and a flag
//! per field would be paid for in every copy of every operation forever.
//!
//! # But a mark of fields says nothing about values
//!
//! **A field that ships on day one never exercises its parity**, because every reader knows it.
//! What grows there is the *vocabulary*, and parity does not look at values. Two of the holes in
//! [`crate::holes`] are that shape — a credential's proof type and a proposal's `método` —
//! and without a second mechanism an old reader would take `ciego` for `abierto`, the value it does
//! know, and count a vote it never understood. So a field whose vocabulary will grow is declared
//! **closed**: a value the reader has no meaning for is refused rather than mistaken for a
//! neighbour.
//!
//! # This governs the payload, not the envelope
//!
//! `objeto`, `prev`, `tipo`, `version`, `emitida`, `payload` and `firmas` are fixed and all of them
//! are critical by construction: a reader that does not understand `tipo` understands nothing.
//! Extensions land inside `payload`, which is where these rules earn their keep.

use crate::cbor::Value;
use std::collections::BTreeMap;

/// A field number inside a payload, which carries its own criticality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Field(u64);

/// What a reader knows how to read.
///
/// Two lists rather than one, because the two mechanisms answer different questions: `fields` says
/// which numbers mean something here, and `closed` says which of them will only ever carry a value
/// from a fixed list — and which values those are, *for this reader*. A newer reader knows more
/// values for the same field, which is the whole point.
#[derive(Debug, Clone, Copy)]
pub struct Vocabulary<'a> {
    /// The field numbers this reader has a meaning for.
    pub fields: &'a [Field],
    /// The fields with a closed vocabulary, and the values this reader knows for each.
    pub closed: &'a [(Field, &'a [Value])],
}

/// Why a reader cannot apply an operation.
///
/// Not *invalid*: a node stores and propagates every operation whether it understands it or not,
/// because a fork detector cannot tell an out-of-date node from an attacker. What it may never do
/// is **serve the previous state as if it were current**.
/// Refusing service is allowed; lying is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unintelligible {
    /// A critical field this reader has no meaning for.
    Field(Field),
    /// A value this reader has no meaning for, in a field whose vocabulary is closed.
    Value(Field),
}

impl Field {
    /// A field by its number.
    #[must_use]
    pub const fn new(number: u64) -> Self {
        Self(number)
    }

    /// The number, which is how it appears in the bytes.
    #[must_use]
    pub const fn number(self) -> u64 {
        self.0
    }

    /// Whether an operation carrying it can be applied by someone who does not know it.
    #[must_use]
    pub const fn is_critical(self) -> bool {
        self.0 % 2 == 1
    }
}

impl<'a> Vocabulary<'a> {
    /// A vocabulary of plain fields, none of them closed.
    #[must_use]
    pub const fn of(fields: &'a [Field]) -> Self {
        Self {
            fields,
            closed: &[],
        }
    }

    /// The same, with the closed-vocabulary fields declared.
    #[must_use]
    pub const fn with_closed(fields: &'a [Field], closed: &'a [(Field, &'a [Value])]) -> Self {
        Self { fields, closed }
    }

    /// The values this reader knows for a field, if that field's vocabulary is closed.
    fn values(&self, field: Field) -> Option<&'a [Value]> {
        self.closed
            .iter()
            .find(|(declared, _)| *declared == field)
            .map(|(_, values)| *values)
    }
}

/// Whether a reader with this vocabulary can apply a payload.
///
/// Unknown **even** fields are passed over in silence, which is the point of having two kinds. The
/// first unknown **odd** field stops the reader, and so does the first unknown value in a field
/// declared closed.
///
/// **Nested maps are not walked, and that is deliberate.** A map inside a field is shaped by that
/// field's own schema, and its integer keys may well be *data* — an epoch, a position, a count.
/// Walking them as if they were field numbers would refuse perfectly good operations whose data
/// happened to be odd. Whatever defines a nested shape checks it; this function checks the payload
/// it was given.
///
/// # Errors
///
/// [`Unintelligible`] naming the field, so that a node can say *which* thing it did not understand
/// rather than only that something went over its head.
pub fn understood(
    payload: &BTreeMap<u64, Value>,
    vocabulary: Vocabulary<'_>,
) -> Result<(), Unintelligible> {
    for (&number, value) in payload {
        let field = Field::new(number);
        let known = vocabulary.fields.contains(&field);

        if !known {
            if field.is_critical() {
                return Err(Unintelligible::Field(field));
            }
            continue;
        }

        if vocabulary
            .values(field)
            .is_some_and(|values| !values.contains(value))
        {
            return Err(Unintelligible::Value(field));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Field, Unintelligible, Vocabulary, understood};
    use crate::cbor::Value;
    use std::collections::BTreeMap;

    fn payload(numbers: &[u64]) -> BTreeMap<u64, Value> {
        numbers.iter().map(|&n| (n, Value::Uint(n))).collect()
    }

    const OPEN: Value = Value::Uint(0);
    const BLIND: Value = Value::Uint(1);

    #[test]
    fn odd_is_critical_and_even_is_not() {
        assert!(Field::new(1).is_critical());
        assert!(Field::new(7).is_critical());
        assert!(!Field::new(2).is_critical());
        assert!(!Field::new(0).is_critical());
    }

    #[test]
    fn a_payload_a_reader_knows_is_understood() {
        let fields = [Field::new(1), Field::new(2)];
        assert_eq!(
            understood(&payload(&[1, 2]), Vocabulary::of(&fields)),
            Ok(())
        );
    }

    #[test]
    fn an_unknown_field_that_may_be_ignored_is_ignored() {
        // The whole reason there are two kinds: this is how a reserved hole gets used without
        // breaking the half of the network that has not updated yet.
        let fields = [Field::new(1)];
        assert_eq!(
            understood(&payload(&[1, 4]), Vocabulary::of(&fields)),
            Ok(())
        );
    }

    #[test]
    fn an_unknown_critical_field_stops_the_reader() {
        // And this is the disaster the mark exists to prevent: without it, a reader would apply
        // the operation having quietly dropped the field that changed its meaning.
        let fields = [Field::new(1)];
        assert_eq!(
            understood(&payload(&[1, 3]), Vocabulary::of(&fields)),
            Err(Unintelligible::Field(Field::new(3)))
        );
    }

    #[test]
    fn the_field_it_names_is_the_first_one_in_order() {
        assert_eq!(
            understood(&payload(&[9, 3, 5]), Vocabulary::of(&[])),
            Err(Unintelligible::Field(Field::new(3)))
        );
    }

    #[test]
    fn an_empty_payload_is_understood_by_everyone() {
        assert_eq!(understood(&payload(&[]), Vocabulary::of(&[])), Ok(()));
    }

    #[test]
    fn a_value_outside_a_closed_vocabulary_stops_the_reader() {
        // The second shape a hole takes. The field is known — it shipped on day one — so its
        // parity never fires; what the reader has never seen is the *value*.
        let method = Field::new(1);
        let fields = [method];
        let closed = [(method, &[OPEN][..])];
        let old = Vocabulary::with_closed(&fields, &closed);

        assert_eq!(
            understood(&BTreeMap::from([(1, BLIND)]), old),
            Err(Unintelligible::Value(method))
        );
    }

    #[test]
    fn a_newer_reader_knows_the_new_value_and_applies_it() {
        // The other half: the same operation, a reader that has been updated. This is what makes
        // the blind vote an addition rather than a migration.
        let method = Field::new(1);
        let fields = [method];
        let closed = [(method, &[OPEN, BLIND][..])];
        let new = Vocabulary::with_closed(&fields, &closed);
        assert_eq!(understood(&BTreeMap::from([(1, BLIND)]), new), Ok(()));
    }

    #[test]
    fn without_a_closed_vocabulary_the_new_value_would_pass_for_the_old_one() {
        // Why the second mechanism had to exist. Declared open, the unknown value sails through,
        // and the reader goes on to treat a blind vote as an open one — the same disaster the
        // criticality mark exists to prevent, through the door parity does not watch.
        let fields = [Field::new(1)];
        assert_eq!(
            understood(&BTreeMap::from([(1, BLIND)]), Vocabulary::of(&fields)),
            Ok(())
        );
    }

    #[test]
    fn a_closed_vocabulary_on_an_even_field_is_still_checked() {
        // Closedness is not parity. A field may be safe to *skip* and still be unsafe to
        // *misread*, and a reader that knows the field has already chosen not to skip it.
        let field = Field::new(2);
        let fields = [field];
        let closed = [(field, &[OPEN][..])];
        assert_eq!(
            understood(
                &BTreeMap::from([(2, BLIND)]),
                Vocabulary::with_closed(&fields, &closed)
            ),
            Err(Unintelligible::Value(field))
        );
    }

    #[test]
    fn a_map_inside_a_field_is_not_walked_as_if_its_keys_were_fields() {
        // Deliberate, and worth a test so nobody "fixes" it: the inner keys belong to that
        // field's own schema and may be data. Walking them would refuse an operation for having
        // an odd number in its data.
        let fields = [Field::new(1)];
        let nested = BTreeMap::from([(1, Value::Map(BTreeMap::from([(7, Value::Uint(0))])))]);
        assert_eq!(understood(&nested, Vocabulary::of(&fields)), Ok(()));
    }
}
