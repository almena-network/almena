/**
 * Whether the operating system starts this application when somebody logs in.
 *
 * A thin way to the plugin and nothing else: no text and no policy live here, only the three
 * things a switch needs — what it is now, turn it on, turn it off.
 *
 * It answers on a computer alone. A phone's operating system owns when an application may run
 * and offers nothing to switch, so `platform.ts` is what a screen asks before it draws a
 * control for any of this.
 */

import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";

/** Whether the system is set to start this application at login. */
export async function autostartEnabled(): Promise<boolean> {
  return await isEnabled();
}

/**
 * Turns starting at login on or off, and returns what it actually is afterwards.
 *
 * The state is read back from the system rather than assumed from what was asked. The asking
 * can succeed and change nothing — a login item a policy forbids, a desktop session with
 * nowhere to write one — and a switch that moved with nothing moving behind it is a lie told
 * by an interface.
 *
 * @param wanted - What the person asked for.
 * @returns What the system says afterwards, which is not always what was asked.
 */
export async function setAutostart(wanted: boolean): Promise<boolean> {
  if (wanted) {
    await enable();
  } else {
    await disable();
  }

  return await isEnabled();
}
