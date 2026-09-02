/**
 * What this application knows about the node it is running.
 *
 * **It asks the node rather than deciding.** What a node reports about itself is worked out in one
 * place, below the interface, so that this window and the terminal cannot start answering the same
 * question differently — which is what happens the first time one of them is changed alone.
 *
 * A node with no network reports having looked at nothing, and every field is therefore `null`.
 * `null` is doing real work here: it is not zero. A count of nought is a measurement, and there
 * has been none — drawing one would be claiming a reading nobody took. `Figure` is the element
 * that knows how to show the difference.
 */

import { invoke } from "@tauri-apps/api/core";

/** What a node reports about itself, exactly as it crosses from below. */
interface Facts {
  /** The network it is on, or `null` where it is on none. */
  network: string | null;
  /** The key it is, or `null` where it has none. */
  identity: string | null;
  /** How many acts it has written down. */
  written: number | null;
  /** The root over them. */
  root: string | null;
  /** What it answers to on the mesh, worked out from its own key. */
  peer: string | null;
  /** How many peers it is connected to on the mesh, or `null` where nobody counted. */
  peers: number | null;
  /** How many nodes the record's observers have lately found silent, or `null` with no record. */
  silent: number | null;
}

/** What a look at the node found. */
export interface NetworkReading {
  /** Which network this node belongs to, or `null` where none was read. */
  network: string | null;
  /** This node's own identifier, or `null` where it has none. */
  identity: string | null;
  /** How many acts it has written down, or `null` where nobody counted. */
  written: number | null;
  /** The root over them, or `null` where there is none to take. */
  root: string | null;
  /**
   * What it answers to on the mesh, or `null` where there is no node to ask.
   *
   * The one thing a node knows that has to go into DNS: everything else the zone carries is the
   * operator's, and this is what turns a record saying where to call into one that says who
   * answers.
   */
  peer: string | null;
  /**
   * How many peers this node is connected to on the mesh — or `null`, meaning nobody counted,
   * which nought would not say.
   *
   * Read off the mesh socket's own handle and not off the record: a fact about connections.
   */
  peers: number | null;
  /**
   * How many nodes the record's own observers have lately found answering nothing.
   *
   * A fact from the record, drawn beside the peer count so the two are never confused: who this
   * node reaches is one thing, and who everybody's daily summaries say has gone quiet is another.
   */
  silent: number | null;
  /**
   * Where this node serves its interface, or `null` where it serves none.
   *
   * **Absent is a state.** A node that has not been asked to serve has no origin, and an address
   * standing in for one would send somebody to a door that is not open.
   */
  origin: string | null;
  /**
   * How many bytes this node keeps on disk, or `null` where there is no node to measure.
   *
   * **What it costs to be this node**: the key, the record, the roots and the entries as they are
   * right now. `null` is nothing measured and is never drawn as a nought — a node with no
   * directory has not been weighed, which is not the same as weighing nothing.
   */
  stored: number | null;
}

/**
 * Opens a development network, on the zone's word that there is nobody to join.
 *
 * **Development alone, and the node refuses production before anything happens** — not on what the
 * zone said, but on the word itself. A production network is opened once in the history of the
 * platform and never once per machine that started while its zone was quiet; there is no ordering
 * of events that reaches one being opened from this window.
 *
 * The zone is asked and its answer is what decides. **Silence is not nobody**: a zone that did not
 * answer has not said the network is empty, and the core is what draws that line.
 *
 * @throws The identifier of whatever went wrong, `there_is_a_network` included — which is not a
 * failure but the other outcome: somebody is there, so there was a network to join after all.
 */
export async function openADevelopmentNetwork(): Promise<Facts> {
  return invoke<Facts>("open_a_network", { which: "development" });
}

/**
 * What a zone would have to carry for this node to be a seed, or `null` where it has no place on
 * the mesh.
 *
 * **A draft, and nothing is published by asking for it.** The host name is left as a placeholder
 * because it is whoever keeps the zone's to choose; the port, the public key and the network's name
 * are the parts only this node can produce. It is composed below the interface so that this window
 * and the terminal hand over the very same record — a `_seed` is a commitment newcomers verify
 * against, and two implementations of it would one day disagree.
 */
export async function readSeedRecord(): Promise<string | null> {
  return invoke<string | null>("seed_record");
}

/** What has crossed this node's mesh, in bytes each way, since it came up. */
export interface Crossed {
  /** Bytes of record traffic read off the wire. */
  taken: number;
  /** Bytes of record traffic written to the wire. */
  given: number;
}

