/**
 * The first screen: whether this node is on the network, and what it is called there.
 *
 * # The shape is a status page, not a grid of cards
 *
 * It reads the way a node application reads everywhere: **one line saying whether it is connected**,
 * a line of two figures under it, and a short block of names. Somebody opening the window is asking
 * one question — *is it up* — and a grid of equal cards answers it in the same voice it answers
 * everything else, which is to say it does not answer it at all. So the state is a heading, at
 * heading size, and the names are a definition list under it.
 *
 * # Only what the node measured
 *
 * The two figures are **what it keeps on disk** and **how many peers it is connected to**, and both
 * come from the node. Neither is drawn where it was not measured: a node with no directory has not
 * been weighed, a node with no place on the mesh has counted nobody, and a nought in either place
 * would be a measurement claimed rather than taken.
 *
 * **The traffic under them is record traffic and says so.** The mesh counts the bytes of the acts,
 * the pages and the roots this node asked for and answered with, which is the whole reason it
 * exists; the handshake, the identify exchange, the pings and whatever a relay carries for
 * somebody else are outside it. One figure mixing *what this node moved* with *what its sockets
 * cost* would answer neither question, so the chart is labelled as what it counts.
 */

import { useTranslation } from "react-i18next";

import Copyable from "@/components/Copyable";
import { Card, CardContent } from "@/components/ui/card";
import Traffic from "@/features/home/Traffic";
import { useNetwork } from "@/hooks/useNetwork";
import { inBytes } from "@/lib/sizes";

/** What this build is. Vite replaces it at build time from `package.json`. */
const VERSION = __APP_VERSION__;

/** The application's first screen. */
function Home() {
  const { t, i18n } = useTranslation();
  const { reading, state } = useNetwork();

  /** Which of the four sentences the heading is, or nothing before the first look. */
  const heading = state === null ? null : t(`home.state.${state.state}`);

  /**
   * One name, with the button that puts the whole of it on the clipboard.
   *
   * A definition list rather than cards: these are labels and values, they are read down the left
   * edge, and the shortest of them is a version. `—` keeps saying in words what it means, because
   * a dash on its own tells a screen reader nothing about the difference between none and nobody
   * looked.
   */
  const named = (label: string, said: string | null, copyable = true) => (
    <div key={label} className="flex flex-wrap items-baseline gap-x-4 gap-y-1 py-1.5">
      <dt className="text-muted-foreground w-28 shrink-0 text-xs tracking-wide uppercase">
        {label}
      </dt>
      <dd className="min-w-0 flex-1 font-mono text-sm">
        {said === null ? (
          <span className="text-faint">
            <span aria-hidden="true">—</span>
            <span className="sr-only">{t("control.unmeasured")}</span>
          </span>
        ) : copyable ? (
          <Copyable value={said} what={label} className="text-sm" />
        ) : (
          <span className="break-all">{said}</span>
        )}
      </dd>
    </div>
  );

  return (
    <div className="screen">
      <Card>
        <CardContent className="flex flex-col gap-6">
          <div className="flex flex-col gap-2">
            {/* The one question this screen answers, at the size of an answer. Nothing at all
                until the first look has come back: a heading saying *not connected* over a node
                that is starting would be an assertion nobody made. */}
            <h1 className="text-2xl font-semibold tracking-tight">
              {heading ?? t("home.state.looking")}
            </h1>

            {/* What went wrong, where something did. Never a sentence from the node: the
                identifier travels and the reading happens on the Network screen, which is where
                the rest of what it means is. */}
            {state?.failing != null && (
              <p className="text-destructive text-sm">{t("home.wrong")}</p>
            )}

            {/* The two figures, on one line, in the order somebody asks them: what this costs me,
                and who it is talking to. Each is absent where it was not measured. */}
            <p className="text-muted-foreground text-sm">
              {reading?.stored == null
                ? t("home.keeping.unmeasured")
                : t("home.keeping.said", { size: inBytes(reading.stored, i18n.language) })}
              {" — "}
              {reading?.peers == null
                ? t("home.peers.unmeasured")
                : t("home.peers.said", { count: reading.peers })}
            </p>
          </div>

          <dl className="flex flex-col divide-y divide-border-soft">
            {named(t("home.on.network"), reading?.network ?? null)}
            {named(t("home.on.identity"), reading?.identity ?? null)}
            {/* **It is the public key**, and the label says so rather than making somebody work it
                out. The `PeerId` libp2p answers to *is* this node's Ed25519 public key with a
                prefix in front of it — not a hash of it (`SPECS.md §4.5`) — which is what lets a
                newcomer check it against the mesh handshake and against the certificate before it
                has any record to look anything up in.

                The row above it is the same key again, as the DID the record knows it by. Two
                encodings of one key is not two facts, and neither is dropped: one is what somebody
                pastes into a zone, the other is what they search the record for. */}
            {named(t("home.on.peer"), reading?.peer ?? null)}
            {/* The one row that is about this program rather than about the node, which is why it
                is last and why nobody copies it. */}
            {named(t("home.on.agent"), `almena ${VERSION}`, false)}
          </dl>

          {/* Last, because it is the one thing here that is not a fact but a shape: the figures
              above are true the moment they are read, and this is a few minutes of them. */}
          <Traffic />
        </CardContent>
      </Card>
    </div>
  );
}

export default Home;
