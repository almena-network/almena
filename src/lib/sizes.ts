/**
 * How many bytes, written the way a person reads them.
 *
 * One function, in a file of its own, because the moment a second screen wants it the two would
 * round differently — and a node that says 12 KiB on one screen and 11.7 KiB on another is a node
 * somebody has to check.
 */

/** The steps, in the order they are climbed. Binary, because a file on disk is counted in 1024s. */
const STEPS = ["B", "KiB", "MiB", "GiB", "TiB"] as const;

/**
 * `bytes` as a number and a unit — `12 KiB`, `1.4 MiB`.
 *
 * **The number is localised and the unit is not.** A thousands separator is a comma in one
 * language and a point in another, so the number goes through the reader's own locale; `KiB` is a
 * unit and is the same in every one of them.
 *
 * One decimal above kibibytes and none below: a size in bytes is exact and there is nothing to
 * round, and a tenth of a kibibyte is the smallest difference worth drawing on a screen that is
 * read at a glance.
 *
 * @param bytes - How many. Negative is not a size and comes back as nought.
 * @param language - The reader's language tag, for the number alone.
 */
export function inBytes(bytes: number, language: string): string {
  const size = Math.max(0, bytes);
  let at = 0;
  let left = size;
  while (left >= 1024 && at < STEPS.length - 1) {
    left /= 1024;
    at += 1;
  }
  const written = left.toLocaleString(language, {
    maximumFractionDigits: at === 0 ? 0 : 1,
  });
  return `${written} ${STEPS[at]}`;
}
