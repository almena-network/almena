# Almena

`almena` is the application people use to reach the Almena network, and the node itself: there
is no daemon beside it, and the network is composed of the computers taking part in it. One
codebase for Windows, Linux and macOS.

Almena on a phone or a tablet is a **client** of that network rather than a node of it, and it
is built in [a repository of its own](https://github.com/almena-network/client). This one built
it too, once, out of this codebase; it does not any more, and nothing here is compiled for a
phone.

It builds **two programs**:

| Program | Binary | What it is |
| --- | --- | --- |
| The windowed application | `almena-app` | Almena with a screen. Tauri 2. |
| The CLI | `almena` | A node for a computer with no graphical system. One job: bring a node up. `ratatui` in a terminal. |

The second one is a returning decision rather than a new one. A terminal interface was built
here, deleted so that everything would be Tauri 2 on one upgrade path, and what that gave up
was named at the time: the machine with no desktop on it. It came back because the machine did
— the separate node daemon that used to serve it no longer exists, so a server with no window
had no way to take part at all.

They are two applications, not two faces of one. Separate directories, separate keys: a machine
running both is **two nodes**.

> **Status: under construction.** This is the starting point of the application, not a
> release: interfaces, data formats, commands and configuration change without notice, and no
> release has been published. The peer-to-peer layer is not written yet, so this build joins
> no network — the first screen says exactly that.

The project's working agreements are kept in the
[almena-network](https://github.com/almena-network/almena-network) repository — the rules this
code is held to. A request to build something is a request to build it: there is no document to
write first. What every change owes is its closing, which is making everything it left
describing the old arrangement true again.

## Stack

- [Tauri 2](https://tauri.app) — Rust core, native shell on every platform.
- [React 19](https://react.dev) with TypeScript, built by [Vite 7](https://vite.dev).
- [shadcn/ui](https://ui.shadcn.com) on [Tailwind CSS 4](https://tailwindcss.com) —
  every element a screen draws, and every value it is drawn from.
- [Lucide](https://lucide.dev) for icons, which is the set shadcn/ui draws with.
- [pnpm](https://pnpm.io) for JavaScript dependencies, [Task](https://taskfile.dev) as the
  command runner.

## Requirements

Four tools, on every platform, and the last of them is how everything in this repository is
run:

| What | Version | What it is for |
| --- | --- | --- |
| [Rust](https://rustup.rs), stable | 1.85 or newer | Both programs. The workspace is edition 2024, which is what sets that number. |
| [Node.js](https://nodejs.org) | 20 or newer | The frontend's toolchain — Vite, TypeScript, and the Tauri CLI, which is a JavaScript package here rather than a global install. |
| [pnpm](https://pnpm.io/installation) | 9 or newer | JavaScript dependencies. Not npm and not yarn: the lockfile is `pnpm-lock.yaml` and the workspace is `pnpm-workspace.yaml`, and both are pnpm's. |
| [Task](https://taskfile.dev/installation/) | 3 | The command runner, and the whole of this repository's interface. Every command below is a `task`, `Taskfile.yml` is where each one is written down, and `task` on its own lists them. |

Beside them, the system dependencies Tauri 2 needs — a webview and a compiler toolchain,
different on each of the three operating systems and listed in
[Tauri prerequisites](https://tauri.app/start/prerequisites/). Nothing else is installed by
hand: `task` installs the JavaScript dependencies itself, and cargo fetches the Rust ones.

There is no task that installs the four. A command runner cannot install the runner that runs
it, and each of the four is a different act on each operating system; every one of the pages
above carries its own, for macOS, Windows and Linux alike.

To reach the network, rather than only to build this application, the device needs a way out to
the internet and nothing more particular than that. **A node listens on both address families**,
`/ip6/::` and `/ip4/0.0.0.0`, because a machine that has an address of each is reachable at each
and which one a caller can use is the caller's business rather than this node's.

Having an address is what lets a node take part; being *reachable* at it is a separate fact and
not one to assume. Most home connections drop what nobody asked for, and a node that cannot be
dialled dials out instead and takes part in full — through a relay where one volunteers to carry
it, which is what keeps *anybody can run a node* true for a machine behind a household router.
Which of the two it is, the application measures rather than guesses.

That is all the application needs **to build itself**. To build it with an agent inside it,
the agent's own requirements apply too — Python 3.14 and `uv`, listed in
[its repository](https://github.com/almena-network/agent). Nothing here needs them: a build with
no agent staged carries none and says so on its own screen, and `task agent:build` is the only
thing that asks for them.

## Getting started

Nothing to set up beyond the requirements:

```bash
task dev
```

The full set, which `task` on its own prints too:

| Command | What it does |
| --- | --- |
| `task` | Lists every available command. `task --list` says the same thing. |
| `task install` | Installs JavaScript dependencies. Skipped when already up to date. |
| `task catalogs` | Checks that every translation catalog holds the same keys. |
| `task check` | Checks the Rust formatting, runs clippy over the workspace, type-checks the frontend, and runs `task catalogs` and `task isolation`. |
| `task isolation` | Asserts that the two programs stay out of each other's dependency graph, and that the three crates beside them reach no framework. Six assertions — see below. |
| `task test` | Runs the test suites across the workspace. Rust only today — see below. |
| `task test:agent` | Drives the staged agent over real pipes, checking that the two halves of the Agent Protocol actually meet. Not part of `task test`: it needs an artifact from another repository, and a test that passed quietly without one would be green on every machine that had nothing to test with. |
| `task format` | Formats the Rust source with `cargo fmt`. |
| `task icons` | Regenerates every icon from `assets/branding`. |
| `task dev` | Runs the windowed application on this computer, with hot reload. Also available as `task dev:desktop`. `ARGS` gives it a command line — `task dev ARGS="--hidden"`. |
| `task dev:cli` | Runs the node in this terminal. No hot reload: this program has no frontend. `ARGS` reaches it directly — `task dev:cli ARGS="--quiet"`. |
| `task build` | Builds the desktop installer for this host's operating system. Also available as `task build:desktop`. The binary itself lands at `target/release/almena-app`. |
| `task build:debug` | The same bundle, unoptimized, at `target/debug/`. It keeps `debug_assertions`, so it is the one bundle that still says *Development* on its status strip. |
| `task build:cli` | Builds the node for this computer. One binary, at `target/release/almena`, and no bundle. |
| `task build:all` | Builds the agent in its own repository first, then both of this computer's programs — the desktop bundle with that agent inside it, and the node beside them. The one build that insists on an agent: it fails when there is no agent repository to build one from, where `task build` carries none and says so. |
| `task agent:build` | Builds the agent from its own repository, so that there is something to stage. `AGENT_REPO` says where it is; `../agent` by default. |
| `task agent:stage` | Copies a built agent into the bundle's resources. Says which of the two things happened — staged, or nothing found — and a build with nothing staged is an ordinary state. |
| `task agent:clean` | Takes the staged agent back out. |
| `task clean` | Removes build artifacts. |

Every task installs dependencies first, so a fresh checkout needs only `task dev`. That includes
staging the agent: `task dev` and `task build` both run `task agent:stage`, which copies one in
if there is one built and says so if there is not.

With no suffix, `dev` and `build` mean the windowed application; the node is always named —
`dev:cli`, `build:cli`. Both target the computer you are sitting at, which is the only kind of
machine anything here is built for.

`ARGS` is the one variable, and it gives whichever program is being run a command line:

```bash
task dev ARGS="--hidden"        # the application starts into the tray, as a login launch does
task dev:cli ARGS="--quiet"     # the node writes records instead of drawing
```

For `task dev` it reaches the application through two `--`, which is the Tauri CLI's own
convention: the first ends what is meant for that CLI, the second what is meant for cargo. For
`task dev:cli` there is nothing in between and it is passed straight through.

## Two programs, and the crates they are built from

The repository is a Cargo workspace, and it is where every crate is held to the same standard:

```
Cargo.toml            the workspace, its lint tables, and every third-party version
clippy.toml           the size thresholds those lints check
cli/                  the CLI, package `almena-cli`, binary `almena`
src-tauri/            the windowed application, package `almena-app`
src/                  its frontend
crates/
  the format everything is written in
    almena-cbor/           what canonical means here, and the check that says whether bytes are
    almena-format/         the log entry, the act, and the name an object gets from its own bytes
    almena-suite/          the one set of algorithms every program signs and hashes with
    almena-time/           the epoch, and every deadline counted in them
    almena-frozen/         the checklist a format has to pass before a network opens for good
  the record, and what a node may say about it
    almena-store/          the append-only log, the chain each object advances along
    almena-node/           everything a node does, under whatever is drawing it
    almena-api/            what can be asked of a node and what comes back
    almena-serve/          the transport that carries those questions, deciding nothing
    almena-tls/            the certificate a node serves under, and nothing else about serving
  reaching other nodes
    almena-mesh/           how nodes reach each other, and what they are called when they do
    almena-lookup/         what a zone publishes, and silence told apart from an empty answer
  what travels between people
    almena-mailbox/        what a mediator holds for somebody whose device is off
    almena-credential/     SD-JWT VC, its disclosures, and what verifying one may conclude
    almena-status/         the bitstring, its cohort, and whether one is fresh enough to use
    almena-sdk/            what an issuer and a verifier are built on
  what a program needs and the network does not
    almena-log/            the record format both programs write
    almena-paths/          where a program with no Tauri keeps things
    almena-agent-protocol/ what the application and an agent say to each other
```

**A directory at the root is a program; `crates/` holds the libraries they are built from.**
`src-tauri/` is at the root because that is Tauri's own layout — its documentation puts the Rust
project there and allows it to be "a member of your Rust workspace", and `tauri.conf.json` is
the marker its CLI uses to find the project. `cli/` sits beside it for the same reason a
program is not a library.

A dependency is named in one place. Members carry `{ workspace = true }` and no version number,
so two of them cannot end up on two versions of the same crate.

**Each crate is a decision, not a drawer**, and the last group is the one to read that way.
`almena-log` owns the shape of a record — the line, the sizes a log is bounded by, the names its
files take. `almena-paths` owns where a program keeps things, for the programs Tauri's resolver
does not serve. `almena-agent-protocol` owns what this application and an agent may say to each
other — a message set with a version number,
framed in MessagePack, naming no graph, no model and no library. Its other half is in another
repository, in another language, which is the argument for it being a crate at all: held inside
the application it would become a private detail of the one program that speaks it today, and
the point of the protocol is that the program at the other end can be replaced without a word
here changing. None of the three exists to share code:
`almena-app` does not even link `almena-paths`, because it has Tauri's resolver to ask. What
the two programs share is the **answer** — held to by
`src-tauri/tests/paths_agree_with_tauri.rs`, which asks both resolvers for every purpose and
fails if any pair differs.

`task isolation` makes six assertions, and each is a way one binary could quietly grow the
other's weight, or one contract quietly stop being one:

| | Must not reach | Because |
| --- | --- | --- |
| `almena-log` | `tauri` | The record format must know about no framework |
| `almena-paths` | `tauri` | The resolver exists for the programs Tauri does not serve |
| `almena-cli` | `tauri` | The node must not link a webview it never draws in |
| `almena-app` | `ratatui` | The windowed application must not link a terminal renderer |
| `almena-agent-protocol` | `tauri` | A wire contract must reach no framework, or no other runtime could speak it |
| `almena-cli` | `almena-agent-protocol` | A node in a rack has no agent beside it to speak to |

`cargo tree -p <crate> -i <package>` fails when the package is not in that crate's graph, so
success is the failure and silence is the pass. A single careless `[dependencies]` line is all
it takes to put a webview in a program that draws in a terminal.

## The node in a terminal

`almena` brings a node up on a computer with no graphical system, and that is its whole job.

```bash
almena              # brings the node up, and draws it
almena --quiet      # brings the node up, and writes records instead
almena --help
almena --version
```

`--quiet` is what a service manager runs, and **a missing terminal implies it**: a unit file
that forgot the flag gets the behaviour it meant rather than a program fighting for a terminal
that is not there. The flag exists on top of that detection because somebody at a real terminal
is entitled to say they would rather read records.

The drawn view reports which network this node belongs to, who it is, and how many peers it is
talking to. All three are a dash today and that is the point: there is no peer-to-peer layer,
so nothing has been measured, and a dash is what *not measured* looks like where a `0` would be
a count somebody took. `q` leaves.

It speaks English and Spanish, from the same two catalogs the screens use — the words for
*network*, *this node* and *peers* are the same keys. Which one is chosen comes from `LC_ALL`,
`LC_MESSAGES` or `LANG`. `--help` is the exception and is English wherever it is read, because
`clap` builds it before a catalog can be.

It keeps its records in its own directory, named `network.almena.cli` — not the windowed
application's. Two applications, two directories, and one day two keys.

### Opening a network, and the question asked before one is opened for good

A node **opens a network only when there is nobody to join**, which it finds out by reading the
zone. That is the one defence against the accident that costs the most — a second production
network beside the first, that nobody can tell apart, because both say the same word about
themselves.

```bash
almena --freeze-checklist     # can this format be frozen? nothing is opened
almena --open-development     # opens dev.almena.network, if nobody is there
almena --open-production      # opens almena.network. Once, ever
```

`--resolver <ADDRESS>` asks named servers instead of whatever this machine uses for DNS, for a
machine whose own resolver is not usable — behind a VPN, pointed at servers it cannot reach, or
simply answering every other tool on the machine and not this one. A node that cannot look up a
zone cannot open a network at all, because reading silence as an empty zone is how a second network
gets started, so being able to name a resolver is the difference between a machine that can take
part and one that cannot.

The two networks are not one thing with a setting. **Development is re-opened as often as the
format moves; production is opened once**, and a record is append-only, so whatever is missing on
the day it opens is missing for as long as that network exists. So the node puts its own freeze
checklist in front of a production genesis and refuses if anything is wanting — and
`--freeze-checklist` is the same question with nothing at stake, which is what to read first. Every
line of it is a probe that runs against this build, not a list somebody keeps up to date.

`--zone` points either of them somewhere else, and `--seed` stands in for a zone that cannot be
asked. A seed given by hand only ever says *somebody is there*, which is the safe direction: no
flag can make a node open a network it has not established is missing.

## What the windowed application answers on a command line

One flag, and it is not for a person to type:

```bash
almena-app --hidden
```

`--hidden` starts the application into the tray with no window, and exists for one caller: the
operating system, which passes it because the login item was registered with it. Typing it
does exactly what the login launch does.

That is the whole surface. `--help` and `--version` were here, answered by
`tauri-plugin-cli`, and left with it: a person who wants a command-line program has one now,
above. It took a documented limitation with it — on Windows a release build has no console to
write back to, so those two printed nothing there — and a flag that prints nothing has no such
problem. What is left is `std::env::args` in `src-tauri/src/launch.rs`, which needs no parser.

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
`notification::show` takes text and not a key. Changing the language on the Settings screen
asks for it again, and what happens then is a rename rather than a second tray: the entry keeps
its identity and its behaviour and takes a new word.

## Open at login, which is not the same as running in the background

**Opening at login** is a setting, in Settings, and off until somebody turns it on. It decides
only *who starts the application*. Running in the tray is what the application does once it is
running, whoever started it, and it is not a setting at all.

macOS is the platform that makes the difference visible, because it keeps two registers and
lists them separately:

| The register | What it is | What Almena does |
| --- | --- | --- |
| **Open at Login** | The application, opened for the person who just logged in, under its own name and icon | This is the setting, registered through `SMAppService` |
| **Allow in the Background** | A helper the system keeps running on its own account. A `LaunchAgent` lands here | Nothing. Almena writes no `LaunchAgent` |

That distinction is why macOS does not go through `tauri-plugin-autostart` like the other two:
the plugin writes a `LaunchAgent`, which starts the application but lists it under the wrong
one. `SMAppService` is also the only way to the right register that does not first ask a person
for permission to drive System Events.

| | Where the entry goes |
| --- | --- |
| macOS | `SMAppService`, which keeps its own register — there is no file of ours to point at |
| Windows | `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run`, which is what Task Manager's *Startup apps* lists |
| Linux | `~/.config/autostart/Almena.desktop`, the XDG entry Ubuntu and Fedora both honour |

Whichever it is, the entry carries `--hidden`, so logging in never puts a window in front of
anybody.

**On macOS this only works from an installed application.** `SMAppService` registers the bundle
around the running binary, so a build started by `task dev` has no bundle to register and the
switch cannot move. Installed, it does.

## Where it runs

Windows, macOS and Linux, all three equally. Linux means both packaging families a bundle
produces: `.deb` for Debian, Ubuntu and their derivatives, `.rpm` for Red Hat, Fedora and
theirs, and `.AppImage` for neither in particular.

The application opens at 1100 × 760 and never goes below 400 × 700, and its layout has two
shapes chosen by the width of the window and by nothing else — a window somebody dragged narrow
and one they dragged wide are the same case. The numbers live in `src-tauri/tauri.conf.json`
and nowhere else.

## What a second launch does

Nothing, twice over: the running application's window comes back instead. That is now also
the way back from the tray that works on every desktop — launching Almena when Almena is
already running is a request to see it, and it is answered as one.

The window also remembers its size, position and state between runs — everything except
whether it was visible. That one is deliberately forgotten: a session that ended with the
window put away would otherwise be restored with nothing on screen, and an application that
starts into nothing is one nobody can tell from a broken one.

**The size is remembered in points, and that matters on a computer with two displays.** The
plugin that keeps the rest of the geometry writes the size in pixels, so a window sized on a
display that draws two pixels to the point comes back twice as large on one that draws one.
`src-tauri/src/geometry.rs` keeps the same size in the unit a person actually chose it in and
corrects the window once at startup, and only when the two disagree — so the ordinary launch,
back onto the display it was on, resizes nothing.

**Dragging the window between two displays needs nothing.** It keeps the size it has, in
points, across a change of scale — that is `tao`'s doing on all three desktops and not
something this application has to arrange. What does change is how large a point is on the
display it arrived at, which is the operating system's business and not an application's to
undo: the same window looks physically larger on a display with fewer pixels to the inch, in
Almena as in everything else.

## Notifications

Registered on all three platforms, which is the reason the dependency was adopted at all: one
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
nothing, and the same binary installed shows what the other two platforms show.

## The agent it runs beside itself

Almena starts a second program beside itself: **`almena-agent`**, an AI agent written in
Python, built from [its own repository](https://github.com/almena-network/agent) and bundled
inside this application. The AI section is where you talk to it.

**One application, one process a person deals with.** The agent is spawned with
`std::process::Command`, given three pipes, and ended when Almena ends. Nothing is installed
separately, no port is opened, no local server is involved, and there is no daemon left behind:
if Almena is killed outright, the agent reads the end of its input and stops on its own. The
pipe *is* the liveness signal, which is why there is no PID file anywhere.

**The windowed application alone**, of the two programs built here. The CLI brings a node up on
a machine in a rack and gets no agent, because an agent is something a person sits in front of
— [deployments.md](https://github.com/almena-network/almena-network/blob/main/.agents/rules/deployments.md).
`task isolation` holds it to that: `almena-cli` may not reach `almena-agent-protocol` at all.

### The Agent Protocol

What crosses the pipe is `almena-agent-protocol`, and it is the whole of what either side knows
about the other. **Nothing in it names a graph, a model or a library**, so the program at the
far end could be rewritten in Rust or compiled to WASM without a word of this application
changing.

One message is one frame:

```
┌──────────────────────────┬────────────────────────────────┐
│ u32, big-endian, 4 bytes │ one MessagePack map, N bytes   │
└──────────────────────────┴────────────────────────────────┘
```

The prefix counts the payload and not itself. Newlines cannot frame a binary encoding, and the
prefix buys three things besides: a decoder is handed a complete slice and needs no stream, an
allocation is bounded before anything is parsed, and a stream that has lost its place can be
diagnosed without a MessagePack parser in hand. A frame over 8 MiB is refused, and there is no
recovery from one — skipping N bytes would mean trusting the number just refused.

Every frame carries `contract_version`, on **every** frame rather than on a handshake, and a
frame from a version this build does not speak is refused as exactly that.

| The application says | Carrying |
| --- | --- |
| `run` | `id`, `intent` (`chat` or `propose`), and what the run is given to work with |
| `cancel` | `id` |
| `tool_result` | `id`, `call_id`, and what came of a call — or that the application declined |

| The agent says | When |
| --- | --- |
| `ready` | Once, unprompted, at startup. Carries what it calls itself and the model it was started with |
| `started` | The run was admitted. Always first |
| `progress` | A stage began or moved. Counts are `null` where nothing counted them, never `0` |
| `token` | One piece of a streamed answer |
| `tool_call` | The run is asking the application to act. Nothing can produce one yet — see below |
| `proposal` | The one answer to a `propose` |
| `completed` / `cancelled` / `failed` | Terminal, and exactly one of them per run |

**Exactly one terminal event per run, and nothing follows it.** That is what lets this side
release everything it holds for a run the moment one arrives, with no timer and no bookkeeping.

`cancelled` means *no further event is coming*. It does **not** withdraw the tokens that already
arrived, and the screen keeps them: an answer somebody has already read is not something to take
off the screen.

**One run at a time.** The wire could carry two — every message is addressed — and this build
refuses a second, here, before anything is written to the pipe. There is one model connection
and one conversation in front of one person. It is a policy of the two programs and not a
property of the contract, which is what would let a runtime that can serve two arrive later
without the wire changing.

### Who executes, and who only asks

`tool_call` is in the contract and **nothing can produce one**: the set of capabilities is a
closed enum with no members, so every name is refused before it can be encoded. That is the
design rather than an omission. What is being written down is the decision — **the agent asks
and the application executes** — because that is the expensive thing to change later, and the
first real capability is a change on both sides in one go.

It is kept firmly apart from a `proposal`, which is prose a model wrote and nothing acts on.
Merging the two is the attack the agent's own `SECURITY.md` names first, and a test asserts the
two shapes have nothing in common.

### The model

**Almena runs no model and downloads none.** The agent asks whatever is serving on this computer
at an OpenAI-compatible address — LM Studio, Ollama, llama.cpp's server, vLLM all serve one —
and Settings chooses which model it asks for. That list is what Almena knows how to **ask for**;
nothing has asked the server what it actually holds, and the card says so. A name that is not
served comes back as `model_unknown`, which is the agent telling *that model is not here* apart
from *the agent is broken*.

The model a run uses is fixed when the agent starts, so changing it applies the next time one
starts — and Settings offers the restart rather than leaving that as a sentence to obey.

### Building with one

The agent is a separate repository, so a checkout of this one alone has nothing to stage — and
that is an ordinary state: the application builds, runs, and says on its own screen that it
carries no agent.

```bash
task agent:build     # builds it in ../agent, or point AGENT_REPO somewhere else
task agent:stage     # copies it to src-tauri/resources/almena-agent/
task test:agent      # drives the staged agent over real pipes, both halves of the contract
task build:all       # the two above, then both programs, in that order
```

`task dev` and `task build` stage it themselves. What it adds to every desktop artifact is about
50 MB.

`task build:all` is the one that builds the agent rather than staging whatever was already
there. That is why it is a task and not a longer `deps` list on `task build`: dependencies run
at the same time as each other, so an agent built as a dependency of a build would be staged in
whatever state the previous one left it. Here the agent is built first and on its own, and only
then is there a bundle to put it in. It is also the only build that **refuses** when there is no
agent repository to build from — `task build` on its own is the one that carries none and says
so on its AI screen.

## Registered, and not yet called

Two plugins are compiled into the binary that nothing in this application reaches yet. They are
here so that the first screen needing one is a screen and not a dependency negotiation.

**Dialogs** — `tauri-plugin-dialog` — are the native question, warning and file picker. Nothing
draws one today.

**Updating** — `tauri-plugin-updater` — is how an application that is a file somebody
downloaded replaces itself.

It is registered **inert**. `plugins.updater` in `src-tauri/tauri.conf.json` carries an empty
`endpoints` and an empty `pubkey`, `bundle.createUpdaterArtifacts` is off, and no code here asks
the plugin anything at all. That is not an oversight waiting for somebody to fill the two fields
in. The Transparency principle is explicit — *no telemetry, no analytics, no crash reporting, no
update ping, in any build* — and what the project has settled about where that line falls is
[updating.md](https://github.com/almena-network/almena-network/blob/main/.agents/rules/deployments.md):
nothing checks unless a person asks, finding is not installing, and a request carries the
target, the architecture and the current version and nothing that could say who this is. What is
**not** settled is which host serves the releases and which key signs them. Until both are, an
endpoint in the configuration would be exactly the ping the principle rules out, aimed at a host
nobody chose.

The frontend bindings for the two — `@tauri-apps/plugin-dialog` and
`@tauri-apps/plugin-updater` — are deliberately not installed. They arrive with the first
caller, which is one `pnpm add` in the change that has one; a package nothing imports is a
dependency nobody is reading.

**The agent added no third entry to this list, and that is worth saying rather than leaving to
be noticed.** Running a second program is exactly the kind of thing that usually arrives as a
plugin — `tauri-plugin-shell` has a sidecar mechanism — and it was not used. Its mechanism
copies one file renamed for the target triple, and the agent is a directory whose executable is
inert without the tree beside it; taking that route would have meant a single-file build, which
its own repository measured at twelve seconds of startup every run. So the agent is spawned with
`std::process::Command` and two threads. **No plugin is registered for it, no capability is
granted, and there is no path from the webview to "run a program" anywhere in this
application.**

## Files kept on your computer

| File | What it is | Where |
| --- | --- | --- |
| `almena-app.log` | This program's records, **and the agent's**: the agent writes to a pipe and this application writes the record, under a target beginning `almena_agent::` so one file reads as two programs. Nothing that crossed the wire is written down — no token, no turn, no argument. Rotated at 10 MiB, ten kept. Deleting the directory while the application is closed costs nothing but the history. | macOS: `~/Library/Logs/<id>/`<br>Windows: `%LOCALAPPDATA%\<id>\logs\`<br>Linux: `~/.local/share/<id>/logs/` |
| `window-state.json` | Where the window was and how big, in pixels. Written by the window-state plugin, which fixes its own location. | The configuration directory for `<id>` |
| `preferences.json` | The palette, the identity colour, the language and the model the agent is asked for, whenever one of them has been chosen. Absent until then, and deleting it puts every one of them back to its default — which for the model means the agent's own, since this side deliberately does not know what that is. | The configuration directory for `<id>` |
| `window.json` | The same window's size, in points rather than in pixels, which is what makes it right on a second display. State rather than configuration, so it sits with the log; deleting it costs the remembered size and nothing else. | Beside `almena-app.log` |
| `agent/` (a directory) | Empty, and it stays empty. The agent is started with it as its working directory and as the place it may read from, because the agent's own default for that is a *relative* path that would resolve somewhere absurd from an installed application. This application hands the agent nothing and names no resource, so nothing is ever written into it. | The cache directory for `<id>` |
| The login entry | Written only while [*open at login*](#open-at-login-which-is-not-the-same-as-running-in-the-background) is on, and removed when it is turned off. Its location is the system's rather than ours, as it must be: an entry is only a login entry where the system looks for one. | macOS: no file — `SMAppService` keeps its own register<br>Linux: `~/.config/autostart/Almena.desktop`<br>Windows: a value under `HKCU\…\CurrentVersion\Run`, not a file |

`<id>` is the bundle identifier, and it is still the scaffold's `network.almena.desktop` — see
[What is not here yet](#what-is-not-here-yet).

## The mark

The application's icon is generated, not hand-placed. `assets/branding/` holds the artwork and
`task icons` turns it into every size and format the three platforms ask for, all of them under
`src-tauri/icons/`.

| Source | What it is for |
| --- | --- |
| `app-icon.png` | The icon everywhere except macOS. Also the source of `icon.png` and `icon.ico`. |
| `app-icon-macos.png` | The same mark with the padding macOS expects inside its rounded square. Without it the icon sits visibly larger in the Dock than its neighbours. |
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
not the platform, not the user agent, not whether there is a touch screen. A window somebody
dragged narrow and one they dragged wide are the same case and get the same layout.

| Width | The navigation is |
| --- | --- |
| Below 600 | A menu floating across the bottom, icons above their names |
| 600 and above | A sidebar down the left, icons beside their names |

Those are the same buttons in the same order and the same place in the document.
`src/styles/shell.css` moves them, and 600 appears once in the whole project — as
`--breakpoint-expanded` in `src/styles/tokens.css`, where Tailwind's own five breakpoints are
cleared so that `sm:` and `md:` do not exist here. There is no hook, no `matchMedia` and no
component that asks how wide it is.

Four sections, every one of them with a screen behind it, and every one of them drawn every
time — there is one shape of this application now, so nothing is listed for some devices and
not others. Four is also close to what the compact shape has room for: at 400 points across, a
fifth entry leaves each one around 70 points, and 44 of that is what a finger is entitled to. So
a fifth section is a line added to a list and a sixth is a change to the navigation.

A section that holds more than one screen draws a **second navigation across the top of
itself**, and it is the first one turned on its side: the same `<nav>` of buttons carrying
`aria-current="page"`, wrapping rather than scrolling sideways, one shape at every width. The
one difference is that it does not wear the identity colour — that says which *section* you are
in, once — and that the menu **is** the screen's heading: a section with one screen titles
itself, a section with a menu does not, because the menu has already named the screen showing. Its screens are declared beside the sections in `src/features/shell/sections.ts` as a
tuple of two or more, so a menu with a single entry in it does not compile, and a section with
one screen simply has none. Three of the four sections have one: Network is *The network* and
*Peers*, AI is *Conversation* and *Agent*, Settings is *Appearance* and *General*. The first
screen has one screen and titles itself, which is the same rule and not an exception to it.

**Every element on those screens comes from [shadcn/ui](https://ui.shadcn.com)**, vendored into
`src/components/ui/` by `pnpm dlx shadcn@latest add <name>` and left as the registry wrote it:
`alert`, `badge`, `button`, `card`, `empty`, `field`, `item`, `label`, `select`, `separator`,
`spinner`, `switch`, `textarea`, `toggle` and `toggle-group`. A screen imports what it needs and never
writes a control of its own, which is the only arrangement in which changing how a button looks
changes how every button looks. One of the fourteen is drawn by nothing: `toggle` is where
`toggle-group` gets its class list, and the registry ships the two together.
`components.json` is the configuration that command reads; its `aliases.utils` points at
`@/lib/cn`, because this project has no file called `utils`.

Three of those are worth naming because they carry a promise rather than a look. `Empty` is what
a region says when it holds nothing, and this project only ever reaches it through `EmptyState`,
which makes the *reason* a required prop — "Nothing to show" is not a sentence shipped here.
`Alert` and `FieldError` are how a refusal reaches a person: both carry `role="alert"`, so they
are absent until there is something to say and are read out by arriving.

`src/components/` beside it holds what shadcn/ui has no answer for, built out of the elements
that it does: `Logo`, `Figure`, `StateBadge`, `Setting`, `CardGrid` and `EmptyState`. Nothing in
there invents a second way of drawing a surface or a control, and a component that the registry
turns out to have an element for is deleted in favour of it.

Two of the registry's own answers are deliberately **not** taken. `sidebar` would bring five
elements no screen draws — `sheet`, `tooltip`, `input`, `skeleton` and a `use-mobile` hook with
a second breakpoint at 768 — and below that breakpoint it is a hamburger opening a drawer,
which is worse than a bar that is simply there. `tabs` would fit the navigation but decides its orientation in
JavaScript, and here the shape follows the width of the viewport and nothing else. The
navigation is therefore a `<nav>` of shadcn buttons carrying `aria-current="page"`.

The set holds what the interface actually draws today and grows with the screen that needs the
next thing —
[interface.md](https://github.com/almena-network/almena-network/blob/main/.agents/rules/interface.md)
is the whole of the agreement, including which of each element's variants a screen may draw.

**Every value comes from `src/styles/tokens.css`** — the palette, the shape, the spacing and the
type scale, declared once as a Tailwind theme and reached only through utilities. Tailwind's
default colour palette is cleared there too, so `bg-red-500` does not exist and the only colours
on a screen are the ones this project named. The identity colour is `primary`; shadcn/ui's
`accent` is a hover grey and means nothing about identity.

```
components.json       what `shadcn add` reads: the style, the icon set, the aliases
src/
  components/ui/      vendored shadcn/ui elements — not ours to write
  components/         Almena's own compositions of them, and the mark
  hooks/              behaviour and state over time
  features/           one directory per feature: its screens and their own pieces
  styles/
    index.css         the one entry Tailwind is handed; it imports the four below
    tokens.css        every value, as a Tailwind theme, and both palettes
    base.css          the document: selection, focus, scrollbars, the canvas
    screen.css        the column a screen is laid out in, and its two text styles
    shell.css         the frame, in its two shapes
  lib/cn.ts           the one function the vendored elements import
```

There is no `tailwind.config.js`: Tailwind 4 is configured in CSS, and `tokens.css` is that
configuration.

One of Almena's own is worth naming here because it is this project's Transparency principle
made into code, and because it is the one thing the registry has no element for. `Figure` draws
a value beside what it is a measurement of, and a value it was given as `null` comes out as a
dash that says *not measured* to a screen reader. That is not the same as nought: a peer count
of nought is a measurement, and there has been none.

**A status strip runs along the bottom of the window**, 28 points tall, spanning the full
width — under the sidebar as well as beside it. It belongs to the frame rather than to a
screen: it is pinned rather than scrolled to, it is the same strip whichever section is open,
and in the compact shape the floating menu sits above it instead of over it.

It has two groups. The right one holds what does not change while the application runs: whether
this is a development build, the version, and the licence. **The left one is where what the
application is doing will go** — which network, how many peers, what it is waiting on — and it
is empty today rather than filled with something plausible, because a status strip is the worst
place in an interface to invent a value.

**A development build says so, on that strip, always.** The word *Development* is there whenever
the binary was built with `debug_assertions` — `task dev`, `task dev:cli`, `task build:debug` —
and absent from anything anybody was given. The strip is the only thing in the application that
is on screen whatever section is open and whichever shape the window is in, which is what makes
it the one place a marker like that can live. It is brighter than everything else there because
it is the one thing on the strip that should be noticed, and it is not drawn in a state colour:
being a development build is not one of the four states, and borrowing one would cost all four
their meaning.

The first screen titles itself and carries two cards. It carried the mark and the product's
name too, until those moved to the head of the navigation where they are bigger and where a
person meets them before any screen. One card says what the application is and that it is on no
network, which is the whole of what it can honestly say — the peer-to-peer layer is not here,
so no figure on it is measured and none is invented. The other is the one thing this build can
actually do, which is [send a notification](#notifications). They flow rather than stack: side
by side once there is room for both, one above the other the moment there is not, out of a
single auto-fitting grid in `src/components/CardGrid.tsx` and with no breakpoint of their own.

Network carries those two cards on **two screens**: what is known about the network this node
is on, and the peers it is talking to. AI carries two as well — the conversation, and the agent
itself, which is where its version and the model in force are drawn. Both had crossed on
`agent_status` since it was written and neither was drawn anywhere. **All of it is a dash and an empty state**, because there is no peer-to-peer
layer and therefore no network, no identity and no peer. What is real is the machinery — a
reading taken every ten seconds and on demand, a refresh button, and the time of the last look
beside it so that pressing the button does something a person can see. The list draws a peer
the day there is one, without this screen changing.

The peer list has **three** ways of holding nothing and says which, because they are three
different facts: nobody has looked yet, there is no network to have peers on, or there is a
network and it has none. A screen that shared one sentence between them would be telling a
reader the wrong one most of the time —
[honest-emptiness.md](https://github.com/almena-network/almena-network/blob/main/.agents/rules/honest-emptiness.md)
is the agreement, and it is why `readNetwork` returns `null` rather than an empty array.

Settings holds four cards across **two screens**, and it opens on the first of them every time.
**Appearance** is a screen of its own: the palette — dark, light, or whatever the operating
system is asking for — and the identity colour, one of five, which is the one place in the
application with five of them on it at once because they are the thing being chosen.
**General** holds the other three. **Language** is English or Spanish, and it is empty of
consequence until somebody uses it: Almena opens in the language the device asks for and stores
nothing until asked to. **Model** is which model the agent is asked for. **Open at login** is
whether the operating system starts Almena when somebody logs in.

Both appearance choices are attributes on the document element, written by
`src/lib/appearance.ts` and read by `src/styles/tokens.css` alone. No screen knows which
palette it is in, and no component branches on one.

## Languages

English and Spanish, both complete from the first screen. English is the source language and
the fallback: a missing translation degrades to readable English rather than to a bare key.

The device is asked first: a Spanish computer opens a Spanish Almena, and nothing is stored
until somebody chooses otherwise on the Settings screen. Until then, changing the language of
the device changes Almena's with it. Nobody is asked at startup which language they want —
the operating system already knows.

`task catalogs` checks that both hold exactly the same keys, and `task check` runs it. Typing
every catalog against the English one means `tsc` also fails on a key added to one and
forgotten in the other.

## What is not here yet

Stated rather than discovered by running something and being surprised.

| | Today |
| --- | --- |
| The peer-to-peer layer | Not written yet. Almena is the node, but neither program joins a network, reads the configuration a network is described by, or speaks to a peer; the first screen and the node's view both say so, because that is the whole truth available to them. |
| Anything a client can reach | Nothing. A phone or a tablet is a client and not a node, and it is built in [its own repository](https://github.com/almena-network/client); which API it speaks, over which protocol, and who serves it are open questions, and this side of the answer is not written. No node here offers a client anything, and nothing here shows a code for one to read. |
| `task check` | Rust and `tsc` only. ESLint, Prettier and the frontend test suite are not installed yet, so `task test` runs Rust alone and there is no `task format` for TypeScript. Until ESLint arrives, the limits on file and function size and the ban on arbitrary Tailwind values are a reviewer's rather than a tool's. |
| Where updates come from | The updater plugin is registered and inert. Which host serves the releases and which key signs them are both undecided, so `endpoints` and `pubkey` are empty and nothing calls the plugin. See [Registered, and not yet called](#registered-and-not-yet-called). |
| `--help` in one language | The one thing a person reads that does not come from a catalog. `clap` builds it from the CLI's argument declarations before a catalog can be loaded, so it is English wherever it is read. The drawn view is not affected and is in both languages. |
| The node's identity | The CLI has somewhere to keep one and nothing to put there. A node is identified by a key generated on its own device, and which kind of key that is belongs to the peer-to-peer layer above. Until then a node is a different participant on every run, which costs nothing while there is no network to be a participant in. |
| A stored language for the CLI | It reads `LC_ALL`, `LC_MESSAGES` and `LANG` and there is no way to overrule them, because overruling would mean storing a choice. The Settings screen does exactly that for the windowed application; the CLI has no settings and is not getting any. |
| Packages for the CLI | `task build:cli` produces a binary. The CLI is named for Fedora and Ubuntu, and which packages that becomes — the `.deb` and `.rpm` families the desktop already bundles for — is undecided. |
| Running the node as a service | `--quiet` is what a service manager runs, and nothing here writes a unit file, a plist or a service entry. Whether shipping those is Almena's job or the operator's is undecided. |
| Signing an agent on macOS | A PyInstaller tree is hundreds of Mach-O objects under `Contents/Resources/`, and notarization wants every one signed with a Developer ID and a secure timestamp, inside-out, before Tauri bundles — plus `disable-library-validation` for a hardened runtime loading a tree of `.so` files. No Apple Developer identity is decided anywhere in this project. Unsigned local builds work; a downloaded, notarized one carrying an agent is unresolved. |
| Where an agent build comes from | `task agent:stage` copies from a directory somebody points it at, because the agent is a separate repository. Fetching a published one would need a host that serves releases and a key that signs them — the same undecided pair as the updater's. A build with nothing staged says so on its own screen; what is still open is a *release* built that way and nobody noticing. |
| Which models are offered | A fixed list of names Almena knows how to ask for, with no discovery behind it: nothing has asked the model server what it serves. The Settings card says so, and `model_unknown` is what comes back for a name that is not there. |
| Handing the agent anything to read | The application hands it **nothing** and names no resource — it is pointed at an empty directory and asked questions with no sources. What should be handed over, and how somebody would choose, is not decided. |
| A capability the agent could ask for | `tool_call` is in the contract and nothing can produce one: the set of capabilities is empty, so every name is refused. The decision recorded is who executes and who asks; the first real capability is a change on both sides at once. |
| Reaching a hosted model | The agent's provider abstraction is the base URL, so a hosted provider is already reachable in principle — what is missing is a credential, and no way to hand it one exists. Where an application governed by Anonymity and No personal data would keep a secret is a question of its own, unanswered. |

## Contributing

[CONTRIBUTING.md](CONTRIBUTING.md) says what a change is expected to follow, how it is closed,
and what `task check` does and does not cover today. By taking part you agree to the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Security

Do not open a public issue for a security problem. [SECURITY.md](SECURITY.md) says how to report
one privately and what happens next.

## License

Apache License 2.0 — see [LICENSE](LICENSE). Copyright 2026 The Almena Network Authors.
