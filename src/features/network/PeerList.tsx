/**
 * The peers, or the reason there are none — and there are three different reasons.
 *
 * Telling them apart is the whole of this file, and they are not interchangeable:
 *
 * - **Nobody has looked yet.** Before the first reading comes back, saying "no peers" would be
 *   reporting a result nobody obtained. That it is only true for a moment is not a defence.
 * - **Nothing has been counted.** `peers` is `null`: the node is on no network, or has not taken
 *   its place on the mesh yet, and either way nobody has counted.
 * - **There is a mesh and nobody is on the other end of it.** `peers` is nought: somebody
 *   counted, and the answer is none.
 *
 * What is drawn when there are some is the count, because the count is what the mesh hands over:
 * who is connected is a fact about sockets, and which node each one is would be a claim this
 * screen has not checked against the record.
 */

import { Radio, Unplug, Users } from "lucide-react";
import { useTranslation } from "react-i18next";

import EmptyState from "@/components/EmptyState";
import { Item, ItemContent, ItemGroup, ItemTitle } from "@/components/ui/item";
import type { NetworkReading } from "@/lib/network";

/** What the list is drawn from. */
interface PeerListProps {
  /** The last reading, or `null` when none has come back yet. */
  reading: NetworkReading | null;
}

/** The peers, or the reason there are none. */
function PeerList({ reading }: PeerListProps) {
  const { t } = useTranslation();

  if (reading === null) {
    return (
      <EmptyState
        icon={<Radio aria-hidden="true" />}
        title={t("network.peers.lookingTitle")}
        reason={t("network.peers.looking")}
      />
    );
  }

  if (reading.peers === null) {
    return (
      <EmptyState
        icon={<Unplug aria-hidden="true" />}
        title={t("network.peers.noNetworkTitle")}
        reason={t("network.peers.noNetwork")}
      />
    );
  }

  if (reading.peers === 0) {
    return (
      <EmptyState
        icon={<Users aria-hidden="true" />}
        title={t("network.peers.noneTitle")}
        reason={t("network.peers.none")}
      />
    );
  }

  return (
    <ItemGroup aria-label={t("network.peers.heading")} className="gap-1">
      {/* `role` is given rather than left off: `ItemGroup` is a list to a screen reader, and a
          list whose children are not entries is one nothing can count. */}
      <Item role="listitem" variant="muted" size="sm">
        <ItemContent>
          <ItemTitle>{t("network.peers.connected", { count: reading.peers })}</ItemTitle>
        </ItemContent>
      </Item>
    </ItemGroup>
  );
}

export default PeerList;
