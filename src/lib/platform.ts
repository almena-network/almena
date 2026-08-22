/**
 * Which shape of the application this is.
 *
 * The one question here is not about layout. A layout follows the width of the viewport and
 * never the platform — `.agents/rules/screen-sizes.md` — and nothing in this file may be used
 * to choose one. This is about what a platform *has*: a tray to sit in, a login to start with.
 * A screen must not draw a control for something the device it is on does not have.
 *
 * It is asked of the Rust side because the binary knows which one it is, and a user agent is a
 * guess. A wrong guess here draws a switch that cannot move anything.
 */

import { invoke } from "@tauri-apps/api/core";

/**
 * Whether this is the build that runs on a computer.
 *
 * @returns `true` on Windows, macOS and Linux. `false` on iOS and Android — and also when the
 *   question could not be asked at all, because of the two wrong answers, the one that offers
 *   nobody a control that does nothing is the better.
 */
export async function isDesktop(): Promise<boolean> {
  try {
    return await invoke<boolean>("is_desktop");
  } catch {
    return false;
  }
}
