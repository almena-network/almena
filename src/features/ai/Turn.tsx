/**
 * One turn of the conversation: who said it, and what they said.
 *
 * A row of a list, so it is the registry's `Item` and not markup written here. What a person
 * typed and what the agent answered are the same element in the same tone: two shapes would be
 * two visual languages on one screen, and the name above each turn already says which is which.
 *
 * A turn that was cut short says so, in words, beside the name. A run that was stopped
 * part-way keeps what it managed to say — `cancelled` means nothing further is coming, not
 * that what arrived is withdrawn — and a reader has to be able to tell an answer that finished
 * from one that did not.
 */

import { useTranslation } from "react-i18next";

import { Item, ItemContent, ItemDescription, ItemTitle } from "@/components/ui/item";
import type { Said } from "@/hooks/useAgent";

/** What one turn is drawn from. */
interface TurnProps {
  /** The turn. */
  said: Said;
}

/** One turn of the conversation. */
function Turn({ said }: TurnProps) {
  const { t } = useTranslation();

  return (
    <Item variant="muted" size="sm" className="items-start">
      <ItemContent>
        <ItemTitle>
          {t(said.role === "person" ? "ai.conversation.person" : "ai.conversation.agent")}
          {said.cut === true && (
            <span className="text-muted-foreground font-normal">
              {" · "}
              {t("ai.state.stopped")}
            </span>
          )}
        </ItemTitle>

        {/* The one place in this application that draws prose somebody else wrote. It is
            whitespace-preserving because an answer arrives with its own line breaks in it,
            and it is never markup: what a model wrote is text.

            And it breaks inside a word, which those two do not do by themselves: an answer
            with a long address in it is an answer with one word in it that no line may be
            broken at, and a hundred and twenty characters of it took the whole screen
            sideways — 991 points of text in a 300-point column, measured. Nothing here
            chooses what an answer contains, so nothing here may assume it has spaces. */}
        <ItemDescription className="whitespace-pre-wrap break-words">
          {said.content}
        </ItemDescription>
      </ItemContent>
    </Item>
  );
}

export default Turn;
