/**
 * Every turn so far, and the one the agent is part-way through.
 *
 * The answer in progress is drawn as a turn like any other rather than as something special:
 * it is the same element in the same place, so nothing moves when it finishes. What tells a
 * reader it is still going is the stage beside it, which is a different fact and gets its own
 * line.
 *
 * The three emptinesses of this screen are decided above it, in `Ai`, because which of them is
 * true is a fact about the agent rather than about the conversation.
 *
 * A run this page adopted is drawn as its own line rather than as an ordinary one that has said
 * nothing yet. It will never say anything here — its tokens are going to a channel that went
 * with the page that started it — and letting the spinner imply otherwise would be showing a
 * reader an answer on its way that is not on its way.
 */

import { useTranslation } from "react-i18next";

import { Spinner } from "@/components/ui/spinner";
import Turn from "@/features/ai/Turn";
import type { Said } from "@/hooks/useAgent";
import type { Stage } from "@/lib/agent";

/** What the conversation is drawn from. */
interface ConversationProps {
  /** Every finished turn, oldest first. */
  turns: Said[];
  /** What the run in flight has said so far, empty while it has said nothing. */
  saying: string;
  /** Which stage the run has reached, or `null` while none has been reported. */
  stage: Stage | null;
  /** Whether a run is in flight. */
  running: boolean;
  /** Whether that run was started by a page that has since gone. */
  adopted: boolean;
}

/** The conversation so far. */
function Conversation({ turns, saying, stage, running, adopted }: ConversationProps) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-3">
      {turns.map((said, at) => (
        // The index is the key because a conversation is only ever appended to: no turn is
        // reordered, removed or edited, so nothing can be pointed at the wrong one.
        // eslint-disable-next-line react/no-array-index-key
        <Turn key={at} said={said} />
      ))}

      {running && saying !== "" && (
        <Turn said={{ role: "agent", content: saying }} />
      )}

      {running && (
        <div className="text-muted-foreground flex items-center gap-2 px-1 text-xs">
          <Spinner className="size-3" aria-label={t("ai.conversation.working")} />
          {/* Three different things are true here and each gets its own sentence: a run
              nobody can hear, a run working with no stage reported, and a run at a stage. */}
          {adopted
            ? t("ai.conversation.adopted")
            : stage === null
              ? t("ai.conversation.working")
              : t(`ai.stage.${stage}`)}
        </div>
      )}
    </div>
  );
}

export default Conversation;
