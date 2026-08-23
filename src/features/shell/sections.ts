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

import { House, Network, Settings, Sparkles, type LucideIcon } from "lucide-react";

/** A section: an entry in the navigation and the screen behind it. */
interface Section {
  /** Stable identifier. It is also how the section's name is looked up in the catalog. */
  id: string;
  /** The icon drawn with its name. */
  icon: LucideIcon;
  /**
   * Present on a section a phone does not have at all.
   *
   * Not a layout question and not a screen size: the interface has two shapes and both of them
   * draw every section they are given (`.agents/rules/screen-sizes.md`). This is about the
   * platform *not having the thing the section is about* — which is a high bar, and today
   * exactly one section clears it.
   */
  desktop?: true;
}

/**
 * Every section, in order.
 *
 * Four, and five is the limit. Below 600 the navigation is a menu the width of a phone, whose
 * narrowest window is 400 points across; that leaves each entry around 70 at five, which is
 * past the 44 a finger is entitled to and leaves no room for a sixth. At four there is room
 * to spare, and a phone draws three of them.
 *
 * Every one of them has a screen behind it, and that is not a coincidence to be maintained by
 * hand: `SCREENS` in `App.tsx` is typed against this list, so a section added here with nothing
 * behind it fails to compile.
 */
export const SECTIONS = [
  { id: "home", icon: House },
  { id: "network", icon: Network },
  // The agent is a second program this application runs as a child process, and a phone's
  // operating system offers no way to run one — iOS gives a sandboxed application no way to
  // start a second program, and Android will not execute a binary out of an application's own
  // directory. So this is a platform without the thing rather than a person unable to do
  // something, which is the test `.agents/rules/supported-platforms.md` sets.
  { id: "ai", icon: Sparkles, desktop: true },
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
