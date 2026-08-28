/**
 * The strip along the true bottom of the window: what the application is, and what it is doing.
 *
 * It is the frame's and not a screen's, which is the whole difference between this and a page
 * footer. It spans the full width — under the sidebar as well as beside it — it is pinned to
 * the bottom rather than reached by scrolling, and it is the same strip whichever screen is
 * open. Nothing on it scrolls away.
 *
 * **It is built to be added to.** The left group is where what the application is *doing*
 * belongs — which network, how many peers, what it is waiting on — and it is deliberately
 * empty today, because none of that is known yet and a status bar that invented something to
 * say would be the worst place in the interface to do it. The right group holds what does not
 * change while the application runs.
 *
 * The mark is drawn in the strip's own faint colour rather than in the identity one. It is
 * here to say which application this is at a glance, not to be the thing anybody is looking
 * at — and the strip is the one place the mark is visible in both shapes.
 *
 * **A development build says so here, and this is the one place it could say it.** The marker
 * has to be on screen whatever section is open and whichever shape the window is in, and this
 * strip is the only thing in the application that is. It is brighter than everything else on
 * the strip because it is the one thing there that should be noticed; it is not a state colour,
 * because being a development build is not one of the four states and borrowing one of those
 * would cost them their meaning.
 */

import { useTranslation } from "react-i18next";

import Logo from "@/components/Logo";
import { Badge } from "@/components/ui/badge";
import { isDevelopmentBuild } from "@/lib/build";

/** What this build is. Vite replaces it at build time from `package.json`. */
const VERSION = __APP_VERSION__;

/** The strip along the bottom of the window. */
function StatusBar() {
  const { t } = useTranslation();

  return (
    <footer className="status" aria-label={t("status.label")}>
      <div className="flex min-w-0 items-center gap-2">
        <Logo size={12} />
        <span className="truncate">{t("app.name")}</span>
      </div>

      <div className="ml-auto flex min-w-0 items-center gap-3">
        {isDevelopmentBuild() && <Badge variant="outline">{t("status.development")}</Badge>}

        <span className="truncate font-mono">
          {t("app.version", { version: VERSION })}
        </span>
        <span className="truncate">{t("status.licence")}</span>
      </div>
    </footer>
  );
}

export default StatusBar;
