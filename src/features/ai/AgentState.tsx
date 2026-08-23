/**
 * Where the agent is, as a colour and as a word.
 *
 * Five states over four colours, and the pairing is deliberate: *not in this build* and *not
 * started* are two different facts with two different sentences, and both of them are the same
 * thing to look at — nothing is happening and nothing is wrong. The colour says whether to
 * worry; the word says what is true (`.agents/rules/visual-identity.md`).
 *
 * Nothing is drawn before the first look has come back. That is a badge arriving rather than a
 * screen claiming the agent is idle, which is the difference between a gap and an assertion.
 */

import { useTranslation } from "react-i18next";

import StateBadge, { type StateTone } from "@/components/StateBadge";
import type { AgentState as Where } from "@/lib/agent";

/** Which of the four colours each state is drawn in. */
const TONE: Record<Where, StateTone> = {
  notBundled: "idle",
  notStarted: "idle",
  running: "ok",
  willNotStart: "bad",
  stopped: "warn",
};

/** What the badge is drawn from. */
interface AgentStateProps {
  /** Where the agent is, or `null` before anybody has looked. */
  state: Where | null;
}

/** The agent's state, or nothing at all until it is known. */
function AgentState({ state }: AgentStateProps) {
  const { t } = useTranslation();

  if (state === null) {
    return null;
  }

  return <StateBadge tone={TONE[state]} label={t(`ai.state.${state}`)} />;
}

export default AgentState;
