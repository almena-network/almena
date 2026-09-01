/**
 * The card that decides which model the agent is asked for.
 *
 * **Almena runs no model and does not go looking for one.** It asks whatever is serving on
 * this computer, at the address the agent was built with, for the name chosen here. So this
 * list is what Almena knows how to *ask for* — not what this computer can answer — and the
 * card says so in as many words rather than leaving somebody to find out by choosing one that
 * is not there. There is no discovery behind it, and pretending otherwise would be the screen
 * claiming a reading nobody took.
 *
 * # Chosen, and in force
 *
 * Two different facts, and the card draws both. The model a run uses is fixed when the agent
 * starts, so from the moment somebody changes this until the agent next starts they disagree —
 * and a card that showed only the choice would be answering a question nobody asked. What is
 * running comes from the agent itself, in the first thing it says.
 *
 * The control that closes that gap is here for the same reason a refresh button is on the
 * Network screen: *this applies next time* is a sentence a person can do nothing with, and a
 * screen that says it should offer the next time.
 */

import { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Field, FieldContent, FieldDescription, FieldLabel } from "@/components/ui/field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { agentStatus, stopAgent, type AgentStatus } from "@/lib/agent";
import { MODELS, isModel } from "@/lib/models";
import { choose, preferences } from "@/lib/preferences";

/** What nobody having chosen is worth in the control, which cannot hold an empty value. */
const NOTHING_CHOSEN = "default";

/** The model the agent is asked for, and what is running now. */
function Model() {
  const { t } = useTranslation();
  const id = useId();

  const stored = preferences().model;
  const [chosen, setChosen] = useState(isModel(stored) ? stored : NOTHING_CHOSEN);
  const [status, setStatus] = useState<AgentStatus | null>(null);

  const look = () => {
    void agentStatus().then(setStatus);
  };

  useEffect(look, []);

  const running = status?.model ?? null;
  // Only worth offering while there is something a restart would change. A control that did
  // nothing observable is one nobody can tell from a broken one.
  const stale = status?.state === "running" && running !== (chosen === NOTHING_CHOSEN ? running : chosen);

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.model.heading")}</CardTitle>
      </CardHeader>

      <CardContent className="flex flex-col gap-4">
        <Field orientation="horizontal">
          <FieldContent className="min-h-11 justify-center">
            <FieldLabel htmlFor={id}>{t("settings.model.label")}</FieldLabel>
          </FieldContent>

          <Select
            value={chosen}
            onValueChange={(next) => {
              setChosen(next);
              void choose({ model: next === NOTHING_CHOSEN ? null : next });
            }}
          >
            <SelectTrigger id={id}>
              <SelectValue />
            </SelectTrigger>

            <SelectContent>
              {/* Nobody having chosen is an answer with its own words, and it is the one this
                  card opens on: the agent has a default of its own, and this side deliberately
                  does not know what it is. */}
              <SelectItem value={NOTHING_CHOSEN}>{t("settings.model.notChosen")}</SelectItem>
              {MODELS.map((name) => (
                <SelectItem key={name} value={name}>
                  {name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>

        <FieldDescription>
          {running === null
            ? t("settings.model.inForceNone")
            : t("settings.model.inForce", { model: running })}
        </FieldDescription>

        {stale && (
          <div className="flex flex-col gap-2">
            <FieldDescription>{t("settings.model.restartHeading")}</FieldDescription>
            <div>
              <Button
                variant="outline"
                onClick={() => {
                  void stopAgent().then(look);
                }}
              >
                {t("settings.model.restart")}
              </Button>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export default Model;
