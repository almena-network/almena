# Almena

`almena` is the application people use to reach the Almena network, and — on a computer — the
node itself: there is no daemon beside it, and the network is composed of the desktop
installations taking part in it. One codebase for iOS, Android, Windows, Linux and macOS.

It builds **one program** on **one framework**. It briefly built two — a terminal interface
beside the windowed application — and that one was deleted: everything here is Tauri 2,
reachable from one set of documentation and one upgrade path. What that gave up, deliberately,
is the machine with no desktop on it.

> **Status: under construction.** This is the starting point of the application, not a
> release: interfaces, data formats, commands and configuration change without notice, and no
> release has been published. The peer-to-peer layer is not written yet, so this build joins
> no network — the first screen says exactly that.

The project's working agreements are kept in the
[almena-network](https://github.com/almena-network/almena-network) repository — the rules this
code is held to, and the specs of work that was agreed in writing before it was built. A spec
is written when one is asked for, so most changes have none; every change, spec or not, is
closed by making everything it left describing the old arrangement true again.

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

To reach the network, rather than only to build this application, the device also needs
**IPv6 connectivity**. Almena is an IPv6 network and there is no second address family.

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
`plugins.cli`. Three things, and only the first two print anything:

```bash
almena-app --help
almena-app --version
almena-app --hidden
```

Both print and exit without opening a window. **On Windows they print nothing**, because a
release build there is a windowed binary with no console attached to write back to — the
plugin's own documented limitation, and the reason it is named here rather than found.

`--hidden` starts the application into the tray with no window, and exists for one caller:
the operating system, which passes it because the login item was registered with it. Nobody
needs to type it, and typing it does exactly what the login launch does.

There is deliberately no argument naming a peer or a network. This build joins no network, so
an address would be accepted, used for nothing, and refused by nothing — and refusing an IPv4
address in any of its disguises is not optional in this project. It arrives with the code that
can honour it.

## Running in the background

On a computer this application has a tray icon, and having one changes what its window means.
**Closing the window no longer ends the application**: it puts it away, and the application
goes on running. It ends from **Quit**, in the tray's own menu, and from nowhere else.

There are three ways back to a window that is not on screen, and they are three because no one
of them works everywhere:

| | Where |
| --- | --- |
| Click the tray icon | Wherever the desktop delivers that click to an application, which is most of them but not all |
| Click the Dock icon | macOS, where an application with no window is still in the Dock |
| Launch the application again | Everywhere. The second launch does not start a second one — see below |

If the tray fails to build — on Linux, most often because nothing on the desktop is serving
one — **the close button goes back to closing.** An application that put itself away with
nowhere to be found again would be worse than one that simply quit, so it does not.

The tray's menu is built from the frontend rather than at startup, because its entries are
text a person reads and only that side holds the catalogs. It is the same reason
`notification::show` takes text and not a key.

**Starting with the system** is a setting, in Settings, and off until somebody turns it on.
When the system starts the application it passes `--hidden`, so logging in never puts a window
in front of anybody.

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

Nothing, twice over: the running application's window comes back instead. That is now also
the way back from the tray that works on every desktop — launching Almena when Almena is
already running is a request to see it, and it is answered as one.

The window also remembers its size, position and state between runs — everything except
whether it was visible. That one is deliberately forgotten: a session that ended with the
window put away would otherwise be restored with nothing on screen, and an application that
starts into nothing is one nobody can tell from a broken one.

Both are compiled out of the mobile binary, where the operating system owns them already.

## Notifications

Registered on all five platforms, which is the reason the dependency was adopted at all: one
that served some of them is one this project does not take.

There are two ways to a notification, and they exist for different sides of the application:

| From | Through | For |
| --- | --- | --- |
| The frontend | `src/lib/notifications.ts`, over `@tauri-apps/plugin-notification` | Anything a screen sets off. This side holds the catalogs, so it is the side that can name what it is announcing. |
| Rust | `notification::show`, in `src-tauri/src/notification.rs` | Code running with no window in front of it. Its text arrives as an argument, because the catalogs are not on this side. |

**Nothing announces itself yet.** There is no network, so nothing has happened that would be
worth announcing, and the Rust function has no caller in this repository. What there is instead
is a control on the first screen that sends one, so the capability can be watched working on
the device in hand rather than taken on trust. It says what the device did with the request,
because two of the three answers — a refusal, and a failure — draw nothing at all, and a person
who pressed a button and saw nothing cannot tell those two apart.

Permission is asked for when that control is pressed, never at startup. A permission dialog
that arrives before anybody has asked for anything is worse than one that arrives a tap later.

**On Windows a notification is only drawn for an installed application.** That is the
platform's rule rather than this project's: a build started from `task dev` there shows
nothing, and the same binary installed shows what the other four platforms show.

## Files kept on your computer

| File | What it is | Where |
| --- | --- | --- |
| `almena-app.log` | This program's records. Rotated at 10 MiB, ten kept. Deleting the directory while the application is closed costs nothing but the history. | macOS: `~/Library/Logs/<id>/`<br>Windows: `%LOCALAPPDATA%\<id>\logs\`<br>Linux: `~/.local/share/<id>/logs/` |
| `window-state.json` | Where the window was and how big. Written by the window-state plugin, which fixes its own location. | The configuration directory for `<id>` |
| The login item | Written only while *starting with the system* is on, and deleted when it is turned off. The autostart plugin fixes its own location, as it must: a login item is only a login item where the system looks for one. | macOS: `~/Library/LaunchAgents/<productName>.plist`<br>Linux: `~/.config/autostart/<productName>.desktop`<br>Windows: a value under `HKCU\…\CurrentVersion\Run`, not a file |

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

Three sections: Home, Network, Settings. Two have screens; Network says it has none rather
than doing nothing when touched.

The first screen carries two cards. One says what the application is and that it is on no
network, which is the whole of what it can honestly say — the peer-to-peer layer is not here,
so no figure on it is measured and none is invented. The other is the one thing this build can
actually do, which is [send a notification](#notifications). They flow rather than stack: side
by side once there is room for both, one above the other the moment there is not, out of a
single `grid-template-columns` in `src/features/home/home.css` and with no breakpoint of their
own.

Settings holds one thing: whether the application
[starts with the system](#running-in-the-background). That belongs to a computer, so on a
phone the screen says there is nothing to set on this device rather than drawing an empty
page — which is the same answer Network gives, for the same reason. Choosing the palette and
the identity colour will live here too; neither is built.

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
| The peer-to-peer layer | Not written yet. On a computer this application is the node, but nothing here joins a network, reads the configuration a network is described by, or speaks to a peer; the first screen says it is on no network because that is the whole truth available to it. |
| `task check` | Rust and `tsc` only. ESLint, Prettier and the frontend test suite are not installed yet, so `task test` runs Rust alone and there is no `task format` for TypeScript. |
| `--help` on Windows | The plugin cannot write back to the console that launched a release build, so `--help` and `--version` print nothing there. Everything else about them is the same, and no other platform is affected. |
| `productName` and `identifier` | Still the scaffold's `desktop` and `network.almena.desktop`, and now visible twice over: the log directory is built from the identifier, and the login item is named after `productName`, so both land under a name nobody chose. Settling the pair is a decision taken on its own — and the two deploy scripts carry a copy of the identifier that changes with it. |
| Choosing the palette and the accent | `tokens.css` has both, `data-theme` and `data-accent` switch between them, and nothing writes either yet. Settings is where they belong and Settings now exists, so this is the next thing that screen grows. |
| `--help` in one language | The only text a person reads that does not come from a catalog. `clap` builds it from `tauri.conf.json` before anything has loaded a catalog, so it is English wherever it is read. |

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
