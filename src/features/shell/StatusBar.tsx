/**
 * The strip along the true bottom of the window: what the application is, and what it is doing.
 *
 * It is the frame's and not a screen's, which is the whole difference between this and a page
 * footer. It spans the full width — under the sidebar as well as beside it — it is pinned to
 * the bottom rather than reached by scrolling, and it is the same strip whichever screen is
 * open. Nothing on it scrolls away.
 *
 * **The left group is what the application is doing**, which is what it was always for and what
 * it stood empty waiting for: which network the node is on, which of the four states it is in,
 * and how many peers it has. Nothing is invented to fill it — before the first look has come
 * back there is no state and nothing is drawn, and a peer count nobody took is a dash rather
 * than a nought. The right group holds what does not change while the application runs.
 *
 * **It never wraps and never scrolls**, which `shell.css` enforces and this obeys. In the compact
 * shape the network's word and the peer count come off it altogether — below 600 points there is
 * no room for all three without every item on the strip becoming an ellipsis, and *what does not
 * fit does not go on it* is the rule rather than a squeeze. The state badge is what stays,
 * because *is anything wrong* is the question a strip is read for.
 *
 * The mark is drawn in the strip's own faint colour rather than in the identity one. It is
 * here to say which application this is at a glance, not to be the thing anybody is looking
 * at — and the strip is the one place the mark is visible in both shapes.
 *
 * **A node on the development network says so here, and this is the one place it could say it.**
 * The marker has to be on screen whatever section is open and whichever shape the window is in,
 * and this strip is the only thing in the application that is.
 *
 * **Production wears nothing.** The marker is not a label saying which of two networks this is —
 * it is a warning that this one is *not the real one*, and a badge on production would be a badge
 * a person stops reading. Nothing drawn is what production looks like, so the marker appearing at
 * all is the whole of the signal.
 *
 * It used to say which *build* this was, which is a different fact and a quieter one: a build made
 * for whoever is writing it, rather than the network the node in front of them is actually on. The
 * one worth interrupting somebody over is the network — it is what a mistake gets written into —
 * so that is what it says.
 *
 * It is filled rather than outlined, which nothing else on the strip is, and it wears a colour no
 * state has. Being on development is not one of the four states a node is in, and borrowing one of
 * those would cost that colour its meaning.
 */

import { useTranslation } from "react-i18next";

import Logo from "@/components/Logo";
import StateBadge, { type StateTone } from "@/components/StateBadge";
import { Badge } from "@/components/ui/badge";
import type { NodeState, NodeStateWord } from "@/lib/network";

/** What this build is. Vite replaces it at build time from `package.json`. */
const VERSION = __APP_VERSION__;

/**
 * Which of the four colours each state is drawn in.
 *
 * *Stopped* is idle and not bad: a node nobody has started is not a node that went wrong, and
 * spending the bad colour on it would leave nothing louder for the node that did. *Starting* is
 * the warning colour because it is the state that should not last.
 */
const TONE: Record<NodeStateWord, StateTone> = {
  stopped: "idle",
  starting: "warn",
  running: "ok",
  failing: "bad",
};

/** What the strip is drawn from. */
interface StatusBarProps {
  /** What the node is doing, or `null` before the first look has come back. */
  state: NodeState | null;
}

/** The strip along the bottom of the window. */
function StatusBar({ state }: StatusBarProps) {
  const { t } = useTranslation();

  return (
    <footer className="status" aria-label={t("status.label")}>
      {/* **What gives way is the one item that can afford to.** Everything on this strip carried
          `truncate` and could shrink, so a width where the strip was two words over — 620 points
          in Spanish, just above the shape's own breakpoint — took those two words off four items
          at once: the product's name became *Alme…*, the version *versión 0.0…* and the licence
          *Apache …*. A truncated wordmark says less than no wordmark, and a truncated version is
          a different version. So each of those holds its width, and the network's word is the
          only thing here allowed to end in an ellipsis — it is the longest item on the strip and
          the one whose full text is two presses away on the Network screen. */}
      <div className="flex shrink-0 items-center gap-2">
        <Logo size={12} />
        <span>{t("app.name")}</span>
      </div>

      {/* What the node is doing. Nothing at all until the first look has come back: a strip
          saying *stopped* over a node that is starting would be an assertion nobody made.

          **The state is the only one of the three drawn in the compact shape.** Below 600 points
          there is not room for all of them without every item on the strip becoming an ellipsis,
          and the rule the strip is written under is that what does not fit does not go on it. The
          badge is what stays, because *is anything wrong* is the question the strip answers; the
          network and the peer count are on the Network screen, which is two presses away. */}
      {state !== null && (
        <div className="flex min-w-0 items-center gap-3">
          <span className="shrink-0">
            <StateBadge tone={TONE[state.state]} label={t(`status.node.${state.state}`)} />
          </span>
          {/* A dash, and never a nought: a peer count of nought is a measurement, and until this
              node has a place on the mesh nobody has taken one. The dash is the same one `Figure`
              draws, and it says so in words to whoever is not looking at it. */}
          <span className="hidden shrink-0 expanded:inline">
            {t("status.peers", { peers: state.peers ?? "—" })}
            {state.peers === null && (
              <span className="sr-only"> {t("control.unmeasured")}</span>
            )}
          </span>
        </div>
      )}

      <div className="ml-auto flex shrink-0 items-center gap-3">
        {/* Drawn in both shapes, unlike everything else that can come off this strip. What it
            says is *this is not production*, and a width narrow enough to hide that is not a
            reason for somebody to stop being told. */}
        {state?.which === "development" && (
          <Badge className="bg-development text-development-foreground">
            {t("status.development")}
          </Badge>
        )}

        <span className="font-mono">{t("app.version", { version: VERSION })}</span>
        {/* The licence is what comes off the strip first in the compact shape: of everything on
            it, it is the one thing nobody is ever reading to find something out. It came off when
            the node's state went on — a strip where every item is an ellipsis says less than one
            with fewer items on it. */}
        <span className="hidden expanded:inline">{t("status.licence")}</span>
      </div>
    </footer>
  );
}

export default StatusBar;
