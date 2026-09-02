/**
 * A value, and the control that takes it out of the application.
 *
 * Nothing in this interface can be selected — `base.css` says so as an element rule, and gives
 * the reason: an application is not a document, and a drag across a sixty-two character
 * identifier gets whatever the gesture happened to cover, which is worth nothing. So this is
 * the way anything leaves the window, and it is better than the selection it replaces rather
 * than a compromise for it: **what goes on the clipboard is the whole value**, never what was
 * drawn. A row showing an address wrapped over three lines hands this one string.
 *
 * # It is a button, and it is always there
 *
 * Never a thing that appears under a pointer. One of the three platforms is a laptop with a
 * touch screen, where there is no pointer to appear under, and a control that is invisible
 * until hovered is a control half the machines this runs on do not have.
 *
 * It is 44 points, which is what a finger is entitled to and what every other thing here that
 * is pressed already spends. The ghost tone is what keeps that from being loud: until it is
 * hovered or focused there is nothing there but the mark.
 *
 * # It says what it copies, and it says when it did not
 *
 * The name is `Copy <what>` and never `Copy`: a screen reader working down the Network screen
 * would otherwise hear the same word seven times and be told nothing by any of them. The
 * caller passes the name the screen already gives the value, so the two agree by construction.
 *
 * What happened is said in one line that is in the document from the start — a region that is
 * empty until there is something to report is the only kind anything announces. It is read out
 * either way and drawn only for a refusal: a check mark is enough for an eye that just pressed
 * the button, where a refusal is a thing a person has to be told in words. A control that
 * looks like it worked and did not is worse than one that says it will not.
 *
 * `navigator.clipboard.writeText`, and no plugin. It is what the webview and a browser both
 * have, and a dependency taken on for a single call is a dependency this project refuses.
 *
 * # What is copied is left on the clipboard
 *
 * There is no timer taking it off again, and that is a fact about the values rather than an
 * omission. Everything this draws is public: a network's name, a node's identifier, the root
 * over the record, the mesh name, the addresses the node publishes, and the challenge — which
 * is drawn as a code on the same screen for anybody in the room to scan, so it is not a thing
 * the clipboard is keeping. Nothing a person would be harmed by leaving there is drawn here,
 * and a control that quietly emptied a clipboard would be taking away something they may have
 * meant to paste an hour later. The day this draws a secret is the day it needs the other
 * behaviour, and that is a decision to take then.
 */

import { Check, Copy } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/cn";

/** How long the mark says it worked before offering to do it again, in milliseconds. */
const CONFIRMED_FOR = 2000;

/**
 * What came of the last press.
 *
 * A refusal outlives the confirmation on purpose: the check mark goes away by itself because
 * the person saw what they asked for happen, and a refusal stays until they press again,
 * because nothing happened and there is nothing else on screen that would say so.
 */
type Outcome = "nothing" | "copied" | "refused";

/** What a copyable value is made of. */
interface CopyableProps {
  /** The whole value, exactly as it goes on the clipboard. */
  value: string;
  /**
   * What the value is, already translated, as the screen beside it already names it.
   *
   * It is the button's name — *Copy Root*, *Copy Mesh name* — so it is a noun and not a
   * sentence, and it is the caller's own label rather than a second one written here.
   */
  what: string;
  /** How the value itself is drawn: a figure is a size larger than a row of a list. */
  className?: string;
  /**
   * What to draw in place of the value, where drawing it as one line of text is wrong.
   *
   * **The clipboard still gets `value`, whole.** It is for the one thing that is not a name: a
   * block of several lines, which as a run of `break-all` text is unreadable and as a `<pre>` is
   * exactly what somebody is about to paste. What must not happen is the caller drawing the block
   * *and* this drawing it again, which is the same value on screen twice.
   */
  children?: ReactNode;
}

/** A value, and the button that copies the whole of it. */
function Copyable({ value, what, className, children }: CopyableProps) {
  const { t } = useTranslation();
  const [outcome, setOutcome] = useState<Outcome>("nothing");

  // The one thing here that outlives a render: the timer that takes the check mark away. An
  // effect rather than a `setTimeout` in the handler, so that leaving the screen cancels it.
  useEffect(() => {
    if (outcome !== "copied") {
      return;
    }

    const timer = setTimeout(() => {
      setOutcome("nothing");
    }, CONFIRMED_FOR);

    return () => {
      clearTimeout(timer);
    };
  }, [outcome]);

  const copy = () => {
    void navigator.clipboard
      .writeText(value)
      .then(() => {
        setOutcome("copied");
      })
      .catch(() => {
        setOutcome("refused");
      });
  };

  return (
    <div className="flex min-w-0 flex-col gap-1">
      <div className="flex min-w-0 items-start gap-1">
        {/* `min-w-0` and `break-all` together are what let a long value shrink inside whatever
            it was put in: without the first the box refuses to be narrower than its content,
            and the row it is in pushes past the card and gives the window a horizontal
            scrollbar, which is the one thing no screen here has.

            The padding is what puts the mark on the same line as the words. The button is 44
            points because a finger is entitled to that, and a line of text is 22 — so without
            it the mark sits half a line below the value it belongs to, on every row of every
            screen. Ten points above and below the first line closes that, and the row stays
            top-aligned so that a value wrapped over four lines keeps its button beside the
            first of them rather than floating in the middle of the block. */}
        {children ?? (
          <span className={cn("min-w-0 py-2.5 font-mono break-all", className)}>{value}</span>
        )}

        <Button
          variant="ghost"
          size="icon"
          className="size-11"
          aria-label={t("copy.action", { what })}
          onClick={copy}
        >
          {outcome === "copied" ? (
            <Check aria-hidden="true" />
          ) : (
            <Copy aria-hidden="true" />
          )}
        </Button>
      </div>

      <p
        role="status"
        className={outcome === "refused" ? "note text-bad" : "sr-only"}
      >
        {outcome === "copied" && t("copy.copied", { what })}
        {outcome === "refused" && t("copy.refused")}
      </p>
    </div>
  );
}

export default Copyable;
