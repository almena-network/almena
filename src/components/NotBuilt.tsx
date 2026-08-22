/**
 * What a section of the interface shows before it has a screen.
 *
 * The alternative is a navigation entry that does nothing when clicked, which reads as a
 * broken application rather than an unfinished one. This says which of the two it is.
 */

import { useTranslation } from "react-i18next";

import Logo from "@/components/Logo";
import "@/components/NotBuilt.css";

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
    <div className="not-built">
      <Logo size={40} color="var(--color-border)" />
      <h2 className="not-built__title">{title}</h2>
      <p className="not-built__note">{t("common.notBuilt")}</p>
    </div>
  );
}

export default NotBuilt;
