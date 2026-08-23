/**
 * The peers this node is connected to, and the two ways the list gets fresher.
 *
 * There are none, and there is no network for there to be any on, so what the list draws today
 * is the sentence saying so. It is not a placeholder: the rows below are written and wired, and
 * the first peer to exist appears in one without this file changing.
 *
 * The moment of the last look is drawn beside the button on purpose. It is the only thing on
 * this card that changes when the button is pressed, and a control with no visible effect is
 * one nobody can tell from a broken one. While a look is in flight the button holds a spinner
 * instead of its icon, which is the same argument a beat earlier.
 *
 * The button is the plain tone and not the identity one. Refreshing a list is not what a person
 * came to this screen for, and the identity colour has one meaning per screen.
 */

import { RefreshCw, Unplug } from "lucide-react";
import { useTranslation } from "react-i18next";

import StateBadge from "@/components/StateBadge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import {
  Item,
  ItemActions,
  ItemContent,
  ItemDescription,
  ItemGroup,
  ItemTitle,
} from "@/components/ui/item";
import { Spinner } from "@/components/ui/spinner";
import type { NetworkReading } from "@/lib/network";

/** What the peers card is drawn from. */
interface PeersProps {
  /** The last reading, or `null` before the first one has come back. */
  reading: NetworkReading | null;
  /** When that reading was taken. */
  lookedAt: Date | null;
  /** Whether a look is in flight, which is when the button cannot be pressed again. */
  looking: boolean;
  /** Takes another reading now. */
  onRefresh: () => void;
}

/** The list of peers, with what keeps it current. */
function Peers({ reading, lookedAt, looking, onRefresh }: PeersProps) {
  const { t, i18n } = useTranslation();
  const peers = reading?.peers ?? [];

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("network.peers.heading")}</CardTitle>
      </CardHeader>

      <CardContent className="flex flex-col gap-4">
        {peers.length === 0 ? (
          <Empty className="border">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <Unplug aria-hidden="true" />
              </EmptyMedia>
              <EmptyTitle>{t("network.peers.noneTitle")}</EmptyTitle>
              <EmptyDescription>{t("network.peers.none")}</EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <ItemGroup aria-label={t("network.peers.heading")} className="gap-1">
            {peers.map((peer) => (
              // `role` is given rather than left off: `ItemGroup` is a list to a screen
              // reader, and a list whose children are not entries is one nothing can count.
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
        )}

        <div className="flex flex-wrap items-center gap-3">
          <Button variant="outline" disabled={looking} onClick={onRefresh}>
            {looking ? (
              <Spinner aria-label={t("control.working")} />
            ) : (
              <RefreshCw aria-hidden="true" />
            )}
            {t("network.peers.refresh")}
          </Button>

          <p className="note" role="status">
            {lookedAt === null
              ? ""
              : t("network.peers.lookedAt", {
                  time: lookedAt.toLocaleTimeString(i18n.language),
                })}
          </p>
        </div>
      </CardContent>
    </Card>
  );
}

export default Peers;
