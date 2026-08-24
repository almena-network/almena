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
 * (`.agents/rules/interface.md`).
 */

import {
  House,
  Network,
  Settings,
  Sparkles,
  type LucideIcon,
} from "lucide-react";

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
  { id: "network", icon: Network },
  { id: "ai", icon: Sparkles },
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
