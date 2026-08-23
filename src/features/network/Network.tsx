/**
 * The Network screen: what this node is part of, and who it is talking to.
 *
 * Two cards, and both of them tell the same truth in different words today — there is no
 * peer-to-peer layer in this build, so there is no network and no peer. What is real here is
 * the machinery: a reading taken on a timer and on demand, figures that know the difference
 * between none and unmeasured, and a list that will draw a peer the day there is one.
 */

import { useTranslation } from "react-i18next";

import NetworkFacts from "@/features/network/NetworkFacts";
import Peers from "@/features/network/Peers";
import { useNetwork } from "@/hooks/useNetwork";

/** The Network screen. */
function Network() {
  const { t } = useTranslation();
  const { reading, lookedAt, looking, refresh } = useNetwork();

  return (
    <div className="screen">
      <h1 className="screen__title">{t("section.network")}</h1>

      <NetworkFacts reading={reading} />

      <Peers
        reading={reading}
        lookedAt={lookedAt}
        looking={looking}
        onRefresh={refresh}
      />
    </div>
  );
}

export default Network;