/**
 * How much record traffic has crossed this node, or `null` where it has no place on the mesh.
 *
 * **Record traffic and not every byte on the wire.** What is counted is the acts, the pages and
 * the roots this node asked for and answered with — the whole reason the mesh exists. The
 * handshake, the identify exchange, the pings and anything a relay carries for somebody else are
 * outside it, and a figure mixing them would answer neither question.
 *
 * **Totals since the node came up, never a rate.** A rate is two of these a moment apart, and how
 * far apart is a decision for whoever draws it.
 */
export async function readCrossed(): Promise<Crossed | null> {
  return invoke<Crossed | null>("crossed");
}

/** One peer this node is connected to. */
export interface Peer {
  /** What it answers to on the mesh: its `PeerId`, which is its key with a prefix. */
  peer: string;
  /** The address this connection is on, as a multiaddress. */
  address: string;
  /**
   * The last round trip to it in milliseconds, or `null` where none has come back yet.
   *
   * **Absent is not nought.** The first ping goes out after a connection settles, so a peer that
   * has just arrived has no round trip, and a zero would be the fastest connection on the list,
   * invented.
   */
  far: number | null;
}

/**
 * Who this node is connected to right now, or `null` where it has no place on the mesh.
 *
 * **An empty list and no list are different answers.** `null` is a node that has not taken its
 * place, where nobody has counted anything; `[]` is a node that has and is talking to nobody. Only
 * the second is a measurement.
 */
export async function readPeers(): Promise<Peer[] | null> {
  return invoke<Peer[] | null>("peers_connected");
}

/** The parts of a multiaddress worth reading at a glance. */
const TRANSPORTS = [
  "ip4", "ip6", "dns", "dns4", "dns6", "tcp", "udp", "quic", "quic-v1", "ws", "wss",
  "p2p-circuit",
];

/**
 * The transport a multiaddress names, the way a person reads it.
 *
 * `/ip4/127.0.0.1/tcp/4002/p2p/12D3…` reads as `ip4/tcp`. Which family, over which transport, is
 * what somebody wants at a glance; the numbers are the part they can already read in the address
 * itself, and the peer is in a column of its own.
 *
 * **Derived and never invented.** What comes back is only what the address says, and a part this
 * build has never heard of is left out rather than guessed at — an address made entirely of them
 * reads as empty, which is the honest answer to *this build cannot say*.
 */
export function transportOf(address: string): string {
  return address
    .split("/")
    .filter((part) => TRANSPORTS.includes(part))
    .join("/");
}

/**
 * Asks the node what it is, and reports what it said.
 *
 * Nothing is worked out here: every field comes back as the node gave it, `null` included. The
 * peer count stays `null` until the node has taken its place on the mesh, because until then
 * nobody has counted and a zero would be a measurement nobody took.
 *
 * @returns What the node reports about itself.
 */
export async function readNetwork(): Promise<NetworkReading> {
  // Both at once, because they are two independent questions about the same node and asking them
  // one after another would make a screen that draws them together draw two different moments.
  const [facts, origin, stored] = await Promise.all([
    invoke<Facts>("node_facts"),
    invoke<string | null>("interface_at"),
    invoke<number | null>("stored"),
  ]);
  return {
    network: facts.network,
    identity: facts.identity,
    written: facts.written,
    root: facts.root,
    peer: facts.peer,
    peers: facts.peers,
    silent: facts.silent,
    origin,
    stored,
  };
}

/**
 * One of the four states a node is in, and there is no fifth.
 *
 * They pair with the four badge tones, which is why there are four: a state is never carried by
 * colour alone, and a fifth would arrive with no colour to be drawn in.
 */
export type NodeStateWord = "stopped" | "starting" | "running" | "failing";

/**
 * What the node is doing, as the one answer decided below and read here.
 *
 * **Nothing on it is assembled on this side.** Whether a node is up is not something an interface
 * can work out from the facts it happens to have — a node holding a record, off the mesh and
 * serving nothing looks exactly like one that has not finished starting — so the node says, and
 * this is what it said.
 */
export interface NodeState {
  /** Which of the four. */
  state: NodeStateWord;
  /**
   * What went wrong, as the identifier the interface looks its sentence up by, or `null`.
   *
   * Never a sentence: the node has no idea what language anybody reads in, and two operators
   * comparing notes need the same word.
   */
  failing: string | null;
  /**
   * Which network this node is on — `development` or `production` — or `null` where it is on none.
   *
   * The word and not the network's own name: the name is a fact and is in the reading above, and
   * this is which of the two, which is what a person reads.
   */
  which: Which | null;
  /** Whether this node has a place on the mesh. */
  mesh: boolean;
  /** Whether this node is serving its interface. */
  serving: boolean;
  /** How many peers it is connected to, or `null` where nobody counted. */
  peers: number | null;
}

