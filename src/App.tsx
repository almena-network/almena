/**
 * The application: which section is open, and the frame it is shown in.
 *
 * This is the only place that knows both — the frame draws whatever it is handed, and a screen
 * knows nothing about the navigation that led to it. Adding a destination is an entry in
 * `@/features/shell/sections` and a line below.
 *
 * The stylesheets every screen relies on are imported here, in the order they have to load:
 * the values first, then the document, then the two surfaces built from both.
 */

import { useState } from "react";
import { useTranslation } from "react-i18next";

import NotBuilt from "@/components/NotBuilt";
import Home from "@/features/home/Home";
import AppShell from "@/features/shell/AppShell";
import { sectionNameKey, type SectionId } from "@/features/shell/sections";
import "@/styles/tokens.css";
import "@/styles/base.css";
import "@/styles/panel.css";
import "@/styles/screen.css";

/** The application. */
function App() {
  const { t } = useTranslation();
  const [section, setSection] = useState<SectionId>("home");

  return (
    <AppShell section={section} onSelect={setSection}>
      {section === "home" ? (
        <Home />
      ) : (
        // Every other entry is listed and has no screen. Saying so is what stops an
        // unfinished application from reading as a broken one.
        <NotBuilt title={t(sectionNameKey(section))} />
      )}
    </AppShell>
  );
}

export default App;
