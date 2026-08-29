//! The attributes Almena publishes itself, and the sources they were copied from.
//!
//! # Why this is data in the repository and not a script somebody ran once
//!
//! The core is what the whole catalogue references (`SPECS.md §9.4`), and every attribute in it is
//! **fixed and copied**: the definition is stored here rather than resolved from anywhere, so an
//! edit somebody else makes to a public schema cannot reinterpret credentials already issued
//! (`SPECS.md §4.3`). That makes the core content with the same standing as the rules — it has to
//! be reviewable, diffable and checkable before anybody signs it, and a list typed into a terminal
//! on the day would be none of the three.
//!
//! So this is the decision, and publishing it is signing what is written here. What the acts say is
//! not settled at publication time; it is settled in review, where two people can read it.
//!
//! # The definitions are Almena's own wording, deliberately
//!
//! Not the source's sentences copied out. What is admitted is a *version of a schema* and what is
//! stored is Almena's rendering of what that version means — which is what makes it answerable to
//! whoever admitted it. The source and the version travel with every attribute so that anybody can
//! go and read the original.
//!
//! # Both languages, and that obligation is Almena's alone
//!
//! `SPECS.md §9.4` puts every language the platform ships in on whoever maintains the core, and
//! English at least on everybody else. It is what keeps the *untranslated* mark rare enough on a
//! consent screen to still mean something, and the cost of it falls where the choice to ship a
//! language was made.

use std::collections::BTreeMap;

use almena_format::cbor::Value;
use almena_format::identifier::{Did, Network};
use almena_format::operation::{Operation, create};
use almena_time::Epoch;

use crate::attribute::{self, Shape};
use crate::kind::Kind;

/// The languages the core is published in.
///
/// The same list a tag is held to, because it is the same obligation and there is no reason for the
/// two to be able to drift apart.
pub const IN_ALL: [&str; 2] = crate::tag::IN_ALL;

/// One place definitions were copied from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Admitted {
    /// What it is called, as its own community calls it.
    pub name: &'static str,
    /// Where it canonically lives.
    pub at: &'static str,
    /// Which version was admitted. **Fixed**, because a version is what makes copying honest.
    pub version: &'static str,
    /// What it is, in each language the platform ships in.
    pub about: &'static [(&'static str, &'static str)],
}

/// The sources Almena has admitted.
///
/// Three, and each of them a published specification with a version anybody can go and read. A
/// source is not a website somebody liked: it is a document that stands still long enough for a
/// definition copied from it to go on meaning what it meant.
pub const SOURCES: &[Admitted] = &[
    Admitted {
        name: "openid-connect-core",
        at: "https://openid.net/specs/openid-connect-core-1_0.html",
        version: "1.0-errata2",
        about: &[
            (
                "en",
                "The standard claims of OpenID Connect Core, which is where most of what an \
                 identity says about a person is already named.",
            ),
            (
                "es",
                "Los claims estándar de OpenID Connect Core, que es donde ya está nombrado casi \
                 todo lo que una identidad dice de una persona.",
            ),
        ],
    },
    Admitted {
        name: "schema.org",
        at: "https://schema.org",
        version: "29.0",
        about: &[
            (
                "en",
                "The vocabulary the web already describes organisations and qualifications in.",
            ),
            (
                "es",
                "El vocabulario con el que la web ya describe organizaciones y titulaciones.",
            ),
        ],
    },
    Admitted {
        name: "iso-18013-5",
        at: "https://www.iso.org/standard/69084.html",
        version: "2021",
        about: &[
            (
                "en",
                "The mobile driving licence standard, which is where asking whether somebody is \
                 old enough — rather than when they were born — is already written down.",
            ),
            (
                "es",
                "La norma del permiso de conducir móvil, donde ya está escrito preguntar si \
                 alguien tiene edad suficiente en lugar de cuándo nació.",
            ),
        ],
    },
];

