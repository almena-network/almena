/**
 * What the agent is, as against what it is saying.
 *
 * The conversation screen answers *what did it reply*; this one answers *what is running*. Two
 * questions, two screens, and the second was until now unanswerable from the interface at all:
 * `agentStatus` has carried the agent's own version and the model in force since the day it was
 * written, and nothing drew either of them.
 *
 * Every figure is measured from the agent that is **running**, never from what was chosen. The
 * two differ from the moment somebody picks a model until the agent next starts, and a screen
 * showing one for the other would be answering a question nobody asked. Where no agent is
 * running there is nothing to measure, the value is `null`, and `Figure` draws a dash — never a
 * zero and never an empty string (`.agents/rules/honest-emptiness.md`).
 */

import { useTranslation } from "react-i18next";

import Figure from "@/components/Figure";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import AgentState from "@/features/ai/AgentState";
import type { AgentStatus } from "@/lib/agent";

/** What this screen is drawn from. */
interface AgentFactsProps {
  /** Where the agent is, or `null` before the first look has come back. */
  status: AgentStatus | null;
}

/** The agent itself: where it is, what it calls itself, and what it is asking. */
function AgentFacts({ status }: AgentFactsProps) {
  const { t } = useTranslation();

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("ai.agent.heading")}</CardTitle>
        <CardDescription>{t("ai.agent.body")}</CardDescription>
      </CardHeader>

      <CardContent className="flex flex-col gap-4">
        <div className="flex items-center gap-2">
          <span className="text-muted-foreground text-xs">{t("ai.state.label")}</span>
          <AgentState state={status?.state ?? null} />
        </div>

        <div className="grid grid-cols-[repeat(auto-fit,minmax(min(100%,var(--width-figure-min)),1fr))] gap-4">
          <Figure label={t("ai.agent.figure.version")} value={status?.agentVersion ?? null} />
          <Figure label={t("ai.agent.figure.model")} value={status?.model ?? null} />
        </div>
      </CardContent>
    </Card>
  );
}

export default AgentFacts;
