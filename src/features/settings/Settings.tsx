/**
 * The Settings screen: what a person can change about this application.
 *
 * Four cards, and two of them are not on every device. How the interface is drawn and which
 * language it speaks are questions any device can answer. The other two belong to a computer:
 * whether the system opens Almena at login is a computer's to decide, and which model the
 * agent is asked for is a setting for a program a phone cannot run at all
 * (`.agents/rules/supported-platforms.md`). A phone without either card is not a person unable
 * to do something — it is a platform with no such thing to do.
 *
 * Which of the two this device is has to be asked of the Rust side, and until the answer is
 * back the card is not drawn. That is a card arriving rather than a screen claiming there is
 * nothing to set: the screen is full either way and says nothing about what has not arrived,
 * which is the difference between a gap and an assertion
 * (`.agents/rules/honest-emptiness.md`).
 */

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import CardGrid from "@/components/CardGrid";
import Appearance from "@/features/settings/Appearance";
import Language from "@/features/settings/Language";
import Model from "@/features/settings/Model";
import OpenAtLogin from "@/features/settings/OpenAtLogin";
import { isDesktop } from "@/lib/platform";

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

      <CardGrid>
        <Appearance />
        <Language />
        {desktop === true && <Model />}
        {desktop === true && <OpenAtLogin />}
      </CardGrid>
    </div>
  );
}

export default Settings;
