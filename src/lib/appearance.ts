/**
 * The palette and the identity colour, as the two attributes the token file reads.
 *
 * `src/styles/tokens.css` holds every value of both and switches between them on `data-theme`
 * and `data-accent`, set on the document element. **This file is the only thing that writes
 * them**, so a screen never asks which palette it is in and no component branches on one.
 *
 * The two vocabularies are here rather than in `@/lib/preferences` because they are these
 * values: a sixth accent is a block in the token file and a name in this list, and nothing else
 * in the application has an opinion about either.
 *
 * Following the operating system is the third thing this file does. `system` is not a palette;
 * it is the absence of a choice, and it has to keep answering as the operating system changes
 * its mind — somebody who switches their computer to light at sunset gets a light Almena
 * without touching it.
 */

/** The palettes a person can ask for, and the one that defers to the operating system. */
export const THEMES = ["system", "light", "dark"] as const;

/** One of the palettes a person can ask for. */
export type Theme = (typeof THEMES)[number];

/** The identity colours a person can choose between. Each is a block in the token file. */
export const ACCENTS = ["orange", "blue", "red", "yellow", "green"] as const;

/** One of the identity colours. */
export type Accent = (typeof ACCENTS)[number];

/** What the interface is when nobody has chosen: the operating system's answer. */
export const DEFAULT_THEME: Theme = "system";

/** What the interface is drawn in when nobody has chosen: the colour of the application icon. */
export const DEFAULT_ACCENT: Accent = "orange";

/** What the document is asked when the palette is the operating system's to decide. */
const SYSTEM_IS_LIGHT = "(prefers-color-scheme: light)";

/** The palette in use, so that a change of the operating system's mind can be answered. */
let theme: Theme = DEFAULT_THEME;

/** The identity colour in use. */
let accent: Accent = DEFAULT_ACCENT;

/**
 * Narrows an arbitrary string to a palette this interface has.
 *
 * @param value - Anything that claims to be one, including what was read off disk.
 */
export function isTheme(value: string | null | undefined): value is Theme {
  return THEMES.includes(value as Theme);
}

/**
 * Narrows an arbitrary string to an identity colour this interface has.
 *
 * @param value - Anything that claims to be one, including what was read off disk.
 */
export function isAccent(value: string | null | undefined): value is Accent {
  return ACCENTS.includes(value as Accent);
}

/**
 * The palette a stored value stands for, or the default when it stands for none.
 *
 * @param value - What was stored, which is `null` until somebody chooses and may be a name an
 *   older or newer build meant something by.
 */
export function themeOf(value: string | null | undefined): Theme {
  return isTheme(value) ? value : DEFAULT_THEME;
}

/**
 * The identity colour a stored value stands for, or the default when it stands for none.
 *
 * @param value - What was stored, which is `null` until somebody chooses.
 */
export function accentOf(value: string | null | undefined): Accent {
  return isAccent(value) ? value : DEFAULT_ACCENT;
}

/** Whether the operating system is asking for a light interface. */
function systemWantsLight(): boolean {
  return window.matchMedia(SYSTEM_IS_LIGHT).matches;
}

/** Writes the two attributes the token file reads, from whatever is in use now. */
function paint(): void {
  const drawn = theme === "system" ? (systemWantsLight() ? "light" : "dark") : theme;

  document.documentElement.dataset.theme = drawn;
  document.documentElement.dataset.accent = accent;
}

/**
 * Draws the interface in a palette and an identity colour.
 *
 * @param chosen - The palette, `system` included.
 * @param colour - The identity colour.
 */
export function applyAppearance(chosen: Theme, colour: Accent): void {
  theme = chosen;
  accent = colour;
  paint();
}

/**
 * Starts answering the operating system's palette for as long as `system` is the choice.
 *
 * Called once, at startup. The listener stays for the life of the application and asks what the
 * choice is each time rather than being added and removed with it, because a subscription that
 * comes and goes is one that can be left behind.
 */
export function followSystemPalette(): void {
  window.matchMedia(SYSTEM_IS_LIGHT).addEventListener("change", () => {
    if (theme === "system") {
      paint();
    }
  });
}
