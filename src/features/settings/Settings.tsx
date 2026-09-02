/**
 * The Settings section: two screens, and the menu between them.
 *
 * It is the first section in the application to hold more than one screen, and it is therefore
 * the shape every other one takes when it grows a second — the menu, the title, the total
 * record, and the first screen being what the section opens on.
 *
 * # The pattern, and why it is this pattern
 *
 * `SCREENS` below is a total `Record` over the screens `sections.ts` declares, which is the
 * same discipline `App.tsx` applies to the sections themselves: a screen listed with nothing
 * behind it fails `tsc` rather than being found by somebody pressing a menu entry that does
 * nothing — a navigation lists no destination that leads nowhere. One level up, one level down,
 * one rule.
 *
 * **The menu is the heading.** A section with one screen titles itself with `.screen__title`;
 * a section with a menu does not, because the menu has already marked which screen is showing
 * and a title under it would print the same word again — three times on Appearance, which holds
 * one card of that name. `ScreenNav` carries the `<h1>` for anything that cannot see the
 * marking.
 *
 * # Opening on the first screen, every time
 *
 * The selection is this component's own state and nothing lifts it, so leaving the section and
 * coming back opens `appearance` again. That is the decision rather than a limitation of where
 * the state happens to live: a section opens the way it opens the first time, and the
 * application already loses far more than this when a screen is swapped — the composer's text,
 * the model card's reading, every outcome any card was reporting.
 */

import { useState } from "react";

import CardGrid from "@/components/CardGrid";
import ScreenNav from "@/components/ScreenNav";
import Appearance from "@/features/settings/Appearance";
import Language from "@/features/settings/Language";
import LeaveTheNetwork from "@/features/settings/LeaveTheNetwork";
import Model from "@/features/settings/Model";
import OpenAtLogin from "@/features/settings/OpenAtLogin";
import { screensOf, type ScreensOf } from "@/features/shell/sections";

/** One of this section's screens. */
type Screen = ScreensOf<"settings">;

/** What the section opens on, every time it is opened. */
const FIRST: Screen = "appearance";

/** The Settings section. */
function Settings() {
  const [screen, setScreen] = useState<Screen>(FIRST);
  const screens = screensOf("settings") ?? [];

  // Total over `Screen`: naming a screen in `sections.ts` and forgetting it here fails `tsc`.
  const shown: Record<Screen, React.ReactNode> = {
    appearance: (
      <CardGrid>
        <Appearance />
      </CardGrid>
    ),
    general: (
      <CardGrid>
        <Language />
        <Model />
        <OpenAtLogin />
        {/* Last, and it is the only thing on this screen that destroys anything. Nothing about the
            grid ranks its cards, so being last in source is the whole of what puts it last on
            screen — which is where the one irreversible control belongs. */}
        <LeaveTheNetwork />
      </CardGrid>
    ),
  };

  return (
    <div className="screen">
      <ScreenNav section="settings" screens={screens} current={screen} onSelect={setScreen} />

      {shown[screen]}
    </div>
  );
}

export default Settings;
