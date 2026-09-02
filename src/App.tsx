/**
 * The application: which section is open, and the frame it is shown in.
 *
 * This is the only place that knows both — the frame draws whatever it is handed, and a screen
 * knows nothing about the navigation that led to it. Adding a destination is an entry in
 * `@/features/shell/sections` and a line below.
 *
 * The one stylesheet is imported here. There is exactly one — `@/styles/index.css` — because
 * Tailwind has to be handed a single entry: a stylesheet imported on its own by some component
 * is compiled without the theme, and every value in it is then a name nothing has defined.
 * What that entry pulls in, and in which order, is written at the top of it.
 */

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import Ai from "@/features/ai/Ai";
import Onboarding from "@/features/onboarding/Onboarding";
import Home from "@/features/home/Home";
import Network from "@/features/network/Network";
import Settings from "@/features/settings/Settings";
import AppShell from "@/features/shell/AppShell";
import { type SectionId } from "@/features/shell/sections";
import { useNetwork } from "@/hooks/useNetwork";
import { comeBack } from "@/lib/network";
import { installTray } from "@/lib/tray";
import "@/styles/index.css";

/**
 * The screen each section shows. Every section, with no gaps.
 *
 * The type is what enforces that: a total `Record<SectionId, …>`, so a section added to
 * `sections.ts` with nothing behind it fails `tsc` rather than being discovered by somebody
 * touching a navigation entry that does nothing. The navigation lists no destination that
 * leads nowhere.
 *
 * A screen with no data yet is still a screen. It is built whole and reports that it has
 * nothing, which is the same rule seen from the other side.
 */
const SCREENS: Record<SectionId, () => React.ReactElement> = {
  home: Home,
  network: Network,
  ai: Ai,
  settings: Settings,
};

/** The application. */
function App() {
  const { t } = useTranslation();
  const [section, setSection] = useState<SectionId>("home");
  /*
   * **What a node with no network is offered instead of the application.** Which network it is for
   * is the one decision that has to be taken before anything else means anything, and a shell with
   * four empty sections behind it would be four screens reporting nothing while the thing that
   * would fill them goes unasked.
   *
   * Read here rather than remembered: whether this node has a network is the node's own answer, and
   * a flag in the browser would be a second one to disagree with it after a directory was moved.
   */
  const { reading, refresh } = useNetwork();
  /*
   * **A start is not a step somebody takes.** A node is a directory with a key in it, and the same
   * directory is the same node however many times it is started — so the first thing this does is
   * come back to whatever network that directory already holds. Only a directory holding nothing
   * reaches the walk, which is how a second start goes straight to the screens instead of asking
   * again for a decision that was taken once.
   */
  const [back, setBack] = useState(false);
  useEffect(() => {
    void comeBack()
      .catch(() => null)
      .finally(() => {
        setBack(true);
        refresh();
      });
  }, [refresh]);

  // The tray is built on the Rust side and named on this one: its menu is text a person reads
  // and the catalogs are here. Asked for once at startup, and harmlessly again whenever a
  // webview reloads — the Rust side keeps one tray however many times it is asked.
  useEffect(() => {
    void installTray(t("tray.quit"));
  }, [t]);

  // Nothing is decided until coming back has been tried and the first reading is in. Drawing the
  // walk over a node that has a network — for the moment before either has answered — would be
  // offering to join one twice.
  if (back && reading !== null && reading.network === null) {
    // **Outside the shell, and that is the point.** The navigation leads to four sections that a
    // node with no network has nothing to say in, and offering them beside the one decision that
    // has to be taken would be offering a way to put it off. The frame comes back when there is
    // something behind it.
    return <Onboarding onJoined={refresh} />;
  }

  const Screen = SCREENS[section];

  return (
    <AppShell section={section} onSelect={setSection}>
      <Screen />
    </AppShell>
  );
}

export default App;
