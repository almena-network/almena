/**
 * The AI screen: the agent's state, the conversation, and the box to add to it.
 *
 * The agent is a *second program this application runs*: it is started beside the window,
 * spoken to over a pipe and ended with it. That is why this screen belongs to the windowed
 * application and to nothing else the project builds — the CLI brings a node up on a machine
 * in a rack, and an agent is something a person sits in front of.
 *
 * # Two screens
 *
 * The conversation, and the agent itself. They answer two questions — *what did it say* and
 * *what is running* — and the second was unanswerable from the interface until now: the agent's
 * own version and the model in force have crossed on `agentStatus` since it was written and
 * nothing drew either.
 *
 * `useAgent` is called **here**, once, and what it holds is passed down. Calling it inside each
 * screen would be two conversations rather than one, and moving between the screens would
 * throw away whichever one was not showing.
 *
 * # Four nothings, and each says which one it is
 *
 * There is no agent in this build; there is one and nobody has asked it anything; there is one
 * and it will not start; there was one and it stopped. A screen that answered all four with
 * *nothing to show* would be telling a reader the wrong one three times out of four, so each
 * has an icon, a title and a reason of its own.
 */

import { CircleSlash, MessageSquare, PowerOff, TriangleAlert } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import EmptyState from "@/components/EmptyState";
import ScreenNav from "@/components/ScreenNav";
import {
  Card,
  CardContent,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import AgentFacts from "@/features/ai/AgentFacts";
import AgentState from "@/features/ai/AgentState";
import Composer from "@/features/ai/Composer";
import Conversation from "@/features/ai/Conversation";
import Failure from "@/features/ai/Failure";
import { screensOf, type ScreensOf } from "@/features/shell/sections";
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
  stopped: {
    Mark: PowerOff,
    title: "ai.empty.stoppedTitle",
    reason: "ai.empty.stopped",
  },
} as const;

/** Which of the four emptinesses a state is, or `null` where the agent is simply running. */
function nothingYet(state: Where): keyof typeof NOTHING | null {
  return state === "running" ? null : state;
}

/** One of this section's screens. */
type Screen = ScreensOf<"ai">;

/** What the section opens on, every time it is opened. */
const FIRST: Screen = "conversation";

/** The AI section. */
function Ai() {
  const [screen, setScreen] = useState<Screen>(FIRST);
  const screens = screensOf("ai") ?? [];
  const { t } = useTranslation();
  const { status, turns, saying, stage, running, adopted, failure, ask, stop } = useAgent();

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

  const conversation = (
    <Card>
      <CardHeader>
        <CardTitle>{t("ai.heading")}</CardTitle>
      </CardHeader>

      <CardContent className="flex flex-col gap-4">
        <div className="flex items-center gap-2">
          <span className="text-muted-foreground text-xs">{t("ai.state.label")}</span>
          <AgentState state={where} />
        </div>

        {failure !== null && <Failure code={failure} />}

        {nothing === null ? (
          <Conversation
            turns={turns}
            saying={saying}
            stage={stage}
            running={running}
            adopted={adopted}
          />
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
  );

  // Total over `Screen`: naming a screen in `sections.ts` and forgetting it here fails `tsc`.
  // Elements rather than components, so that moving between screens reconciles what is drawn
  // instead of remounting it and throwing away what the composer was holding.
  const shown: Record<Screen, React.ReactNode> = {
    conversation,
    agent: <AgentFacts status={status} />,
  };

  return (
    <div className="screen">
      <ScreenNav section="ai" screens={screens} current={screen} onSelect={setScreen} />

      {shown[screen]}
    </div>
  );
}

export default Ai;
