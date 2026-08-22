//! Tauri's build step: it generates the context the application is compiled against.
//!
//! Everything it produces comes from `tauri.conf.json` and the `capabilities` directory, so
//! this file has nothing of its own to say and should stay that way — a build script is the
//! one place where a mistake is hardest to see from the code it affects.

fn main() {
    tauri_build::build()
}
