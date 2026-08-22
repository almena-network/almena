/**
 * The control that sends a notification, and what the device did with the request.
 *
 * The outcome is said on screen rather than left to the notification itself to be the
 * evidence, because two of the three outcomes draw nothing at all: a person who pressed a
 * button and saw nothing happen cannot tell a refusal from a failure, and would be left
 * guessing which. An empty answer is an answer — `AGENTS.md`, Transparency.
 */

import { useState } from "react";
import { useTranslation } from "react-i18next";

import { notify, type NotifyOutcome } from "@/lib/notifications";

/**
 * What each outcome is called in the catalogs.
 *
 * A lookup rather than a key built from the outcome, so that `tsc` checks all three against
 * the English catalog the way it checks every other key.
 */
const OUTCOME_KEY = {
  sent: "home.notify.outcome.sent",
  refused: "home.notify.outcome.refused",
  failed: "home.notify.outcome.failed",
} as const;

/** The button that sends a notification, with the last outcome under it. */
function NotifyButton() {
  const { t } = useTranslation();
  const [outcome, setOutcome] = useState<NotifyOutcome | null>(null);

  async function send() {
    setOutcome(await notify(t("app.name"), t("home.notify.message")));
  }

  return (
    <div className="home__notify">
      <button
        type="button"
        className="home__notify-action"
        onClick={() => {
          void send();
        }}
      >
        {t("home.notify.action")}
      </button>

      {/* Always in the document, empty until there is something to say: a region added to the
          page at the same moment its text appears is one a screen reader may never announce. */}
      <p className="home__notify-outcome" role="status">
        {outcome === null ? "" : t(OUTCOME_KEY[outcome])}
      </p>
    </div>
  );
}

export default NotifyButton;
