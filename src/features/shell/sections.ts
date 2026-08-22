/**
 * The sections of the application, in the order the navigation lists them.
 *
 * One list, so that adding a section is one edit rather than three: the navigation reads it,
 * and the screen shown for a section is chosen from it.
 *
 * There is no text here. A section's name is a catalog key derived from its identifier, which
 * is what keeps this file free of anything a person reads.
 */

import type { IconName } from "@/components/Icon";

/** A section: an entry in the navigation and the screen behind it. */
interface Section {
  /** Stable identifier. It is also how the section's name is looked up in the catalog. */
  id: string;
  /** The icon drawn with its name. */
  icon: IconName;
}

/**
 * Every section, in order.
 *
 * Three, and five is the limit. Below 600 the navigation is a menu the width of a phone, whose
 * narrowest window is 400 points across; that leaves each entry around 70 at five, which is
 * past the 44 a finger is entitled to and leaves no room for a sixth. Three sit wider than
 * that and are the better for it.
 *
 * Two of the three have no screen yet and show `NotBuilt`. They are listed anyway, because the
 * shape of the application is what a navigation is for — and because an entry that appears
 * later moves everything beside it, which is worse than an entry that was always there.
 */
export const SECTIONS = [
  { id: "home", icon: "home" },
  { id: "network", icon: "network" },
  { id: "settings", icon: "settings" },
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
