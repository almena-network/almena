/**
 * Putting this application on the system tray, named in the reader's own language.
 *
 * The tray is built in Rust, because it is a native thing and there is no other way to build
 * one. Its menu, though, is text a person reads, and the catalogs are on this side
 * (`.agents/rules/user-facing-text.md`) — so the entries are looked up here and handed over.
 * This file is the whole of that hand-off and holds no text of its own.
 */

import { invoke } from "@tauri-apps/api/core";

import { isDesktop } from "@/lib/platform";

/**
 * Asks the Rust side to put the tray icon on the bar.
 *
 * Does nothing anywhere but a computer, and nothing on a second call: the Rust side keeps one
 * tray however many times it is asked, which is what makes this safe to call from an effect
 * that runs again every time a webview reloads.
 *
 * A failure goes no further than the Rust log, because there is nothing this side could do
 * about it and nothing useful it could say. What the failure costs is covered there too — a
 * window will not hide itself away when there is no tray to find it in again.
 *
 * @param quit - The name of the entry that ends the application, already translated.
 */
export async function installTray(quit: string): Promise<void> {
  if (!(await isDesktop())) {
    return;
  }

  try {
    await invoke("install_tray", { quit });
  } catch {
    // Said in the Rust log, where the reason is.
  }
}