/// One attribute of the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct One {
    /// The claim name it resolves to, which is what a credential carries.
    pub claim: &'static str,
    /// What kind of value it carries.
    pub shape: Shape,
    /// Which of [`SOURCES`] the definition was copied from, by name.
    pub source: &'static str,
    /// The definition, in the language the source is written in.
    pub definition: &'static str,
    /// Whether it may be asked for as an answer about the value rather than as the value.
    pub predicate: bool,
    /// The label a person reads.
    pub labels: &'static [(&'static str, &'static str)],
    /// What it means exactly, for the person being asked for it.
    pub means: &'static [(&'static str, &'static str)],
}

/// How many attributes the core holds, at the least and at the most.
///
/// **A range and not a number** (`SPECS.md §9.4`). Too few and everybody publishes their own
/// version of a date of birth, which is the fragmentation the core exists to stop; too many and the
/// core has started deciding what an ecosystem may say, which is not Almena's to decide.
pub const HOW_MANY: std::ops::RangeInclusive<usize> = 20..=30;

/// The core.
///
/// In the order they were copied in, which is by source: what a schema names together stays
/// together, and a reader checking one of these against its original reads one document at a time.
pub const CORE: &[One] = &[
    One {
        claim: "given_name",
        shape: Shape::Text,
        source: "openid-connect-core",
        definition: "The given name or first name, which may hold several names.",
        predicate: false,
        labels: &[("en", "First name"), ("es", "Nombre")],
        means: &[
            ("en", "The name you are called by, as it is written down."),
            (
                "es",
                "El nombre por el que te llaman, tal como está escrito.",
            ),
        ],
    },
    One {
        claim: "family_name",
        shape: Shape::Text,
        source: "openid-connect-core",
        definition: "The surname or last name, which may hold several names.",
        predicate: false,
        labels: &[("en", "Surname"), ("es", "Apellidos")],
        means: &[
            ("en", "Your family name or names, as they are written down."),
            ("es", "Tu apellido o apellidos, tal como están escritos."),
        ],
    },
    One {
        claim: "middle_name",
        shape: Shape::Text,
        source: "openid-connect-core",
        definition: "The middle name or names, where the person has any.",
        predicate: false,
        labels: &[("en", "Middle name"), ("es", "Segundo nombre")],
        means: &[
            (
                "en",
                "A name between the first and the family name, where there is one.",
            ),
            (
                "es",
                "Un nombre entre el primero y los apellidos, si lo hay.",
            ),
        ],
    },
    One {
        claim: "name",
        shape: Shape::Text,
        source: "openid-connect-core",
        definition: "The full name, written the way the person's locale writes one.",
        predicate: false,
        labels: &[("en", "Full name"), ("es", "Nombre completo")],
        means: &[
            (
                "en",
                "Your whole name in one line, the way it is usually written.",
            ),
            (
                "es",
                "Tu nombre entero en una línea, como se escribe habitualmente.",
            ),
        ],
    },
    One {
        claim: "preferred_username",
        shape: Shape::Text,
        source: "openid-connect-core",
        definition: "The shorthand name the person chooses to be referred to by.",
        predicate: false,
        labels: &[("en", "Username"), ("es", "Nombre de usuario")],
        means: &[
            (
                "en",
                "A short name you chose, which is not your legal name.",
            ),
            (
                "es",
                "Un nombre corto que elegiste tú, que no es tu nombre legal.",
            ),
        ],
    },
    One {
        claim: "birthdate",
        shape: Shape::Date,
        source: "openid-connect-core",
        definition: "The date of birth, as a calendar date.",
        predicate: false,
        labels: &[("en", "Date of birth"), ("es", "Fecha de nacimiento")],
        means: &[
            (
                "en",
                "The day you were born. Asking for it gives away your exact age.",
            ),
            ("es", "El día que naciste. Pedirlo revela tu edad exacta."),
        ],
    },
    One {
        claim: "gender",
        shape: Shape::Text,
        source: "openid-connect-core",
        definition: "The gender the person states, which is not a closed list.",
        predicate: false,
        labels: &[("en", "Gender"), ("es", "Género")],
        means: &[
            ("en", "The gender you state, in your own words."),
            ("es", "El género que declaras, con tus propias palabras."),
        ],
    },
    One {
        claim: "email",
        shape: Shape::Text,
        source: "openid-connect-core",
        definition: "An email address the person is reachable at.",
        predicate: false,
        labels: &[("en", "Email address"), ("es", "Correo electrónico")],
        means: &[
            ("en", "An address somebody can write to you at."),
            ("es", "Una dirección a la que alguien puede escribirte."),
        ],
    },
    One {
        claim: "email_verified",
        shape: Shape::Boolean,
        source: "openid-connect-core",
        definition: "Whether whoever issued this checked that the address reaches the person.",
        predicate: true,
        labels: &[("en", "Email checked"), ("es", "Correo comprobado")],
        means: &[
            (
                "en",
                "Whether whoever issued this checked the address reaches you.",
            ),
            (
                "es",
                "Si quien lo emitió comprobó que esa dirección llega a ti.",
            ),
        ],
    },
    One {
        claim: "phone_number",
        shape: Shape::Text,
        source: "openid-connect-core",
        definition: "A telephone number the person is reachable at, in E.164 form.",
        predicate: false,
        labels: &[("en", "Telephone number"), ("es", "Teléfono")],
        means: &[
            ("en", "A number somebody can call or write to you on."),
            (
                "es",
                "Un número al que alguien puede llamarte o escribirte.",
            ),
        ],
    },
    One {
        claim: "phone_number_verified",
        shape: Shape::Boolean,
        source: "openid-connect-core",
        definition: "Whether whoever issued this checked that the number reaches the person.",
        predicate: true,
        labels: &[("en", "Telephone checked"), ("es", "Teléfono comprobado")],
        means: &[
            (
                "en",
                "Whether whoever issued this checked the number reaches you.",
            ),
            (
                "es",
                "Si quien lo emitió comprobó que ese número llega a ti.",
            ),
        ],
    },
    One {
        claim: "address",
        shape: Shape::Text,
        source: "openid-connect-core",
        definition: "The postal address, written out as it would go on an envelope.",
        predicate: false,
        labels: &[("en", "Postal address"), ("es", "Dirección postal")],
        means: &[
            ("en", "Where you live or receive post, written out in full."),
            ("es", "Dónde vives o recibes correo, escrito entero."),
        ],
    },
    One {
        claim: "locale",
        shape: Shape::Text,
        source: "openid-connect-core",
        definition: "The language and region the person prefers to be addressed in.",
        predicate: false,
        labels: &[("en", "Language"), ("es", "Idioma")],
        means: &[
            ("en", "The language you prefer to be written to in."),
            ("es", "El idioma en el que prefieres que te escriban."),
        ],
    },
    One {
        claim: "zoneinfo",
        shape: Shape::Text,
        source: "openid-connect-core",
        definition: "The time zone the person is in, by its name in the IANA database.",
        predicate: false,
        labels: &[("en", "Time zone"), ("es", "Zona horaria")],
        means: &[
            ("en", "Which time zone you are in."),
            ("es", "En qué zona horaria estás."),
        ],
    },
    One {
        claim: "picture",
        shape: Shape::Text,
        source: "openid-connect-core",
        definition: "Where a photograph of the person is, as a URL.",
        predicate: false,
        labels: &[("en", "Photograph"), ("es", "Fotografía")],
        means: &[
            ("en", "Where a photograph of you is kept."),
            ("es", "Dónde está guardada una fotografía tuya."),
        ],
    },
    One {
        claim: "website",
        shape: Shape::Text,
        source: "openid-connect-core",
        definition: "A page about the person, as a URL.",
        predicate: false,
        labels: &[("en", "Website"), ("es", "Sitio web")],
        means: &[("en", "A page about you."), ("es", "Una página sobre ti.")],
    },
    One {
        claim: "nationality",
        shape: Shape::Text,
        source: "schema.org",
        definition: "The nationality of a person, named as a country.",
        predicate: false,
        labels: &[("en", "Nationality"), ("es", "Nacionalidad")],
        means: &[
            ("en", "Which country's national you are."),
            ("es", "De qué país eres nacional."),
        ],
    },
    One {
        claim: "jobTitle",
        shape: Shape::Text,
        source: "schema.org",
        definition: "The title a person holds in an organisation.",
        predicate: false,
        labels: &[("en", "Job title"), ("es", "Puesto")],
        means: &[
            ("en", "What your position is called where you work."),
            ("es", "Cómo se llama tu puesto donde trabajas."),
        ],
    },
    One {
        claim: "worksFor",
        shape: Shape::Text,
        source: "schema.org",
        definition: "The organisation a person works for.",
        predicate: false,
        labels: &[("en", "Employer"), ("es", "Empleador")],
        means: &[("en", "Who you work for."), ("es", "Para quién trabajas.")],
    },
    One {
        claim: "alumniOf",
        shape: Shape::Text,
        source: "schema.org",
        definition: "An institution a person studied at.",
        predicate: false,
        labels: &[("en", "Studied at"), ("es", "Estudiaste en")],
        means: &[("en", "Where you studied."), ("es", "Dónde estudiaste.")],
    },
    One {
        claim: "hasCredential",
        shape: Shape::Text,
        source: "schema.org",
        definition: "A qualification, certification or award a person holds.",
        predicate: false,
        labels: &[("en", "Qualification"), ("es", "Titulación")],
        means: &[
            ("en", "A qualification you hold."),
            ("es", "Una titulación que tienes."),
        ],
    },
    One {
        claim: "taxID",
        shape: Shape::Text,
        source: "schema.org",
        definition: "The identifier a tax authority knows the party by.",
        predicate: false,
        labels: &[("en", "Tax identifier"), ("es", "Identificador fiscal")],
        means: &[
            ("en", "The number the tax authority knows you by."),
            ("es", "El número por el que te conoce Hacienda."),
        ],
    },
    One {
        claim: "vatID",
        shape: Shape::Text,
        source: "schema.org",
        definition: "The value-added tax identifier of an organisation.",
        predicate: false,
        labels: &[("en", "VAT number"), ("es", "Número de IVA")],
        means: &[
            ("en", "The VAT number of an organisation."),
            ("es", "El número de IVA de una organización."),
        ],
    },
    One {
        claim: "addressCountry",
        shape: Shape::Text,
        source: "schema.org",
        definition: "The country of an address, by its two-letter code.",
        predicate: false,
        labels: &[("en", "Country"), ("es", "País")],
        means: &[
            (
                "en",
                "Which country your address is in — not the whole address.",
            ),
            (
                "es",
                "En qué país está tu dirección — no la dirección entera.",
            ),
        ],
    },
    One {
        claim: "postalCode",
        shape: Shape::Text,
        source: "schema.org",
        definition: "The postal code of an address.",
        predicate: false,
        labels: &[("en", "Postcode"), ("es", "Código postal")],
        means: &[
            (
                "en",
                "The postcode of where you live — not the whole address.",
            ),
            (
                "es",
                "El código postal de donde vives — no la dirección entera.",
            ),
        ],
    },
    One {
        claim: "age_over_16",
        shape: Shape::Boolean,
        source: "iso-18013-5",
        definition: "Whether the person had reached sixteen at the moment this was answered.",
        predicate: true,
        labels: &[("en", "Sixteen or over"), ("es", "Dieciséis o más")],
        means: &[
            (
                "en",
                "Whether you are sixteen or over. It does not give your age.",
            ),
            ("es", "Si tienes dieciséis o más. No dice tu edad."),
        ],
    },
    One {
        claim: "age_over_18",
        shape: Shape::Boolean,
        source: "iso-18013-5",
        definition: "Whether the person had reached eighteen at the moment this was answered.",
        predicate: true,
        labels: &[("en", "Eighteen or over"), ("es", "Dieciocho o más")],
        means: &[
            (
                "en",
                "Whether you are eighteen or over. It does not give your age.",
            ),
            ("es", "Si tienes dieciocho o más. No dice tu edad."),
        ],
    },
    One {
        claim: "age_over_21",
        shape: Shape::Boolean,
        source: "iso-18013-5",
        definition: "Whether the person had reached twenty-one at the moment this was answered.",
        predicate: true,
        labels: &[("en", "Twenty-one or over"), ("es", "Veintiuno o más")],
        means: &[
            (
                "en",
                "Whether you are twenty-one or over. It does not give your age.",
            ),
            ("es", "Si tienes veintiuno o más. No dice tu edad."),
        ],
    },
    One {
        claim: "age_over_65",
        shape: Shape::Boolean,
        source: "iso-18013-5",
        definition: "Whether the person had reached sixty-five at the moment this was answered.",
        predicate: true,
        labels: &[
            ("en", "Sixty-five or over"),
            ("es", "Sesenta y cinco o más"),
        ],
        means: &[
            (
                "en",
                "Whether you are sixty-five or over. It does not give your age.",
            ),
            ("es", "Si tienes sesenta y cinco o más. No dice tu edad."),
        ],
    },
];

