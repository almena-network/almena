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
 * machine, and beside the challenge it was a second square that looked the same and bound
 * nothing. The one code a person scans is the challenge, and it is asked for from the controls
 * below this card — which is now the only place in the window it is drawn at all.
 *
 * # Every address here leaves the application by button
 *
 * These four are the values on the whole screen most likely to be wanted somewhere else — an
 * address goes into a browser, the link goes into a client — and until now the only way to take
 * one was to read it off the screen and type it back in. Each carries the same copy control
 * every identifier in the application does, and what it copies is the whole address rather than
 * the wrapped lines it is drawn as.
 */

import { useState } from "react";
import { useTranslation } from "react-i18next";

import Copyable from "@/components/Copyable";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { nodeLink, readSeedRecord, type NetworkReading } from "@/lib/network";

/** Where the description of this interface is served, which is the same path on every node. */
const DESCRIPTION = "/openapi.json";

/** What this node publishes. */
function Published({ reading }: { reading: NetworkReading | null }) {
  const { t } = useTranslation();
  /* The draft, once it has been asked for. `undefined` is nobody asked, `null` is asked and this
     node has no place on the mesh to be a seed from — two different things and drawn differently. */
  const [record, setRecord] = useState<string | null | undefined>(undefined);

  /**
   * One address, or the mark that says there is none.
   *
   * The divider is `border-border-soft`, which is the token file's own name for a line inside a
   * panel. It was written as an arbitrary value naming `--line-soft`, which this project has
   * never defined, so what every one of these rows drew was the fallback every undefined colour
   * gets — the text colour, at full strength, as a rule across the card.
   */
  const line = (what: string, said: string | null) => (
    <div
      key={what}
      className="flex flex-wrap items-center justify-between gap-2 border-t border-border-soft py-3 text-sm first:border-t-0"
    >
      <span className="text-muted-foreground">{what}</span>

      {said === null ? (
        <span className="font-mono text-faint">
          <span aria-hidden="true">—</span>
          <span className="sr-only">{t("control.unmeasured")}</span>
        </span>
      ) : (
        <Copyable value={said} what={what} />
      )}
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

          {/* **Asking is not publishing, and the wording says so.** Nothing here writes to a zone:
              it hands the operator the parts of the record only this node can produce, so that a
              request to be a seed carries something correct instead of something assembled by
              hand. Whether to make that request at all is theirs — a node behind a household
              router should not, and the line under the draft says why. */}
          <div className="mt-4 flex flex-col gap-2 border-t border-border-soft pt-4">
            {record === undefined ? (
              <>
                <Button
                  variant="secondary"
                  onClick={() => {
                    void readSeedRecord()
                      .then(setRecord)
                      .catch(() => {
                        setRecord(null);
                      });
                  }}
                >
                  {t("network.published.beASeed")}
                </Button>
                <p className="text-muted-foreground text-sm">
                  {t("network.published.beASeedWhy")}
                </p>
              </>
            ) : record === null ? (
              /* Not a failure: a node with no place on the mesh has no port to be a seed on, and
                 saying that is more use than an empty box. */
              <p className="text-muted-foreground text-sm">
                {t("network.published.noSeedRecord")}
              </p>
            ) : (
              <>
                <p className="text-sm font-medium">{t("network.published.seedRecord")}</p>
                {/* The whole block leaves by the same button every value on this card leaves by,
                    because what somebody does with it is paste it into a request — and it is drawn
                    as the block it is rather than as a run of wrapped text, which is what a
                    `_seed` record turns into anywhere else. */}
                <Copyable value={record} what={t("network.published.seedRecord")}>
                  <pre className="bg-sunken min-w-0 flex-1 overflow-x-auto rounded-md p-3 font-mono text-xs">
                    {record}
                  </pre>
                </Copyable>
                <p className="text-muted-foreground text-sm">
                  {t("network.published.seedRecordCheck")}
                </p>
              </>
            )}
          </div>
        </CardContent>
      </Card>
    </>
  );
}

export default Published;
