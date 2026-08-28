# Conformance — `almena`

Which sections of `SPECS.md` this repository owns, and how much of each is built. The rule is
**`SPECS.md §13.13`**:

- **The matrix belongs to the project.** The specification says what must exist; this file says
  where it lives and how far along it is. `SPECS.md` keeps nobody's accounts.
- **A section closes where it is implemented, not where it is read.** A row moves to *done* when
  it points at code whose behaviour matches its section — not when something compiles or its
  tests pass.
- **When `SPECS.md` changes, the affected rows go to *under review*** until someone checks them
  against the new text.
- **If code and the specification disagree, the specification wins — or the specification is
  fixed.** What is never done is leaving the disagreement unspoken.

**States:** `not started` · `partial` · `done` · `under review`. Any row past *not started*
carries the path to the code that implements it.

**Phase** points at `PLAN.md`, which orders the work by dependency. This file answers *how much
is built*; that one answers *what comes next and why*.

> `SPECS.md`, `PLAN.md` and `AGENTS.md` live in the
> [almena-network](https://github.com/almena-network/almena-network) repository.

---

## Identifiers and resolution

| Section | What it requires here | Phase | State |
|---|---|---|---|
| §3.1 | The `did:almena` method: syntax, identifier as the hash of the creation operation (without `objeto`, without `firmas`), `dev` mark, multibase/multihash | F0 | done — `crates/almena-format/src/identifier.rs`, with base58btc against the published vectors, and `operation.rs` computing the name over `naming_bytes` so that a creation names itself |
| §3.2 | The three resolution answers, told apart and never mistaken for one another | F1 | partial — all four states exist in `crates/almena-store/src/chain.rs` and travel on the wire as a closed numeric vocabulary. **Two of the three are observed end to end** in `crates/almena-serve/tests/exit_criterion.rs`, which also **composes a DID document from materials** — walking the chain act by act and checking every signature, never asking the node for a finished answer. *Not here* has nothing producing it until the shared level does |

## The network (chapter 4 — the core of this repository)

| Section | What it requires here | Phase | State |
|---|---|---|---|
| §4.1 | Activable capabilities (registry API, mediator, status-list replication, mesh relay), announced in the node's DID document; mesh participation is not a capability but what makes a node a node | F1 · F2 | not started |
| §4.2 | Per-object chains as the seat of validity; hash log; cross-signed roots; what Sybil does and does not buy, including firmness anchored on measured history and served portion | F1 · F2 | partial — the chains are `crates/almena-store/src/chain.rs`, the Certificate Transparency tree with its inclusion proofs is `src/tree.rs` (held to a corpus a second implementation written from RFC 6962 agrees with), and the signed epoch root is `src/root.rs`, including the predicate for what a contradiction is and is not. **Exchanging and countersigning roots is F2**, and so is everything Sybil touches |
| §4.3 | Model A: forward only, no retroactive invalidation; reserved hole for entry-to-entry references | F1 | not started |
| §4.4 | Finality policy: revocations on one source, concessions waiting on roots **of nodes with measured history and served portion**, configurable threshold with conservative defaults | F2 | partial — **the two things it counts are both there and both checkable**: a node's root for an epoch, signed and held to the key its mesh name carries, and an inclusion proof against that same root carrying the size inside the signature. **And they are counted** — `crates/almena-store/src/firm.rs` says how many independent trees carry an act, holding each root to the key the record says that node has and refusing to count one node twice; proved with three nodes on a mesh. What is missing is the threshold: *how many is enough* depends on **measured history** and **served portion**, and neither exists |
| §4.5 | Two zones, three TXT record sets, the four rules; caching the last good set; the honest limit on whole-zone hijack | F1 | partial — `crates/almena-node/src/zone.rs` reads `_seed` and `_api`, refuses a seed with no peer identity and one that does not say which network it speaks for — **`net=`, without which somebody arriving for the first time cannot form the protocol name and would call whatever they were handed the network they joined**, keeps the last good set, and **tells a zone that is down from a zone that is empty** — collapsing those two is how a node opens a second network. **The lookup itself is not built** and needs a resolver; `_mediator` arrives with the mediator |
| §4.6 | What a node keeps: universal log, hot state universal / dormant distributed, checkpoints **verified against the log**, `N = 32`, replication factors, rotation seed as `hash(genesis) + period` | F2 | partial — **checkpoints are verified against the log**: `crates/almena-store/src/checkpoint.rs` makes every field cite the act that last set it, and the log's own index by object says whether a governing act came after, so a summary that leaves one out falls over without anybody being asked or believed. What governs what comes from the operation table and covers the objects that exist. Nothing *emits* a checkpoint yet, `N = 32` counts nothing, and neither the replication factors nor the rotation seed exist — every node still keeps everything |
| §4.7 | Write cost: per-object quota floor, per-connection limits as self-protection, hot/dormant classification with the anchored-reference rule. Credit is P4; `bind`/`unbind` arrive with node onboarding | F1 · P4 | not started |
| §4.8 | Replicate without understanding; *unresolvable, never stale*; **criticality marks** in the format; version announced and measured | F0 · F2 | partial — both marks are built in `crates/almena-format/src/field.rs`: parity for a new **field**, and a **closed vocabulary** for a new **value** in a field that already shipped, which parity cannot see. Announcing and measuring the version is F2 |
| §18 | The six reserved extension holes, **contemplated by the format** so that using one is an addition and not a migration | F0 | done — `crates/almena-format/src/holes.rs` transcribes the table, and its tests refuse a row that claims a protection its carrier cannot provide. **No number is reserved**, deliberately: a payload is a sparse integer-keyed map, so what had to be fixed was which mechanism covers each hole |
| §4.9 | Log entry and operation schema, `firmas` as a list, canonical CBOR (RFC 8949 §4.2, no floats), object and operation table, fork handling, hourly epochs, `emitida` validated against the epoch with one epoch of tolerance | F0 · F1 | partial — the profile is `crates/almena-cbor`, the entry and operation `crates/almena-format`, the fifty-one operation numbers `crates/almena-store/src/kind.rs`. Four of them are built: the genesis, a holder's chain, **a node's `announce`, which is its creation and gives it its name** (`crates/almena-store/src/announce.rs`), and **a contradiction, which carries its own proof** (`crates/almena-store/src/contradiction.rs`), the record itself `src/log.rs` (bytes as they arrived, `sujeto` indexed, inclusion proofs), and chaining, signature-against-the-previous-state, the epoch tolerance and fork handling `src/chain.rs`. **Holder acts and the genesis only**; every other object arrives with the work that builds it |
| §3.2 | The three resolution answers, told apart and never mistaken for one another | F1 | partial — all four states exist in `crates/almena-store/src/chain.rs`; *not here* has nothing producing it until the shared level does. Composing the DID document is the caller's, and there is no API to ask through yet |
| §4.10 | Genesis: opens the log, fixes epoch zero, declares the network, creates Almena Government self-signed; refuses if the zone already publishes seeds. Its hash **is the network's name** | F1 · F10 | partial — `crates/almena-store/src/genesis.rs`, tested end to end against the store and the tree: it opens the record, leaves a trust anchor that resolves, and refuses when there is anybody to join. **What it reads the zone from is not built** — the seeds are passed in — and production genesis is F10 |
| §4.11 | The crypto suite as a versioned protocol parameter: SHA-256, Ed25519 (control, nodes), P-256 (devices, ES256 issuance), BIP39 + SLIP-0010; the issuance key as its own concept | F0 | done — `crates/almena-suite`: SHA-256 against FIPS 180-4, both signature planes, and the seed derivation against SLIP-0010's own vector. The issuance key as a *concept* is F8, where something issues |
| §4.12 | The libp2p mesh: `/almena/<genesis-hash>/…`, `PeerId` derived from the node key, gossip plus incremental pull, per-hash request, **roots and witnesses outside the log** | F1 · F2 | partial — a root is already an artefact rather than an entry, signed and asked for by hash, in `crates/almena-store/src/root.rs`; it carries the network so one cannot be replayed onto another, and it names **the node** that published it rather than the network everybody shares. A root asked for over the API now comes back with the key and the signature, so a reader can hold the answer to the node that gave it — *the channel proves the machine; the signature proves the node*. **A node knows its own name on the mesh** — `crates/almena-node/src/peer.rs` derives it from the key that already persists, checked against a name produced outside this project, so the one value in the zone that comes from a node can be published before there is a mesh to use it. **The zone is read** — `crates/almena-lookup` asks DNS for the text records and keeps *did not answer* apart from *answered with nothing*, which is what arms §4.10's refusal to open a network where somebody already is. **A node takes a place on the mesh** — `crates/almena-mesh`, libp2p over TCP with Noise and yamux — built from the same key, with the network's own name inside the protocol name so two networks have nothing to negotiate. Its identity there is checked against what libp2p itself produces, so the value a zone publishes is the value a node answers to. **Two nodes pass the record between them** — `crates/almena-mesh/src/sync.rs`, pull by position and request by hash, in the project's own canonical bytes, proved between two real nodes over real sockets. What arrives is admitted by the same rule as anything else, so a peer vouches for nothing including itself. **And a node joins one rather than opening one** — `Node::join` builds a node from acts another handed it, admits them by the same rule, and announces itself; proved end to end from a `_seed` record read by the code that reads a real one. **And they keep themselves up to date** — `crates/almena-mesh/src/keeping.rs` dials the seeds, asks on meeting and then on a floor, answers the same question, and admits what arrives by the same rule; where it has got to is remembered per peer, because a position belongs to the record it is a position in. A node with no record bootstraps from whoever will hand one over. **Roots travel and are checked** — asked for by epoch, answered as the whole signed artefact, and held to the key the peer's own mesh name carries, which the connection already proved they hold; two different roots from one node for one epoch are kept as the pair they are. **Witnesses countersign and the word comes back**, so a node collects other people's signatures over the same bytes it signed and carries them with its root — which is what stops it showing one root privately to one observer and another to somebody else. **And a node says when it has grown or closed an epoch**, so nothing waits to be asked for — the notice carries nothing to believe, and everything that moves still moves by being asked for and admitted like anything else. **A contradiction is an object with its own chain** — `crates/almena-store/src/contradiction.rs` — carrying both signed roots, so it is checked by whoever reads it rather than believed; the record refuses anything that is not one, and two people finding the same pair write the same object. Acting on one, asking for one act by hash, the countersigning witnesses, contradictions as an object and everything measured is still **F2** |
| §4.13 | The node API: reading is not authenticated, writing is a signed operation, responses declare epoch and root, the three resolution answers, **errors as state and never as prose** | F1 | partial — the shape is `crates/almena-api` and the transport `crates/almena-serve`, over hyper, tested against real sockets. All four rules hold structurally: no signature takes a caller, handing over an act takes bytes alone, an act comes back byte-for-byte, and the limits are published so saying and doing can be compared. Every response is stamped, refusals and throttles included, and the state vocabulary is closed and numeric. **`404` is about the path and never about the object.** **A root now comes back signed** — its bytes, the key and the signature — so an answer can be held to the node that gave it. **TLS is there** and is the face's, not the transport's: `crates/almena-tls` turns a certificate and a key into something that wraps a stream, and serving still does not ask what it was handed. **An inclusion proof is asked for against an epoch and comes back with the signed root it is against**, so the size arrives inside the signature and whoever receives it can check it without being told anything else. Still missing: the listings, witnesses and measurements, which have no method on the core to serve from |
| §4.14 | UTC inside, deadlines counted in epochs; the reader's zone outside, always stated. **Universal row** | F0 | partial — `crates/almena-time` holds the epoch and every deadline the protocol counts in them; the reader's-zone half belongs to whatever draws a screen |

## Monitoring

| Section | What it requires here | Phase | State |
|---|---|---|---|
| §5.1 | Cross-observation, raw observations off-log, **daily summary over a UTC day** in the observer's own chain, proved contradictions | F2 | partly: summaries are written, over a UTC day, in the observer's own chain, carrying the hash of what they were drawn from, and nothing serves that; contradictions are published and checkable. Most of what §5 names is not measured — see the network view |
| §5.2 | Capacity and deficit by capability, protocol version fraction, propagation latency, concentration, diversity of root signers | F2 | not started |

## Messaging (mediator capability)

| Section | What it requires here | Phase | State |
|---|---|---|---|
| §6.2 | Mediator policy: mailbox per device grouped by account, quotas, expiry, deletion after delivery, inactivity, multiple mediators | F4 | not started |
| §6.3 | Notification: the wake-up endpoint is **private with the mediator**, never in the root DID; iOS relay with opaque handle; UnifiedPush; the push text travels as a translation key | F4 | not started |
| §6.5 | Doorbell separate from mailbox, per-account and per-relation quota **with a reserved floor**, rejection visible to the recipient, message states | F4 | not started |

## Entities, revocation, holder

| Section | What it requires here | Phase | State |
|---|---|---|---|
| §8.5 | The node **counts** individual owner signatures against the current set and the operation's threshold class — the registry has no seal | F5 | not started |
| §10.2 | Status-list distribution: only the version hash in the log, deterministic replication, publication node, *replica first* | F8 | not started |
| §11.12 | Deferred effect for operations signed by the control key alone, and validation of `cancel` from a live device or a guardian quorum | F3 | not started |

## The issuer and verifier SDK

Published from here and not a project of its own (§13): the same house that validates these rules
on one side of the wire writes the library that speaks them on the other, so a disagreement
between SDK and node is a bug inside one repository rather than an argument between two.

| Section | What it requires here | Phase | State |
|---|---|---|---|
| §9.1 | SD-JWT VC signed **ES256** with the issuer's issuance key; selective disclosure by commitment; proof type as a field; an occultable credential identifier; issuer identification method behind an interface | F8 | not started |
| §9.2 | Building the request in the adopted standard, **referencing the authorising template by hash**; anything exceeding it is a malformed request | F8 | not started |
| §9.3 | Challenges with nonce and verified identity; in an operation, the concrete content is part of what is signed | F8 | not started |
| §9.4 | Resolving a template from the registry and checking the request against it — never against what the verifier sent | F8 | not started |
| §10.1 | Bitstring status lists; **non-revocability as an explicit signed claim**, never an absent field; letting a verifier demand revocability | F8 | not started |
| §10.2 | Consulting status lists: never a version older than the freshest hash seen, *replica first and publication node only if stale*, any source valid because the hash decides | F8 | not started |
| §12.2 | Letting a verifier decide whether it accepts credentials from entities already closed | F8 | not started |
| §17.12 | **Distinguishing *"could not be verified"* from *"not valid"*** — if they are conflated, the integrator's staff learn to wave people through when the network fails | F8 | not started |

## This repository as a product

| Section | What it requires here | Phase | State |
|---|---|---|---|
| §13.4 | Two variants — windowed and terminal — as façades over one core with **no logic of their own**, plus an automated parity check; operator panel read-only; manual updates | F1 | partial — the core is `crates/almena-node`, `task isolation` keeps it out of both frameworks, and the **parity check runs in `task check`** against the table in `src/facade.rs`, where *not yet drawn* has to be written out in words. Four of seven capabilities are drawn by both faces from the same place: watching the node, choosing the language, opening a development network and serving the interface. **Serving never replaces drawing** — the node is held ready to answer from the moment it exists. Closing an epoch is the one left, and only because nothing starts a timer from a face yet. **A node now has a name as well as a key**: its first `announce` creates it and its hash is its DID, so the roots it publishes say who published them and two honest nodes no longer read as contradicting each other. **A node's identity is its directory's** — `src/identity.rs` writes the key once and reads it back after, so the same directory is the same node however often it starts. Its **record survives a restart** — `src/record.rs` keeps the acts and the roots beside the key, and `Node::rejoin` replays them — and **two processes over one directory are refused**, by `src/directory.rs` holding an operating-system lock for as long as the process lives. Closing an epoch is now offered by both faces, so every capability in the table is true in both or false in both with a reason |
| §13.9 | English and Spanish complete, English the default and fallback; **adding a language must not mean touching code**; log lines carry a stable code, not a translated phrase. **Universal row** | F0 | done — **the catalog directory is the list of languages**, read by Vite in the webview and by `cli/build.rs` in the terminal variant, so a language arrives as `src/i18n/locales/<tag>.json` and nothing else; each catalog carries its own `language.name`, so the picker never has to be taught one. `scripts/check-catalogs.mjs` holds every catalog to English in both directions, refuses a catalog with no name of its own, and refuses two that call themselves the same thing. Log lines already carried codes (`crates/almena-log`) |
| §13.9 · §13.4 | The **terminal variant** can be told what language to speak, and remembers it — the choice overriding what the environment says | F0 | done — `cli/src/preferences.rs` and `--language`, in this application's own configuration directory rather than the window's, because `almena-paths` and spec `0001` make them two applications. `cli/src/lib.rs::settle` is the order — asked now, chosen before, else the environment — with its own tests |
| §13.13 | This file, kept true. **Universal row** | F0 | partial — this file |
| §13.13 | **The golden vectors**, which are the contract with `client` rather than a shared library | F0 | done — `crates/almena-format/tests/golden_vectors.rs` carries the three criteria plus the closed-vocabulary case. **The twin belongs to `client` and is that repository's row**, scheduled where it is refounded; until then the corpus is held by one side only |
| §14.4 | Ballot sealing as a node capability, blind signatures, admission in both voting modes | P1 · P2 | not started |

---

## Replicated code — rows that exist twice

**The repositories share no code** (§13.13). The format lives here **and** in `client`, written
twice on purpose so that neither project owns the other's ground. What keeps the two copies from
drifting is not a library: it is **the golden vectors as the contract** — the same case file in
both repositories, each running it against its own implementation.

| Replicated with | Which rows | What a change owes |
|---|---|---|
| `client` | §3.1 (identifier), §4.9 (canonical CBOR, operation schema), §4.11 (suite), §4.14 (epoch arithmetic) | The same change in the other repository, **and the vector that covers the new case in both** — a case covered here and not there is where divergence enters |

---

## What is **not** this repository's

Named because the boundary is where duplicated work happens:

- **`did:peer` and the relationships that use it** (§3.3) — the holder's app.
- **Composing and drawing anything a person consents to** (§9.2) — the client draws it; the node
  never serves a screen.
- **The trust decisions themselves** (§7) — the node validates operations and counts signatures;
  who deserves a seal is Almena Government's judgement through the Registry.
- **The public catalogue and monitoring portal** (§13.6, §5) — the Registry serves them; this
  repository serves the API they read.
- **Holding or presenting a credential** (§9.5, §11) — the client. The node never sees one, and
  the SDK published here builds and verifies them without ever holding one either.