/// One purpose a request may be classified under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Purpose {
    /// What it is called, which is what a template names.
    pub name: &'static str,
    /// The label a person reads.
    pub labels: &'static [(&'static str, &'static str)],
}

/// How many purposes the closed list holds, at the most.
///
/// **Closed and short, and the shortness is the mechanism** (`SPECS.md §9.4`). A long list is an
/// open list wearing a limit: with enough headings, everybody finds one nobody else is under, and a
/// purpose nobody shares is a purpose declared so as to be compared with nobody.
pub const PURPOSES_AT_MOST: usize = 12;

/// The closed list of what a request may be for.
///
/// Broad on purpose. These are not what a request *does* — they are what somebody is being asked
/// for it *for*, at the coarseness where two organisations doing the same errand land under the
/// same heading and can be read side by side.
pub const PURPOSES: &[Purpose] = &[
    Purpose {
        name: "age-verification",
        labels: &[("en", "Checking an age"), ("es", "Comprobar una edad")],
    },
    Purpose {
        name: "identity-verification",
        labels: &[
            ("en", "Checking who somebody is"),
            ("es", "Comprobar quién es alguien"),
        ],
    },
    Purpose {
        name: "proof-of-address",
        labels: &[
            ("en", "Proving where somebody lives"),
            ("es", "Acreditar dónde vive alguien"),
        ],
    },
    Purpose {
        name: "proof-of-qualification",
        labels: &[
            ("en", "Proving a qualification"),
            ("es", "Acreditar una titulación"),
        ],
    },
    Purpose {
        name: "proof-of-employment",
        labels: &[("en", "Proving employment"), ("es", "Acreditar un empleo")],
    },
    Purpose {
        name: "account-opening",
        labels: &[("en", "Opening an account"), ("es", "Abrir una cuenta")],
    },
    Purpose {
        name: "access-control",
        labels: &[
            ("en", "Letting somebody in"),
            ("es", "Dar acceso a alguien"),
        ],
    },
    Purpose {
        name: "membership",
        labels: &[
            ("en", "Belonging to something"),
            ("es", "Pertenecer a algo"),
        ],
    },
    Purpose {
        name: "tax-and-billing",
        labels: &[
            ("en", "Tax and invoicing"),
            ("es", "Impuestos y facturación"),
        ],
    },
    Purpose {
        name: "regulatory-compliance",
        labels: &[
            ("en", "Meeting an obligation in law"),
            ("es", "Cumplir una obligación legal"),
        ],
    },
];

