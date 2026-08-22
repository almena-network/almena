/**
 * The first screen: what this application is, and what it can honestly say today.
 *
 * It is not a dashboard. There is no client of the node API in this repository, so there is
 * nothing to reach and nothing measured — and a screen that filled the space with a peer count
 * of zero, an empty list of networks or a grey status dot would be reporting measurements
 * nobody took. Each of those reads as data; none would be.
 *
 * What replaces the emptiness is the truth about it, said once. When there is a node to reach
 * and something to report, this is the screen that reports it — and until then it says which
 * of the two situations a reader is in, rather than dressing the first as the second.
 */

import { useTranslation } from "react-i18next";

import Logo from "@/components/Logo";
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

      <section className="panel home__standing">
        <h2 className="home__standing-title">{t("home.disconnected.title")}</h2>
        <p className="home__standing-body">{t("home.disconnected.body")}</p>
      </section>
    </div>
  );
}

export default Home;
