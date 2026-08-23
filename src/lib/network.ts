/**
 * What this application knows about the network it is on.
 *
 * Nothing, today, and saying so precisely is the whole of this file. There is no peer-to-peer
 * layer in this repository, so there is no network to be on, no configuration read from any
 * origin and no peer to have connected to.
 *
 * Every field below is therefore `null`, and `null` is doing real work: it is not zero. A peer
 * count of nought is a measurement, and there has been no measurement — drawing one would be
 * the exact thing `AGENTS.md` forbids under Transparency. `Figure` is the element that knows
 * how to show the difference.
 *
 * When the layer exists, this is the one file that changes. Everything above it already draws
 * whatever it returns.
 */

/** One peer this node is connected to. */
export interface Peer {
  /** Its identifier: a key generated on its own device, and nothing else about whoever runs it. */
  id: string;
  /** The address it was reached at. */
  address: string;
  /** How the connection to it is faring, as one of the four states the project has. */
  health: "ok" | "warn" | "bad" | "idle";
}

/** What a look at the network found. */
export interface NetworkReading {
  /** Which network this node belongs to, or `null` where none was read. */
  network: string | null;
  /** This node's own identifier, or `null` where it has none. */
  identity: string | null;
  /** The peers — or `null`, meaning nobody counted, which an empty array would not say. */
  peers: Peer[] | null;
}

/**
 * Looks at the network and reports what is there.
 *
 * Asynchronous with nothing to await, deliberately: the call that replaces this one crosses to
 * a node and every caller above is already written for the wait.
 *
 * @returns What is true of this build, which is that nothing has been measured.
 */
export async function readNetwork(): Promise<NetworkReading> {
  return await Promise.resolve({ network: null, identity: null, peers: null });
}
