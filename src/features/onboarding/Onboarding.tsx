/**
 * What a node with no network is shown: one screen, one press, and then the application.
 *
 * # There was a walk here, and the choice in it is gone
 *
 * It asked three things — recognise the product, choose a network, say who contributed this node —
 * and only the first was a question somebody arriving could answer. **Which network** was the one
 * decision that could not be undone, and it was being put to a person in their first minute with
 * the product, before they had anything to judge it by. It is not a question any more: the build
 * says which, and the next section is how.
 *
 * **Who contributed this node** left with it, and it costs something: a node bound to whoever
 * contributed it is how that person earns write credit (`SPECS.md §4.7`), and the window no longer
 * shows the challenge at all. It is written down in the table both faces are held to, and a node
 * run from this window goes uncredited unless its operator also has a terminal.
 *
 * # Which network, and it is the build that says
 *
 * **A development build is for the development network; a build somebody was given is for the real
 * one.** It is the same decision the terminal takes with `--network`, defaulting the same way and
 * for the same reason: what is in front of whoever is writing the software is not the real network.
 * Nobody is asked, because it is a property of the binary rather than a preference.
 *
 * # It opens development, and never production
 *
 * A zone that names nobody means there is a network to **open** rather than one to join, and what
 * happens then depends entirely on which network it is:
 *
 * - **Development** is opened as often as it needs to be, so the press falls through and opens one.
 *   It is what makes a machine with no network able to start at all.
 * - **Production** is opened once in the history of the platform, not once per machine that started
 *   while the zone was quiet. So the press does not fall through: it reports that nobody is there
 *   and the frame says so. Automatic, it would be the accident `SPECS.md §4.5` calls the one that
 *   costs the most, and an append-only log does not undo it.
 *
 * **Neither half of that is enforced here.** The node refuses to open production on the argument
 * itself, before anything happens, so there is no ordering of events in this file that could reach
 * it; what this file does is not ask.
 *
 * # A press that could not join still ends here
 *
 * The frame comes up either way. **A refusal is not a reason to hold somebody on a screen with one
 * button on it** — the node wrote what went wrong into its own state on the way out
 * (`join_a_network` and `open_a_network` both do), and the status strip and the Network screen
 * read a reason out of the same table they read a failed start out of.
 *
 * The refusal a shipped build will meet until production is opened is `nobody_is_there`, and it is
 * the true answer: there is no production network yet, and there will be exactly one, opened once,
 * deliberately, by whoever opens it — from a terminal, which is the only face that can.
 *
 * It is the shape a person already knows from every other node application: the window opens on
 * what the node is doing, and says so there rather than in front of it.
 */

import { useState } from "react";
import { useTranslation } from "react-i18next";

import Logo from "@/components/Logo";
import { Button } from "@/components/ui/button";
import { networkOfThisBuild } from "@/lib/build";
import { comeUp, joinANetwork, openADevelopmentNetwork } from "@/lib/network";

/** The opening screen, and the press that starts the node. */
function Onboarding({ onStarted }: { onStarted: () => void }) {
  const { t } = useTranslation();
  // The press takes a moment — a zone to ask, a record to pull and replay — so the button says it
  // is working rather than sitting there looking unpressed.
  const [starting, setStarting] = useState(false);

  const start = () => {
    setStarting(true);
    void (async () => {
      try {
        const which = networkOfThisBuild();
        try {
          // The zone is that network's own, and nothing here names another.
          await joinANetwork(which);
        } catch (why) {
          // **Nobody there is the other outcome, not a failure — and only on development.** A
          // development zone naming nobody is a network to open; a production one naming nobody
          // is a network that does not exist yet, and this is where that difference is kept.
          if (why !== "nobody_is_there" || which !== "development") throw why;
          await openADevelopmentNetwork();
        }
        // The same call every start after this one makes, so that a first start and every one
        // after it leave the same node running.
        await comeUp();
      } catch {
        // **Deliberately dropped, and this is the one place it is right to.** The identifier is
        // already in the node's own state and in the records; catching it to draw it here would
        // be a second copy of a sentence the frame is about to draw better, beside the controls
        // that do something about it.
      }
      onStarted();
    })();
  };

  return (
    /*
     * The screen takes the whole window and centres what is on it. There is one mark and one
     * button: a column of two things pinned to the top of a window this tall reads as a page that
     * failed to load the rest of itself.
     */
    <div className="screen h-full">
      <div className="flex flex-1 flex-col items-center justify-center gap-6 text-center">
        {/* The application's own mark, larger here than anywhere else and in the identity colour,
            because this is the one screen where it is the thing being looked at rather than a
            label beside a title. */}
        <Logo size={144} color="var(--identity)" />
        <h1 className="text-2xl font-semibold">{t("onboarding.name")}</h1>
        <Button disabled={starting} onClick={start}>
          {starting ? t("onboarding.starting") : t("onboarding.start")}
        </Button>
      </div>
    </div>
  );
}

export default Onboarding;
