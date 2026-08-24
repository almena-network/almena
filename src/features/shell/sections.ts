/**
 * The sections of the application, in the order the navigation lists them.
 *
 * One list, so that adding a section is one edit rather than three: the navigation reads it,
 * and the screen shown for a section is chosen from it.
 *
 * There is no text here. A section's name is a catalog key derived from its identifier, which
 * is what keeps this file free of anything a person reads.
 *
 * # A section may hold more than one screen
 *
 * Where it does, they are listed here and the screen draws a menu across the top of itself to
 * move between them. The list is what makes that menu checkable: a screen named here with
 * nothing behind it fails `tsc`, the same way a section named here with no screen does.
 *
 * **The type refuses a list of one.** `screens` is a tuple of at least two, so a section cannot
 * declare a single screen and get a menu with one entry in it — which is furniture, not
 * navigation. A section with one screen simply has no `screens` and draws no menu, and that is
 * the arrangement complying with the rule rather than an exception to it.
 *
 * The icons are Lucide's, which is the set shadcn/ui draws with and therefore the project's —
 * one set, one grid, one stroke weight, and no icon of our own beside them
 * (`.agents/rules/interface.md`).
 */

import { House, Network, Settings, Sparkles, type LucideIcon } from "lucide-react";

/** A section: an entry in the navigation and the screen behind it. */
interface Section {
  /** Stable identifier. It is also how the section's name is looked up in the catalog. */
  id: string;
  /** The icon drawn with its name. */
  icon: LucideIcon;
  /**
   * The screens inside this section, in the order the menu draws them.
   *
   * Absent where a section is one screen, which is most of them. Present only with **two or
   * more** — the tuple says so, and it is the rule about a one-entry menu written where the
   * compiler can hold it. The first is what the section opens on, every time it is opened.
   */
  screens?: readonly [string, string, ...string[]];
}

/**
 * Every section, in order.
 *
 * Four, and every deployment of this application draws all four: the windowed application runs
 * on computers alone, so there is no longer a platform here that has one of these and not
 * another. A section marked for some devices and not others was the shape this list had while
 * a phone was one of the things it was drawn on, and nothing needs it now
 * (`.agents/rules/deployments.md`).
 *
 * Four is also close to what the compact shape has room for. Below 600 the navigation is a
 * menu the width of the narrowest window this application opens in, which is 400 points
 * across; a fifth entry leaves each of them around 70, and 44 of that is what a finger is
 * entitled to — a computer with a touch screen is still a computer, and every interaction here
 * is reachable both ways (`.agents/rules/deployments.md`).
 *
 * So a fifth section is a line added here and nothing more, and a sixth is not: that one is
 * either a change to the shape of the navigation, or an argument that one of the others should
 * go.
 *
 * Every one of them has a screen behind it, and that is not a coincidence to be maintained by
 * hand: `SCREENS` in `App.tsx` is typed against this list, so a section added here with nothing
 * behind it fails to compile.
 */
export const SECTIONS = [
  { id: "home", icon: House },
  { id: "network", icon: Network, screens: ["about", "peers"] },
  { id: "ai", icon: Sparkles, screens: ["conversation", "agent"] },
  { id: "settings", icon: Settings, screens: ["appearance", "general"] },
] as const satisfies readonly Section[];

/** The identifier of one of the sections. */
export type SectionId = (typeof SECTIONS)[number]["id"];

/**
 * The catalog key holding a section's name.
 *
 * @param id - The section.
 */
export function sectionNameKey(id: SectionId): `section.${SectionId}` {
  return `section.${id}`;
}

/** A section that holds more than one screen. */
type Sectioned = Extract<(typeof SECTIONS)[number], { screens: readonly string[] }>;

/**
 * Every screen name a catalog has to carry, as `screen.<section>.<screen>`.
 *
 * Derived rather than written, so that listing a screen above and forgetting its name is a
 * type error in both catalogs rather than a dotted key drawn at somebody.
 */
export type ScreenNameKey = Sectioned extends infer S
  ? S extends { id: string; screens: readonly string[] }
    ? `screen.${S["id"]}.${S["screens"][number]}`
    : never
  : never;

/**
 * The screens of one section, as the union of their identifiers.
 *
 * `never` for a section that is a single screen, which makes a `Record` over it uninhabited
 * and therefore a compile error to fill in — a section without a menu cannot grow one by
 * accident.
 */
export type ScreensOf<S extends SectionId> =
  Extract<(typeof SECTIONS)[number], { id: S }> extends { screens: readonly (infer T)[] }
    ? T & string
    : never;

/**
 * The screens of one section, or `undefined` where it is a single screen.
 *
 * The cast is the one place the literal types are re-asserted after `find` widens them, and it
 * is safe because the search is by the same identifier the type is derived from.
 *
 * @param id - The section.
 */
export function screensOf<S extends SectionId>(id: S): readonly ScreensOf<S>[] | undefined {
  const found = SECTIONS.find((entry) => entry.id === id);
  if (found === undefined || !("screens" in found)) {
    return undefined;
  }
  return found.screens as readonly string[] as readonly ScreensOf<S>[];
}

/**
 * The catalog key holding a screen's name.
 *
 * @param section - The section the screen is inside.
 * @param screen - The screen.
 */
export function screenNameKey<S extends SectionId>(
  section: S,
  screen: ScreensOf<S>,
): ScreenNameKey {
  return `screen.${section}.${String(screen)}` as ScreenNameKey;
}
