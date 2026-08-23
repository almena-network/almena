/**
 * The palette and the identity colour, as something a screen can read and change.
 *
 * The interface itself does not need this: `@/lib/appearance` writes two attributes on the
 * document and every screen is drawn from tokens, so nothing re-renders when the palette
 * changes. What needs it is the one screen that has to *show* the current choice back to the
 * person making it, and that is the whole of what this hook is for.
 *
 * Nothing is drawn before it is stored. A control that moved and then moved back would be worse
 * than one that moves a moment later, and writing a small file is not a wait anybody sees.
 */

import { useState } from "react";

import {
  type Accent,
  type Theme,
  accentOf,
  applyAppearance,
  themeOf,
} from "@/lib/appearance";
import { choose, preferences } from "@/lib/preferences";

/** What a screen gets to read and to change. */
interface Appearance {
  /** The palette in use, `system` included. */
  theme: Theme;
  /** The identity colour in use. */
  accent: Accent;
  /** Asks for a palette. What arrives is what was stored, which may not be what was asked. */
  chooseTheme: (theme: Theme) => void;
  /** Asks for an identity colour, on the same terms. */
  chooseAccent: (accent: Accent) => void;
}

/** The palette and the identity colour a person chose. */
export function useAppearance(): Appearance {
  const [theme, setTheme] = useState<Theme>(() => themeOf(preferences().theme));
  const [accent, setAccent] = useState<Accent>(() =>
    accentOf(preferences().accent),
  );

  async function store(chosen: { theme?: Theme; accent?: Accent }) {
    const stored = await choose(chosen);
    const settledTheme = themeOf(stored.theme);
    const settledAccent = accentOf(stored.accent);

    applyAppearance(settledTheme, settledAccent);
    setTheme(settledTheme);
    setAccent(settledAccent);
  }

  return {
    theme,
    accent,
    chooseTheme: (next) => {
      void store({ theme: next });
    },
    chooseAccent: (next) => {
      void store({ accent: next });
    },
  };
}
