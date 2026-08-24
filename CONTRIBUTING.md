# Contributing to Almena

Thanks for taking the time. This document covers what you need to know before opening an
issue or a pull request.

By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

> **This repository is under construction.** `almena` is the application and the node of the Almena
> network, and today it is its starting point: no release has been published. What that means
> for a contributor is the next section — interfaces and configuration move underneath you,
> and a change is not finished until everything that described what it moved is true again.

## Making a change, and closing it

The project's working agreements live in the
[almena-network](https://github.com/almena-network/almena-network) repository, in
`.agents/rules/`. **A request to build something is a request to build it**: there is no
document to write first and no plan to agree. That requirement existed once and was deleted.

So:

- **A fix needs no ceremony.** A broken command, a wrong sentence in the README, a dependency
  that will not build: open the pull request.
- **Open an issue for anything you want argued about first** — a screen, a data format, a
  dependency that brings a second way to do something. An issue, not a specification.
- **A finished change leaves nothing describing the old arrangement** — the rules, this
  repository's five documentation files, the translation catalogs. That closing checklist is
  part of the change, not a follow-up.

## Reporting a problem

Open an issue describing what you did, what you expected, and what happened instead, with the
device or computer, the operating system and its version, and the commit you were on. If the
problem is a security vulnerability, do **not** open an issue — follow
[SECURITY.md](SECURITY.md).

## Setting up

See [Requirements](README.md#requirements) in the README, then:

```bash
task dev           # the windowed application, with hot reload
task dev:cli       # the node in this terminal
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
identifier the frontend translates, never as prose. The catalogs live in `src/i18n/locales/`,
English and Spanish together, and `task catalogs` holds them to the same keys.

**Nothing about a person is stored, sent or inferred.** No account, no sign-up, no field
asking for a name, an e-mail address or a telephone number, and no value pre-filled from the
hostname or the logged-in user. This application talks to the peers of its network, and to
the origin it read that network's configuration from, and to nothing else: no telemetry, no
analytics, no crash reporting, no update ping, in any build.

The updater plugin is in the binary and does not change that sentence. It is registered and
inert — no endpoint, no key, and no code calling it — and the day something does call it, what
it may do is
[deployments.md](https://github.com/almena-network/almena-network/blob/main/.agents/rules/deployments.md):
nothing checks unless a person asks, finding is not installing, and the request says the target,
the architecture and the current version and nothing that could identify anybody. A check on a
timer, on startup, or on a window regaining focus is the thing that rule exists to refuse.

**Every platform moves together.** Windows, Linux and macOS are supported equally: none gets a
feature first. A change that needs platform-specific code carries the equivalent path for the
rest in the same pull request, and no dependency is adopted unless it builds everywhere. There
is no such asymmetry in the application today, and the one place it took work to avoid is
opening at login, where macOS is served by `SMAppService` because the plugin the other two use
writes the wrong register.

Almena on a phone or a tablet is a **client**, and it is built in
[a repository of its own](https://github.com/almena-network/client). Nothing here is compiled
for one, and a change to this repository is never the place a client's behaviour is decided.

**Assume neither input method.** Every action is reachable by touch and by mouse and keyboard —
a laptop with a touch screen is one of the three platforms — and the layout survives from a
window 400 points across to a maximised one. A keyboard shortcut or a context menu is an
accelerator, never the only way to do something.

**The application fills the window.** No screen, column or card carries a `max-width`. Where a
screen has several cards they flow — side by side once there is room, stacked when there is
not — with one `repeat(auto-fit, minmax(min(100%, …), 1fr))` grid and no new breakpoint. 600
is the only width written anywhere: it is `--breakpoint-expanded` in `src/styles/tokens.css`,
and the only variant that reads it is `expanded:`. See it at 400 × 700 and at a window dragged wide before
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
occurrence rather than the second, when you know which part is genuinely shared.

**A screen with no data is still built, and reports that it has none.** Never left out, never
filled with sample data — not under a flag, not "just to see the design". And *nobody has looked
yet*, *there is nothing to look at* and *somebody looked and found none* are three different
facts that never share a sentence: a value nobody measured is `null` and never `0` or `[]`, and
every emptiness is drawn with `EmptyState`, which will not draw one without the reason for it.

**Screens are drawn out of shadcn/ui.** Everything a person operates, and every surface those
things sit on, comes from `src/components/ui/` — `alert`, `badge`, `button`, `card`, `empty`,
`field`, `item`, `label`, `select`, `separator`, `spinner`, `switch`, `toggle`, `toggle-group`.
Look in the registry before writing
one: it is larger than it seems, and an element written by hand that already existed is the
expensive kind of mistake, because it looks finished. Those files are vendored: they arrive by
`pnpm dlx shadcn@latest add <name>` and are left as the registry wrote them, with a file header
on top and any deviation named in it. A feature never writes its own `<button>`, `<input>` or
card, and never nudges an imported element with a `className` that changes how it is drawn
rather than where it sits. A screen that needs an element to look different is asking for a
change to that element, and the change has to be nameable as a meaning — which is why only a few
of the variants the registry ships are drawn here, and the list of them is in the rule.

What shadcn/ui has no answer for is composed from what it does, in `src/components/`: `Logo`,
`Figure`, `StateBadge`, `Setting`, `CardGrid`, `EmptyState`. Nothing in there invents a second way
of drawing a surface or a control, and a component the registry turns out to have an element for
is deleted in favour of it.

**A vendored element's own English is overridden, not edited.** `Spinner` ships
`aria-label="Loading"`; the translated value is passed as a prop at the call site. Editing the
vendored file would work until the next update quietly put the English back.

**Every value is a token.** Screens are Tailwind utilities, and every one of them resolves
against `src/styles/tokens.css` — the one file of colour, shape, spacing and type. An arbitrary
value carrying a design value is the violation: `bg-[#eb7229]`, `text-[13px]`, `rounded-[12px]`
and `style={{ … }}` are all the same mistake, and Tailwind's own colour palette and breakpoints
are cleared in that file so that `bg-red-500` and `md:` do not exist here at all. The identity
colour is `primary`; shadcn/ui's `accent` is a hover grey and means nothing about identity.
Icons come from `lucide-react` and from nowhere else.

`crates/` holds two crates beside the two programs, and each is one because it owns a
**decision** rather than because two programs happened to need it. `almena-log` holds the shape
of a record — see
[logging.md](https://github.com/almena-network/almena-network/blob/main/.agents/rules/storage-and-logs.md).
`almena-paths` holds where a program keeps things, for the programs Tauri's own resolver does
not serve; `almena-app` does not even link it, because it keeps Tauri's, and what the two share
is the answer rather than the code — held to by `src-tauri/tests/paths_agree_with_tauri.rs`.
A third crate needs an argument of that kind, not a pile of things that had nowhere else to go.

**A directory at the root is a program.** `cli/` and `src-tauri/` are the two; anything under
`crates/` is a library one of them is built from.

**`unsafe` is denied, and lifted in exactly one place.** The workspace denies `unsafe_code`.
One module lifts it — `src-tauri/src/open_at_login.rs`, whose macOS half calls into
Objective-C because no safe API reaches the register it has to write — and every block there
carries a `// SAFETY:` comment saying why it is sound. A second one would take the same shape:
per module, never crate-wide, and never without the comment.

**`task check` compiles for this machine and no other.** `--all-targets` means every kind of
target — lib, bins, tests — and not every operating system, so code behind a `#[cfg]` for one
of the other two is checked by whoever builds there. There are five such blocks and they are
all in `open_at_login`, `launch` and the `Reopen` arm of `run`.

**The tools settle style.** `task check` — `cargo fmt --check`, `cargo clippy -D warnings`,
`tsc --noEmit`, `task catalogs` and `task isolation` — passes before you push, and
`task format` is what settles an argument about layout. Both are narrower than they will end
up: ESLint, Prettier and the frontend test suite are not installed yet, so `task test` runs
Rust alone and `task format` formats Rust alone — and the two agreements above that a linter
would catch, the size limits and the ban on arbitrary Tailwind values, are a reviewer's until
it arrives.

**The two programs stay out of each other's graph.** This repository builds a windowed
application and a CLI, and each binary links what its own dependency graph holds. `task
isolation` fails a build in which any of these becomes false:

| | Must not reach |
| --- | --- |
| `almena-log` | `tauri` |
| `almena-paths` | `tauri` |
| `almena-cli` | `tauri` |
| `almena-app` | `ratatui` |
| `almena-agent-protocol` | `tauri` |
| `almena-cli` | `almena-agent-protocol` |

The three crates hold decisions — the shape of a record, where a program keeps things, and what
this application and an agent may say to each other — which is why they are crates at all, and a
framework in any of them is how that gets lost. It matters most for the third: its other half is
in another repository and another language, and a contract that had grown a `tauri::ipc::Channel`
in one of its types is one no other runtime could ever link. The two programs are the other
half: a node that linked a webview it never draws in would take minutes longer to build and tens
of megabytes more to ship, and nothing but this check would say so.

**The agent is a second program, not a library.** It is built from [its own
repository](https://github.com/almena-network/agent), staged with `task agent:stage` and
bundled — `task build:all` does both halves in order, building the agent there before building
anything here. A checkout of this repository alone has none, and a build with nothing staged is
an ordinary state — the application runs and says on its own AI screen that it carries no
agent. If your change touches the protocol, run `task test:agent`, which drives the staged
agent over real pipes: it is not part of `task test`, because a test that passed quietly
without an artifact from another repository would be green on every machine that had nothing to
test with. A change to the protocol is a change to **both** repositories, including the golden
frames each holds a copy of.

**Two frameworks, and no third.** Tauri 2 for the windowed application, `ratatui` for the
terminal, and that is the whole list. The second one is not an accident of what was reached for
first — a terminal interface was built here, deleted so that everything would be Tauri 2, and
brought back when a computer with no graphical system had no other way to be a node. A
dependency that brings a third framework is a change that has to argue for it. The README's
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
3. Say what the change is for, in a sentence, where the diff does not say it itself.
4. Make sure `task check` and `task build` succeed before pushing. Say in the pull request
   which of the three platforms you could not build.
5. Write the pull request description in English: what changes, and why.

Commit messages are written in English, in the imperative ("Add locale switcher", not
"Added" or "Adds"), with the reasoning in the body when the change is not self-evident.

## License of your contributions

Almena is licensed under the [Apache License 2.0](LICENSE). By contributing you agree that
your contributions are licensed under the same terms.
