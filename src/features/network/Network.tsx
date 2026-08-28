/**
 * The Network screen: what this node is part of, and who it is talking to.
 *
 * A node that has not opened or joined a network reports having looked at nothing, and the controls
 * offer the one thing it can do about that. There is no mesh in this build, so there are no peers
 * and the list says so — figures here know the difference between none and unmeasured.
 */

import { useState } from "react";

import ScreenNav from "@/components/ScreenNav";
import NetworkFacts from "@/features/network/NetworkFacts";
import NodeControls from "@/features/network/NodeControls";
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
    about: (
      <>
        <NetworkFacts reading={reading} />
        <NodeControls
          onNetwork={reading?.network !== null && reading?.network !== undefined}
          onChanged={refresh}
        />
      </>
    ),
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
