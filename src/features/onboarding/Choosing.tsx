/**
 * Which network this node is for, and the joining that follows from it.
 *
 * # The one decision, and it does not come undone
 *
 * Signing against the wrong network is not something a later screen fixes: the log does not forget.
 * So the two are not offered as equals — production is the network and leads, development is where
 * something is tried before it is real and says so.
 *
 * # Everything after the choice is the node's own work
 *
 * Finding somebody already there, pulling the record, replaying it and announcing itself. A walk
 * that asked for a press at each of those would be asking somebody to approve steps they cannot
 * judge, and would make a failure in any of them look like something they did.
 *
 * # Nobody there means opening, under the same press
 *
 * A zone that names nobody means there is a network to **open** rather than one to join. It is a
 * different act — a production network is opened once in the history of the platform, not once per
 * machine — and it is not a different decision: which network was the decision, and it has been
 * taken by the time the zone is asked.
 *
 * **Production is still gated, and not here.** `Node::open_in` refuses to open one on a format that
 * is still moving, and what comes back is `format_is_not_frozen` with the reason readable from
 * `freeze_checklist`. Nothing in this file repeats that check: a second implementation of a rule is
 * two rules that will one day disagree. What this file does is show the checklist under the
 * production card, so that a refusal is never the first anybody hears of it.
 *
 * # One press, and the node is a node
 *
 * Joining alone left a node holding the record and reachable by nobody. So the same press takes
 * its place on the mesh and serves the interface, on the ports the terminal's development node
 * leaves free; the ports are remembered so the next start takes the same ones, and the Network
 * screen is where either is changed.
 *
 * # Advanced, behind a disclosure
 *
 * Another zone, a seed by hand, and *nobody is there* are for a network being tried out on one
 * machine — the terminal's `--zone`, `--seed` and `--nobody-is-there` — and a person setting up a
 * node for the real network never needs them. They are here so the window can be tried against
 * an emulated zone the way the terminal can, and folded away so the one decision stays alone.
 */

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  freezeChecklist,
  joinANetwork,
  joinTheMesh,
  openANetwork,
  serveInterface,
  type Line,
  type Which,
} from "@/lib/network";
import { choose, preferences } from "@/lib/preferences";

/**
 * The port this node listens on for the mesh, unless one is remembered.
 *
 * **Chosen and not discovered**, because it is the one that gets published in the zone: a node that
 * took whatever was free would be a node whose published record is wrong the next time it starts.
 * Not 4001, which is what a terminal node on the same computer takes while developing.
 */
const PORT = 4002;

/** The address this node serves the interface on, unless one is remembered. Not the terminal's. */
const INTERFACE = "127.0.0.1:8791";

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
  the_zone_is_unreadable: "theZoneIsUnreadable",
  // Opening, which is what happens where the zone named nobody.
  there_is_a_network: "thereIsANetwork",
  format_is_not_frozen: "formatIsNotFrozen",
  record_would_not_start: "recordWouldNotStart",
  no_clock: "noClock",
  directory_cannot_be_held: "directoryCannotBeHeld",
  government_key_not_kept: "governmentKeyNotKept",
  nobody_is_there_is_for_development: "nobodyIsThereIsForDevelopment",
  resolver_not_an_address: "resolverNotAnAddress",
  // Taking a place and serving, which the same press does.
  mesh_address_unavailable: "meshAddressUnavailable",
  address_unavailable: "addressUnavailable",
} as const;

/** Which sentence an identifier reads as. */
function reason(code: string) {
  const known = Object.entries(SAYS).find(([said]) => said === code)?.[1];
  return known === undefined
    ? ("onboarding.says.somethingElse" as const)
    : (`onboarding.says.${known}` as const);
}

/** What is asked on this screen, and what is done about the answer. */
interface ChoosingProps {
  /** Back to the screen before. */
  onBack: () => void;
  /** Called once this node is on a network, with which one. */
  onJoined: (which: Which) => void;
}

/** The identifier an error carries, or nothing readable. */
function said(why: unknown): string {
  return typeof why === "string" ? why : "";
}

