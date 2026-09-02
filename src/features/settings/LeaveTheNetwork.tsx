/**
 * The way out: erase this node from this machine and start again from the first screen.
 *
 * # Why this is in Settings and not with the node
 *
 * It used to sit at the bottom of a card of operator's controls on the Network screen, and that
 * card is gone: what a node is doing by hand is the terminal's, and the window came up as a node
 * that starts by itself. **This did not go with it**, because it is not operating a node — it is
 * the opposite, and it is the only thing in this application a person cannot get out of any other
 * way. An application whose one irreversible act had no button would be one somebody has to be
 * told how to leave.
 *
 * Settings is where it belongs for the ordinary reason: it is the section for what is true of the
 * whole application rather than of one screen, and starting over is exactly that.
 *
 * # What it does, and the order
 *
 * **The network is told first, while there is still a node to tell it with.** Then the node is
 * stopped, the directory goes — the key, the record, the roots — and the notes the node kept in
 * the preferences with it. What a person chose about the application is not the node's and is
 * kept: the palette, the identity colour, the language and the model all survive.
 *
 * **It does not need a node that works.** A close that could not be said does not stop it: whoever
 * reaches for this is often the person whose node will not come up. What that costs is a node the
 * record's observers find silent rather than one that said it was going, which is why the line
 * under the button says so before it is pressed and not after.
 *
 * # Two presses, and the second is a second decision
 *
 * Armed by one and done by the next, because it does not come back. There is no dialog: a
 * confirmation somebody dismisses without reading is not a decision, and a button that says what
 * the next press will do is one they have to read to press.
 */

import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { FieldError } from "@/components/ui/field";
import { eraseThisNode } from "@/lib/network";

/** Erasing this node, and starting over. */
function LeaveTheNetwork() {
  const { t } = useTranslation();
  const [armed, setArmed] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.leave.heading")}</CardTitle>
      </CardHeader>

      <CardContent className="flex flex-col gap-2">
        <Button
          variant="destructive"
          onClick={() => {
            if (!armed) {
              setArmed(true);
              return;
            }
            setArmed(false);
            setFailed(null);
            // **The application starts over, and that is what the reload is.** Whether the first
            // screen is drawn is decided at the top, from a reading this card cannot reach. A
            // reload runs the start again — which now comes back to nothing — and that is
            // precisely the state being asked for.
            //
            // Only where it worked. A refusal keeps the screen, because the node is still there
            // and the sentence under the button is what to do about it.
            void eraseThisNode()
              .then(() => {
                window.location.reload();
              })
              .catch((why: unknown) => {
                setFailed(typeof why === "string" ? why : "");
              });
          }}
        >
          {armed ? t("settings.leave.armed") : t("settings.leave.erase")}
        </Button>

        <p className="text-muted-foreground text-sm">
          {t("settings.leave.whileDown")}
        </p>

        {/* Absent rather than empty until there is one: `FieldError` carries `role="alert"`, so it
            is read out by arriving. */}
        {failed !== null && (
          <FieldError>
            {t(failed === "not_erased" ? "settings.leave.notErased" : "settings.leave.notErasedWhy")}
          </FieldError>
        )}
      </CardContent>
    </Card>
  );
}

export default LeaveTheNetwork;
