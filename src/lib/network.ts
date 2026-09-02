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
  const [facts, origin] = await Promise.all([
    invoke<Facts>("node_facts"),
    invoke<string | null>("interface_at"),
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
  };
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

/** Where to look for the network, when not in its own zone through this machine's resolver. */
export interface WhereToLook {
  /** Another zone to look in, or empty for the network's own. */
  zone?: string;
  /** Seed records given by hand, written as the zone writes them, or none to ask the zone. */
  seeds?: string[];
}

/**
 * Puts this node on a network, by asking somebody already on it for the record.
 *
 * **The only thing anybody is asked is which.** Finding a seed, pulling the record and announcing
 * are the node's own work — a wizard that walked an operator through them would be asking for
 * presses on steps they cannot judge. A zone or a seed given by hand is for a network being tried
 * out on one machine; a seed only ever says *somebody is there*, which is the safe direction.
 *
 * @param which - Which network.
 * @param port - The port to listen on, which is the one that gets published in the zone.
 * @param where - Another zone, or seeds by hand.
 * @throws The identifier of whatever went wrong, which the interface looks its words up by.
 */
export async function joinANetwork(
  which: Which,
  port: number,
  where: WhereToLook = {},
): Promise<Facts> {
  return invoke<Facts>("join_a_network", {
    asked: { which, port, zone: where.zone, seeds: where.seeds ?? [] },
  });
}

/**
 * Opens a network, on the zone's word that there is nobody to join.
 *
 * **A production network is opened once in the history of the platform**, not once per machine —
 * and what stops a second one is the zone answering that somebody is already there, not this
 * function's manners. The core refuses to open production on a format that is still moving; see
 * {@link freezeChecklist} for showing that before it happens rather than after.
 *
 * `nobodyIsThere` opens without asking the zone, on the operator's word. Development alone: the
 * node refuses it for production before anything happens.
 *
 * @throws The identifier of whatever went wrong.
 */
export async function openANetwork(
  which: Which,
  zone?: string,
  nobodyIsThere = false,
): Promise<Facts> {
  return invoke<Facts>("open_a_network", { which, zone, nobodyIsThere });
}

/** What taking a place on the mesh is asked with. */
export interface Place {
  /** The port to listen on, which is the one somebody publishes. */
  port: number;
  /** Whether this node carries other nodes' traffic. */
  carry?: boolean;
  /** Whether this node holds post for other people, and says so in the record. */
  mediator?: boolean;
  /** Relays to ask to carry this one, for a node that cannot be dialled. */
  carriedBy?: string[];
}

/**
 * Takes this node's place on the mesh, so that other nodes can reach it.
 *
 * The port is chosen and not discovered: it is the one somebody publishes in the zone, and a node
 * that took whatever was free would make that record wrong on its next start.
 *
 * @throws The identifier of whatever went wrong.
 */
export async function joinTheMesh(place: Place): Promise<void> {
  return invoke<void>("join_the_mesh", {
    asked: {
      port: place.port,
      carry: place.carry ?? false,
      mediator: place.mediator ?? false,
      carriedBy: place.carriedBy ?? [],
    },
  });
}

/**
 * Serves the interface on `address`, under the node's own key unless a certificate is named.
 *
 * @throws The identifier of whatever went wrong.
 */
export async function serveInterface(
  address: string,
  certificate?: string,
  privateKey?: string,
): Promise<void> {
  return invoke<void>("serve_interface", { address, certificate, privateKey });
}

/**
 * Closes this node for good, so that it stops counting.
 *
 * **Not how a node is taken down for the afternoon.** A closed node does not come back; coming
 * back means a new node, with a new key and a new name.
 *
 * @throws The identifier of whatever went wrong.
 */
export async function closeThisNode(): Promise<void> {
  return invoke<void>("close_this_node");
}

/** One line of the format's freeze checklist. */
export interface Line {
  /** What is being asked. */
  called: string;
  /** What went wrong, where something did. `null` is a line that holds. */
  wanting: string | null;
}

/**
 * Whether this build's format may be frozen, item by item.
 *
 * **The question, without the act**: nothing is opened, joined or written. It is the same list the
 * core holds a production network to, so it is a preview of that answer and never a second opinion.
 */
export async function freezeChecklist(): Promise<Line[]> {
  return invoke<Line[]>("freeze_checklist");
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

/**
 * A challenge for whoever is claiming this node, good for `forEpochs` hours.
 *
 * **The challenge and not the identifier.** A node's identifier is public — it is in the record and
 * in the zone — so a code carrying only it could be answered by anybody who looked it up. What the
 * challenge adds is a nonce this node made and remembers, which is what makes an approval of it an
 * approval by somebody who was in front of this machine.
 *
 * @throws The identifier of whatever went wrong.
 */
export async function claimingCode(forEpochs: number): Promise<string> {
  return invoke<string>("who_contributed_me", { forEpochs });
}
