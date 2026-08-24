/**
 * What a person chose, and what it takes to keep it.
 *
 * Four choices — the palette, the identity colour, the language, and which model the agent is
 * asked for — held in one file by the Rust side, which is where a file this application writes
 * belongs (`.agents/rules/storage-and-logs.md`). Nothing here knows what any of them
 * *mean*: this file is the store, and the vocabularies live where they are already written
 * down — `@/lib/appearance` for the two the interface is drawn from, `@/i18n` for the
 * language, `@/lib/models` for the model.
 *
 * The choices are read **once**, before anything is drawn, and held here afterwards. That is
 * what lets a screen ask what the palette is without waiting: a settings screen whose controls
 * arrive a moment after the rest of it is a settings screen that flickers into its own state.
 */

import { invoke } from "@tauri-apps/api/core";

/**
 * The four choices, as they are stored.
 *
 * Every one of them may be `null`, and `null` is not a default written down: it is nobody
 * having chosen. What the absence means is the reader's — the operating system's palette, the
 * icon's own colour, the device's language — and none of those is a value that could have been
 * stored in advance.
 */
export interface Preferences {
  /** The palette a person asked for, or `null` while they have asked for none. */
  theme: string | null;
  /** The identity colour a person asked for, or `null` while they have asked for none. */
  accent: string | null;
  /** The language a person asked for, or `null` while they have asked for none. */
  language: string | null;
  /**
   * The model the agent is asked for, or `null` while nobody has chosen one.
   *
   * `null` is not a default written down here either: it means the agent is told nothing and
   * uses its own, which is a value this side deliberately does not know — see
   * `@/lib/models`, which holds the list this side *can* name.
   */
  model: string | null;
}

/** Nobody has chosen anything. What a first launch has, and what a failed read falls back to. */
const NOTHING_CHOSEN: Preferences = {
  theme: null,
  accent: null,
  language: null,
  model: null,
};

/** What was read the last time anybody asked the Rust side. */
let held: Preferences = NOTHING_CHOSEN;

/**
 * Reads the choices from disk and holds them.
 *
 * Called once, before the first render. A failure is the defaults rather than an error: the
 * question could not be asked, and an interface drawn in the palette nobody chose is a working
 * interface, where one that refused to draw at all is not.
 *
 * @returns What is now held, which is what every later call to {@link preferences} answers.
 */
export async function loadPreferences(): Promise<Preferences> {
  try {
    held = await invoke<Preferences>("preferences");
  } catch {
    held = NOTHING_CHOSEN;
  }

  return held;
}

/** What a person has chosen, as of the last read. */
export function preferences(): Preferences {
  return held;
}

/**
 * Stores a choice and returns what is stored afterwards.
 *
 * The answer comes back from the store rather than echoing the request, which is what lets a
 * caller tell a change from a refusal by comparing the two — the same reason
 * `@/lib/openAtLogin` reads its setting back. A choice that could not be written is a control
 * that goes back to where it was, rather than one that claims something nobody kept.
 *
 * @param chosen - The choices that changed. The rest are carried over unread.
 * @returns Everything that is stored now.
 */
export async function choose(chosen: Partial<Preferences>): Promise<Preferences> {
  const wanted = { ...held, ...chosen };

  try {
    held = await invoke<Preferences>("set_preferences", { preferences: wanted });
  } catch {
    // The command could not be reached at all. Read them back the ordinary way rather than
    // reporting a choice nobody kept.
    return await loadPreferences();
  }

  return held;
}
