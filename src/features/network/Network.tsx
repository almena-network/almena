/**
 * The Network screen: what this node is part of, and who it is talking to.
 *
 * Two cards, and both of them tell the same truth in different words today — there is no
 * peer-to-peer layer in this build, so there is no network and no peer. What is real here is
 * the machinery: a reading taken on a timer and on demand, figures that know the difference
 * between none and unmeasured, and a list that will draw a peer the day there is one.
 */

import { useState } from "react";

import ScreenNav from "@/components/ScreenNav";
import NetworkFacts from "@/features/network/NetworkFacts";
import Peers from "@/features/network/Peers";
import { screensOf, type ScreensOf } from "@/features/shell/sections";
import { useNetwork } from "@/hooks/useNetwork";

/** One of this section's screens. */
type Screen = ScreensOf<"network">;

/** What the section opens on, every time it is opened. */
const FIRST: Screen = "about";

/** The Network section. */
function Network() {
  const [screen, setScreen] = useState<Screen>(FIRST);
  const screens = screensOf("network") ?? [];
  // Read here, once, and handed down. Both screens are looking at the same network, and two
  // calls would be two independent looks disagreeing about when the last one came back.
  const { reading, lookedAt, looking, refresh } = useNetwork();

  // Total over `Screen`: naming a screen in `sections.ts` and forgetting it here fails `tsc`.
  const shown: Record<Screen, React.ReactNode> = {
    about: <NetworkFacts reading={reading} />,
    peers: (
      <Peers reading={reading} lookedAt={lookedAt} looking={looking} onRefresh={refresh} />
    ),
  };

  return (
    <div className="screen">
      <ScreenNav section="network" screens={screens} current={screen} onSelect={setScreen} />

      {shown[screen]}
    </div>
  );
}

export default Network;