/// The act that admits one source, unsigned.
///
/// **Unsigned, because signing is somebody's.** What is written here is the decision; putting
/// Almena Government's name on it is a separate act taken with a key that is not in this repository.
#[must_use]
pub fn admitting(source: &Admitted, by: &Did, network: Network, at: Epoch) -> Operation {
    use crate::source::field;
    create(
        network,
        Kind::SOURCE_ADMIT.number(),
        1,
        at,
        BTreeMap::from([
            (field::NAME, Value::Text(source.name.to_owned())),
            (field::ABOUT, said(source.about)),
            (field::AT, Value::Text(source.at.to_owned())),
            (field::VERSION, Value::Text(source.version.to_owned())),
            (field::BY, Value::Text(by.to_string())),
        ]),
    )
}

/// The act that publishes one attribute of the core, unsigned.
///
/// `from` is the identifier the source was given when it was admitted — which is why the sources go
/// first and cannot be published in the same breath: an attribute names the source it copied from,
/// and a source is named by the act that admitted it (`SPECS.md §9.4`).
#[must_use]
pub fn publishing(one: &One, from: &Did, by: &Did, at: Epoch) -> Operation {
    use crate::attribute::field;
    let mut payload = BTreeMap::from([
        (field::CLAIM, Value::Text(one.claim.to_owned())),
        (field::MEANS, said(one.means)),
        (field::TYPE, Value::Uint(one.shape.number())),
        (field::SOURCE, Value::Text(from.to_string())),
        (field::DEFINITION, Value::Text(one.definition.to_owned())),
        (field::LABELS, said(one.labels)),
        (field::BY, Value::Text(by.to_string())),
    ]);
    if one.predicate {
        payload.insert(field::PREDICATE, Value::Uint(1));
    }
    create(
        from.network(),
        Kind::ATTRIBUTE_PUBLISH.number(),
        1,
        at,
        payload,
    )
}

