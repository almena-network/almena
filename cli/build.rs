//! Reads the catalog directory and writes the table this program looks words up in.
//!
//! Adding a language must not mean touching code. The webview gets that from Vite, which can
//! glob a directory at build time; Rust has no equivalent, and the alternative — a `match`
//! with one arm per language — is exactly the code adding one would mean touching. So the
//! directory is read here, once, and the table is generated.
//!
//! The catalogs are still compiled in: a node on a server has no frontend beside it to read them
//! from, and a catalog that could go missing at run time is a program that could start speechless.

use std::{env, ffi::OsStr, fs, path::PathBuf};

/// Where the catalogs are, relative to this package.
///
/// The same files the webview reads. A second set beside them would agree with the first for
/// about a month.
const LOCALES: &str = "../src/i18n/locales";

/// The language everything else falls back to, which therefore has to be there.
const SOURCE: &str = "en";

fn main() {
    // The list is the directory, so a file added or removed has to rebuild this.
    println!("cargo:rerun-if-changed={LOCALES}");

    let mut tags = catalog_tags();
    tags.sort();

    if !tags.iter().any(|tag| tag == SOURCE) {
        panic!("{LOCALES} has no {SOURCE}.json, and every other language falls back to it");
    }
    if tags.len() < 2 {
        panic!(
            "{LOCALES} holds {} catalog: the platform is multilingual from the first day",
            tags.len()
        );
    }

    // Absolute, because `include_str!` in the generated file resolves against `OUT_DIR` and not
    // against this package. Backslashes are escaped so a Windows path survives being a literal.
    let Some(manifest) = env::var_os("CARGO_MANIFEST_DIR") else {
        panic!("cargo did not say where this package is");
    };
    let directory = PathBuf::from(manifest).join(LOCALES);

    let rows: String = tags
        .iter()
        .map(|tag| {
            let path = directory.join(format!("{tag}.json"));
            let path = path.display().to_string().replace('\\', "\\\\");
            format!("    (\"{tag}\", include_str!(\"{path}\")),\n")
        })
        .collect();

    let table = format!(
        "/// Every catalog in `{LOCALES}`, by the language it is written in.\n\
         ///\n\
         /// Generated from the directory listing by `build.rs`. Adding a language is adding a\n\
         /// file; nothing here is written by hand.\n\
         pub static CATALOGS: &[(&str, &str)] = &[\n{rows}];\n"
    );

    let Some(out) = env::var_os("OUT_DIR") else {
        panic!("cargo did not say where to write");
    };
    let destination = PathBuf::from(out).join("catalogs.rs");

    if let Err(error) = fs::write(&destination, table) {
        panic!("could not write {}: {error}", destination.display());
    }
}

/// The language each `*.json` in the catalog directory is written in.
fn catalog_tags() -> Vec<String> {
    let entries = match fs::read_dir(LOCALES) {
        Ok(entries) => entries,
        Err(error) => panic!("could not read {LOCALES}: {error}"),
    };

    entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension() != Some(OsStr::new("json")) {
                return None;
            }
            // Every file in there also has to rebuild this: a key changed is a word changed.
            println!("cargo:rerun-if-changed={}", path.display());
            Some(path.file_stem()?.to_str()?.to_owned())
        })
        .collect()
}
