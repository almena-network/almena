//! What this node's interface is, written where a machine can read it.
//!
//! # Why it is here and not generated
//!
//! The interface is a `match` over method and path (`almena_api::parse`), not a table something
//! could walk. Generating this from it would mean turning that match into data for the sake of a
//! document, which is a worse trade than writing the document — the match is what serves every
//! request, and it should stay the shape that is easiest to read while doing so.
//!
//! **What a hand-written document costs is drift**, so it is not left to care: `every_path_is_one`
//! below holds every path here to being one the parser accepts. A route renamed without this file
//! being touched fails the tests rather than being found by whoever believed the document.
//!
//! # It is JSON, and it is the only thing here that is
//!
//! Every answer a node gives about the record is CBOR: it is what the acts are written in and what
//! their signatures are over, and re-encoding one would rename it. This is not an answer about the
//! record — it is a description of a door — and the tools that read a description of a door read
//! JSON. So the one document that is *about* the interface is in the format its readers speak, and
//! everything the interface *serves* stays in the format the record speaks.

/// Where the description is served.
pub const AT: &str = "/openapi.json";

/// Every path the document names, for the test that holds it to the parser.
#[cfg(test)]
const PATHS: &[(&str, &str)] = &[
    ("GET", "/limits"),
    ("GET", "/capacity"),
    ("GET", "/anchor"),
    ("GET", "/catalogue"),
    ("GET", "/object/x"),
    ("GET", "/act/x"),
    ("GET", "/state/did:almena:dev:x"),
    ("GET", "/state/did:almena:dev:x/y"),
    ("GET", "/about/did:almena:dev:x"),
    ("GET", "/inclusion/1/x"),
    ("GET", "/root/1"),
    ("GET", "/list/x"),
    ("GET", "/watching/1"),
    ("GET", "/kept/1"),
];

