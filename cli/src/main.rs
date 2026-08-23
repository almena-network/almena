//! Where Almena the node starts, and nothing else.
//!
//! Everything it does lives in the library beside this file, so that a test can link against
//! it. This is the launch.

fn main() {
    std::process::exit(almena_cli::run().into());
}
