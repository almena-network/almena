/**
 * The language the interface is showing, as something a screen can read and change.
 *
 * It is i18next that holds the answer, and this is what keeps a screen from reaching into it:
 * `useTranslation` already re-renders everything drawn from a catalog when the language
 * changes, so the one thing missing was somewhere for the control that changes it to ask what
 * the current one is.
 */

import { useTranslation } from "react-i18next";

import { type Language, deviceLanguage, isLanguage, setLanguage } from "@/i18n";

/** What a screen gets to read and to change. */
interface LanguageChoice {
  /** The language in use. Never a tag this build does not ship. */
  language: Language;
  /** Asks for a language. What arrives is what was stored. */
  chooseLanguage: (language: Language) => void;
}

/** The language a person chose, else the one the device asked for. */
export function useLanguage(): LanguageChoice {
  const { i18n } = useTranslation();
  const settled = i18n.resolvedLanguage;

  return {
    language: isLanguage(settled) ? settled : deviceLanguage(),
    chooseLanguage: (next) => {
      void setLanguage(next);
    },
  };
}
