//! Tauri's path resolver and `almena-paths` must answer the same question the same way.
//!
//! This repository builds two programs and they put things in the same kinds of place. The
//! windowed application asks Tauri; the CLI has no Tauri in it and asks `almena-paths`. Both
//! compute `dirs::<purpose>_dir()` joined to a bundle
//! identifier, so they agree **by construction** — and this file is what turns that from a
//! sentence in a document into something a build can refuse.
//!
//! It lives here rather than in `almena-paths` because only this side can hold a real Tauri
//! resolver. `almena-paths` is a **dev-dependency** of this crate for exactly this reason:
//! nothing in the application links it, which is what keeps `task isolation` meaningful.
//!
//! The identifiers of the two programs differ on purpose — `network.almena.desktop` and
//! `network.almena.cli`. Separate directories mean separate keys, so a machine running both is
//! two nodes rather than one. Both sides are handed the same one here: what is under test is
//! the computation, not the name.

use almena_paths::Paths;
use tauri::Manager as _;

/// Every purpose the two resolvers both answer, asked of both.
#[test]
fn the_two_resolvers_agree_on_every_purpose() {
    let app = tauri::test::mock_app();
    let tauri_paths = app.path();
    let identifier = &app.config().identifier;
    let ours = Paths::for_application(identifier);

    let pairs: [(&str, std::path::PathBuf, std::path::PathBuf); 4] = [
        (
            "application data",
            tauri_paths
                .app_local_data_dir()
                .expect("Tauri resolves application data"),
            ours.application_data()
                .expect("we resolve application data"),
        ),
        (
            "configuration",
            tauri_paths
                .app_config_dir()
                .expect("Tauri resolves configuration"),
            ours.configuration().expect("we resolve configuration"),
        ),
        (
            "cache",
            tauri_paths.app_cache_dir().expect("Tauri resolves cache"),
            ours.cache().expect("we resolve cache"),
        ),
        (
            "logs",
            tauri_paths.app_log_dir().expect("Tauri resolves logs"),
            ours.logs().expect("we resolve logs"),
        ),
    ];

    for (purpose, theirs, ours) in pairs {
        assert_eq!(
            theirs, ours,
            "the two resolvers disagree about where {purpose} goes"
        );
    }
}

/// The agreement is about the computation, so a different name must produce a different place.
///
/// Without this, a resolver that ignored the identifier entirely would pass the test above.
#[test]
fn the_identifier_is_what_makes_two_programs_differ() {
    let desktop = Paths::for_application("network.almena.desktop");
    let cli = Paths::for_application("network.almena.cli");

    assert_ne!(
        desktop.application_data().expect("a home directory"),
        cli.application_data().expect("a home directory")
    );
}
