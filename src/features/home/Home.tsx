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
 * It titles itself the way every section of one screen does. It carried the mark and the
 * product's name instead, once — the head of the first screen was one of the two places the
 * mark wore the identity colour — and that moved to the head of the navigation, where it is
 * bigger and where a person meets it before they meet any screen. What it costs is stated
 * rather than discovered: the navigation's head is drawn in the expanded shape alone, so below
 * 600 the mark now appears only in the status strip, in the strip's own faint grey.
 */

import { useTranslation } from "react-i18next";

import CardGrid from "@/components/CardGrid";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import NotifyButton from "@/features/home/NotifyButton";

/** The application's first screen. */
function Home() {
  const { t } = useTranslation();

  return (
    <div className="screen">
      <h1 className="screen__title">{t("section.home")}</h1>

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
