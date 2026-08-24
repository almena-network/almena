/**
 * The language the interface is showing.
 *
 * Two things can decide it and they are asked in that order: what a person chose on the
 * Settings screen, and — until somebody chooses — what the device asks for. Nothing in this
 * file has an opinion of its own; it reads those two, hands the answer to i18next, and applies
 * the strings the webview draws rather than a component. Everything a person reads is looked up
 * from here by key, and the catalogs sit beside this file, one per language.
 */

import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en from "@/i18n/locales/en.json";
import es from "@/i18n/locales/es.json";
import { choose } from "@/lib/preferences";

/**
 * Every language the interface ships in, one catalog each.
 *
 * A language belongs in this list only once its catalog is complete: partial support is not
 * shipped, and until then the language is not offered at all.
 *
 * The list is repeated once, in `scripts/check-catalogs.mjs`, which cannot read this file.
 * Adding a language means both places, which is the point — a language is added
 * deliberately, never by accident.
 */
export const LANGUAGES = ["en", "es"] as const;

/** One of the languages the interface ships in. */
export type Language = (typeof LANGUAGES)[number];

/**
 * The language keys are written in, and the one shown when another catalog lacks a key.
 *
 * English is both by design: a missing translation degrades to readable English rather than
 * to a bare key or an empty line.
 */
const SOURCE_LANGUAGE: Language = "en";

/**
 * The catalogs, bundled into the application rather than fetched.
 *
 * There are two and they are small, so loading them up front costs nothing and buys a
 * startup with no asynchronous step — no moment where a screen shows keys instead of text.
 *
 * Typing every catalog as `typeof en` is the parity rule working at compile time: a key
 * added to English and forgotten in Spanish fails `tsc`. The other direction, a key that
 * exists only in Spanish, is not a type error and is what `task catalogs` is for.
 */
const catalogs: Record<Language, { translation: typeof en }> = {
  en: { translation: en },
  es: { translation: es },
};

/**
 * Narrows an arbitrary string to a language this interface ships in.
 *
 * @param value - Anything that claims to be a language tag: an entry from the device's list,
 *   what was read off disk, or nothing at all.
 */
export function isLanguage(value: string | null | undefined): value is Language {
  return LANGUAGES.includes(value as Language);
}

/**
 * The language the device asks for, else English.
 *
 * The device's preference arrives as a list of tags like `es-419`, most wanted first. Only
 * the part before the dash is matched, so a Spanish of any region gets Spanish.
 *
 */
export function deviceLanguage(): Language {
  for (const tag of navigator.languages ?? [navigator.language]) {
    const base = tag.split("-")[0].toLowerCase();
    if (isLanguage(base)) {
      return base;
    }
  }

  return SOURCE_LANGUAGE;
}

/**
 * Applies the text the webview itself draws.
 *
 * The document language is what a screen reader and the browser's own hyphenation go by, and
 * the document title is what a browser tab would show.
 *
 * **What the operating system draws is not here yet.** A native window title or a tray label
 * is drawn by the platform, not the webview, so localizing one means handing catalog strings
 * to the Rust side from this function — only this side has the catalogs. This application has
 * no tray and does not yet set its native title from a catalog: the title in
 * `tauri.conf.json` is what the platform draws until the step that adds the hand-off.
 *
 * @param language - The language now in use, as a tag for the document element.
 */
function applyLanguage(language: string): void {
  document.documentElement.lang = language;
  document.title = i18n.t("app.name");
}

/**
 * Settles the language before anything is drawn.
 *
 * Called once, from `main.tsx`, and awaited: a screen that rendered before this returned would
 * be a screen showing keys, and a screen that re-rendered when it returned would be one that
 * changed language in front of the reader.
 *
 * @param chosen - What was stored, which is `null` until somebody chooses one and may be a tag
 *   this build no longer ships. Either way the device's own answer is what it falls back to.
 */
export async function startLanguage(chosen: string | null): Promise<void> {
  await i18n.use(initReactI18next).init({
    resources: catalogs,
    lng: isLanguage(chosen) ? chosen : deviceLanguage(),
    fallbackLng: SOURCE_LANGUAGE,
    supportedLngs: LANGUAGES,
    // React escapes what it renders, so escaping here would do it twice and put an entity
    // on screen where an apostrophe belongs.
    interpolation: { escapeValue: false },
  });

  i18n.on("languageChanged", applyLanguage);
  applyLanguage(i18n.resolvedLanguage ?? SOURCE_LANGUAGE);
}

/**
 * Changes the language and remembers that it was asked for.
 *
 * @param language - The language to show from now on.
 * @returns The language in use afterwards, which is what was stored and not what was asked.
 */
export async function setLanguage(language: Language): Promise<Language> {
  const stored = await choose({ language });
  const settled = isLanguage(stored.language) ? stored.language : deviceLanguage();

  await i18n.changeLanguage(settled);

  return settled;
}
