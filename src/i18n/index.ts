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
import { choose } from "@/lib/preferences";

/**
 * The language keys are written in, and the one shown when another catalog lacks a key.
 *
 * English is both by design: a missing translation degrades to readable English rather than
 * to a bare key or an empty line.
 */
const SOURCE_LANGUAGE = "en";

/** One of the languages the interface ships in — a tag, checked with {@link isLanguage}. */
export type Language = string;

/**
 * Every catalog in the directory beside this file, bundled in rather than fetched.
 *
 * **The directory is the list of languages, and that is the whole design.** Adding a language
 * must not mean touching code, so there is no list here to add it to: dropping `fr.json` in
 * beside the others is the entire operation, and every place that used to carry a copy of the
 * list — this file, the picker, the checker, the terminal variant — now reads the directory
 * instead.
 *
 * They are bundled rather than fetched because there is no moment in a startup where showing
 * keys instead of text would be acceptable.
 */
const bundled = import.meta.glob<{ default: typeof en }>("./locales/*.json", {
  eager: true,
});

/** The language a catalog's path names: `./locales/es.json` is `es`. */
function tagOf(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1, -".json".length);
}

/**
 * Every language the interface ships in, the source first and the rest alphabetically.
 *
 * The source leads because it is the fallback: a list that opens with the language everything
 * degrades to is a list that reads in the order the rules apply.
 */
export const LANGUAGES: readonly Language[] = Object.keys(bundled)
  .map(tagOf)
  .sort((left, right) =>
    left === SOURCE_LANGUAGE
      ? -1
      : right === SOURCE_LANGUAGE
        ? 1
        : left.localeCompare(right),
  );

/** The catalogs, by the language each one is written in. */
const catalogs: Record<Language, { translation: typeof en }> = Object.fromEntries(
  Object.entries(bundled).map(([path, module]) => [
    tagOf(path),
    { translation: module.default },
  ]),
);

/**
 * What each language calls itself, in itself.
 *
 * Read out of each catalog's own `language.name` rather than from a table here, for the same
 * reason the list is read from the directory: a new catalog has to arrive knowing its own name,
 * because nowhere else would learn it without being edited. And a person looking for their
 * language in a list is looking for the word they would recognise, which is never the
 * translation of it.
 */
export const LANGUAGE_NAMES: Readonly<Record<Language, string>> =
  Object.fromEntries(
    Object.entries(catalogs).map(([tag, catalog]) => [
      tag,
      catalog.translation.language.name,
    ]),
  );

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
 * the part before the dash is matched, so a Spanish of any region gets Spanish — a region this
 * project does not distinguish must never become a language it does not have.
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
