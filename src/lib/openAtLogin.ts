/**
 * Whether the operating system opens this application when somebody logs in.
 *
 * It goes through this application's own Rust side rather than straight at the autostart
 * plugin, and the reason is macOS: that platform keeps two registers — *Open at Login* and
 * *Allow in the Background* — and the plugin can only write the second, which is the wrong one.
 * `src-tauri/src/open_at_login.rs` is where each platform is served and where the difference is
 * set out.
 *
 * Running in the tray with no window is a different thing and is not a setting at all: it is
 * what the application does once it is running, whoever started it.
 */

import { invoke } from "@tauri-apps/api/core";

/**
 * Whether the system is set to open this application at login.
 *
 * @returns `false` when the question could not be asked — of the two wrong answers, the one
 *   that does not claim a setting nobody checked is the better.
 */
export async function opensAtLogin(): Promise<boolean> {
  try {
    return await invoke<boolean>("opens_at_login");
  } catch {
    return false;
  }
}

/**
 * Asks for opening at login to be turned on or off, and returns what it is afterwards.
 *
 * The answer comes back from the system rather than echoing the request, which is what lets a
 * caller tell a change from a refusal by comparing the two. Both happen: on macOS a person can
 * switch the registration off in System Settings and it stays off, and on every platform a
 * policy can decline outright.
 *
 * @param wanted - What the person asked for.
 * @returns What the system says now, which is not always what was asked.
 */
export async function setOpensAtLogin(wanted: boolean): Promise<boolean> {
  try {
    return await invoke<boolean>("set_opens_at_login", { wanted });
  } catch {
    // The command could not be reached at all. Read the setting back the ordinary way rather
    // than reporting a state nobody checked.
    return await opensAtLogin();
  }
}
