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

/** One peer this node is connected to. */
export interface Peer {
  /** Its identifier: a key generated on its own device, and nothing else about whoever runs it. */
  id: string;
  /** The address it was reached at. */
  address: string;
  /** How the connection to it is faring, as one of the four states the project has. */
  health: "ok" | "warn" | "bad" | "idle";
}

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
  /** The peers — or `null`, meaning nobody counted, which an empty array would not say. */
  peers: Peer[] | null;
}

/**
 * Asks the node what it is, and reports what it said.
 *
 * Nothing is worked out here: every field comes back as the node gave it, `null` included. The
 * peers are still `null` because nothing counts peers — there is no mesh to count over, and a
 * zero would be a measurement nobody took.
 *
 * @returns What the node reports about itself.
 */
export async function readNetwork(): Promise<NetworkReading> {
  const facts = await invoke<Facts>("node_facts");
  return {
    network: facts.network,
    identity: facts.identity,
    written: facts.written,
    root: facts.root,
    peer: facts.peer,
    peers: null,
  };
}
