/**
 * The frame every screen is drawn inside: the navigation, and the region that scrolls.
 *
 * It owns the arrangement and nothing else — which section is open is decided above it, and
 * what a section looks like is decided below it. That is what lets a new screen be written
 * without touching the frame around it.
 *
 * # One frame, two shapes, and no JavaScript in the choosing
 *
 * Below 600 points the navigation is a floating menu across the bottom; at 600 and above it is
 * a sidebar down the left. Those are the same buttons in the same order and the same place in
 * the document — one media query in `shell.css` moves them. Nothing here asks how wide the
 * window is, which is the rule: the shape follows the width of the viewport and nothing else,
 * so a phone in landscape, a tablet and a window somebody dragged wider are one case.
 */

import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import NavItem from "@/features/shell/NavItem";
import { SECTIONS, type SectionId } from "@/features/shell/sections";
import "@/features/shell/shell.css";

/** The frame, and the screen inside it. */
interface AppShellProps {
  /** The section currently on screen. */
  section: SectionId;
  /** Called with the section the user chose. */
  onSelect: (section: SectionId) => void;
  /** The screen for that section. */
  children: ReactNode;
}

/** The application's frame, filling the screen or the window. */
function AppShell({ section, onSelect, children }: AppShellProps) {
  const { t } = useTranslation();

  return (
    <div className="shell">
      {/* The one region that scrolls. On a phone it scrolls the whole way under the menu. */}
      <main className="shell__screen">{children}</main>

      <nav className="shell__nav nav" aria-label={t("shell.nav")}>
        {SECTIONS.map((entry) => (
          <NavItem
            key={entry.id}
            id={entry.id}
            icon={entry.icon}
            current={entry.id === section}
            onSelect={onSelect}
          />
        ))}
      </nav>
    </div>
  );
}

export default AppShell;
