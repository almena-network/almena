/**
 * The control that sends a notification, and what the device did with the request.
 *
 * The outcome is said on screen rather than left to the notification itself to be the
 * evidence, because two of the three outcomes draw nothing at all: a person who pressed a
 * button and saw nothing happen cannot tell a refusal from a failure, and would be left
 * guessing which. An empty answer is an answer — `AGENTS.md`, Transparency.
 *
 * It is shadcn/ui's `Alert`, which carries `role="alert"` and is therefore read out the moment
 * it appears — so it is absent rather than empty until there is an outcome. Two of the three
 * outcomes are failures and wear the danger tone; the third is not, and does not.
 *
 * The button is the identity colour, which is the second of the two things that colour means:
 * it is the thing this card is for. Nothing else on the screen wears it.
 */

import { Bell, BellOff, BellRing, CircleAlert } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { notify, type NotifyOutcome } from "@/lib/notifications";

/**
 * How each outcome is reported: its sentence in the catalogs, the icon beside it, and whether
 * it is a failure.
 *
 * A lookup rather than a key built from the outcome, so that `tsc` checks all three against
 * the English catalog the way it checks every other key.
 */
const OUTCOME = {
  sent: { key: "home.notify.outcome.sent", Icon: BellRing, failed: false },
  refused: { key: "home.notify.outcome.refused", Icon: BellOff, failed: true },
  failed: { key: "home.notify.outcome.failed", Icon: CircleAlert, failed: true },
} as const;

/** The button that sends a notification, with the last outcome under it. */
function NotifyButton() {
  const { t } = useTranslation();
  const [outcome, setOutcome] = useState<NotifyOutcome | null>(null);

  async function send() {
    setOutcome(await notify(t("app.name"), t("home.notify.message")));
  }

  const said = outcome === null ? null : OUTCOME[outcome];

  return (
    <div className="flex flex-col items-start gap-3">
      <Button
        onClick={() => {
          void send();
        }}
      >
        <Bell aria-hidden="true" />
        {t("home.notify.action")}
      </Button>

      {said !== null && (
        <Alert variant={said.failed ? "destructive" : "default"}>
          <said.Icon aria-hidden="true" />
          <AlertDescription>{t(said.key)}</AlertDescription>
        </Alert>
      )}
    </div>
  );
}

export default NotifyButton;
