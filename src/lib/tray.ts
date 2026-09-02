/**
 * Putting this application on the system tray, named in the reader's own language.
 *
 * The tray is built in Rust, because it is a native thing and there is no other way to build
 * one. Its menu, though, is text a person reads, and the catalogs are on this side — so the
 * entries are looked up here and handed over. This file is the whole of that hand-off and
 * holds no text of its own.
 *
 * The state is handed over as a whole reading rather than as a key for the same reason: which of
 * the four the node is, is a word a catalogue holds and the Rust side has none.
 */

import { invoke } from "@tauri-apps/api/core";

/**
 * Asks the Rust side to put the tray icon on the bar.
 *
 * Builds no second tray on a second call: the Rust side keeps one however many times it is
 * asked and renames its entries instead, which is what makes this safe to call from an effect
 * that runs again every time a webview reloads — and what makes it the way the state on the
 * menu is kept current.
 *
 * A failure goes no further than the Rust log, because there is nothing this side could do
 * about it and nothing useful it could say. What the failure costs is covered there too — a
 * window will not hide itself away when there is no tray to find it in again.
 *
 * @param state - What the node is doing, already read. The first entry, and not pressable.
 * @param show - The name of the entry that brings the window back, already translated.
 * @param quit - The name of the entry that ends the application, already translated.
 */
export async function installTray(
  state: string,
  show: string,
  quit: string,
): Promise<void> {
  try {
    await invoke("install_tray", { state, show, quit });
  } catch {
    // Said in the Rust log, where the reason is.
  }
}
