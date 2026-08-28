/**
 * The menu across the top of a section that holds more than one screen.
 *
 * The same navigation as the frame's, one level down and turned on its side: a `<nav>` of
 * shadcn buttons drawn `ghost` and carrying `aria-current="page"`, which is already what an
 * entry of a navigation is drawn as here. Nothing new is drawn and no variant is added — an
 * entry of a navigation is an entry of a navigation whichever navigation it belongs to.
 *
 * # It does not wear the identity colour, and that is the whole of the difference
 *
 * The frame's current entry does. That colour means one of two things in this application, and
 * two current entries wearing it at once would be a second accent, which a screen never
 * carries — so **the colour says which section you are in, once**, and this menu says where you
 * are inside it with a surface and a weight instead. What carries it for a screen reader is
 * `aria-current`, the same as upstairs, because colour is never the only thing saying anything.
 *
 * # It is the screen's heading, and prints it once
 *
 * A screen inside a section draws no title of its own. The section's name is already in the
 * frame, marked in the identity colour, and printing the current screen's name under a menu
 * that has just marked it is the same word twice — three times where the screen holds one card
 * of the same name, which is what it looked like before this was taken out. What the menu shows
 * is what the heading would have said, so the menu says it.
 *
 * The heading itself survives for anything that cannot see the marking: an `sr-only` `<h1>`
 * naming the current screen. A region with no heading at all is a different problem from a
 * region with a redundant one, and this fixes the second without causing the first.
 *
 * # One shape at every width
 *
 * `expanded:` belongs to the frame and a screen does not get a second use of it, so this is one
 * row from 400 points to unbounded. It **wraps**; it never scrolls sideways, because reaching a
 * control by scrolling horizontally is the thing the smallest size rules out. Entries keep the
 * 44 points a finger is entitled to at every width, not only the narrow one: a laptop with a
 * touch screen is a computer, and it is one of the three platforms.
 */

import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  screenNameKey,
  sectionNameKey,
  type ScreensOf,
  type SectionId,
} from "@/features/shell/sections";
import { cn } from "@/lib/cn";

/** What the menu is drawn from. */
interface ScreenNavProps<S extends SectionId> {
  /** The section whose screens these are. It names the menu for a screen reader. */
  section: S;
  /** The screens, in the order they are drawn. */
  screens: readonly ScreensOf<S>[];
  /** The one being shown. */
  current: ScreensOf<S>;
  /** Called with the screen the person chose. */
  onSelect: (screen: ScreensOf<S>) => void;
}

/** The menu between the screens of one section. */
function ScreenNav<S extends SectionId>({
  section,
  screens,
  current,
  onSelect,
}: ScreenNavProps<S>) {
  const { t } = useTranslation();

  return (
    <>
      {/* The heading every screen has, said once and drawn by the menu below rather than
          printed again under it. */}
      <h1 className="sr-only">{t(screenNameKey(section, current))}</h1>

      <nav className="flex flex-wrap gap-1" aria-label={t(sectionNameKey(section))}>
        {screens.map((screen) => (
          <Button
            key={screen}
            variant="ghost"
            aria-current={screen === current ? "page" : undefined}
            onClick={() => {
              onSelect(screen);
            }}
            className={cn(
              "min-h-11 px-3 text-sm text-muted-foreground",
              screen === current &&
                "bg-secondary font-medium text-foreground hover:bg-secondary hover:text-foreground",
            )}
          >
            {t(screenNameKey(section, screen))}
          </Button>
        ))}
      </nav>
    </>
  );
}

export default ScreenNav;
