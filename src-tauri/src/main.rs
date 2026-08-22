//! Where the windowed application starts, and nothing else.
//!
//! Everything it does lives in the library beside this file, so that a test can link against
//! it. This is the launch, and the one attribute that keeps Windows from opening a console
//! behind the window.

// Prevents an additional console window on Windows in release. Do not remove.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    almena_app_lib::run()
}
