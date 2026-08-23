/**
 * The grid a screen's cards flow in.
 *
 * A screen with more than one card puts them in here and gets the same behaviour every other
 * screen gets: side by side once there is room, stacked the moment there is not. It is a
 * component rather than a class a screen remembers to add, because the third screen to want it
 * is the one that would have written the declaration slightly differently.
 *
 * One declaration and no breakpoint. The project has exactly one — 600, in the token file —
 * and a screen does not get a second of its own: `auto-fit` decides how many columns fit and
 * `min(100%, …)` is what stops a column overflowing a window narrower than itself. It is
 * written as an arbitrary value because Tailwind has no utility for an auto-fitting grid; the
 * number in it is a token and not a literal.
 */

import type { ReactNode } from "react";

/** What the grid holds. */
interface CardGridProps {
  /** The cards. One is allowed: it simply spans the width, which is correct. */
  children: ReactNode;
}

/** The grid a screen's cards flow in. */
function CardGrid({ children }: CardGridProps) {
  return (
    <div className="grid grid-cols-[repeat(auto-fit,minmax(min(100%,var(--width-card-min)),1fr))] gap-4">
      {children}
    </div>
  );
}

export default CardGrid;
