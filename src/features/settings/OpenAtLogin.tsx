/**
 * The switch that decides whether the system opens Almena when somebody logs in.
 *
 * It draws what the system says, never what was asked for: every move re-reads the setting and
 * shows what came back. A switch that slides across while nothing changed behind it is the one
 * failure worth spending a component on avoiding — and it is not hypothetical here, because on
 * macOS a person can switch the registration off in System Settings and it stays off.
 *
 * A refusal is `FieldError`, which carries `role="alert"` and is therefore read out the moment
 * it appears. That is why it is absent rather than empty until there is one: an alert is
 * announced by arriving, where the status lines elsewhere in this application have to be in the
 * document from the start to be announced at all.
 */

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import Setting from "@/components/Setting";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { FieldError } from "@/components/ui/field";
import { opensAtLogin, setOpensAtLogin } from "@/lib/openAtLogin";

/** The setting, and what the system said the last time it was asked. */
function OpenAtLogin() {
  const { t } = useTranslation();
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [refused, setRefused] = useState(false);

  useEffect(() => {
    void opensAtLogin().then(setEnabled);
  }, []);

  async function toggle() {
    if (enabled === null) {
      return;
    }

    const wanted = !enabled;
    const now = await setOpensAtLogin(wanted);

    setEnabled(now);
    setRefused(now !== wanted);
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.openAtLogin.heading")}</CardTitle>
        <CardDescription>{t("settings.openAtLogin.body")}</CardDescription>
      </CardHeader>

      <CardContent className="flex flex-col gap-2">
        <Setting
          label={t("settings.openAtLogin.label")}
          checked={enabled}
          onToggle={() => {
            void toggle();
          }}
        />

        {refused && <FieldError>{t("settings.openAtLogin.refused")}</FieldError>}
      </CardContent>
    </Card>
  );
}

export default OpenAtLogin;
