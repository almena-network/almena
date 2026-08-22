/**
 * The Settings screen: what a person can change about this application.
 *
 * There is one thing to change today and it belongs to a computer, so on a phone this screen
 * says that rather than drawing an empty page. It is the same choice `NotBuilt` makes for a
 * section with no screen: an empty answer is an answer, and a page that draws nothing at all
 * reads as broken rather than as unfinished.
 *
 * Nothing is drawn while the answer is still being fetched. That moment is short and a control
 * that appears and then vanishes is worse than one that arrives a beat late.
 */

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import Autostart from "@/features/settings/Autostart";
import { isDesktop } from "@/lib/platform";
import "@/features/settings/settings.css";

/** The Settings screen. */
function Settings() {
  const { t } = useTranslation();
  const [desktop, setDesktop] = useState<boolean | null>(null);

  useEffect(() => {
    void isDesktop().then(setDesktop);
  }, []);

  return (
    <div className="screen">
      <h1 className="screen__title">{t("section.settings")}</h1>

      {desktop === true && (
        <div className="settings__cards">
          <Autostart />
        </div>
      )}

      {desktop === false && (
        <section className="panel settings__card">
          <p className="settings__body">{t("settings.nothingHere")}</p>
        </section>
      )}
    </div>
  );
}

export default Settings;
