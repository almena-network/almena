/**
 * The Network screen: what this node is part of, and who it is talking to.
 *
 * **It reports and it does not operate.** There was a card of controls under these — every act the
 * terminal takes as a flag, one disclosure down — and it is gone. A node comes up with the
 * application and joins the network by itself; what was left in that card was a second way to do
 * what the start already does, and a shelf of operator's acts that made the window read as a
 * console for a node rather than as the node's own window.
 *
 * What that costs is written down where it has to be: the table in `almena-node::facade` says
 * which face offers which capability, and every one this screen stopped offering now says in words
 * why the window does not have it. The terminal has all of them and is where they live.
 *
 * A node that has not joined a network reports having looked at nothing, and every figure is a
 * dash. Until it has taken its place on the mesh nobody has counted its peers, and the list says so
 * — figures here know the difference between none and unmeasured.
 */

import { useState } from "react";

import CardGrid from "@/components/CardGrid";
import ScreenNav from "@/components/ScreenNav";
import NetworkFacts from "@/features/network/NetworkFacts";
import Peers from "@/features/network/Peers";
import Published from "@/features/network/Published";
import { screensOf, type ScreensOf } from "@/features/shell/sections";
import { useNetwork, usePeers } from "@/hooks/useNetwork";

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
  const { reading, state, lookedAt, looking, refresh } = useNetwork();
  // Kept beside the reading and refreshed with it: the count at the head of the other screen and
  // the list on this one are two views of one moment, and two buttons would let them disagree.
  const { peers, refresh: lookAtPeers } = usePeers();

  // Total over `Screen`: naming a screen in `sections.ts` and forgetting it here fails `tsc`.
  const shown: Record<Screen, React.ReactNode> = {
    about: (
      <>
        {/* Two cards that report, side by side once there is room for them. `CardGrid` finds its
            own column count from the width it is given, so this is two columns at 1100 points and
            one at 400 without a breakpoint of its own. */}
        <CardGrid>
          <NetworkFacts reading={reading} state={state} />
          <Published reading={reading} />
        </CardGrid>
      </>
    ),
    peers: (
      <Peers
        reading={reading}
        peers={peers}
        lookedAt={lookedAt}
        looking={looking}
        onRefresh={() => {
          refresh();
          lookAtPeers();
        }}
      />
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
