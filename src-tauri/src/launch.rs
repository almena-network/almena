//! Whether this launch was asked to start with no window.
//!
//! One question, and it has one caller that is not a person. The autostart entry is registered
//! carrying `--hidden` (see `run` in `lib.rs`), so the flag is how the operating system says
//! *this launch is a login, not somebody asking for the application*. An application that put
//! a window in front of somebody who was logging in is one they switch off within the week.
//!
//! # Why this is not `tauri-plugin-cli`
//!
//! It was, until the CLI existed. The plugin answered three things — `--help`, `--version` and
//! this — and the first two were the windowed application being polite on the way to opening a
//! window. A person who wants a command-line program now has one, `almena`, whose whole purpose
//! is to be typed; this application went back to having no command line to speak of.
//!
//! What is left has no text to print, no surface to describe and no catalog to read, so it is
//! `std::env::args` and not a parser. It also stopped inheriting the plugin's own documented
//! limitation: on Windows a release build has no console to write back to, which made `--help`
//! print nothing there. A flag nothing prints has no such problem.

/// Whether the command line asked this launch to start with no window.
///
/// Read from the process arguments directly. There is nothing else on this application's
/// command line, and an argument it does not know is not an error: a person who mistyped
/// something is better served by the application they asked for than by a refusal written in
/// a language nobody chose — nothing on a command line has read a catalog yet.
///
/// # Examples
///
/// ```
/// # use almena_app_lib::launch;
/// assert!(launch::asked_for_hidden(["almena-app", "--hidden"]));
/// assert!(!launch::asked_for_hidden(["almena-app"]));
/// ```
pub fn asked_for_hidden<I, S>(arguments: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    arguments
        .into_iter()
        .skip(1)
        .any(|argument| argument.as_ref() == "--hidden")
}

/// Whether this process was launched with `--hidden`.
#[must_use]
pub fn starts_hidden() -> bool {
    asked_for_hidden(std::env::args())
}

#[cfg(test)]
mod tests {
    use super::asked_for_hidden;

    #[test]
    fn the_flag_is_recognised() {
        assert!(asked_for_hidden(["almena-app", "--hidden"]));
    }

    #[test]
    fn an_ordinary_launch_is_not_hidden() {
        assert!(!asked_for_hidden(["almena-app"]));
    }

    #[test]
    fn the_program_name_is_never_the_flag() {
        // The first argument is what the process was invoked as, which a person controls: a
        // binary somebody renamed `--hidden` must not start hidden for ever.
        assert!(!asked_for_hidden(["--hidden"]));
    }

    #[test]
    fn a_flag_that_merely_looks_like_it_is_not_it() {
        for argument in ["--hidden=true", "--hide", "-hidden", "hidden", "--hiddenx"] {
            assert!(!asked_for_hidden(["almena-app", argument]), "{argument}");
        }
    }

    #[test]
    fn it_is_found_wherever_it_sits() {
        assert!(asked_for_hidden(["almena-app", "--other", "--hidden"]));
    }
}
