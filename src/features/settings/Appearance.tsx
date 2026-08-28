/**
 * The card that decides what the interface looks like: its palette and its identity colour.
 *
 * Two choices and neither of them is a setting that can be refused, so there is no `FieldError`
 * here: the values go into a file this application owns, and what comes back is drawn.
 *
 * **The palette has three answers and not two.** `system` is not a third colour scheme — it is
 * the absence of a choice, and it goes on answering as the operating system changes its mind.
 * A control that offered only light and dark would be one that quietly overrode a computer that
 * turns light at sunrise.
 *
 * The identity colour is the one place in the application where five colours are on screen at
 * once, and it is the exception that proves the rule: everywhere else exactly one of them
 * exists, because it means "this is Almena" or "this is the thing you came here for". Here they
 * are the thing being chosen, so each is drawn as itself — and each says its name to a screen
 * reader and to the line beside the row, because a colour is never the only carrier of meaning.
 */

import { useId } from "react";
import { useTranslation } from "react-i18next";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Field, FieldTitle } from "@/components/ui/field";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { useAppearance } from "@/hooks/useAppearance";
import { ACCENTS, THEMES, isAccent, isTheme } from "@/lib/appearance";

/** The word for each palette. Written out because a key assembled at runtime is not a key. */
const THEME_NAME = {
  system: "settings.appearance.theme.system",
  light: "settings.appearance.theme.light",
  dark: "settings.appearance.theme.dark",
} as const;

/** The word for each identity colour, on the same terms. */
const ACCENT_NAME = {
  orange: "settings.appearance.accent.orange",
  blue: "settings.appearance.accent.blue",
  red: "settings.appearance.accent.red",
  yellow: "settings.appearance.accent.yellow",
  green: "settings.appearance.accent.green",
} as const;

/**
 * The swatch each identity colour is shown as.
 *
 * A lookup rather than a class assembled from the name, for the reason `StateBadge` gives:
 * Tailwind reads the source for whole class names, so `bg-identity-${name}` would compile to
 * nothing at all. The five are tokens of `src/styles/tokens.css` and change with the palette,
 * which is what makes a swatch show the colour a person would actually get.
 */
const SWATCH = {
  orange: "bg-identity-orange",
  blue: "bg-identity-blue",
  red: "bg-identity-red",
  yellow: "bg-identity-yellow",
  green: "bg-identity-green",
} as const;

/** How the interface is drawn, and the two things about it a person chooses. */
function Appearance() {
  const { t } = useTranslation();
  const { theme, accent, chooseTheme, chooseAccent } = useAppearance();
  const themeLabel = useId();
  const accentLabel = useId();

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.appearance.heading")}</CardTitle>
        <CardDescription>{t("settings.appearance.body")}</CardDescription>
      </CardHeader>

      <CardContent className="flex flex-col gap-6">
        <Field>
          <FieldTitle id={themeLabel}>
            {t("settings.appearance.theme.label")}
          </FieldTitle>

          <ToggleGroup
            type="single"
            variant="outline"
            value={theme}
            aria-labelledby={themeLabel}
            onValueChange={(next) => {
              // Radix reports the empty string when the pressed entry was already the chosen
              // one. There is no "no palette", so that is not a change.
              if (isTheme(next)) {
                chooseTheme(next);
              }
            }}
          >
            {THEMES.map((name) => (
              <ToggleGroupItem key={name} value={name}>
                {t(THEME_NAME[name])}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </Field>

        <Field>
          <FieldTitle id={accentLabel}>
            {t("settings.appearance.accent.label")}
          </FieldTitle>

          <div className="flex flex-wrap items-center gap-3">
            <ToggleGroup
              type="single"
              variant="outline"
              value={accent}
              aria-labelledby={accentLabel}
              onValueChange={(next) => {
                if (isAccent(next)) {
                  chooseAccent(next);
                }
              }}
            >
              {ACCENTS.map((name) => (
                <ToggleGroupItem
                  key={name}
                  value={name}
                  aria-label={t(ACCENT_NAME[name])}
                >
                  <span
                    className={`size-4 rounded-full ${SWATCH[name]}`}
                    aria-hidden="true"
                  />
                </ToggleGroupItem>
              ))}
            </ToggleGroup>

            <span className="text-xs text-faint">{t(ACCENT_NAME[accent])}</span>
          </div>
        </Field>
      </CardContent>
    </Card>
  );
}

export default Appearance;
