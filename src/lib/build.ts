/**
 * Which build this is.
 *
 * One question, and it is not the one `@/lib/platform` answers. That one is about what a
 * platform *has*; this one is about the binary in front of the person: whether it is one built
 * for whoever is writing it, or one somebody was given. Everything drawn from it says exactly
 * that and nothing more.
 *
 * It is asked once, before anything is drawn, because it cannot change while the application
 * runs. A marker that arrived a moment after the first screen would read as something
 * happening rather than as a fact about the build.
 */

import { invoke } from "@tauri-apps/api/core";

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
