/**
 * One entry of the navigation, whichever shape the navigation is in.
 *
 * There is one of these and not two. What changes between a phone and a window is where the
 * entries sit and how they are laid out, and both of those are the stylesheet's — see
 * `shell.css`. A component that asked how wide the window was would be a second answer to a
 * question CSS has already answered.
 */

import { useTranslation } from "react-i18next";

import Icon, { type IconName } from "@/components/Icon";
import { sectionNameKey, type SectionId } from "@/features/shell/sections";

/** What one entry is drawn from. */
interface NavItemProps {
  /** The section this entry leads to. */
  id: SectionId;
  /** The icon drawn with its name. */
  icon: IconName;
  /** Whether this is the section on screen. */
  current: boolean;
  /** Called when the entry is chosen. */
  onSelect: (id: SectionId) => void;
}

/** One navigation entry. */
function NavItem({ id, icon, current, onSelect }: NavItemProps) {
  const { t } = useTranslation();

  return (
    <button
      type="button"
      className="nav__item"
      // Which entry is current is said to a screen reader as well as drawn, because colour is
      // never the only carrier of meaning.
      aria-current={current ? "page" : undefined}
      onClick={() => {
        onSelect(id);
      }}
    >
      <Icon name={icon} />
      <span className="nav__name">{t(sectionNameKey(id))}</span>
    </button>
  );
}

export default NavItem;
