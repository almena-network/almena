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
//! neighbour. Where such a field carries a list, each of its entries is one of its values and each
//! is checked — a field naming the four things a node offers is carrying four values, not one.
//!
//! # Numbers belong to the kind, except above a line
//!
//! Which field is which is settled per kind of act: whoever opens a payload already knows the
//! `tipo`, because it travels in the same envelope, so two kinds may spend the number `1` on
//! entirely different things and nobody is confused.
//!
//! That leaves nowhere to put a field that means the same thing **whatever kind carries it**, and
//! there is at least one — a summary of the state an act leaves behind can ride on any act there
//! is. Writing it as an exception per kind would make it as many coincidences as there are kinds,
//! which every future kind would then have to keep repeating. So the space is split instead:
//! **below [`COMMON`] a number belongs to the kind; from it upwards a number means one thing
//! everywhere.**
//!
//! The line sits far above any per-kind space that will ever exist — an act with fifty fields is
//! not an act, it is a document — so a kind cannot collide with what is above it by growing. And it
//! is a line rather than a list of reserved numbers: reserving numbers ahead of time would fix the
//! shape of fields whose contents nobody has designed.
//!
//! # This governs the payload, not the envelope
//!
//! `objeto`, `prev`, `tipo`, `version`, `emitida`, `payload` and `firmas` are fixed and all of them
//! are critical by construction: a reader that does not understand `tipo` understands nothing.
//! Extensions land inside `payload`, which is where these rules earn their keep.

use crate::cbor::Value;
use std::collections::BTreeMap;

/// The first field number that means the same thing whatever kind of act carries it.
///
/// Below it a number belongs to the kind, and two kinds may spend one number on different things
/// because a reader always knows the kind before it opens the payload. From here upwards a number
/// means one thing everywhere, so a reader that knows the number needs to know nothing else.
pub const COMMON: u64 = 100;

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

    /// Whether this number means the same thing whatever kind of act carries it.
    #[must_use]
    pub const fn is_common(self) -> bool {
        self.0 >= COMMON
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

        if let Some(known) = vocabulary.values(field)
            && !all_known(value, known)
        {
            return Err(Unintelligible::Value(field));
        }
    }
    Ok(())
}

/// Whether every value a closed field carries is one this reader knows.
///
/// **A closed field that carries a list is carrying several of its own values**, and each is one the
/// reader either knows or does not — a node saying which four things it offers, say. Checking only
/// the list as a whole would mean no list ever matched, so the mechanism would either reject
/// everything or be quietly switched off for exactly the fields that most need it.
///
/// A map is not walked, and that is a different case on purpose: the keys of a map inside a field
/// are shaped by that field's own schema and may well be data — an epoch, a position, a count — so
/// reading them as values of this vocabulary would refuse perfectly good acts.
fn all_known(value: &Value, known: &[Value]) -> bool {
    match value {
        Value::Array(several) => several.iter().all(|one| known.contains(one)),
        one => known.contains(one),
    }
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
    fn a_number_below_the_line_belongs_to_its_kind_and_one_above_it_to_everybody() {
        // Two kinds may both spend `1`, because a reader knows the kind before it opens the
        // payload. A field that means the same thing on every kind has nowhere to live below the
        // line, and every kind would otherwise have to remember to avoid it for ever.
        assert!(!Field::new(1).is_common());
        assert!(!Field::new(99).is_common());
        assert!(Field::new(super::COMMON).is_common());
        assert!(Field::new(1_000).is_common());
    }

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

    #[test]
    fn every_entry_of_a_closed_list_is_one_of_its_values() {
        // A closed field carrying a list is carrying several of its own values. Checking only the
        // list as a whole would mean no list ever matched, and the mechanism would be switched off
        // for exactly the fields that most need it.
        const FIELDS: &[Field] = &[Field::new(3)];
        const KNOWN: &[(Field, &[Value])] = &[(Field::new(3), &[OPEN, BLIND])];
        let closed = Vocabulary::with_closed(FIELDS, KNOWN);

        let all_known = BTreeMap::from([(3, Value::Array(vec![OPEN, BLIND]))]);
        assert_eq!(understood(&all_known, closed), Ok(()));

        let one_is_not = BTreeMap::from([(3, Value::Array(vec![OPEN, Value::Uint(9_999)]))]);
        assert_eq!(
            understood(&one_is_not, closed),
            Err(Unintelligible::Value(Field::new(3))),
            "and one unknown among known ones is still unknown"
        );

        let empty = BTreeMap::from([(3, Value::Array(Vec::new()))]);
        assert_eq!(
            understood(&empty, closed),
            Ok(()),
            "nothing said is nothing this reader failed to understand"
        );
    }
}
