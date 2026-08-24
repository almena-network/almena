/**
 * The card that decides which language the interface speaks.
 *
 * **Nobody has to open it.** The device is asked first and its answer is what a person meets:
 * a Spanish computer opens a Spanish Almena, and this card exists for the person that is wrong
 * for. Until one of them uses it nothing is stored, so a device that later changes its own
 * language changes Almena's with it.
 *
 * Each language is written in itself — *English*, *Español* — and not in the language the
 * interface happens to be showing. Somebody looking for their own language in a list is looking
 * for the word they would recognise, which is never the translation of it.
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
import { Field, FieldContent, FieldLabel } from "@/components/ui/field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { LANGUAGES, isLanguage } from "@/i18n";
import { useLanguage } from "@/hooks/useLanguage";

/** The name of each language, in itself. */
const LANGUAGE_NAME = {
  en: "language.en",
  es: "language.es",
} as const;

/** The language the interface speaks, and the way to change it. */
function Language() {
  const { t } = useTranslation();
  const { language, chooseLanguage } = useLanguage();
  const id = useId();

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("settings.language.heading")}</CardTitle>
        <CardDescription>{t("settings.language.body")}</CardDescription>
      </CardHeader>

      <CardContent>
        {/* The row is 44 points tall and the label fills it, so that what a finger has to hit
            is the setting's name and not the control beside it
            (`.agents/rules/deployments.md`). */}
        <Field orientation="horizontal">
          <FieldContent className="min-h-11 justify-center">
            <FieldLabel htmlFor={id}>{t("settings.language.label")}</FieldLabel>
          </FieldContent>

          <Select
            value={language}
            onValueChange={(next) => {
              if (isLanguage(next)) {
                chooseLanguage(next);
              }
            }}
          >
            <SelectTrigger id={id}>
              <SelectValue />
            </SelectTrigger>

            <SelectContent>
              {LANGUAGES.map((name) => (
                <SelectItem key={name} value={name}>
                  {t(LANGUAGE_NAME[name])}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
      </CardContent>
    </Card>
  );
}

export default Language;