/**
 * Asks the node what it is doing.
 *
 * Answered whether or not there is a node: a directory holding no record is `stopped`, which is a
 * state and not a failure, so there is never a gap to tell from an answer.
 */
export async function readNodeState(): Promise<NodeState> {
  return invoke<NodeState>("node_state");
}

/**
 * Brings the node the rest of the way up: onto the mesh, and serving.
 *
 * **The same call a start makes.** Joining or opening leaves a node on its network and nothing
 * more; this is what makes it a node somebody can reach, on the port and the address the
 * preferences remember — and remembering them is the node's own doing, so nothing here has to.
 *
 * A mesh port somebody else has taken, or an address that will not bind, does not throw: the node
 * stays up and the state that comes back says what did not happen.
 *
 * @throws `no_network`, where there is no node to bring up.
 */
export async function comeUp(): Promise<NodeState> {
  return invoke<NodeState>("come_up");
}

/**
 * The link a client reads to choose this node: where its interface is, and who answers there.
 *
 * **Exactly the string the client reads**, built here so that both faces of the node draw the
 * same one. The address is `host:port` — the origin without its scheme, which is always `https`
 * — and the peer is the identity the zone carries and the client pins the certificate against.
 * `null` until both halves exist, because a link with one of them would be a door with no lock.
 *
 * @param reading - The last reading.
 * @returns The link, or `null`.
 */
export function nodeLink(reading: NetworkReading | null): string | null {
  if (reading === null || reading.origin === null || reading.peer === null) return null;
  const address = reading.origin.replace(/^https?:\/\//, "");
  return `almena://node?address=${address}&peer=${reading.peer}`;
}

/** Which of the two networks a node is being put on. */
export type Which = "production" | "development";


/**
 * Puts this node on a network, by asking somebody already on it for the record.
 *
 * **The only thing anybody is asked is which.** Finding a seed, pulling the record and announcing
 * are the node's own work — a wizard that walked an operator through them would be asking for
 * presses on steps they cannot judge. A zone or a seed given by hand is for a network being tried
 * out on one machine; a seed only ever says *somebody is there*, which is the safe direction.
 *
 * The port the record is pulled on — the one that gets published in the zone — is the node's own
 * to decide: what the preferences remember, and 4002 where they remember nothing. It is the same
 * port a start takes, and naming it here as well would be a second opinion about one fact.
 *
 * @param which - Which network.
 * @param where - Another zone, or seeds by hand.
 * @throws The identifier of whatever went wrong, which the interface looks its words up by.
 */
export async function joinANetwork(which: Which): Promise<Facts> {
  return invoke<Facts>("join_a_network", { asked: { which } });
}






/**
 * Erases this node from this machine, leaving a machine that is not a node.
 *
 * **The network is told first and the files go second**, which is the order and not a preference:
 * closing is an act this node signs into its own chain, and once the key is gone there is nothing
 * to sign it with. Then the directory — the key, the acts, the roots — and then the notes the node
 * kept in the preferences, so that the next launch comes back to nothing and the walk is what
 * happens instead.
 *
 * **It does not need a node that works.** A close that could not be said does not stop it: whoever
 * reaches for this is often the person whose node will not come up, and a way out that needed a
 * working node would not be one. What that costs is a node the record's observers find silent
 * rather than one that said it was going, which is why the control says so before it is pressed.
 *
 * What a person chose about the application — the palette, the colour, the language, the model —
 * is not the node's and is kept.
 *
 * @throws `no_directory` where the platform will not say where the data lives, and `not_erased`
 * where the files would not go, which leaves the node whole rather than half of it.
 */
export async function eraseThisNode(): Promise<void> {
  return invoke<void>("erase_this_node");
}



/**
 * Comes back to the network this node's directory already holds, if it holds one.
 *
 * **Not a decision and never offered as one.** A node is a directory with a key in it, and the same
 * directory is the same node however many times it is started — so this is what every start after
 * the first does, and `null` is a directory holding no record, which is what sends somebody to the
 * one decision that does have to be taken.
 *
 * @throws The identifier of whatever went wrong.
 */
export async function comeBack(): Promise<Facts | null> {
  return invoke<Facts | null>("come_back");
}

