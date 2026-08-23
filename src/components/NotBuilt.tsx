/**
 * What a section of the interface shows before it has a screen.
 *
 * The alternative is a navigation entry that does nothing when clicked, which reads as a
 * broken application rather than an unfinished one. This says which of the two it is.
 *
 * It is shadcn/ui's `Empty` with the mark in its media slot. What this file adds is the two
 * things the element cannot know: which mark, and what an unbuilt section is called in the
 * reader's language.
 *
 * The mark is drawn in the border colour rather than the identity one: this is the one place
 * it appears where it is not saying "this is Almena" so much as filling the space that is
 * honestly empty, and a screen with nothing on it is not the thing anybody came here for.
 */

import { useTranslation } from "react-i18next";

import Logo from "@/components/Logo";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";

/** What the empty screen is standing in for. */
interface NotBuiltProps {
  /**
   * The section's name, already translated. A name rather than a catalog key, so that this
   * component works for any section without knowing how sections are named.
   */
  title: string;
}

/** An empty screen naming the section it belongs to. */
function NotBuilt({ title }: NotBuiltProps) {
  const { t } = useTranslation();

  return (
    <Empty className="h-full">
      <EmptyHeader>
        <EmptyMedia>
          <Logo size={40} color="var(--border)" />
        </EmptyMedia>
        <EmptyTitle>{title}</EmptyTitle>
        <EmptyDescription>{t("common.notBuilt")}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}

export default NotBuilt;
