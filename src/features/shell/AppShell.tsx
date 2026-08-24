/**
 * The frame every screen is drawn inside: the navigation, the region that scrolls, and the
 * strip along the bottom.
 *
 * It owns the arrangement and nothing else — which section is open is decided above it, and
 * what a section looks like is decided below it. That is what lets a new screen be written
 * without touching the frame around it.
 *
 * The status strip belongs to the frame and not to a screen: it is pinned to the bottom of the
 * window, it spans the full width in both shapes — under the sidebar, not beside it — and it
 * does not scroll away with the content. In the compact shape the floating menu sits above it
 * rather than over it.
 *
 * # One frame, two shapes, and no JavaScript in the choosing
 *
 * Below 600 points the navigation is a floating menu across the bottom; at 600 and above it is
 * a sidebar down the left, with the mark and the product's name at its head. Those are the
 * same buttons in the same order and the same place in the document — `shell.css` moves them.
 * Nothing here asks how wide the window is, which is the rule: the shape follows the width of
 * the viewport and nothing else, so a window somebody dragged narrow and one they dragged wide
 * are one case (`.agents/rules/interface.md`).
 *
 * Every section is drawn, and this frame decides nothing about which. It filtered two of them
 * once, while a phone was one of the things this application was drawn on and each deployment
 * had a section the other could not have. It runs on computers alone now, so there is nothing
 * left to filter and no answer to wait for before the menu can be drawn at all.
 */

import { type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import Logo from "@/components/Logo";
import { Separator } from "@/components/ui/separator";
import NavItem from "@/features/shell/NavItem";
import StatusBar from "@/features/shell/StatusBar";
import { SECTIONS, type SectionId } from "@/features/shell/sections";

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
      {/* The one region that scrolls. In the compact shape it scrolls the whole way under
          the floating menu. */}
      <main className="shell__screen">{children}</main>

      <nav className="shell__nav" aria-label={t("shell.nav")}>
        {/* The head of the sidebar, and one of the two places the mark wears the identity
            colour. There is no room for it in the compact shape — the menu there is a bar
            the width of a thumb — so it is drawn only where there is, and the first screen
            carries the mark in a window too narrow for a sidebar. */}
        <div className="hidden flex-col gap-3 pt-1 pb-2 expanded:flex">
          <div className="flex items-center gap-2 px-3">
            <Logo size={20} color="var(--identity)" />
            <span className="font-semibold tracking-tight">{t("app.name")}</span>
          </div>
          <Separator />
        </div>

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

      <StatusBar />
    </div>
  );
}

export default AppShell;
