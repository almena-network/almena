/**
 * The first thing a node with no network shows: which network it is for.
 *
 * # One decision, and it is the only one worth asking for
 *
 * Signing against the wrong network does not come undone — the log does not forget — so which one
 * this node joins is a decision and never a default. **Everything after it is the node's own
 * work**: finding somebody already there, pulling the record, replaying it and announcing itself.
 * A wizard that walked an operator through those would be asking for presses on steps nobody can
 * judge, and would make a failure in any of them look like something they did.
 *
 * # Production leads, and development is offered as what it is
 *
 * The two are not equals. Production is the network; development is where a thing is tried before
 * it is real. So one is the offer and the other is the alternative, and the second says what it is
 * for rather than sitting beside the first as though the choice were a coin toss.
 *
 * # Development can be opened; production never is
 *
 * A network is opened once, ever. Where the development zone names nobody, this node opens one —
 * which is what *nobody is there* means on a network that is re-opened whenever the format moves.
 * Production has no such path and must not: arriving at production is arriving at the one that
 * exists.
 */

import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { joinANetwork, openDevelopment, type Which } from "@/lib/network";

/**
 * The port this node listens on for the mesh.
 *
 * **Chosen and not discovered**, because it is the one that gets published in the zone: a node that
 * took whatever was free would be a node whose published record is wrong the next time it starts.
 */
const PORT = 4001;

/**
 * The identifiers this build knows how to read.
 *
 * A node's own list is allowed to grow without this being rebuilt, so anything from outside it is
 * drawn with the general sentence instead of being put on screen raw. The identifier is already in
 * the records — the Rust side writes it the moment it happens — so nothing is lost by keeping it
 * off the screen.
 */
const SAYS = {
  already_on_a_network: "alreadyOnANetwork",
  no_such_network: "noSuchNetwork",
  zone_silent: "zoneSilent",
  nobody_is_there: "nobodyIsThere",
  no_transport: "noTransport",
  seed_unreachable: "seedUnreachable",
  seed_would_not_answer: "seedWouldNotAnswer",
  seed_too_slow: "seedTooSlow",
  not_the_promised_network: "notThePromisedNetwork",
  record_does_not_add_up: "recordDoesNotAddUp",
  unreadable_record: "unreadableRecord",
  no_directory: "noDirectory",
  directory_held: "directoryHeld",
  no_randomness: "noRandomness",
  unreadable_identity: "unreadableIdentity",
} as const;

/** Which sentence an identifier reads as. */
function reason(code: string) {
  const known = Object.entries(SAYS).find(([said]) => said === code)?.[1];
  return known === undefined
    ? ("onboarding.says.somethingElse" as const)
    : (`onboarding.says.${known}` as const);
}

/** What a node with no network is offered. */
function Onboarding({ onJoined }: { onJoined: () => void }) {
  const { t } = useTranslation();
  const [joining, setJoining] = useState<Which | null>(null);
  const [said, setSaid] = useState<string | null>(null);

  const go = (which: Which) => {
    setJoining(which);
    setSaid(null);
    void (async () => {
      try {
        await joinANetwork(which, PORT);
        onJoined();
      } catch (why) {
        const identifier = typeof why === "string" ? why : "";
        // **Nobody there is not a failure on development.** It is the one condition under which a
        // network is opened, and opening one is what this node does about it. On production it
        // stays what it is: there is nothing to join yet.
        if (identifier === "nobody_is_there" && which === "development") {
          try {
            await openDevelopment();
            onJoined();
            return;
          } catch (instead) {
            setSaid(typeof instead === "string" ? instead : "");
          }
        } else {
          setSaid(identifier);
        }
      } finally {
        setJoining(null);
      }
    })();
  };

  return (
    <div className="screen">
      <h1 className="screen__title">{t("onboarding.title")}</h1>
      <p className="screen__lead">{t("onboarding.lead")}</p>

      <div className="mt-6 grid gap-4">
        {/* The offer. It carries the identity colour because it is the thing this node is for. */}
        <Card className="border-[var(--brand-edge)] bg-[var(--brand-wash)]">
          <CardHeader>
            <CardTitle>{t("onboarding.production.title")}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground">
              {t("onboarding.production.body")}
            </p>
            <Button
              className="mt-4"
              disabled={joining !== null}
              onClick={() => go("production")}
            >
              {joining === "production"
                ? t("onboarding.joining")
                : t("onboarding.production.go")}
            </Button>
          </CardContent>
        </Card>

        {/* The alternative, said as what it is for rather than as an equal. */}
        <Card>
          <CardHeader>
            <CardTitle>{t("onboarding.development.title")}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground">
              {t("onboarding.development.body")}
            </p>
            <Button
              className="mt-4"
              variant="outline"
              disabled={joining !== null}
              onClick={() => go("development")}
            >
              {joining === "development"
                ? t("onboarding.joining")
                : t("onboarding.development.go")}
            </Button>
          </CardContent>
        </Card>
      </div>

      {said !== null && (
        <p className="mt-4 text-sm text-[var(--tone-bad)]">
          {t(reason(said))}
        </p>
      )}
    </div>
  );
}

export default Onboarding;
