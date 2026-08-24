/**
 * A setting that is on or off, and says which in words as well as in colour.
 *
 * The words are not the caller's to choose. Every setting in the application says the same
 * three, out of the catalogs and out of this file, because a screen that invented its own pair
 * would be a screen where "on" means something slightly different —
 * `.agents/rules/interface.md`.
 *
 * `checked` is allowed to be `null`, and that is not an oversight: a setting read from the
 * operating system is not known for the first moment of a screen's life, and a switch that
 * said "off" in the meantime would be stating something nobody had checked. Unknown is a third
 * word and a control nobody can touch, not a guess.
 *
 * The row is shadcn/ui's `Field` in its horizontal orientation, which is the shape it ships for
 * exactly this — a name and its explanation on the left, the control on the right — and the
 * switch and the label are its own. What this file adds is the word at the end, which is the
 * part that has to be the same everywhere.
 */

import { useId } from "react";
import { useTranslation } from "react-i18next";

import { Field, FieldContent, FieldLabel } from "@/components/ui/field";
import { Switch } from "@/components/ui/switch";

/**
 * What the word at the end says, for each of the three things a setting can be.
 *
 * @param checked - Whether the setting is on, or `null` while nobody has read it yet.
 * @returns The catalog key holding the word.
 */
function stateKey(checked: boolean | null) {
  if (checked === null) {
    return "control.unmeasured";
  }

  return checked ? "control.on" : "control.off";
}

/** What a setting is made of. */
interface SettingProps {
  /** What the setting is called, already translated. */
  label: string;
  /** Whether it is on, or `null` while the answer is still being fetched. */
  checked: boolean | null;
  /** Called when it is pressed. Not called at all while `checked` is `null`. */
  onToggle: () => void;
}

/** One setting. */
function Setting({ label, checked, onToggle }: SettingProps) {
  const { t } = useTranslation();
  const id = useId();

  return (
    // The row is 44 points tall and the label fills it, so that what a finger has to hit is the
    // setting's name and not the 18-point switch beside it
    // (`.agents/rules/deployments.md`).
    <Field orientation="horizontal" data-disabled={checked === null}>
      <FieldContent className="min-h-11 justify-center">
        <FieldLabel htmlFor={id}>{label}</FieldLabel>
      </FieldContent>

      <span className="text-xs text-faint">{t(stateKey(checked))}</span>

      <Switch
        id={id}
        checked={checked ?? false}
        disabled={checked === null}
        onCheckedChange={onToggle}
      />
    </Field>
  );
}

export default Setting;
