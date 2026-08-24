/**
 * Putting a notification on the screen of the device this is running on.
 *
 * Two of the three platforms refuse one until they have been asked, and the asking has to
 * happen where a person can answer it, so it lives here rather than at startup: a permission
 * dialog nobody expected, before anybody asked for anything, is a worse thing than a
 * notification that arrives one press later.
 *
 * **No text lives in this file.** What a notification says arrives from the caller, which is
 * the side that took it out of the catalogs — `.agents/rules/language.md`. The Rust
 * side has its own way to the same plugin, in `src-tauri/src/notification.rs`, for the code
 * that runs with no window in front of it.
 */

import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

/** What became of a notification somebody asked for. */
export type NotifyOutcome = "sent" | "refused" | "failed";

/**
 * Shows a notification, asking the device for permission the first time.
 *
 * Asking twice is not a thing this does: every platform remembers the answer, and a person who
 * said no is told so on screen rather than asked again.
 *
 * @param title - The first line, already translated. The operating system draws the
 *   application's name beside it, so this is not the place to repeat it.
 * @param body - The rest, already translated.
 * @returns `sent` when the platform took it, `refused` when permission was not given, and
 *   `failed` when the plugin itself could not — which is the one case with nothing to say
 *   about why, because there is nowhere left to report it.
 */
export async function notify(
  title: string,
  body: string,
): Promise<NotifyOutcome> {
  try {
    const granted =
      (await isPermissionGranted()) || (await requestPermission()) === "granted";

    if (!granted) {
      return "refused";
    }

    sendNotification({ title, body });
    return "sent";
  } catch {
    return "failed";
  }
}