/// The act that adds one purpose to the closed list, unsigned.
#[must_use]
pub fn adding(purpose: &Purpose, by: &Did, network: Network, at: Epoch) -> Operation {
    use crate::tag::field;
    create(
        network,
        Kind::TAG_ADD.number(),
        1,
        at,
        BTreeMap::from([
            (field::NAME, Value::Text(purpose.name.to_owned())),
            (field::LABELS, said(purpose.labels)),
            (field::BY, Value::Text(by.to_string())),
        ]),
    )
}

/// Something written in several languages, in the one order it may be written in.
fn said(written: &[(&str, &str)]) -> Value {
    let ordered: BTreeMap<String, String> = written
        .iter()
        .map(|(tag, what)| ((*tag).to_owned(), (*what).to_owned()))
        .collect();
    attribute::carried(&ordered)
}

#[cfg(test)]
mod tests {
    use super::{
        CORE, HOW_MANY, IN_ALL, PURPOSES, PURPOSES_AT_MOST, SOURCES, adding, admitting, publishing,
    };
    use crate::attribute;
    use almena_format::identifier::{Did, Name, Network};
    use almena_time::Epoch;
    use std::collections::BTreeSet;

    fn almena() -> Did {
        Did::new(Network::Development, Name::of(b"almena government"))
    }

