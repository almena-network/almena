/**
 * One of the four states, drawn as a colour and said as a word.
 *
 * It exists because the pairing is the rule and not the screen's to choose: a state is never
 * carried by colour alone, and the four are the only four. Two screens showing the same state
 * in two different words, or a fifth colour arriving because somebody needed one, are both
 * what this closes.
 *
 * The badge around it is shadcn/ui's, in its outline tone. A badge *filled* with a state colour
 * would put a second, louder red on a screen that already has one meaning for red, so the
 * colour is the dot inside it rather than the whole of it.
 */

import { Badge } from "@/components/ui/badge";

/**
 * The dot each state is drawn with.
 *
 * A lookup rather than a class assembled from the tone, because Tailwind reads the source for
 * whole class names: `bg-${tone}` would compile to nothing at all.
 */
const DOT = {
  ok: "bg-ok",
  warn: "bg-warn",
  bad: "bg-bad",
  idle: "bg-idle",
} as const;

/** Which of the four: well, degraded, failed, idle. */
export type StateTone = keyof typeof DOT;

/** What a state badge is made of. */
interface StateBadgeProps {
  /** The state. */
  tone: StateTone;
  /** The word for it, already translated. It is not optional: the two arrive together. */
  label: string;
}

/** One state, in colour and in words. */
function StateBadge({ tone, label }: StateBadgeProps) {
  return (
    <Badge variant="outline" className="gap-2 text-muted-foreground">
      <span className={`size-2 rounded-full ${DOT[tone]}`} aria-hidden="true" />
      {label}
    </Badge>
  );
}

export default StateBadge;
