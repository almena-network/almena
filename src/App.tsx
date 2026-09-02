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
   * is what the one press on that screen settles — it joins production, and nobody is asked which
   * — and a shell drawn before it would be four sections reporting nothing beside the press that
   * would fill them.
   *
   * Read here rather than remembered: whether this node has a network is the node's own answer, and
   * a flag in the browser would be a second one to disagree with it after a directory was moved.
   */
  const { reading, state, refresh } = useNetwork();
  /*
   * **A start is not a step somebody takes.** A node is a directory with a key in it, and the same
   * directory is the same node however many times it is started — so the first thing this does is
   * come back to whatever network that directory already holds. Only a directory holding nothing
   * reaches the first screen, which is how a second start goes straight to the sections instead of
   * asking again for a press that was made once.
   */
  const [back, setBack] = useState(false);
  /*
   * **Whether the one press has been made, which is not the same as whether it worked.**
   *
   * The walk used to end when the node was on a network, because it could not end any other way:
   * every screen behind it was about a node that had one. It ends on the press now — a start that
   * could not join lands in the frame like any other, where the state says what went wrong and the
   * controls that do something about it are. Holding somebody on a screen with one button and a
   * refusal under it would be the application refusing to open because the network did not answer.
   *
   * It lives for this run and is written nowhere. A launch that finds a directory holding no record
   * is a machine that is not a node yet, whatever anybody pressed yesterday, and that is exactly
   * what has to reach the press again.
   */
  const [started, setStarted] = useState(false);
  // **Once, and the dependency list says so.** Coming back is what a launch does, not what a
  // render does: it takes the directory, reads the record and brings the node up. Asked for again
  // it is mostly harmless — a node already up is answered with what it is — but on a machine with
  // no node it writes *stopped* every time, over whatever the state had become. That is how a
  // start that could not join reported *failing* for one render and *stopped* ever after.
  useEffect(() => {
    void comeBack()
      .catch(() => null)
      .finally(() => {
        setBack(true);
        refresh();
      });
    // `refresh` is stable for the life of the hook, so this runs on the first render alone.
  }, [refresh]);

  /*
   * The tray is built on the Rust side and named on this one: its menu is text a person reads and
   * the catalogs are here. Asked for once at startup, and harmlessly again whenever a webview
   * reloads — the Rust side keeps one tray however many times it is asked and renames it instead.
   *
   * **The state is one of its entries, so this runs again whenever the node does something else.**
   * With the window put away the tray is all that is left on screen, and a tray still saying
   * *running* over a node that stopped would be worse than one saying nothing. Before the first
   * look has come back there is no state and the entry says so, which is not the same as saying
   * the node is stopped.
   */
  useEffect(() => {
    const doing =
      state === null ? t("tray.looking") : t(`status.node.${state.state}`);
    void installTray(doing, t("tray.show"), t("tray.quit"));
  }, [t, state]);

  // Nothing is decided until coming back has been tried and the first reading is in. Drawing the
  // walk over a node that has a network — for the moment before either has answered — would be
  // offering to join one twice.
  if (!started && back && reading !== null && reading.network === null) {
    // **Outside the shell, and that is the point.** Until the press is made there is no node and
    // nothing for four sections to be about; a frame drawn around them would be four screens
    // reporting nothing beside the one press that would fill them. The frame comes with the press
    // — whether or not the press found a network, which is what `started` is for.
    return (
      <Onboarding
        onStarted={() => {
          setStarted(true);
          refresh();
        }}
      />
    );
  }

  const Screen = SCREENS[section];

  return (
    <AppShell section={section} onSelect={setSection} state={state}>
      <Screen />
    </AppShell>
  );
}

export default App;
