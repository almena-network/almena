/**
 * What this node publishes: where it can be reached, and the link a client reads to choose it.
 *
 * # The services, as addresses somebody can actually use
 *
 * A node is only worth anything to somebody who can reach it, so what it serves is drawn as
 * addresses rather than as a list of capability names: the interface it answers on, the address it
 * answers to on the mesh, the description of that interface — which is the one door here that
 * exists so a machine can find the rest without being told — and the link that carries the first
 * and the second together, which is what the client reads.
 *
 * **Absent is drawn as absent.** A node that is not serving the interface has no origin to show,
 * and a blank where an address goes says that better than a plausible-looking `localhost` would.
 *
 * # There is no code here, on purpose
 *
 * There was one, and it carried the node's identifier alone. A node's identifier is public — it
 * is in the record and in the zone — so scanning it proved nothing about who was in front of the
 * machine, and beside the challenge the walk draws it was a second square that looked the same
 * and bound nothing. The one code a person scans is the challenge, and it is asked for from the
 * controls below this card.
 */

import { useTranslation } from "react-i18next";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { nodeLink, type NetworkReading } from "@/lib/network";

/** Where the description of this interface is served, which is the same path on every node. */
const DESCRIPTION = "/openapi.json";

/** What this node publishes. */
function Published({ reading }: { reading: NetworkReading | null }) {
  const { t } = useTranslation();

  /** One address, or the mark that says there is none. */
  const line = (what: string, said: string | null) => (
    <div key={what} className="flex flex-wrap justify-between gap-2 border-t border-[var(--line-soft)] py-3 text-sm first:border-t-0">
      <span className="text-muted-foreground">{what}</span>
      <span className="font-mono break-all">{said ?? "—"}</span>
    </div>
  );

  return (
    <>
      <Card>
        <CardHeader>
          <CardTitle>{t("network.published.title")}</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">
            {t("network.published.body")}
          </p>
          <div className="mt-4">
            {line(t("network.published.interface"), reading?.origin ?? null)}
            {line(t("network.published.mesh"), reading?.peer ?? null)}
            {line(
              t("network.published.description"),
              reading?.origin == null ? null : `${reading.origin}${DESCRIPTION}`,
            )}
            {line(t("network.published.link"), nodeLink(reading))}
          </div>
        </CardContent>
      </Card>
    </>
  );
}

export default Published;