    fn at() -> Epoch {
        Epoch::new(100)
    }

    #[test]
    fn the_core_is_the_size_it_was_meant_to_be() {
        // Too few and everybody publishes their own date of birth; too many and Almena has started
        // deciding what an ecosystem may say.
        assert!(
            HOW_MANY.contains(&CORE.len()),
            "{} attributes, and the core is {HOW_MANY:?}",
            CORE.len()
        );
    }

    #[test]
    fn every_attribute_of_the_core_reads_in_every_language_the_platform_ships_in() {
        // **The one translation obligation there is, and it falls here** (`SPECS.md §9.4`). It is
        // what keeps the *untranslated* mark rare enough on a consent screen to still be read.
        for one in CORE {
            for language in IN_ALL {
                assert!(
                    one.labels.iter().any(|(tag, _)| *tag == language),
                    "{} has no label in {language}",
                    one.claim
                );
                assert!(
                    one.means.iter().any(|(tag, _)| *tag == language),
                    "{} says what it means in no {language}",
                    one.claim
                );
            }
        }
        for source in SOURCES {
            for language in IN_ALL {
                assert!(
                    source.about.iter().any(|(tag, _)| *tag == language),
                    "{} says what it is in no {language}",
                    source.name
                );
            }
        }
    }

    #[test]
    fn one_claim_is_one_attribute_and_it_comes_from_a_source_that_was_admitted() {
        // Two attributes with one claim name would be two things a credential cannot tell apart,
        // and a definition copied from a source nobody admitted is one copied from anywhere at all.
        let mut seen = BTreeSet::new();
        let admitted: BTreeSet<&str> = SOURCES.iter().map(|source| source.name).collect();
        for one in CORE {
            assert!(seen.insert(one.claim), "{} is in the core twice", one.claim);
            assert!(
                admitted.contains(one.source),
                "{} was copied from {}, which nobody admitted",
                one.claim,
                one.source
            );
        }
    }

