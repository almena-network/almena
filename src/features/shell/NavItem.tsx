/**
 * One entry of the navigation, whichever shape the navigation is in.
 *
 * There is one of these and not two. What changes between the two shapes is how an entry is
 * laid out, and that is the `expanded:` half of the classes below — the same button, at the
 * same place in the document, told to stack its icon over its name until there is room for
 * them side by side. A component that asked how wide the window was would be a second answer
 * to a question CSS has already given.
 *
 * It is shadcn/ui's button rather than markup of its own: an entry of a navigation is a thing
 * a person operates, and there is one button in this application. `ghost` is the tone for an
 * entry that is not the current one; the current one is the same button wearing the identity
 * colour, which is one of exactly two things that colour is allowed to mean.
 */

import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/cn";
import { sectionNameKey, type SectionId } from "@/features/shell/sections";
import type { LucideIcon } from "lucide-react";

/** What one entry is drawn from. */
interface NavItemProps {
  /** The section this entry leads to. */
  id: SectionId;
  /** The icon drawn with its name. */
  icon: LucideIcon;
  /** Whether this is the section on screen. */
  current: boolean;
  /** Called when the entry is chosen. */
  onSelect: (id: SectionId) => void;
}

/** One navigation entry. */
function NavItem({ id, icon: Icon, current, onSelect }: NavItemProps) {
  const { t } = useTranslation();

  return (
    <Button
      variant="ghost"
      // Which entry is current is said to a screen reader as well as drawn, because colour is
      // never the only carrier of meaning.
      aria-current={current ? "page" : undefined}
      onClick={() => {
        onSelect(id);
      }}
      className={cn(
        // Compact: stacked, and at least the 44 points a finger is entitled to. That number is
        // defended here rather than inherited, because the button it is put on is 36.
        "h-auto min-h-11 flex-1 flex-col gap-1 rounded-full px-3 text-xs text-muted-foreground",
        // Expanded: a row down a sidebar, and no longer stretching to fill it — but still 44
        // points tall. It dropped the minimum here once, on the reasoning that a wide window is
        // a window with a pointer in it. That is not one of the things this application knows: a
        // laptop with a touch screen is a computer like any other and is one of the three
        // platforms, and it is as wide as any of them. `ScreenNav` says the same in as many
        // words, and two navigations answering one question two ways is the fault this closes.
        "expanded:flex-none expanded:flex-row expanded:justify-start expanded:rounded-md expanded:py-2 expanded:text-sm",
        current &&
          "bg-identity-dim text-identity hover:bg-identity-dim hover:text-identity",
      )}
    >
      <Icon aria-hidden="true" />
      <span className="whitespace-nowrap">{t(sectionNameKey(id))}</span>
    </Button>
  );
}

export default NavItem;
