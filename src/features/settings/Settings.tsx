/**
 * The Settings screen: what a person can change about this application.
 *
 * Four cards, and every one of them is drawn every time. Two of them were once conditional —
 * opening at login and which model the agent is asked for are both things a computer has and a
 * phone has not — and this application runs on computers alone now
 * (`.agents/rules/deployments.md`), so there is no longer a device here that has to be asked
 * about before the screen can be built.
 *
 * What the cards themselves report is a different question and unchanged: a setting whose
 * value has not arrived says so rather than drawing a guess
 * (`.agents/rules/honest-emptiness.md`).
 */

import { useTranslation } from "react-i18next";

import CardGrid from "@/components/CardGrid";
import Appearance from "@/features/settings/Appearance";
import Language from "@/features/settings/Language";
import Model from "@/features/settings/Model";
import OpenAtLogin from "@/features/settings/OpenAtLogin";

/** The Settings screen. */
function Settings() {
  const { t } = useTranslation();

  return (
    <div className="screen">
      <h1 className="screen__title">{t("section.settings")}</h1>

      <CardGrid>
        <Appearance />
        <Language />
        <Model />
        <OpenAtLogin />
      </CardGrid>
    </div>
  );
}

export default Settings;
