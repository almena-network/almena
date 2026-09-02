/**
 * The network reading, kept current.
 *
 * Two things ask for a fresh one and they go through the same call: a timer, so a screen left
 * open does not go stale, and a person pressing refresh, because a timer nobody can see is not
 * something anybody should have to trust.
 *
 * The moment of the last look is part of what it returns. Without it the refresh button would
 * have no visible effect at all while the network does not exist, and a control that does
 * nothing observable is one nobody can tell from a broken one.
 */

import { useCallback, useEffect, useState } from "react";

import {
  readNetwork,
  readNodeState,
  readPeers,
  type NetworkReading,
  type NodeState,
  type Peer,
} from "@/lib/network";

/**
 * How often the screen looks again, in milliseconds.
 *
 * Ten seconds: often enough that a peer appearing is noticed in the time it takes to look up
 * from the keyboard, and rare enough to be nothing on a battery. It is a constant rather than
 * an argument because every screen showing this should agree about it.
 */
const EVERY_MS = 10_000;

/** A network reading, when it was taken, and the two ways to take another. */
interface Network {
  /** The last reading, or `null` before the first one has come back. */
  reading: NetworkReading | null;
  /**
   * What the node was doing at that same moment, or `null` before the first look.
   *
   * Taken with the reading and not on a timer of its own: the strip, the tray and the Network
   * screen all draw from this, and two timers would have them describing the node a second apart.
   */
  state: NodeState | null;
  /** When that reading was taken, or `null` before the first one. */
  lookedAt: Date | null;
  /** Whether a look is in flight. */
  looking: boolean;
  /** Takes another reading now. */
  refresh: () => void;
}

/** Keeps a network reading current, and hands back the means to force one. */
export function useNetwork(): Network {
  const [reading, setReading] = useState<NetworkReading | null>(null);
  const [state, setState] = useState<NodeState | null>(null);
  const [lookedAt, setLookedAt] = useState<Date | null>(null);
  const [looking, setLooking] = useState(false);

  const look = useCallback(async () => {
    setLooking(true);

    try {
      // Both at once, because they are two questions about one node at one moment.
      const [found, doing] = await Promise.all([readNetwork(), readNodeState()]);
      setReading(found);
      setState(doing);
      setLookedAt(new Date());
    } finally {
      setLooking(false);
    }
  }, []);

  useEffect(() => {
    void look();

    const timer = setInterval(() => {
      void look();
    }, EVERY_MS);

    return () => {
      clearInterval(timer);
    };
  }, [look]);

  // **Stable across renders, and that is not a detail.** A `refresh` rebuilt on every render is a
  // new value on every render, so every effect that lists it runs again — and one of them is the
  // effect that comes back to the node. `look` is already a `useCallback` over nothing, so this is
  // the same function for the life of the hook and an effect depending on it runs once.
  const refresh = useCallback(() => {
    void look();
  }, [look]);

  return { reading, state, lookedAt, looking, refresh };
}

/** Who this node is connected to, kept current, and the means to look again. */
interface Connected {
  /**
   * Who is connected, `null` where nobody counted, or `undefined` before the first look.
   *
   * Three values, because the screen draws three different things. `undefined` is *nobody has
   * asked yet*, which a moment later becomes one of the other two — and saying *no peers* in that
   * moment would report a result nobody obtained.
   */
  peers: Peer[] | null | undefined;
  /** Looks again now. */
  refresh: () => void;
}

/**
 * Keeps the list of connected peers current.
 *
 * The same ten seconds as every other look, from the same constant. It is a hook of its own rather
 * than another field on {@link useNetwork} because it is asked for by one screen: the figures at
 * the head of the Network screen are read on every one of them, and a list of hundreds of peers
 * fetched behind a screen that never draws it is a cost nobody asked for.
 */
export function usePeers(): Connected {
  const [peers, setPeers] = useState<Peer[] | null | undefined>(undefined);

  const look = useCallback(async () => {
    setPeers(await readPeers());
  }, []);

  useEffect(() => {
    void look();
    const timer = setInterval(() => {
      void look();
    }, EVERY_MS);
    return () => {
      clearInterval(timer);
    };
  }, [look]);

  const refresh = useCallback(() => {
    void look();
  }, [look]);

  return { peers, refresh };
}

/** What the node is doing, kept current, and the means to look again. */
interface Doing {
  /** What the node is doing, or `null` before the first look has come back. */
  state: NodeState | null;
  /** Looks again now. */
  refresh: () => void;
}

/**
 * Keeps only the node's state current, for whatever draws that and not the figures.
 *
 * The same ten seconds, from the same constant, because a screen showing the state twice at two
 * cadences would show it changing twice. It exists beside {@link useNetwork} rather than inside
 * it so that a card drawing what the node is doing does not have to be handed the whole reading
 * by whoever renders it.
 */
export function useNodeState(): Doing {
  const [state, setState] = useState<NodeState | null>(null);

  const look = useCallback(async () => {
    setState(await readNodeState());
  }, []);

  useEffect(() => {
    void look();

    const timer = setInterval(() => {
      void look();
    }, EVERY_MS);

    return () => {
      clearInterval(timer);
    };
  }, [look]);

  // The same reason as above: a hook that hands back a new function every render makes every
  // effect depending on it run every render.
  const refresh = useCallback(() => {
    void look();
  }, [look]);

  return { state, refresh };
}
