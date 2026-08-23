/**
 * The head of the Network screen: what is known about the network this node is on.
 *
 * Three figures, and today all three are a dash. That is the point rather than a placeholder:
 * this build reads no configuration, has no identity and has counted no peers, and `Figure` is
 * the element that draws the difference between "none" and "nobody looked".
 */

import { useTranslation } from "react-i18next";

import Figure from "@/components/Figure";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { NetworkReading } from "@/lib/network";

/** What the head is drawn from. */
interface NetworkFactsProps {
  /** The last reading, or `null` before the first one has come back. */
  reading: NetworkReading | null;
}

/** The figures at the head of the Network screen. */
function NetworkFacts({ reading }: NetworkFactsProps) {
  const { t } = useTranslation();
  const peers = reading?.peers;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("network.about.heading")}</CardTitle>
        <CardDescription>{t("network.about.body")}</CardDescription>
      </CardHeader>

      <CardContent>
        {/* The figures flow the way cards do, and for the same reason: at 400 points across
            three of them do not fit side by side, and a row that overflowed would be the one
            place you could scroll sideways. */}
        <div className="grid grid-cols-[repeat(auto-fit,minmax(min(100%,var(--width-figure-min)),1fr))] gap-4">
          <Figure
            label={t("network.about.figure.network")}
            value={reading?.network ?? null}
          />
          <Figure
            label={t("network.about.figure.identity")}
            value={reading?.identity ?? null}
          />
          <Figure
            label={t("network.about.figure.peers")}
            value={peers === null || peers === undefined ? null : String(peers.length)}
          />
        </div>
      </CardContent>
    </Card>
  );
}

export default NetworkFacts;
