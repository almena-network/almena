/**
 * Where the interface starts.
 *
 * Everything the first frame needs is settled before there is a first frame: what a person
 * chose, the palette and colour that choice stands for, the language, and whether this is a
 * development build. All of it is awaited rather than applied as it arrives, because each is
 * something a reader would otherwise watch the application change its mind about — a screen
 * drawn dark and repainted light, or drawn in English and retranslated, is a screen that looks
 * unsure of itself.
 */

import React from "react";
import ReactDOM from "react-dom/client";

import App from "@/App";
import { startLanguage } from "@/i18n";
import {
  accentOf,
  applyAppearance,
  followSystemPalette,
  themeOf,
} from "@/lib/appearance";
import { loadBuild } from "@/lib/build";
import { loadPreferences } from "@/lib/preferences";

/** Settles what the interface looks like, what it speaks and which build it is, then draws it. */
async function start(): Promise<void> {
  // Two questions of the same side, asked together because neither waits on the other.
  const [chosen] = await Promise.all([loadPreferences(), loadBuild()]);

  applyAppearance(themeOf(chosen.theme), accentOf(chosen.accent));
  followSystemPalette();

  await startLanguage(chosen.language);

  const root = document.getElementById("root");

  if (root) {
    ReactDOM.createRoot(root).render(
      <React.StrictMode>
        <App />
      </React.StrictMode>,
    );
  }
}

void start();
