/**
 * The last screen: who contributed this node, and what has been written down.
 *
 * # The code carries a challenge and not the identifier
 *
 * A node's identifier is public — it is in the record and in the zone — so a code carrying only it
 * could be answered by anybody who had looked it up, and the node would write down that they
 * contributed it. **What the challenge adds is a nonce this node made and remembers**: approving it
 * is something only somebody looking at this screen can do, it is good for a stated while so that a
 * screenshot does not bind this machine a year later, and it names this node inside itself so an
 * approval cannot be lifted onto another one.
 *
 * The whole of that already exists below the interface — the challenge, the approval, and the check
 * against the key the claimant authorises in **their own** chain. This draws it.
 *
 * # It is optional, and the screen says so by having a way past it
 *
 * A node nobody claimed is a node: it serves, it keeps time, it is counted. What claiming decides is
 * **who would be credited** for what it serves, and somebody who wants to do that later does it from
 * the Network screen. A walk that made it the last gate would be making an unclaimed node look
 * unfinished.
 *
 * # And there is no *save* here
 *
 * The directory was taken, the key written and the record replayed as part of joining. A button
 * afterwards would be a second place a node can come from, whose failure mode is a node that joined
 * and then was not saved. What this says instead is what was written.
 */

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import ClaimCode from "@/components/ClaimCode";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { claimingCode, type Which } from "@/lib/network";

/**
 * How long a challenge stays good, in epochs — which are hours.
 *
 * Long enough to fetch a phone from another room, short enough that one left on a screen overnight
 * has stopped being answerable by morning.
 */
const FOR_EPOCHS = 1;

/** What the last screen shows. */
interface ClaimingProps {
  /** Which network this node joined, which is what the screen confirms. */
  which: Which;
  /** Called when the walk is done, whether or not anybody claimed it. */
  onDone: () => void;
}

/** Claiming the node, and what joining wrote down. */
function Claiming({ which, onDone }: ClaimingProps) {
  const { t } = useTranslation();
  const [challenge, setChallenge] = useState<string | null>(null);
  const [refused, setRefused] = useState(false);

  /* Asked for once the node is on a network, because a challenge names the node and a node has no
     name until it has announced itself on one. Nothing is set until the answer is back. */
  useEffect(() => {
    let alive = true;
    void claimingCode(FOR_EPOCHS)
      .then((shown) => {
        if (alive) setChallenge(shown);
      })
      .catch(() => {
        if (alive) setRefused(true);
      });
    return () => {
      alive = false;
    };
  }, []);

  return (
    <div className="py-8">
      <h1 className="screen__title">{t("onboarding.claim.title")}</h1>
      <p className="screen__lead">
        {t(
          which === "production"
            ? "onboarding.claim.onProduction"
            : "onboarding.claim.onDevelopment",
        )}
      </p>

      <Card className="mt-6">
        <CardHeader>
          <CardTitle>{t("onboarding.claim.scan")}</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">
            {t("onboarding.claim.body")}
          </p>
          <div className="mt-4">
            {refused ? (
              <p className="text-sm text-muted-foreground">{t("onboarding.claim.none")}</p>
            ) : challenge === null ? (
              <p className="text-sm text-muted-foreground">{t("onboarding.claim.drawing")}</p>
            ) : (
              <ClaimCode challenge={challenge} />
            )}
          </div>
          <p className="mt-4 text-xs text-muted-foreground">
            {t("onboarding.claim.expires", { hours: FOR_EPOCHS })}
          </p>
        </CardContent>
      </Card>

      {/* **No back from here.** Joining is written down, and a control that looked like it could
          take that back would be lying about what it does. */}
      <Button className="mt-6" onClick={onDone}>
        {t("onboarding.claim.done")}
      </Button>
    </div>
  );
}

export default Claiming;