    #[test]
    fn the_core_is_publishable_by_the_rules_that_govern_it() {
        // **The whole point of the core being data.** What is written above is what gets signed, so
        // whether it is admissible is a thing this repository can answer before anybody signs it —
        // rather than something a node says at the moment somebody publishes.
        let almena = almena();
        for source in SOURCES {
            let act = admitting(source, &almena, Network::Development, at());
            let held = crate::source::born(&act).expect("a source");
            assert_eq!(held.name, source.name);
            assert_eq!(held.by, almena);
        }

        let from = admitting(&SOURCES[0], &almena, Network::Development, at()).object;
        for one in CORE {
            let act = publishing(one, &from, &almena, at());
            let held = attribute::born(&act).expect("an attribute");
            assert_eq!(held.claim, one.claim);
            assert_eq!(held.predicate, one.predicate);
            assert_eq!(held.source, *from.name());
            assert_eq!(held.labels.len(), IN_ALL.len());
            // A predicate may only be asked of something that says it answers one, and a date of
            // birth is the case the rule exists for: an age-restricted site has no business
            // knowing it.
            assert!(!held.predicate || held.shape == attribute::Shape::Boolean);
        }
    }

    #[test]
    fn the_closed_list_of_purposes_is_short_and_reads_in_both_languages() {
        // **The shortness is the mechanism.** With enough headings everybody finds one nobody else
        // is under, which is an open list wearing a limit.
        assert!(PURPOSES.len() <= PURPOSES_AT_MOST);
        let almena = almena();
        let mut seen = BTreeSet::new();
        for purpose in PURPOSES {
            assert!(
                seen.insert(purpose.name),
                "{} is listed twice",
                purpose.name
            );
            let act = adding(purpose, &almena, Network::Development, at());
            let held = crate::tag::born(&act).expect("a purpose");
            assert_eq!(held.name, purpose.name);
            assert_eq!(held.labels.len(), IN_ALL.len());
        }
    }

    #[test]
    fn asking_whether_somebody_is_old_enough_is_in_the_core_beside_the_date_of_birth() {
        // **What makes the choice visible in the catalogue.** Without both published, a verifier
        // asking for a date of birth could say there was nothing else to ask for.
        assert!(
            CORE.iter()
                .any(|one| one.claim == "birthdate" && !one.predicate)
        );
        assert!(
            CORE.iter()
                .any(|one| one.claim == "age_over_18" && one.predicate)
        );
    }
}
