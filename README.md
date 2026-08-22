# Almena

`almena` is the application people use to reach the Almena network, and — on a computer — to
operate the node running on it. One codebase for iOS, Android, Windows, Linux and macOS.

It builds **one program** on **one framework**. It briefly built two — a terminal interface, in
[spec 0003](https://github.com/almena-network/almena-network/blob/main/specs/0003-a-workspace-and-a-terminal-interface.md)
— and [spec 0006](https://github.com/almena-network/almena-network/blob/main/specs/0006-one-framework-and-tauris-own-cli.md)
deleted it: everything here is Tauri 2, reachable from one set of documentation and one upgrade
path. What that gave up, deliberately, is the machine with no desktop on it.

> **Status: under construction.** This is the starting point of the application, not a
> release: interfaces, data formats, commands and configuration change without notice, and no
> release has been published. There is no client of the node API yet, so the application
> reaches no node — the first screen says exactly that.

Development advances one written step at a time, and each step is specified before it is
taken. The specs live in the
[almena-network](https://github.com/almena-network/almena-network/tree/main/specs)
repository, which is also where the project's working agreements are kept. This repository's
task runner is
[spec 0001](https://github.com/almena-network/almena-network/blob/main/specs/0001-a-task-runner-for-almena.md).

## Stack

- [Tauri 2](https://tauri.app) — Rust core, native shell on every platform.
- [React 19](https://react.dev) with TypeScript, built by [Vite 7](https://vite.dev).
- [pnpm](https://pnpm.io) for JavaScript dependencies, [Task](https://taskfile.dev) as the
  command runner.

## Requirements

Common to every platform:

- A stable Rust toolchain, plus the system dependencies Tauri 2 needs — see
  [Tauri prerequisites](https://tauri.app/start/prerequisites/).
- Node.js 20 or newer, pnpm, and Task 3.

To reach a node, rather than only to build this application, the device also needs **IPv6
connectivity**. Almena is an IPv6 network and there is no second address family.

That is all the desktop build needs. The mobile builds add:

- **Android**, on any host: JDK 17, Android Studio with the SDK and the NDK. `ANDROID_HOME`
  and `NDK_HOME` have to be set — `task init:android` refuses to run without them and says so.
- **iOS**, on macOS only: Xcode with its command line tools, and CocoaPods.

## Getting started

On a computer, nothing to set up beyond the requirements:

```bash
task dev
```

For a phone, generate the native project once, then run:

```bash
task devices       # what is plugged in right now
task init          # generates the native projects this host can build
task dev:android   # or task dev:ios
```

`task` on its own lists every available command. The full set:

| Command | What it does |
| --- | --- |
| `task install` | Installs JavaScript dependencies. Skipped when already up to date. |
| `task catalogs` | Checks that every translation catalog holds the same keys. |
| `task check` | Checks the Rust formatting, runs clippy over the workspace, type-checks the frontend, and runs `task catalogs` and `task isolation`. |
| `task check:mobile` | Type-checks the application for the mobile targets. `task check` does not — it only ever compiles for this machine. |
| `task isolation` | Asserts that `almena-log`, which holds the log format, reaches no framework. |
| `task test` | Runs the test suites across the workspace. Rust only today — see below. |
| `task format` | Formats the Rust source with `cargo fmt`. |
| `task icons` | Regenerates every icon from `assets/branding`. |
| `task devices` | Lists connected Android and iOS devices. |
| `task init` | Generates the native mobile projects for every platform this host can build. |
| `task init:android`, `task init:ios` | The same, one platform at a time. |
| `task dev` | Runs the windowed application on this computer, with hot reload. Also available as `task dev:desktop`. |
| `task dev:android`, `task dev:ios` | Runs the app on a connected device or emulator, with hot reload. |
| `task build` | Builds the desktop installer for this host's operating system. Also available as `task build:desktop`. |
| `task build:android`, `task build:ios` | Builds the mobile packages. |
| `task deploy:android`, `task deploy:ios` | Chooses a destination, builds for it, and installs it there. |
| `task clean` | Removes build artifacts. The generated native projects are kept. |

Every task installs dependencies first, so a fresh checkout needs only `task dev`.

With no suffix, `dev` and `build` target the computer you are sitting at. Mobile is always
named explicitly — `dev:android`, `build:ios`, and so on — because building for a phone needs
an SDK the desktop build does not, and because `task init` has to have run first.

The two deploy tasks are a script each rather than a line each, because what gets built
depends on where it is going: the destination is chosen, and started if it is an emulator that
was not running, before anything is compiled. Set `DEVICE` to skip the prompt —
`task deploy:android DEVICE=emulator-5554`.

## One program, and the crates it is built from

The repository is a Cargo workspace. One program comes out of it today, and the workspace is
still where every crate is held to the same standard:

```
Cargo.toml            the workspace, its lint tables, and every third-party version
clippy.toml           the size thresholds those lints check
crates/
  almena-log/         the record format every program here writes
src-tauri/            the application, package `almena-app`
src/                  its frontend
```

`src-tauri/` stays at the root rather than moving under `crates/`. That is Tauri's own layout —
its documentation puts the Rust project there and allows it to be "a member of your Rust
workspace", `tauri.conf.json` is the marker its CLI uses to find the project, and the generated
`gen/android` and `gen/apple` hang off it.

A dependency is named in one place. Members carry `{ workspace = true }` and no version number,
so two of them cannot end up on two versions of the same crate.

`almena-log` is a crate rather than a call because it is a decision — the line shape, the sizes
a log is bounded by, the names its files take — and a decision is easier to hold to in one
place than in a builder call halfway down a plugin registration.

`task isolation` is down to one assertion from four. Three of them kept two dependency graphs
apart and cannot fail with one program, so they went with the terminal interface; an assertion
that cannot fail reads as a check on every run and checks nothing. The one that survives is
**`almena-log` cannot reach `tauri`** — which is exactly what makes it a crate rather than a
call on the log plugin's builder, and which a single careless `[dependencies]` line would undo.

## What it answers on a command line

`tauri-plugin-cli` is registered on the desktop builds, configured in `tauri.conf.json` under
`plugins.cli`. Today the surface is what `clap` gives it and no argument of our own:

```bash
almena-app --help
almena-app --version
```

Both print and exit without opening a window. **On Windows they print nothing**, because a
release build there is a windowed binary with no console attached to write back to — the
plugin's own documented limitation, and the reason it is named here rather than found.

There is deliberately no `--node`. This build has no client of the node API, so an address
would be accepted, used for nothing, and refused by nothing — and refusing an IPv4 address in
any of its disguises is not optional in this project. It arrives with the code that can honour
it.

## Where it runs

| | Runs on |
| --- | --- |
| On a phone or tablet | iOS, Android |
| On a computer | Windows, macOS, Linux |

Linux means both packaging families a bundle produces: `.deb` for Debian, Ubuntu and their
derivatives, `.rpm` for Red Hat, Fedora and theirs, and `.AppImage` for neither in particular.

The application opens at 800 × 700 and never goes below 400 × 700, and its layout has
two shapes chosen by the width of the window and by nothing else — a phone in landscape, a
tablet and a window somebody dragged wider are the same case. The numbers live in
`src-tauri/tauri.conf.json` and nowhere else.

## What a second launch does

Nothing, twice over: the running application's window comes back instead. On a computer the
window also remembers its size, position and state between runs. Both are compiled out of the
mobile binary, where the operating system owns them already.

## Files kept on your computer

| File | What it is | Where |
| --- | --- | --- |
| `almena-app.log` | This program's records. Rotated at 10 MiB, ten kept. Deleting the directory while the application is closed costs nothing but the history. | macOS: `~/Library/Logs/<id>/`<br>Windows: `%LOCALAPPDATA%\<id>\logs\`<br>Linux: `~/.local/share/<id>/logs/` |
| `window-state.json` | Where the window was and how big. Written by the window-state plugin, which fixes its own location. | The configuration directory for `<id>` |

`<id>` is the bundle identifier, and it is still the scaffold's `network.almena.desktop` — see
[What is not here yet](#what-is-not-here-yet).

## The mark

The application's icon is generated, not hand-placed. `assets/branding/` holds the artwork and
`task icons` turns it into every size and format the platforms ask for — `src-tauri/icons/` for
the desktop and the Windows Store, and, where a native project has been generated, straight
into `src-tauri/gen/`.

| Source | What it is for |
| --- | --- |
| `app-icon.png` | The icon everywhere except macOS. Also the source of `icon.png` and `icon.ico`. |
| `app-icon-macos.png` | The same mark with the padding macOS expects inside its rounded square. Without it the icon sits visibly larger in the Dock than its neighbours. |
| `app-icon-bg.png`, `app-icon-fg.png` | Android's adaptive icon, which is two layers the system moves against each other. |
| `app-icon-mono.png` | Android's themed icon, drawn in the system's own colour. |
| `app-icon-negative.png` | The mark reversed, for a dark ground. |
| `tray/tray-icon.png` | The bare glyph. Scaled to 32 × 32 it becomes `src-tauri/icons/tray.png`, which is a template image: the system tints it, so it carries no square of its own. |
| `tray/tray-icon-negative.png` | The tray glyph reversed. Becomes `src-tauri/icons/trayNegative.png`, for the platforms that do not tint a template image themselves. |
| `icon-manifest.json` | What `tauri icon` reads: which file plays which role, and the background colour. |

`public/almena.svg` is the favicon, and it is the same mark drawn rather than rendered.

Three of the steps in `task icons` run on macOS only — the `.icns` and the tray glyph need
`sips`. On another host the task regenerates everything else and leaves those two files as they
were committed.

## The interface

One frame in two shapes, and the shape follows **the width of the viewport and nothing else** —
not the platform, not the user agent, not whether there is a touch screen. A phone in landscape,
a tablet and a window somebody dragged wider are the same case and get the same layout.

| Width | The navigation is |
| --- | --- |
| Below 600 | A menu floating across the bottom, icons above their names |
| 600 and above | A sidebar down the left, icons beside their names |

Those are the same buttons in the same order and the same place in the document. One media
query in `src/features/shell/shell.css` moves them, and 600 appears there and nowhere else —
there is no hook, no `matchMedia` and no component that asks how wide it is.

Three sections: Home, Network, Settings. Only the first has a screen; the other two say so
rather than doing nothing when touched. The first screen shows what the application is and that
it is connected to no node, which is the whole of what it can honestly say — there is no client
of the node API here, so no figure on it is measured and none is invented.

## Languages

English and Spanish, both complete from the first screen. English is the source language and
the fallback: a missing translation degrades to readable English rather than to a bare key.

`task catalogs` checks that both hold exactly the same keys, and `task check` runs it. Typing
every catalog against the English one means `tsc` also fails on a key added to one and
forgotten in the other.

## What is not here yet

Stated rather than discovered by running something and being surprised.

| | Today |
| --- | --- |
| A client of the node API | Not written yet. The application reaches no daemon and shows no figure about one; the first screen says it is connected to no node because that is the whole truth available to it. |
| `task check` | Rust and `tsc` only. ESLint, Prettier and the frontend test suite are not installed yet, so `task test` runs Rust alone and there is no `task format` for TypeScript. |
| `--help` on Windows | The plugin cannot write back to the console that launched a release build, so `--help` and `--version` print nothing there. Everything else about them is the same, and no other platform is affected. |
| `productName` and `identifier` | Still the scaffold's `desktop` and `network.almena.desktop`, and now visible: the log directory is built from the identifier, so records land under a name nobody chose. The bundle identifier names the directories this application will write to, so settling it is a decision taken in its own step — and the two deploy scripts carry a copy of it that changes with it. |

## Contributing

Development advances one specified step at a time, and a step is agreed before it is
implemented — [CONTRIBUTING.md](CONTRIBUTING.md) says how that works, what a change is expected
to follow, and what `task check` does and does not cover today. By taking part you agree to the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Security

Do not open a public issue for a security problem. [SECURITY.md](SECURITY.md) says how to report
one privately and what happens next.

## License

Apache License 2.0 — see [LICENSE](LICENSE). Copyright 2026 The Almena Network Authors.
