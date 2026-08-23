/**
 * What a person types, and the one control that sends it or stops it.
 *
 * One button, two jobs, and that is the point rather than a saving: while a run is in flight
 * the thing a person wants is to stop it, and a screen that offered *ask* beside *stop* would
 * be offering one control that does nothing. It wears the identity colour because it is the
 * thing this screen is for (`.agents/rules/visual-identity.md`).
 *
 * Enter sends and shift-enter makes a line, which is what everybody expects of a box like
 * this — and both are only ever an accelerator: the button does the same thing and is
 * reachable by touch, which is what `.agents/rules/supported-platforms.md` asks for.
 *
 * This is the first field in the application, and therefore the one place text can be selected
 * — the exception in `.agents/rules/no-text-selection.md`, which is already written as an
 * element rule and needs nothing here.
 */

import { useId, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import { Textarea } from "@/components/ui/textarea";

/** What the composer is drawn from. */
interface ComposerProps {
  /** Whether a run is in flight, which is what the button offers to stop. */
  running: boolean;
  /** Whether there is an agent to ask at all. */
  ready: boolean;
  /** Called with what was typed. */
  onAsk: (asked: string) => void;
  /** Called when the run in flight should stop. */
  onStop: () => void;
}

/** The box and the button. */
function Composer({ running, ready, onAsk, onStop }: ComposerProps) {
  const { t } = useTranslation();
  const [asked, setAsked] = useState("");
  const id = useId();

  const send = () => {
    if (asked.trim() === "") {
      return;
    }
    onAsk(asked);
    setAsked("");
  };

  return (
    <Field orientation="vertical">
      {/* The label is what a screen reader says and is not drawn: the placeholder says the
          same thing to an eye, and two of them one above the other would be one too many. */}
      <FieldLabel htmlFor={id} className="sr-only">
        {t("ai.composer.label")}
      </FieldLabel>

      <Textarea
        id={id}
        value={asked}
        rows={2}
        disabled={!ready}
        placeholder={t("ai.composer.placeholder")}
        onChange={(typed) => {
          setAsked(typed.target.value);
        }}
        onKeyDown={(pressed) => {
          if (pressed.key === "Enter" && !pressed.shiftKey && !running) {
            pressed.preventDefault();
            send();
          }
        }}
      />

      <div className="flex justify-end">
        {running ? (
          <Button variant="outline" onClick={onStop}>
            {t("ai.composer.stop")}
          </Button>
        ) : (
          <Button onClick={send} disabled={!ready || asked.trim() === ""}>
            {t("ai.composer.send")}
          </Button>
        )}
      </div>
    </Field>
  );
}

export default Composer;
