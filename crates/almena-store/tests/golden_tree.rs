//! The roots every implementation of this tree must agree on, byte for byte.
//!
//! A node's tree looks like its own business, and it is not: **nodes sign each other's roots**,
//! and what that catches is two incompatible roots from one node for one epoch. Two honest
//! implementations that build the tree a shade differently would accuse each other of exactly the
//! misconduct the mechanism exists to detect — so the shape of the tree is a contract, and this is
//! the corpus that holds it.
//!
//! The entries are `entry 0`, `entry 1`, … and the sizes are the ones where the shape changes:
//! powers of two, one past them, one short of them, and the empty tree.
//!
//! These were checked against a second implementation written from RFC 6962's own description
//! rather than from this one, and the two agreed at every size. The empty tree's root is the
//! SHA-256 of nothing, which is a value anybody can recognise on sight.

// A corpus that cannot be read is a failing test, which is exactly what a panic here is.
#![allow(clippy::expect_used, reason = "the whole of this file is a test")]

use almena_store::tree::{Tree, included};

/// Every size, and the root over that many entries.
const ROOTS: &[(usize, &str)] = &[
    (
        0,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ),
    (
        1,
        "773885a613489e24ce2cf76199d6a423f042e4bbf12d7eecee912ef276c65701",
    ),
    (
        2,
        "5a47662fd8a317d96049a3f9f47c55dc67ca66051baa3683dbb19b2fe09a07b0",
    ),
    (
        3,
        "94fbd0dd836f50301692e6d0eade728ee19ec52bfff1606ed807c8575d5aaa19",
    ),
    (
        4,
        "9799f307517ef517c2205df9b67762bf34756b20099fb7dfcce76bcebd273b2e",
    ),
    (
        5,
        "7caa345dbd892a66454d6c6512ea3c3ea3f0d3ec21be3fc2e2375705fd38f672",
    ),
    (
        7,
        "98c97f0ba3175cd08b031dd084b9dc4e649b64d1a28e6ea694646503173ab587",
    ),
    (
        8,
        "b69732ce5c914162ca9accfaf4e95d3f9874d68805437088aaff184a4db96773",
    ),
    (
        9,
        "240d9bb6a55f0cb375b5d008251e7145458b7b1adc8463ab8fef76fd14f75f6b",
    ),
    (
        16,
        "c71e4e22f9da1d6aa1121e96ed7e085765ac67b45568326a8ff8b7c8dde23a5f",
    ),
    (
        17,
        "8dd8d5c55caa45c3b95982fc165e5edd722e7923ef190bb29d2dd4ec1184b9ef",
    ),
    (
        100,
        "106a6d1594d73248c9634639896cf8a76d2c3418f8fb8b4e24386025a27f2472",
    ),
];

/// A tree holding `size` entries, and the entries themselves.
fn built(size: usize) -> (Tree, Vec<Vec<u8>>) {
    let entries: Vec<Vec<u8>> = (0..size)
        .map(|n| format!("entry {n}").into_bytes())
        .collect();
    let mut tree = Tree::new();
    for entry in &entries {
        tree.append(entry);
    }
    (tree, entries)
}

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn the_roots_are_what_the_corpus_says() {
    for &(size, expected) in ROOTS {
        let (tree, _) = built(size);
        assert_eq!(hex_of(tree.root().bytes()), expected, "size {size}");
    }
}

#[test]
fn the_empty_tree_is_the_hash_of_nothing() {
    // Worth its own line because it is the one value in the corpus a reader can check without
    // running anything: it is the SHA-256 of the empty input, and nothing else.
    assert_eq!(
        hex_of(Tree::new().root().bytes()),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn every_entry_proves_itself_against_the_root_in_the_corpus() {
    // The roots agreeing is half of it. An auditor holds an entry and a path, and what they get
    // has to be the root everybody else already has.
    for &(size, expected) in ROOTS {
        let (tree, entries) = built(size);
        let root = tree.root();
        assert_eq!(hex_of(root.bytes()), expected);

        for (index, entry) in entries.iter().enumerate() {
            let path = tree.inclusion(index).expect("it is in there");
            assert!(
                included(entry, index, size, &path, &root),
                "size {size}, index {index}"
            );
        }
    }
}
