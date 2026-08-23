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

import NotBuilt from "@/components/NotBuilt";
import Home from "@/features/home/Home";
import Network from "@/features/network/Network";
import Settings from "@/features/settings/Settings";
import AppShell from "@/features/shell/AppShell";
import { sectionNameKey, type SectionId } from "@/features/shell/sections";
import { installTray } from "@/lib/tray";
import "@/styles/index.css";

/**
 * The screen each section shows, where it has one.
 *
 * A section missing from this map is one that is listed and not built, and it says so rather
 * than doing nothing when touched. That is what keeps an unfinished application from reading
 * as a broken one — and it is why this is a map with a gap in it instead of a chain of
 * conditions with no end.
 */
const SCREENS: Partial<Record<SectionId, () => React.ReactElement>> = {
  home: Home,
  network: Network,
  settings: Settings,
};

/** The application. */
function App() {
  const { t } = useTranslation();
  const [section, setSection] = useState<SectionId>("home");

  // The tray is built on the Rust side and named on this one: its menu is text a person reads
  // and the catalogs are here. Asked for once at startup, and harmlessly again whenever a
  // webview reloads — the Rust side keeps one tray however many times it is asked.
  useEffect(() => {
    void installTray(t("tray.quit"));
  }, [t]);

  const Screen = SCREENS[section];

  return (
    <AppShell section={section} onSelect={setSection}>
      {Screen ? <Screen /> : <NotBuilt title={t(sectionNameKey(section))} />}
    </AppShell>
  );
}

export default App;
