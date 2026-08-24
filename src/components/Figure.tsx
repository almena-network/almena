/**
 * A figure: something measured, beside what it is a measurement of.
 *
 * It exists to make one of this project's promises hard to break. A screen must never show a
 * value nobody measured (`AGENTS.md`, Transparency), and the tempting way to break that is a
 * zero — a peer count of nought reads as a measurement and is not one when there is no network
 * to have counted. So a figure with nothing behind it is `null`, and what it draws is a dash
 * that says so in words to anything that cannot see it.
 *
 * shadcn/ui has no element for this and it is not one of its variants: a figure is Almena's
 * own, built from the same values as everything else — see
 * `.agents/rules/interface.md`.
 */

import { useTranslation } from "react-i18next";

/** What a figure is made of. */
interface FigureProps {
  /** What it is a measurement of, already translated. */
  label: string;
  /** The value, already formatted for the reader's locale — or `null` where nothing was measured. */
  value: string | null;
}

/** One figure. */
function Figure({ label, value }: FigureProps) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-1">
      <span className="text-xs text-muted-foreground">{label}</span>

      {/* The dash is drawn in the faintest of the three text colours rather than in a state
          colour: "nobody measured this" is not one of the four states, and borrowing one would
          make it look like an answer. */}
      {value === null ? (
        <span className="font-mono text-base text-faint">
          <span aria-hidden="true">—</span>
          <span className="sr-only">{t("control.unmeasured")}</span>
        </span>
      ) : (
        <span className="font-mono text-base">{value}</span>
      )}
    </div>
  );
}

export default Figure;
