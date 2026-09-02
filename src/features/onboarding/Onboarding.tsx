/**
 * What a node with no network is shown: three screens, in the order the decisions come.
 *
 * # Why it is a walk and not one card
 *
 * They are not one question. **What this application is** is something to recognise before anything
 * is asked; **which network** is the one decision that cannot be undone; and **who contributed this
 * node** is a claim somebody else signs, on their own device, and which the node works without.
 * Putting them on one screen would put a decision that costs nothing beside one that costs
 * everything, at the same weight.
 *
 * **Where the zone names nobody, the chosen network is opened rather than joined**, and that
 * happens under the same press. There was a screen between the two that said so first; it is gone
 * because it asked for a decision that had already been taken — which network — and offered no
 * second one.
 *
 * **Back is always there and never costs anything until the network is chosen.** After that it is
 * gone from the screen it would undo: joining a network is written down, and a control that looked
 * like it could take that back would be lying about what it does.
 *
 * # What is persisted, and when
 *
 * There is no *save* button, and the absence is deliberate. The directory is taken, the key is
 * written and the record is replayed **as part of joining** — a button afterwards would be a second
 * place a node can come from, and its failure mode is a node that joined and then was not saved.
 * What the last screen does instead is say what was written and where.
 */

import { useState } from "react";
import { useTranslation } from "react-i18next";

import Logo from "@/components/Logo";
import { Button } from "@/components/ui/button";
import Claiming from "@/features/onboarding/Claiming";
import Choosing from "@/features/onboarding/Choosing";
import type { Which } from "@/lib/network";

/** Where the walk has got to. */
type Step = "welcome" | "which" | "claim";

/** The walk a node with no network is taken through. */
function Onboarding({ onJoined }: { onJoined: () => void }) {
  const { t } = useTranslation();
  const [step, setStep] = useState<Step>("welcome");
  const [which, setWhich] = useState<Which | null>(null);

  return (
    <div className="screen">
      {step === "welcome" && (
        <div className="flex flex-col items-center gap-6 py-16 text-center">
          {/* The application's own mark, at the size it is drawn everywhere else. It is here to be
              recognised, which is a different job from decorating a heading. */}
          <Logo size={96} />
          <h1 className="text-2xl font-semibold">{t("onboarding.name")}</h1>
          <Button onClick={() => setStep("which")}>{t("onboarding.next")}</Button>
        </div>
      )}

      {step === "which" && (
        <Choosing
          onBack={() => setStep("welcome")}
          onJoined={(joined) => {
            setWhich(joined);
            setStep("claim");
          }}
        />
      )}

      {step === "claim" && which !== null && (
        <Claiming which={which} onDone={onJoined} />
      )}
    </div>
  );
}

export default Onboarding;
