/**
 * The card holding the peers, and the two ways the list gets fresher.
 *
 * What the list itself draws — the peers, or which of the three reasons there are none — is
 * `PeerList`. This file is the card around it and the row underneath.
 *
 * The moment of the last look is drawn beside the button on purpose. On a screen with no peers
 * it is the only thing that changes when the button is pressed, and a control with no visible
 * effect is one nobody can tell from a broken one. While a look is in flight the button holds a
 * spinner instead of its icon, which is the same argument a beat earlier.
 *
 * The button is the plain tone and not the identity one. Refreshing a list is not what a person
 * came to this screen for, and the identity colour has one meaning per screen.
 */

import { RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";

import PeerList from "@/features/network/PeerList";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Spinner } from "@/components/ui/spinner";
import type { NetworkReading, Peer } from "@/lib/network";

/** What the peers card is drawn from. */
interface PeersProps {
  /** The last reading, or `null` before the first one has come back. */
  reading: NetworkReading | null;
  /** When that reading was taken, or `null` when none has been. */
  lookedAt: Date | null;
  /** Whether a look is in flight, which is when the button cannot be pressed again. */
  looking: boolean;
  /** Who is connected, `null` where nobody counted, `undefined` before the first look. */
  peers: Peer[] | null | undefined;
  /** Takes another reading now. */
  onRefresh: () => void;
}

/** The list of peers, with what keeps it current. */
function Peers({ reading, peers, lookedAt, looking, onRefresh }: PeersProps) {
  const { t, i18n } = useTranslation();

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("network.peers.heading")}</CardTitle>
      </CardHeader>

      <CardContent className="flex flex-col gap-4">
        <PeerList reading={reading} peers={peers} />

        <div className="flex flex-wrap items-center gap-3">
          <Button variant="outline" disabled={looking} onClick={onRefresh}>
            {looking ? (
              <Spinner aria-label={t("control.working")} />
            ) : (
              <RefreshCw aria-hidden="true" />
            )}
            {t("network.peers.refresh")}
          </Button>

          {/* In the document from the start, empty until there has been a look to report. */}
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