/// The description itself.
///
/// **Every answer carries the same envelope**, which is what the schema below says once rather than
/// fourteen times: the epoch it was answered in, the root over everything the node had written down
/// at that moment, and what state the answer is in. Whoever reads it works the rest out from the
/// acts inside and checks them on the way — a node hands over materials and never a verdict
/// (`SPECS.md §4.13`).
pub const DOCUMENT: &str = r##"{
  "openapi": "3.1.0",
  "info": {
    "title": "Almena node interface",
    "version": "1",
    "summary": "What one node will answer about the record it keeps.",
    "description": "Every answer is CBOR and carries the same envelope: the epoch it was answered in, the root over everything this node had written down at that moment, and the state of the answer. What comes back about an object is the acts their authors signed, never a state this node composed — whoever asked replays them and checks the signatures on the way. This document is the one thing here served as JSON, because it describes the door rather than the record.",
    "license": { "name": "Apache-2.0" }
  },
  "paths": {
    "/limits": { "get": { "summary": "What this node will and will not do", "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/capacity": { "get": { "summary": "What the network says it is running, and how many nodes speak each version", "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/anchor": { "get": { "summary": "Who this network is anchored on", "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/catalogue": { "get": { "summary": "The names in the catalogue, by what each object is", "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/object/{name}": { "get": { "summary": "What an object is now", "parameters": [ { "$ref": "#/components/parameters/Name" } ], "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/act/{name}": { "get": { "summary": "One act, by the name it is called", "parameters": [ { "$ref": "#/components/parameters/Name" } ], "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/state/{did}": { "get": { "summary": "The acts needed to work out what an object is", "description": "A page, with a cursor where there is more. A caller that folded a page thinking it was the whole chain would land on a state from earlier with nothing saying so.", "parameters": [ { "$ref": "#/components/parameters/Did" } ], "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/state/{did}/{after}": { "get": { "summary": "The next page of an object's acts", "parameters": [ { "$ref": "#/components/parameters/Did" }, { "$ref": "#/components/parameters/After" } ], "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/about/{did}": { "get": { "summary": "What has been said about somebody by somebody else", "parameters": [ { "$ref": "#/components/parameters/Did" } ], "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/inclusion/{epoch}/{name}": { "get": { "summary": "Where an act sits in this node's tree, and the path that proves it", "description": "The epoch is not optional: a path proves an entry against a root of a stated size, and the only roots with this node's name on them are the ones it published at the ends of epochs.", "parameters": [ { "$ref": "#/components/parameters/Epoch" }, { "$ref": "#/components/parameters/Name" } ], "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/root/{epoch}": { "get": { "summary": "What this node signed about an epoch", "parameters": [ { "$ref": "#/components/parameters/Epoch" } ], "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/list/{version}": { "get": { "summary": "The bytes of one status list version, by the hash of those bytes", "description": "Any node will do, because the hash decides. Asking the issuer's own node every time would tell it when and how often its credentials are verified.", "parameters": [ { "name": "version", "in": "path", "required": true, "schema": { "type": "string" } } ], "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/watching/{day}": { "get": { "summary": "What this node was watching on a day", "parameters": [ { "$ref": "#/components/parameters/Day" } ], "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/kept/{day}": { "get": { "summary": "What was found on a day, with the denominator it came from", "parameters": [ { "$ref": "#/components/parameters/Day" } ], "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/acts": { "post": { "summary": "Hand an act over", "description": "The bytes its author signed, unreserialised: the name of an act is the hash of those bytes, and a well-meant tidy-up would rename it. Whether it was taken is the state inside the answer.", "requestBody": { "required": true, "content": { "application/cbor": { "schema": { "type": "string", "format": "binary" } } } }, "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/list": { "post": { "summary": "Hand over the bytes of a status list this node will serve", "description": "Not an act and never checked against a chain: opaque bytes a node holds because the record already names their hash, and it never reads one.", "requestBody": { "required": true, "content": { "application/cbor": { "schema": { "type": "string", "format": "binary" } } } }, "responses": { "200": { "$ref": "#/components/responses/Said" } } } },
    "/post/{to}": { "post": { "summary": "Leave a message for somebody, under the address they gave out", "description": "Only on a node that runs a mailbox, which is what the node itself answers. The address is a relationship's own and not an account's; which of its customers answers to one is the node's question.", "parameters": [ { "name": "to", "in": "path", "required": true, "schema": { "type": "string" } } ], "requestBody": { "required": true, "content": { "application/cbor": { "schema": { "type": "string", "format": "binary" } } } }, "responses": { "200": { "$ref": "#/components/responses/Said" } } } }
  },
  "components": {
    "parameters": {
      "Name": { "name": "name", "in": "path", "required": true, "description": "The hash something is called, written out.", "schema": { "type": "string" } },
      "Did": { "name": "did", "in": "path", "required": true, "description": "An object's identifier, with its network inside it.", "schema": { "type": "string", "example": "did:almena:z…" } },
      "After": { "name": "after", "in": "path", "required": true, "description": "Where the last page stopped.", "schema": { "type": "string" } },
      "Epoch": { "name": "epoch", "in": "path", "required": true, "description": "Whole hours since this network began.", "schema": { "type": "integer", "format": "int64" } },
      "Day": { "name": "day", "in": "path", "required": true, "description": "A UTC day, as a number.", "schema": { "type": "integer", "format": "int64" } }
    },
    "responses": {
      "Said": {
        "description": "An answer, stamped with what this node was at when it gave it.",
        "content": { "application/cbor": { "schema": { "$ref": "#/components/schemas/Said" } } }
      }
    },
    "schemas": {
      "Said": {
        "type": "object",
        "description": "A canonical CBOR map with integer keys. Every answer this interface gives has this shape, so that two answers can be compared and neither has to be read a way of its own.",
        "properties": {
          "1": { "type": "integer", "description": "The epoch it was answered in." },
          "2": { "type": "string", "format": "byte", "description": "The root over everything this node had written down at that moment." },
          "3": { "type": "integer", "description": "What state the answer is in: here, does not exist, malformed, no such question, throttled." },
          "4": { "description": "What was asked for, where there is anything. Its shape is the question's." }
        }
      }
    }
  }
}"##;

#[cfg(test)]
mod tests {
    use super::{DOCUMENT, PATHS};

    /// Every path the document names is a route the interface has.
    ///
    /// **What a hand-written description costs is drift**, and this is what it is bought back with.
    /// A route renamed without this file being touched fails here rather than being found by
    /// somebody who believed the document.
    ///
    /// What is asserted is that the route exists, and not that these particular values are good:
    /// `NoSuchQuestion` is the parser saying *there is no such door*, and `Malformed` is it saying
    /// *there is, and that is not a name*. Only the first is drift. Putting real hashes in a
    /// document's test would be testing the hashes.
    #[test]
    fn every_path_is_one() {
        for (method, path) in PATHS {
            assert!(
                !matches!(
                    almena_api::parse(method, path),
                    Err(almena_api::Unreadable::NoSuchQuestion)
                ),
                "{method} {path} is described and is not a route"
            );
        }
    }

    /// And every path the interface answers is named in the document.
    ///
    /// The other direction, which is the one that catches a route **added** without being written
    /// down — an undocumented door is one nobody outside this repository can find.
    #[test]
    fn every_path_is_described() {
        for (_, path) in PATHS {
            let templated = path
                .replace("did:almena:dev:x/y", "{did}/{after}")
                .replace("did:almena:dev:x", "{did}")
                .replace("/object/x", "/object/{name}")
                .replace("/act/x", "/act/{name}")
                .replace("/list/x", "/list/{version}")
                .replace("/inclusion/1/x", "/inclusion/{epoch}/{name}")
                .replace("/root/1", "/root/{epoch}")
                .replace("/watching/1", "/watching/{day}")
                .replace("/kept/1", "/kept/{day}");
            assert!(
                DOCUMENT.contains(&format!("\"{templated}\"")),
                "{templated} is answered and is not described"
            );
        }
    }

    /// It has to be readable as JSON, which is the whole point of serving it.
    #[test]
    fn it_is_json() {
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(DOCUMENT);
        assert!(parsed.is_ok(), "the description is not JSON");
    }
}
