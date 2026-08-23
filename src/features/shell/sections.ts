/**
 * The sections of the application, in the order the navigation lists them.
 *
 * One list, so that adding a section is one edit rather than three: the navigation reads it,
 * and the screen shown for a section is chosen from it.
 *
 * There is no text here. A section's name is a catalog key derived from its identifier, which
 * is what keeps this file free of anything a person reads.
 *
 * The icons are Lucide's, which is the set shadcn/ui draws with and therefore the project's —
 * one set, one grid, one stroke weight, and no icon of our own beside them
 * (`.agents/rules/visual-identity.md`).
 */

import { House, Network, Settings, type LucideIcon } from "lucide-react";

/** A section: an entry in the navigation and the screen behind it. */
interface Section {
  /** Stable identifier. It is also how the section's name is looked up in the catalog. */
  id: string;
  /** The icon drawn with its name. */
  icon: LucideIcon;
}

/**
 * Every section, in order.
 *
 * Three, and five is the limit. Below 600 the navigation is a menu the width of a phone, whose
 * narrowest window is 400 points across; that leaves each entry around 70 at five, which is
 * past the 44 a finger is entitled to and leaves no room for a sixth. Three sit wider than
 * that and are the better for it.
 *
 * Every one of them has a screen behind it, and that is not a coincidence to be maintained by
 * hand: `SCREENS` in `App.tsx` is typed against this list, so a section added here with nothing
 * behind it fails to compile.
 */
export const SECTIONS = [
  { id: "home", icon: House },
  { id: "network", icon: Network },
  { id: "settings", icon: Settings },
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
