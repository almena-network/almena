# Contributing to Almena

Thanks for taking the time. This document covers what you need to know before opening an
issue or a pull request.

By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

> **This repository is under construction.** `almena` is `node`'s desktop application and
> `client` becoming one, and today it is a scaffold: `node` and `client` are what ships. What
> that means for a contributor is the next section — the work here advances one written step
> at a time, and a change that is not part of a step is a change nobody agreed to.

## The work happens in steps, and a step is specified first

Every piece of the unification is written down before it is taken, as a numbered spec in the
[almena-network](https://github.com/almena-network/almena-network/tree/develop/specs)
repository, which is also where the project's working agreements live. A spec says what moves,
what deliberately does not, how it is checked, and what it makes wrong elsewhere.

So, before writing code:

- **Find the spec for what you are doing.** A spec marked `accepted` may be implemented; one
  marked `proposed` has not been agreed yet, and nothing is implemented from it.
- **If there is no spec, propose one** rather than opening a pull request. An issue describing
  the step is the right start.
- **A finished step leaves nothing describing the old arrangement** — the rules, this
  repository's five documentation files, the translation catalogs of every interface it
  touched. That closing checklist is part of the change, not a follow-up.

A fix that is not part of the migration — a broken command, a wrong sentence in the README, a
dependency that will not build — needs no spec. Open it directly.

## Reporting a problem

Open an issue describing what you did, what you expected, and what happened instead, with the
device or computer, the operating system and its version, and the commit you were on. If the
problem is a security vulnerability, do **not** open an issue — follow
[SECURITY.md](SECURITY.md).

Check first whether the problem is really here: a screen that has not moved yet belongs to
[node](https://github.com/almena-network/node) or
[client](https://github.com/almena-network/client), and so does its bug.

## Setting up

See [Requirements](README.md#requirements) in the README, then:

```bash
task dev           # on this computer
task init          # once per checkout, for the mobile targets
task dev:android   # or task dev:ios
```

`task` on its own lists every command, and the README documents them one by one.

## What every change is expected to follow

**Everything in the repository is written in English** — identifiers, file names, comments,
documentation, commit messages, branch names, log messages and test names. The one exception
is text an end user reads.

**No user-facing text in the source.** Every string a person sees in the running application
comes from a translation catalog, looked up by key. This includes the easily forgotten ones:
window titles, menu entries, notifications, empty states, validation messages, and errors
that surface to the user — including errors raised in the Rust backend, which travel as an
identifier the frontend translates, never as prose. There is no catalog here yet; the first
screen that needs one brings it, in English and Spanish together.

**Nothing about a person is stored, sent or inferred.** No account, no sign-up, no field
asking for a name, an e-mail address or a telephone number, and no value pre-filled from the
hostname or the logged-in user. This application talks to a node over the node API and to
nothing else: no telemetry, no analytics, no crash reporting, no update ping, in any build.

**Every platform moves together.** iOS, Android, Windows, Ubuntu and macOS are supported
equally: none gets a feature first. A change that needs platform-specific code carries the
equivalent path for the rest in the same pull request, and no dependency is adopted unless it
builds everywhere.

**It runs on a phone and on a computer.** Design for both: every action reachable by touch and
by mouse and keyboard, and a layout that survives from a phone screen to a resizable window. A
keyboard shortcut or a context menu is an accelerator, never the only way to do something.

**The application fills the window.** No screen, column or card carries a `max-width`. Where a
screen has several cards they flow — side by side once there is room, stacked when there is
not — with one `repeat(auto-fit, minmax(min(100%, …), 1fr))` grid and no new breakpoint. 600
is the only width written anywhere. See it at 400 × 700 and at a window dragged wide before
you call it done.

**Nothing on screen is selectable.** `user-select: none` is declared once, on `body` in the
project's base stylesheet, and the only exception is `input` and `textarea`. A value worth
taking out of the application gets a button that copies the whole of it, named from the
catalogs — never selection re-enabled on one screen.

**Files go where the operating system expects them.** Ask the path resolver
(`@tauri-apps/api/path`, or `tauri::Manager::path()` in Rust) for the directory that matches
what you are storing — data, configuration, cache, logs and state, and runtime files each
have their own. Never build a path from `$HOME`, `%APPDATA%` or any other literal, and never
put in the cache anything you cannot rebuild.

**Small files, small functions.** A source file stays under 400 lines and a function under 50
— a React component under 100, since JSX legitimately spends lines on markup. Comments and
blank lines never count, so documenting something can never be what pushes it over. When you
hit a limit, split by responsibility; if you cannot name both halves, the seam is wrong.

**Every file says what it is for.** A file opens with a header — `//!` in Rust, a `/** */`
block in TypeScript — and every public item carries a doc comment: what it does for its
caller, a `# Errors` section on anything returning `Result`, and a documented parameter
wherever its name and type do not already say everything.

**Modules have one responsibility.** Name it without the word "and". Extract on the third
occurrence rather than the second, when you know which part is genuinely shared, and never
across repositories: `node`, `client`, `registry` and `almena` share no source. Code arriving
here from another repository is copied, not linked.

`crates/almena-log` is the one crate beside the application today, and it is one because the
shape of a record is a decision rather than a helper — see
[logging.md](https://github.com/almena-network/almena-network/blob/develop/.claude/rules/logging.md).
A second crate needs an argument of that kind, not a pile of things that had nowhere else to go.

**`task check` never compiles for a phone.** `--all-targets` means every kind of target — lib,
bins, tests — and not every platform, so a `#[cfg(mobile)]` path can be wrong for weeks and the
first thing to say so is a device on somebody's desk. Run `task check:mobile` before pushing
anything under a `mobile` cfg, and before a release. It names every target it skipped for want
of a `rustup` toolchain rather than passing quietly.

**The tools settle style.** `task check` — `cargo fmt --check`, `cargo clippy -D warnings`,
`tsc --noEmit` and `task isolation` — passes before you push, and `task format` is what settles
an argument about layout. Both are narrower here than in the other repositories: ESLint,
Prettier and the frontend test suite are not installed yet, so `task test` runs Rust alone and
`task format` formats Rust alone.

**The log format knows about no framework.** `task isolation` fails a build in which
`almena-log` can reach `tauri`. That crate holds the shape of a record and the sizes a log is
bounded by, which is why it is a crate at all; a dependency added to it is the whole of how
that gets lost, and nothing else would notice.

**One framework.** Everything here is Tauri 2, and that is a decision rather than an accident
of what was reached for first: a second framework beside it is a second set of documentation, a
second upgrade path and a second way to do everything. A dependency that brings one is a change
that argues for it in a spec of its own. The README's
[What is not here yet](README.md#what-is-not-here-yet) lists what the checks do not cover yet. A
limit that is wrong is changed in the configuration, in its own commit, with the reason — not
silenced in passing.

**Documentation keeps up with the code.** If your change adds a command, a prerequisite, a
file this application writes to a device, or changes how the project is run, update the README
in the same pull request. A step that fills in one of the gaps listed under
[What is not here yet](README.md#what-is-not-here-yet) deletes that row in the same change.
This applies to all five documentation files: a change that makes one of them wrong fixes it
then, not later.

## Pull requests

1. Branch off `develop`, which is where the work happens. `main` is what has been released.
2. Keep the change focused — one concern per pull request, and one step per pull request.
3. Name the spec your change implements, if there is one.
4. Make sure `task check` and `task build` succeed before pushing, and `task build:android` or
   `task build:ios` when your change touches a platform you can build. Say in the pull request
   which platforms you could not build.
5. Write the pull request description in English: what changes, and why.

Commit messages are written in English, in the imperative ("Add locale switcher", not
"Added" or "Adds"), with the reasoning in the body when the change is not self-evident.

## License of your contributions

Almena is licensed under the [Apache License 2.0](LICENSE). By contributing you agree that
your contributions are licensed under the same terms.
