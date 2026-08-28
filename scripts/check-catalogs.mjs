/**
 * Checks that every translation catalog holds exactly the same keys.
 *
 * A key present in one catalog and missing from another is not a state this project has: the
 * English text would silently stand in for the Spanish one, and nobody would notice until a
 * user did.
 *
 * It exists as a script, outside the application, because the application no longer names its
 * languages: adding one must not mean touching code, and the design does not assume two, so the
 * catalog directory *is* the list, and nothing in TypeScript enumerates it any more. What used
 * to be `tsc`'s half — a key in English and not in Spanish — is checked here too, in both
 * directions, along with every catalog having a name of its own. `task build` and `task check`
 * run this.
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
 * The key every catalog carries its own name under.
 *
 * Each language is shown the way it names itself, never translated into the other, and the
 * picker reads this key rather than a table it would have to be taught. A catalog without it
 * is a language nobody could choose.
 */
const NAME = "language.name";

/** Every leaf of a catalog, as a dotted path. */
function keysOf(value, prefix = "") {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return [prefix];
  }

  return Object.entries(value).flatMap(([key, child]) =>
    keysOf(child, prefix ? `${prefix}.${key}` : key),
  );
}

/** What a catalog says at a dotted key, or undefined. */
function at(value, key) {
  return key.split(".").reduce((node, part) => node?.[part], value);
}

const parsed = new Map(
  readdirSync(CATALOGS)
    .filter((name) => name.endsWith(".json"))
    .map((name) => [
      name.slice(0, -".json".length),
      JSON.parse(readFileSync(join(CATALOGS, name), "utf8")),
    ]),
);

const catalogs = new Map(
  [...parsed].map(([language, value]) => [language, new Set(keysOf(value))]),
);

let failed = false;

function report(message) {
  console.error(message);
  failed = true;
}

// The directory is the list of languages: adding one must not mean touching code, so there is
// no list here either. What is still checked is what would make the folder a lie: no source to
// compare against, only one language in a platform that is multilingual from the first day, or
// two languages that call themselves the same thing.
if (catalogs.size < 2) {
  report(
    `Only ${catalogs.size} catalog in ${CATALOGS}. The platform is multilingual from the first day.`,
  );
}

const names = new Map();

for (const [language, value] of parsed) {
  const name = at(value, NAME);

  if (typeof name !== "string" || name.trim() === "") {
    report(`${language}: no "${NAME}". A language has to arrive knowing its own name.`);
    continue;
  }

  const taken = names.get(name);

  if (taken) {
    report(`${language}: calls itself "${name}", and so does ${taken}.`);
  }

  names.set(name, language);
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
