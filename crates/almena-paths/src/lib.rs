//! Where an Almena program keeps things, for the programs that have no Tauri to ask.
//!
//! No location is assembled out of `$HOME`, `%APPDATA%` or `~/Library` anywhere in this
//! repository: a person has to be able to find, back up and delete what an application keeps
//! using the conventions of their own operating system, so every path comes from a resolver.
//! The windowed application has one: Tauri's. A program with no Tauri in it has none, and this
//! is it.
//!
//! # It is not an approximation of Tauri's resolver
//!
//! Tauri's desktop resolver is `dirs::<purpose>_dir()` joined to the bundle identifier, with
//! one special case for logs on macOS, and it depends on `dirs` 6. This crate calls the same
//! crate at the same major version, so the two agree **by construction**. That is not left to
//! be believed: `almena-app` takes this crate as a dev-dependency and a test there hands both
//! resolvers the same identifier and fails if any pair of answers differs.
//!
//! # A caller never branches on the platform
//!
//! The resolver already differs per platform; the code calling it does not. [`Paths::runtime`]
//! is where that earns its keep — `$XDG_RUNTIME_DIR` exists on Linux and nowhere else, so this
//! crate falls back to the temporary directory rather than making every caller know that.
//!
//! # It is handed a name rather than knowing one
//!
//! This repository builds two programs and they are two applications: `network.almena.desktop`
//! and `network.almena.cli`. Neither is this crate's to know. What that costs — a machine
//! running both is two nodes — is the decision and not an accident of this file: separate
//! directories are separate keys and so separate identities, the network has no opinion about
//! two participants that happen to share hardware, and either program can be uninstalled
//! without reaching into the other's data.

use std::path::PathBuf;

/// Why a directory could not be named.
///
/// One variant, because there is one reason: the platform did not tell us where the user's
/// home is. A program that gets this cannot store anything anywhere, which is worth saying
/// as its own error rather than as an absent path somebody treats as "use the current
/// directory".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoHomeDirectory;

impl std::fmt::Display for NoHomeDirectory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("no_home_directory")
    }
}

impl std::error::Error for NoHomeDirectory {}

/// The directories one Almena program keeps things in.
///
/// Built from the name that program calls itself — its bundle identifier — which is the only
/// thing that differs between two of these.
#[derive(Debug, Clone)]
pub struct Paths {
    /// The bundle identifier, which is the name of the per-application directory on every
    /// platform.
    identifier: String,
}

impl Paths {
    /// The directories belonging to the application called `identifier`.
    ///
    /// `identifier` is a bundle identifier in reverse-DNS form — `network.almena.cli`. It is
    /// the name of the directory on every platform, so it is one value and never one per
    /// operating system.
    ///
    /// # Examples
    ///
    /// ```
    /// let paths = almena_paths::Paths::for_application("network.almena.cli");
    /// assert!(paths.logs().is_ok());
    /// ```
    #[must_use]
    pub fn for_application(identifier: &str) -> Self {
        Self {
            identifier: identifier.to_owned(),
        }
    }

    /// What the user created and cannot get back if it is lost — one day, keys and identity.
    ///
    /// Local rather than roaming: on Windows this is `%LOCALAPPDATA%`, and data that should
    /// follow a person between machines is a different question nobody here has asked.
    ///
    /// # Errors
    ///
    /// [`NoHomeDirectory`] when the platform does not say where the user's home is.
    pub fn application_data(&self) -> Result<PathBuf, NoHomeDirectory> {
        self.under(dirs::data_local_dir())
    }

    /// What the person chose. Small, meant to be readable, and absent on a first run.
    ///
    /// # Errors
    ///
    /// [`NoHomeDirectory`] when the platform does not say where the user's home is.
    pub fn configuration(&self) -> Result<PathBuf, NoHomeDirectory> {
        self.under(dirs::config_dir())
    }

    /// What the program can rebuild on its own, and whose loss must cost only time.
    ///
    /// # Errors
    ///
    /// [`NoHomeDirectory`] when the platform does not say where the user's home is.
    pub fn cache(&self) -> Result<PathBuf, NoHomeDirectory> {
        self.under(dirs::cache_dir())
    }

