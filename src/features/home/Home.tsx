/**
 * The first screen: what this application is, and what it can honestly say today.
 *
 * It is not a dashboard. On a computer this application is the node, and the peer-to-peer
 * layer that would put it on a network is not written here — so there is nothing joined and
 * nothing measured, and a screen that filled the space with a peer count of zero, an empty
 * list of networks or a grey status dot would be reporting measurements nobody took. Each of
 * those reads as data; none would be.
 *
 * What replaces the emptiness is the truth about it, said once. When this application is on a
 * network and has something to report, this is the screen that reports it — and until then it
 * says which of the two situations a reader is in, rather than dressing the first as the
 * second.
 *
 * The second card is the other half of the same honesty. Notifications are something this
 * build can genuinely do, on all three platforms, so the screen says so and lets a person try
 * it — a capability that works, not a figure standing in for one that does not.
 *
 * The mark at its head is the one place in the compact shape it appears, and it wears the
 * identity colour because that is the first of the two things the colour means: this is
 * Almena (`.agents/rules/interface.md`).
 */

import { useTranslation } from "react-i18next";

import CardGrid from "@/components/CardGrid";
import Logo from "@/components/Logo";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import NotifyButton from "@/features/home/NotifyButton";

/** What this build is. Vite replaces it at build time from `package.json`. */
const VERSION = __APP_VERSION__;

/** The application's first screen. */
function Home() {
  const { t } = useTranslation();

  return (
    <div className="screen">
      <header className="flex flex-wrap items-center gap-3">
        <Logo size={28} color="var(--identity)" />
        <h1 className="text-2xl font-semibold tracking-tight">{t("app.name")}</h1>
        <p className="font-mono text-xs text-faint">
          {t("app.version", { version: VERSION })}
        </p>
      </header>

      <CardGrid>
        <Card>
          <CardHeader>
            <CardTitle>{t("home.notJoined.title")}</CardTitle>
            <CardDescription>{t("home.notJoined.body")}</CardDescription>
          </CardHeader>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>{t("home.notify.heading")}</CardTitle>
            <CardDescription>{t("home.notify.body")}</CardDescription>
          </CardHeader>
          <CardContent>
            <NotifyButton />
          </CardContent>
        </Card>
      </CardGrid>
    </div>
  );
}

export default Home;
