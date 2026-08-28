/**
 * The peers, or the reason there are none — and there are three different reasons.
 *
 * Telling them apart is the whole of this file, and they are not interchangeable:
 *
 * - **Nobody has looked yet.** Before the first reading comes back, saying "no peers" would be
 *   reporting a result nobody obtained. That it is only true for a moment is not a defence.
 * - **There is no network to have peers on.** `peers` is `null`: nothing was counted, because
 *   there is nothing to count. This is where this build lives.
 * - **There is a network and it has no peers.** `peers` is an empty array: somebody counted,
 *   and the answer is none. Unreachable today and written anyway, because the day it happens
 *   it must not borrow the sentence above.
 */

import { Radio, Unplug, Users } from "lucide-react";
import { useTranslation } from "react-i18next";

import EmptyState from "@/components/EmptyState";
import StateBadge from "@/components/StateBadge";
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemTitle,
} from "@/components/ui/item";
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

  if (reading.peers.length === 0) {
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
      {reading.peers.map((peer) => (
        // `role` is given rather than left off: `ItemGroup` is a list to a screen reader, and
        // a list whose children are not entries is one nothing can count.
        <Item key={peer.id} role="listitem" variant="muted" size="sm">
          <ItemContent>
            {/* A peer's name may be an IPv6 address or a key. It wraps rather than
                overflowing: there is no horizontal scrolling anywhere here. */}
            <ItemTitle className="block w-auto font-mono break-all whitespace-normal">
              {peer.id}
            </ItemTitle>
            <ItemDescription className="font-mono break-all">
              {peer.address}
            </ItemDescription>
          </ItemContent>

          <ItemActions>
            <StateBadge
              tone={peer.health}
              label={t(`network.health.${peer.health}`)}
            />
          </ItemActions>
        </Item>
      ))}
    </ItemGroup>
  );
}

export default PeerList;