/** Choosing a network, and joining it. */
function Choosing({ onBack, onJoined }: ChoosingProps) {
  const { t } = useTranslation();
  const [joining, setJoining] = useState<Which | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [zone, setZone] = useState("");
  const [seed, setSeed] = useState("");
  const [nobodyIsThere, setNobodyIsThere] = useState(false);
  const [checklist, setChecklist] = useState<Line[] | null>(null);

  /* The checklist is asked of this build once, so that a refusal to open production is never the
     first anybody hears of it. Nothing is opened by asking. */
  useEffect(() => {
    let alive = true;
    void freezeChecklist()
      .then((lines) => {
        if (alive) setChecklist(lines);
      })
      .catch(() => {
        if (alive) setChecklist([]);
      });
    return () => {
      alive = false;
    };
  }, []);

  const remembered = preferences();
  const port = remembered.mesh ?? PORT;
  const address = remembered.interface ?? INTERFACE;
  const where = {
    zone: zone.trim() || undefined,
    seeds: seed.trim() ? [seed.trim()] : [],
  };

  /** On a network, by joining or — where nobody is there — by opening. */
  const onto = async (which: Which) => {
    // Somebody's word instead of the zone's is an opening and not a joining, and it reaches
    // development alone; the node refuses it for production before anything happens.
    if (nobodyIsThere && which === "development") {
      await openANetwork(which, where.zone, true);
      return;
    }
    try {
      await joinANetwork(which, port, where);
    } catch (why) {
      // Nobody there is the other outcome and not a failure: there is a network to open rather
      // than one to join, and opening it is what this node does about it.
      if (said(why) !== "nobody_is_there") throw why;
      await openANetwork(which, where.zone);
    }
  };

  const go = (which: Which) => {
    setJoining(which);
    setFailed(null);
    void (async () => {
      try {
        await onto(which);
        // The same press makes it a node somebody can reach: a place on the mesh and the
        // interface, on the ports the next start will take again.
        await joinTheMesh({ port });
        await serveInterface(address);
        void choose({ mesh: port, interface: address });
        onJoined(which);
      } catch (why) {
        setFailed(said(why));
      } finally {
        setJoining(null);
      }
    })();
  };

  return (
    <div className="py-8">
      <h1 className="screen__title">{t("onboarding.title")}</h1>
      <p className="screen__lead">{t("onboarding.lead")}</p>

      <div className="mt-6 grid gap-4">
        {/* The offer. It carries the identity colour because it is what this node is for. */}
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
            {/* The question asked before production is opened for good, with nothing at
                stake: every line is a probe that just ran against this build. */}
            <div className="mt-4 border-t border-[var(--line-soft)] pt-3">
              <p className="text-xs font-medium">{t("onboarding.checklist.title")}</p>
              {checklist === null ? (
                <p className="mt-1 text-xs text-muted-foreground">
                  {t("onboarding.checklist.looking")}
                </p>
              ) : (
                <ul className="mt-1 flex flex-col gap-1">
                  {checklist.map((line) => (
                    <li key={line.called} className="flex flex-wrap gap-2 font-mono text-xs">
                      <span>{line.called}</span>
                      <span
                        className={
                          line.wanting === null
                            ? "text-muted-foreground"
                            : "text-[var(--tone-bad)]"
                        }
                      >
                        {line.wanting ?? t("onboarding.checklist.holds")}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </div>
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

        {/* Folded away: another zone, a seed by hand and somebody's word that nobody is there
            are for a network being tried out on one machine, and the one decision stays alone. */}
        <details className="rounded-lg border border-[var(--line-soft)] p-3">
          <summary className="cursor-pointer text-sm">{t("onboarding.advanced.title")}</summary>
          <div className="mt-3 flex flex-col gap-2">
            <input
              className="border-input bg-transparent h-9 rounded-md border px-3 text-sm"
              aria-label={t("onboarding.advanced.zone")}
              placeholder={t("onboarding.advanced.zone")}
              value={zone}
              onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
                setZone(event.target.value)
              }
            />
            <input
              className="border-input bg-transparent h-9 rounded-md border px-3 font-mono text-sm"
              aria-label={t("onboarding.advanced.seed")}
              placeholder={t("onboarding.advanced.seed")}
              value={seed}
              onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
                setSeed(event.target.value)
              }
            />
            <div className="flex items-center gap-2">
              <input
                id="nobody-is-there"
                type="checkbox"
                checked={nobodyIsThere}
                onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
                  setNobodyIsThere(event.target.checked)
                }
              />
              <label htmlFor="nobody-is-there" className="text-sm">
                {t("onboarding.advanced.nobodyIsThere")}
              </label>
            </div>
          </div>
        </details>
      </div>

      {failed !== null && (
        <p className="mt-4 text-sm text-[var(--tone-bad)]">{t(reason(failed))}</p>
      )}

      {/* Back costs nothing here: nothing has been written yet. It is gone from the screen after
          this one, where it would be offering to take back something the record holds. */}
      <Button className="mt-6" variant="ghost" disabled={joining !== null} onClick={onBack}>
        {t("onboarding.back")}
      </Button>
    </div>
  );
}

export default Choosing;
