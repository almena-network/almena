/**
 * The head of the Network screen: what is known about the network this node is on.
 *
 * Every figure is a dash until the node is on a network, and that is the point rather than a
 * placeholder: a node on no network has looked at nothing, and `Figure` is the element that draws
 * the difference between "none" and "nobody looked". The peer count stays a dash a little longer
 * — until the node has taken its place on the mesh — because until then nobody has counted.
 *
 * The same figures the terminal draws, from the same place. Neither face works a figure out.
 */

import { useTranslation } from "react-i18next";

import Figure from "@/components/Figure";
import {
  Card,
  CardContent,
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

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("network.about.heading")}</CardTitle>
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
            label={t("network.about.figure.written")}
            value={
              reading?.written === null || reading?.written === undefined
                ? null
                : String(reading.written)
            }
          />
          <Figure
            label={t("network.about.figure.root")}
            value={reading?.root ?? null}
          />
          <Figure
            label={t("network.about.figure.peer")}
            value={reading?.peer ?? null}
          />
          <Figure
            label={t("network.about.figure.peers")}
            value={
              reading?.peers === null || reading?.peers === undefined
                ? null
                : String(reading.peers)
            }
          />
          <Figure
            label={t("network.about.figure.silent")}
            value={
              reading?.silent === null || reading?.silent === undefined
                ? null
                : String(reading.silent)
            }
          />
        </div>
      </CardContent>
    </Card>
  );
}

export default NetworkFacts;
