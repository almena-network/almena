/**
 * The Settings screen: what a person can change about this application.
 *
 * There is one thing to change today and it belongs to a computer, so on a phone this screen
 * says that rather than drawing an empty page. It is the same choice `NotBuilt` makes for a
 * section with no screen, and it is drawn with the same element: an empty answer is an answer,
 * and a page that draws nothing at all reads as broken rather than as unfinished.
 *
 * Nothing is drawn while the answer is still being fetched. That moment is short and a control
 * that appears and then vanishes is worse than one that arrives a beat late.
 */

import { SlidersHorizontal } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import CardGrid from "@/components/CardGrid";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
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

      {desktop === true && (
        <CardGrid>
          <OpenAtLogin />
        </CardGrid>
      )}

      {desktop === false && (
        <Empty className="border">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <SlidersHorizontal aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t("settings.nothingHereTitle")}</EmptyTitle>
            <EmptyDescription>{t("settings.nothingHere")}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      )}
    </div>
  );
}

export default Settings;
