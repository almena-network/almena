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

import { readNetwork, type NetworkReading } from "@/lib/network";

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
  const [lookedAt, setLookedAt] = useState<Date | null>(null);
  const [looking, setLooking] = useState(false);

  const look = useCallback(async () => {
    setLooking(true);

    try {
      setReading(await readNetwork());
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

  return {
    reading,
    lookedAt,
    looking,
    refresh: () => {
      void look();
    },
  };
}
