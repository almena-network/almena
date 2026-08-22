/**
 * The switch that decides whether the system starts Almena when somebody logs in.
 *
 * It draws what the system says, never what was asked for: every move re-reads the setting and
 * shows what came back. A switch that slides across while nothing changed behind it is the one
 * failure worth spending a whole component on avoiding.
 */

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { autostartEnabled, setAutostart } from "@/lib/autostart";

/** The switch, its name, and what the system said the last time it was asked. */
function Autostart() {
  const { t } = useTranslation();
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [refused, setRefused] = useState(false);

  useEffect(() => {
    void autostartEnabled()
      .then(setEnabled)
      .catch(() => {
        setRefused(true);
      });
  }, []);

  async function toggle() {
    if (enabled === null) {
      return;
    }

    try {
      setEnabled(await setAutostart(!enabled));
      setRefused(false);
    } catch {
      setRefused(true);
    }
  }

  return (
    <section className="panel settings__card">
      <h2 className="settings__title">{t("settings.autostart.heading")}</h2>
      <p className="settings__body">{t("settings.autostart.body")}</p>

      <button
        type="button"
        // A switch rather than a button that toggles: a screen reader says what state it is
        // in, so the knob's position is never the only thing carrying that.
        role="switch"
        aria-checked={enabled ?? false}
        className="settings__switch"
        disabled={enabled === null}
        onClick={() => {
          void toggle();
        }}
      >
        <span className="settings__switch-track" aria-hidden="true">
          <span className="settings__switch-knob" />
        </span>
        <span>{t("settings.autostart.label")}</span>
        <span className="settings__switch-state">
          {enabled ? t("settings.autostart.on") : t("settings.autostart.off")}
        </span>
      </button>

      {/* In the document from the start, empty until there is something to say — a region that
          appears at the same moment as its text is one a screen reader may never announce. */}
      <p className="settings__note" role="status">
        {refused ? t("settings.autostart.refused") : ""}
      </p>
    </section>
  );
}

export default Autostart;
