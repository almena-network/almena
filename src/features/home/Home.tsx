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
 * build can genuinely do, on all five platforms, so the screen says so and lets a person try
 * it — a capability that works, not a figure standing in for one that does not.
 */

import { useTranslation } from "react-i18next";

import Logo from "@/components/Logo";
import NotifyButton from "@/features/home/NotifyButton";
import "@/features/home/home.css";

/** What this build is. Vite replaces it at build time from `package.json`. */
const VERSION = __APP_VERSION__;

/** The application's first screen. */
function Home() {
  const { t } = useTranslation();

  return (
    <div className="screen home">
      <header className="home__brand">
        <Logo size={28} color="var(--color-accent)" />
        <h1 className="home__name">{t("app.name")}</h1>
        <p className="home__version">{t("home.version", { version: VERSION })}</p>
      </header>

      <div className="home__cards">
        <section className="panel home__card">
          <h2 className="home__card-title">{t("home.notJoined.title")}</h2>
          <p className="home__card-body">{t("home.notJoined.body")}</p>
        </section>

        <section className="panel home__card">
          <h2 className="home__card-title">{t("home.notify.heading")}</h2>
          <p className="home__card-body">{t("home.notify.body")}</p>
          <NotifyButton />
        </section>
      </div>
    </div>
  );
}

export default Home;
