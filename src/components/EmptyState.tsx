/**
 * What a screen draws where something would be, when there is nothing.
 *
 * Almena's own, over shadcn's `Empty`, and it exists to make one thing impossible: an
 * emptiness with no reason on it. The reason is a required prop, so "Nothing to show" — a
 * sentence that tells a reader neither why nor whether the screen works — cannot be drawn
 * without somebody deliberately writing it as the reason.
 *
 * The icon is passed in rather than fixed, because the three reasons a list can be empty are
 * three different facts and a reader should be able to tell them apart before reading a word:
 * still looking, nothing to look at, looked and found none —
 * `.agents/rules/honest-emptiness.md`.
 */

import type { ReactNode } from "react";

import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";

/** What an emptiness is made of. */
interface EmptyStateProps {
  /** The mark for this particular nothing. Given `aria-hidden`, since the title says it. */
  icon: ReactNode;
  /** What is missing, in a few words and already translated. */
  title: string;
  /** Why it is missing, and what would fill it. Already translated. */
  reason: string;
}

/** An emptiness, with the reason for it. */
function EmptyState({ icon, title, reason }: EmptyStateProps) {
  return (
    <Empty className="border">
      <EmptyHeader>
        <EmptyMedia variant="icon">{icon}</EmptyMedia>
        <EmptyTitle>{title}</EmptyTitle>
        <EmptyDescription>{reason}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}

export default EmptyState;
