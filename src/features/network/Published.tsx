/**
 * What this node publishes, and the code that claims it.
 *
 * # The services, as addresses somebody can actually use
 *
 * A node is only worth anything to somebody who can reach it, so what it serves is drawn as
 * addresses rather than as a list of capability names: the interface it answers on, the address it
 * answers to on the mesh, and the description of that interface — which is the one door here that
 * exists so a machine can find the rest without being told.
 *
 * **Absent is drawn as absent.** A node that is not serving the interface has no origin to show,
 * and a blank where an address goes says that better than a plausible-looking `localhost` would.
 *
 * # The code is the node's identifier and nothing else
 *
 * Somebody with the client scans it and signs, on the network, that they contributed this node.
 * **It carries no challenge**, which is a decision with a cost written here rather than left to be
 * discovered: a node's identifier is public — it is in the record and in the zone — so the code
 * proves somebody was in front of this machine only in the sense that they could have been.
 * A nonce is what would make *I scanned this now* different from *I looked it up*.
 */

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import QRCode from "qrcode";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { NetworkReading } from "@/lib/network";

/** Where the description of this interface is served, which is the same path on every node. */
const DESCRIPTION = "/openapi.json";

/** What this node publishes. */
function Published({ reading }: { reading: NetworkReading | null }) {
  const { t } = useTranslation();
  const [code, setCode] = useState<string | null>(null);
  const identity = reading?.identity ?? null;

  /* Drawn once the identifier is known, and again if it ever changes. Nothing is set until the
     drawing is back, so the card never holds a code for a different node. */
  useEffect(() => {
    if (identity === null) return;
    let alive = true;
    void QRCode.toString(identity, {
      type: "svg",
      margin: 1,
      errorCorrectionLevel: "Q",
      color: { dark: "#e9ecf1", light: "#0e1116" },
    })
      .then((svg) => {
        if (alive) setCode(svg);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [identity]);

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
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("network.claim.title")}</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">
            {t("network.claim.body")}
          </p>
          {code === null ? (
            <p className="mt-4 text-sm text-muted-foreground">
              {t("network.claim.none")}
            </p>
          ) : (
            <div
              className="mt-4 w-full max-w-[240px] rounded-lg border border-[var(--line)] bg-[var(--surface-0)] p-4 [&_svg]:block [&_svg]:h-auto [&_svg]:w-full"
              dangerouslySetInnerHTML={{ __html: code }}
            />
          )}
          <p className="mt-4 font-mono text-xs break-all">{identity ?? "—"}</p>
        </CardContent>
      </Card>
    </>
  );
}

export default Published;
