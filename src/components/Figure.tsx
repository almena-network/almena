/**
 * A figure: something measured, beside what it is a measurement of.
 *
 * It exists to make one of this project's promises hard to break. A screen must never show a
 * value nobody measured, and the tempting way to break that is a zero — a peer count of nought
 * reads as a measurement and is not one when there is no network to have counted. So a figure
 * with nothing behind it is `null`, and what it draws is a dash that says so in words to
 * anything that cannot see it.
 *
 * shadcn/ui has no element for this and it is not one of its variants: a figure is Almena's
 * own, built from the same values as everything else so that it stays a composition of that
 * set rather than a second visual language beside it.
 *
 * # Two kinds of figure, and the shape follows the value
 *
 * A count is three characters and a name is sixty-two, and a grid that gave them the same
 * column serves neither: laid out for the name, seven counts sit in a row of mostly nothing;
 * laid out for the count, the name is a stack of five two-word lines — and before this it did
 * not even do that, because the value did not wrap and simply painted over the figures beside
 * it.
 *
 * So a figure whose value is an **identifier** takes the whole width of the grid it is in
 * rather than one column of it, and a figure whose value is a measurement takes a column. It
 * is one flag rather than two because the two things always travel together: a value long
 * enough to want the width is a value somebody copies rather than reads, and that is the same
 * value that gets the button. Nobody pastes a peer count.
 *
 * Every value wraps either way, and the two wrap differently: an identifier has nowhere a line
 * may be broken, so it breaks anywhere, and everything else breaks at its spaces first.
 * `min-w-0` on the cell is the half of that which is easy to miss: a grid track will not shrink
 * a cell below its content unless it is told it may, so without it neither of them ever gets the
 * chance to break anything.
 */

import { useTranslation } from "react-i18next";

import Copyable from "@/components/Copyable";
import { cn } from "@/lib/cn";

/** What a figure is made of. */
interface FigureProps {
  /** What it is a measurement of, already translated. */
  label: string;
  /** The value, already formatted for the reader's locale — or `null` where nothing was measured. */
  value: string | null;
  /**
   * Whether the value is an identifier: a name, a key, a root, an address.
   *
   * One flag for two consequences, because they are one fact about the value. It is drawn
   * across the whole grid instead of in one column of it, and it carries the button that puts
   * the whole of it on the clipboard — which a measurement does not, since a peer count is
   * something to read and never something to paste.
   */
  identifier?: boolean;
}

/** One figure. */
function Figure({ label, value, identifier = false }: FigureProps) {
  const { t } = useTranslation();

  return (
    <div className={cn("flex min-w-0 flex-col gap-1", identifier && "col-span-full")}>
      <span className="text-xs text-muted-foreground">{label}</span>

      {/* The dash is drawn in the faintest of the three text colours rather than in a state
          colour: "nobody measured this" is not one of the four states, and borrowing one would
          make it look like an answer. It carries no button either way: there is nothing to
          copy, and a control that would put a dash on the clipboard is a control that lies. */}
      {value === null ? (
        <span className="font-mono text-base text-faint">
          <span aria-hidden="true">—</span>
          <span className="sr-only">{t("control.unmeasured")}</span>
        </span>
      ) : identifier ? (
        <Copyable value={value} what={label} className="text-base" />
      ) : (
        // `break-words` and not `break-all`: a value in a column is broken inside a word only
        // where the word itself will not fit, so anything with a space in it breaks at the space
        // first. It is the rule `Turn` already draws prose under, and the difference is visible
        // on the Agent screen — `break-all` put "almena-agent 0.4." on one line and "2 (bundled)"
        // on the next, which reads as a different version. A count has nothing to break either
        // way, so nothing that this element was written for changes.
        <span className="font-mono text-base break-words">{value}</span>
      )}
    </div>
  );
}

export default Figure;
