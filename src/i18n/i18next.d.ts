/**
 * What i18next knows about this project's catalogs.
 *
 * It exists apart from the setup because it is a type declaration and nothing else: it turns
 * `t()` from a function taking any string into one taking the keys that actually exist.
 */

import en from "@/i18n/locales/en.json";

declare module "i18next" {
  /** English is the source catalog, so its shape is the list of valid keys. */
  interface CustomTypeOptions {
    defaultNS: "translation";
    resources: { translation: typeof en };
  }
}
