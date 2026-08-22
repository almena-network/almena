/**
 * Checks that every translation catalog holds exactly the same keys.
 *
 * A key present in one catalog and missing from another is not a state this project has: the
 * English text would silently stand in for the Spanish one, and nobody would notice until a
 * user did.
 *
 * It exists as a script, outside the application, because it catches the half TypeScript
 * cannot. Every catalog is typed against the English one, so a key added to `en.json` and
 * forgotten in `es.json` already fails `tsc`; a key that exists only in `es.json` is an extra
 * property, which is not a type error. `task build` runs this.
 */

import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const CATALOGS = fileURLToPath(
  new URL("../src/i18n/locales/", import.meta.url),
);

/** The source language: the one every other catalog is compared against. */
const SOURCE = "en";

/**
 * Every language the interface has to ship in.
 *
 * Kept here as well as in `src/i18n/index.ts` on purpose. A language is added deliberately
 * and only with a complete catalog, so the second place to edit is a feature: it will not
 * happen by accident.
 */
const REQUIRED = ["en", "es"];

/** Every leaf of a catalog, as a dotted path. */
function keysOf(value, prefix = "") {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return [prefix];
  }

  return Object.entries(value).flatMap(([key, child]) =>
    keysOf(child, prefix ? `${prefix}.${key}` : key),
  );
}

const catalogs = new Map(
  readdirSync(CATALOGS)
    .filter((name) => name.endsWith(".json"))
    .map((name) => [
      name.slice(0, -".json".length),
      new Set(keysOf(JSON.parse(readFileSync(join(CATALOGS, name), "utf8")))),
    ]),
);

let failed = false;

function report(message) {
  console.error(message);
  failed = true;
}

for (const language of REQUIRED) {
  if (!catalogs.has(language)) {
    report(
      `${language}: no catalog. Every supported language ships with a complete one.`,
    );
  }
}

const source = catalogs.get(SOURCE);

if (!source) {
  console.error(
    `Cannot compare anything without ${SOURCE}.json, the source catalog.`,
  );
  process.exit(1);
}

for (const [language, keys] of catalogs) {
  if (language === SOURCE) {
    continue;
  }

  const missing = [...source].filter((key) => !keys.has(key)).sort();
  const extra = [...keys].filter((key) => !source.has(key)).sort();

  for (const key of missing) {
    report(`${language}: missing "${key}", which ${SOURCE} has.`);
  }

  for (const key of extra) {
    report(`${language}: has "${key}", which ${SOURCE} does not.`);
  }
}

if (failed) {
  console.error(
    "\nCatalogs disagree. Every language holds the same keys, always.",
  );
  process.exit(1);
}

console.log(`Catalogs agree: ${[...catalogs.keys()].sort().join(", ")}.`);