    /// Diagnostics and history: worth keeping between runs, never required to start.
    ///
    /// macOS keeps logs somewhere of its own — `~/Library/Logs` — rather than beside the
    /// application's data, and this is the one purpose where the three platforms do not agree
    /// on the shape. Tauri's resolver has the same special case, and this one exists to match
    /// it.
    ///
    /// # Errors
    ///
    /// [`NoHomeDirectory`] when the platform does not say where the user's home is.
    pub fn logs(&self) -> Result<PathBuf, NoHomeDirectory> {
        #[cfg(target_os = "macos")]
        {
            dirs::home_dir()
                .map(|home| home.join("Library/Logs").join(&self.identifier))
                .ok_or(NoHomeDirectory)
        }

        #[cfg(not(target_os = "macos"))]
        {
            self.application_data().map(|dir| dir.join("logs"))
        }
    }

    /// Files that exist only while a process runs, and that nothing reads after a reboot.
    ///
    /// `$XDG_RUNTIME_DIR` is a Linux idea and there is no equivalent elsewhere, so on macOS
    /// and Windows this is the temporary directory. **A caller does not branch on that** —
    /// that it differs is exactly what this crate is for.
    ///
    /// This is the one purpose that is not per-application: the directory is the platform's,
    /// and a program names its own files inside it.
    ///
    /// # Errors
    ///
    /// Never. The signature matches the rest so that a caller handles the whole table one
    /// way, and so that a platform that one day cannot answer does not change this crate's
    /// public surface to say so.
    pub fn runtime(&self) -> Result<PathBuf, NoHomeDirectory> {
        #[cfg(target_os = "linux")]
        {
            Ok(dirs::runtime_dir().unwrap_or_else(std::env::temp_dir))
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok(std::env::temp_dir())
        }
    }

    /// A platform directory with this application's name inside it, or the error.
    fn under(&self, base: Option<PathBuf>) -> Result<PathBuf, NoHomeDirectory> {
        base.map(|dir| dir.join(&self.identifier))
            .ok_or(NoHomeDirectory)
    }
}

#[cfg(test)]
mod tests {
    use super::Paths;

    /// The identifier of the program these tests stand in for.
    const IDENTIFIER: &str = "network.almena.cli";

    fn paths() -> Paths {
        Paths::for_application(IDENTIFIER)
    }

    #[test]
    fn every_per_application_directory_is_named_after_the_application() {
        let paths = paths();

        for directory in [
            paths.application_data().unwrap(),
            paths.configuration().unwrap(),
            paths.cache().unwrap(),
            paths.logs().unwrap(),
        ] {
            assert!(
                directory.to_string_lossy().contains(IDENTIFIER),
                "{directory:?} is not named after the application"
            );
        }
    }

    #[test]
    fn the_purposes_are_told_apart() {
        let paths = paths();

        // Not that they are all different: on macOS the resolver returns one directory for
        // application data and for configuration, and that is the platform's answer rather
        // than a fault. What must not happen is a purpose landing inside another one's
        // directory, which is how a cache sweep takes somebody's keys with it.
        assert_ne!(paths.cache().unwrap(), paths.application_data().unwrap());
        assert_ne!(paths.cache().unwrap(), paths.logs().unwrap());
    }

    #[test]
    fn two_applications_never_share_a_directory() {
        let cli = Paths::for_application("network.almena.cli");
        let desktop = Paths::for_application("network.almena.desktop");

        assert_ne!(
            cli.application_data().unwrap(),
            desktop.application_data().unwrap()
        );
        assert_ne!(cli.logs().unwrap(), desktop.logs().unwrap());
    }

    #[test]
    fn the_runtime_directory_answers_on_every_platform() {
        // The one purpose with no home directory in it, and the one a caller would otherwise
        // have to write a `cfg` for.
        assert!(paths().runtime().is_ok());
    }
}
