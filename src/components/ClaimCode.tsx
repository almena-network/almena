/**
 * The challenge a node shows, drawn as a code and written out beneath it.
 *
 * **One element, so that the two places a challenge is shown draw the same thing.** The walk
 * shows one on its last screen and the Network screen shows one on request; two squares that
 * looked alike and carried different payloads were how one of them came to carry the node's
 * public identifier, which binds nothing. What is drawn here is the challenge and only ever the
 * challenge: the nonce the node made and remembers, good for a stated while, naming the node
 * inside itself.
 *
 * The text is drawn beside the code because the terminal face has no code to draw — the same
 * string typed approves the same thing — and because a code that cannot be scanned from where the
 * window happens to be is still a challenge somebody can copy.
 */

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import QRCode from "qrcode";

/** What the code is drawn from. */
interface ClaimCodeProps {
  /** The challenge, exactly as the node showed it. */
  challenge: string;
}

/** The challenge, as a code and as text. */
function ClaimCode({ challenge }: ClaimCodeProps) {
  const { t } = useTranslation();
  const [code, setCode] = useState<string | null>(null);

  /* Drawn for this challenge and again for the next one. Nothing is set until the drawing is
     back, so the square never holds a code for a challenge that has been replaced. */
  useEffect(() => {
    let alive = true;
    setCode(null);
    void QRCode.toString(challenge, {
      type: "svg",
      margin: 1,
      errorCorrectionLevel: "Q",
      color: { dark: "#e9ecf1", light: "#0e1116" },
    })
      .then((svg) => {
        if (alive) setCode(svg);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [challenge]);

  return (
    <div className="flex flex-col gap-3">
      {code === null ? (
        <p className="text-sm text-muted-foreground">{t("onboarding.claim.drawing")}</p>
      ) : (
        <div
          className="w-full max-w-[240px] rounded-lg border border-[var(--line)] bg-[var(--surface-0)] p-4 [&_svg]:block [&_svg]:h-auto [&_svg]:w-full"
          dangerouslySetInnerHTML={{ __html: code }}
        />
      )}
      {/* Selectable text and never an input: it is the node's to show, not anybody's to edit,
          and what gets approved has to be what was shown. */}
      <p className="bg-muted rounded-md p-2 font-mono text-xs break-all select-all">{challenge}</p>
    </div>
  );
}

export default ClaimCode;
