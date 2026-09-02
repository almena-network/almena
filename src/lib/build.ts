/**
 * Which build this is, and therefore which network its node is for.
 *
 * # One question, and it decides something now
 *
 * It used to draw a marker on the status strip and nothing else. It decides **which network a
 * start joins**: a development build is for the development network, and a build somebody was
 * given is for the real one. Nobody is asked, because there is nothing here a person arriving can
 * judge — and because the answer is a property of the binary in front of them rather than a
 * preference.
 *
 * **It is the same decision the terminal takes with `--network`**, which defaults to development
 * for the same reason: what is in front of whoever is writing the software is not the real network.
 *
 * # Why this is safe in the direction that matters
 *
 * A shipped build has no way to reach a development network, and a development build has no way to
 * open a production one — the node refuses that on the argument, before anything happens. The two
 * mistakes that would cost something are both closed, and neither is closed by this file being
 * careful: one is the absence of a path, the other is a refusal below the interface.
 *
 * It is asked once, before anything is drawn, because it cannot change while the application runs.
 */

import { invoke } from "@tauri-apps/api/core";
import type { Which } from "@/lib/network";

/** Whether this is a development build. Settled by {@link loadBuild}, and `false` until it is. */
let development = false;

/**
 * Asks the Rust side which build this is, and holds the answer.
 *
 * The fallback is not a guess. A command that cannot be reached at all means there is no Tauri
 * around this interface, which is what happens when the frontend is opened in a browser against
 * the dev server — and `import.meta.env.DEV` is then the same question asked of the bundle
 * instead of of the binary, which is the only honest answer left.
 */
export async function loadBuild(): Promise<void> {
  try {
    development = await invoke<boolean>("is_development");
  } catch {
    development = import.meta.env.DEV;
  }
}

/** Whether this is a development build rather than one somebody was given. */
export function isDevelopmentBuild(): boolean {
  return development;
}

/**
 * Which network this build's node is for.
 *
 * The whole of the decision, in one place, so that the screen that starts the node and anything
 * that ever reports which network it meant cannot disagree about it.
 */
export function networkOfThisBuild(): Which {
  return development ? "development" : "production";
}
