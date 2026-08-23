/**
 * The AI screen: the agent's state, the conversation, and the box to add to it.
 *
 * **Desktop only**, and the argument is not the one about the node — that one has already
 * explained something and `.agents/rules/supported-platforms.md` forbids leaning on it twice.
 * This section is a *second program the application runs*: a computer's operating system lets
 * a process start another, hand it two pipes and end it, and a phone's does not — iOS gives a
 * sandboxed application no way to run a second program at all, and Android will not execute a
 * binary out of an application's own directory. There is also no model server beside it for
 * the agent to reach. The platform does not have the thing this screen is about, so it is not
 * listed there rather than listed and empty.
 *
 * # Four nothings, and each says which one it is
 *
 * There is no agent in this build; there is one and nobody has asked it anything; there is one
 * and it will not start; there was one and it stopped. A screen that answered all four with
 * *nothing to show* would be telling a reader the wrong one three times out of four
 * (`.agents/rules/honest-emptiness.md`), so each has an icon, a title and a reason of its own.
 */

import { CircleSlash, MessageSquare, PowerOff, TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";

import EmptyState from "@/components/EmptyState";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import AgentState from "@/features/ai/AgentState";
import Composer from "@/features/ai/Composer";
import Conversation from "@/features/ai/Conversation";
import { useAgent } from "@/hooks/useAgent";
import type { AgentState as Where } from "@/lib/agent";

/**
 * The four nothings this screen can be showing, and what each is drawn from.
 *
 * Written out rather than assembled from the state's name. The keys are decisions — which
 * sentence a person reads, and which mark they see before reading it — and a key built by
 * concatenation is one `tsc` cannot check against the catalogs, which is the whole reason the
 * catalogs are typed.
 */
const NOTHING = {
  notBundled: {
    Mark: CircleSlash,
    title: "ai.empty.notBundledTitle",
    reason: "ai.empty.notBundled",
  },
  notStarted: {
    Mark: MessageSquare,
    title: "ai.empty.notStartedTitle",
    reason: "ai.empty.notStarted",
  },
  willNotStart: {
    Mark: TriangleAlert,
    title: "ai.empty.willNotStartTitle",
    reason: "ai.empty.willNotStart",
  },
  stopped: { Mark: PowerOff, title: "ai.empty.stoppedTitle", reason: "ai.empty.stopped" },
} as const;

/** Which of the four emptinesses a state is, or `null` where the agent is simply running. */
function nothingYet(state: Where): keyof typeof NOTHING | null {
  return state === "running" ? null : state;
}

/**
 * Every failure this interface has words for.
 *
 * Written out rather than derived, because it is exactly the list the catalogs carry — and the
 * agent's own list is longer and is allowed to grow without this build being rebuilt. What
 * arrives from outside it is drawn with the general sentence below.
 */
const SAYS = [
  "agent_will_not_start",
  "agent_stopped",
  "run_already_in_flight",
  "model_unreachable",
  "model_unknown",
  "resource_unknown",
] as const;

/**
 * The catalog key for one failure, or the general one where this build has never heard of it.
 *
 * The narrowing is here and it is deliberate: an identifier is not text, and an application
 * that drew one because it had nothing better would be putting a subprocess's vocabulary in
 * front of a person (`.agents/rules/user-facing-text.md`). The code itself is already in the
 * records — the Rust side writes it the moment the failure arrives — so nothing is lost by
 * keeping it off the screen.
 */
function reasonFor(code: string) {
  const known = SAYS.find((said) => said === code);
  return known === undefined ? ("ai.error.unknown" as const) : (`ai.error.${known}` as const);
}

/** The AI screen. */
function Ai() {
  const { t } = useTranslation();
  const { status, turns, saying, stage, running, failure, ask, stop } = useAgent();

  // Before the first look nothing is known, and nothing is claimed: the conversation is drawn
  // empty and the composer waits. `notStarted` is what a person meets on an ordinary opening,
  // and it is the one emptiness that is not a problem.
  const where = status?.state ?? null;
  const ready = where !== "notBundled" && where !== "willNotStart";

  // Which nothing this screen is showing, or `null` where there is a conversation to draw.
  // A conversation outlives the run that produced it: an agent that has stopped still has
  // everything it said, and replacing that with *the agent stopped* would take an answer off
  // the screen to report something the badge already says.
  const state = where === null ? "notStarted" : nothingYet(where);
  const nothing = turns.length > 0 || state === null ? null : NOTHING[state];

  return (
    <div className="screen">
      <h1 className="screen__title">{t("section.ai")}</h1>

      <Card>
        <CardHeader>
          <CardTitle>{t("ai.heading")}</CardTitle>
          <CardDescription>{t("ai.body")}</CardDescription>
        </CardHeader>

        <CardContent className="flex flex-col gap-4">
          <div className="flex items-center gap-2">
            <span className="text-muted-foreground text-xs">{t("ai.state.label")}</span>
            <AgentState state={where} />
          </div>

          {failure !== null && (
            <Alert variant="destructive">
              <AlertTitle>{t("ai.error.heading")}</AlertTitle>
              {/* A code this build has no words for is drawn with the general sentence, and
                  the identifier itself goes to the records rather than onto the screen —
                  `.agents/rules/user-facing-text.md`. */}
              <AlertDescription>{t(reasonFor(failure))}</AlertDescription>
            </Alert>
          )}

          {nothing === null ? (
            <Conversation turns={turns} saying={saying} stage={stage} running={running} />
          ) : (
            <EmptyState
              icon={<nothing.Mark aria-hidden="true" />}
              title={t(nothing.title)}
              reason={t(nothing.reason)}
            />
          )}
        </CardContent>

        <CardFooter>
          <Composer running={running} ready={ready} onAsk={ask} onStop={stop} />
        </CardFooter>
      </Card>
    </div>
  );
}

export default Ai;
