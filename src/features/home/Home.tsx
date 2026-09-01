/**
 * The first screen: what this application is, and what it can honestly say today.
 *
 * **It is a dashboard now, because there is something to report.** A node joins a network by
 * itself and this is where what came of that is said: which network it is on, what it is called
 * there, how much of the record it holds and the root over it. What it draws is what the node
 * answered — `null` stays absent rather than becoming a nought, because a count nobody took and a
 * count that came back nought are different things and only one of them is a measurement.
 *
 * A node with no network never reaches this screen: it is offered the one decision that has to be
 * taken first instead.
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
import { useNetwork } from "@/hooks/useNetwork";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import NotifyButton from "@/features/home/NotifyButton";

/** The application's first screen. */
function Home() {
  const { t } = useTranslation();
  const { reading } = useNetwork();

  /** One fact, or the mark that says the node did not give one. */
  const fact = (what: string, said: string | null) => (
    <Card key={what}>
      <CardHeader>
        <CardTitle>{what}</CardTitle>
      </CardHeader>
      <CardContent>
        <p className="font-mono text-sm break-all">{said ?? "—"}</p>
      </CardContent>
    </Card>
  );

  return (
    <div className="screen">
      <h1 className="screen__title">{t("section.home")}</h1>

      <CardGrid>
        {fact(t("home.on.network"), reading?.network ?? null)}
        {fact(t("home.on.identity"), reading?.identity ?? null)}
        {fact(
          t("home.on.written"),
          reading?.written == null ? null : String(reading.written),
        )}
        {fact(t("home.on.root"), reading?.root ?? null)}

        <Card>
          <CardHeader>
            <CardTitle>{t("home.notify.heading")}</CardTitle>
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
