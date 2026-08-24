/**
 * The failure of a run, drawn as a sentence this build wrote rather than one the agent sent.
 *
 * Apart from the screen because the narrowing below is a decision of its own and will outlive
 * the shape of the screen around it: the agent is released on its own schedule, its list of
 * identifiers is allowed to grow without this build being rebuilt, and what a person reads has
 * to come from the catalogs either way.
 */

import { useTranslation } from "react-i18next";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

/**
 * Every failure this interface has words for.
 *
 * Written out rather than derived, because it is exactly the list the catalogs carry — and the
 * agent's own list is longer and is allowed to grow without this build being rebuilt. What
 * arrives from outside it is drawn with the general sentence below.
 */
const SAYS = [
  "agent_will_not_start",
  "agent_stopped",
  "run_already_in_flight",
  "model_unreachable",
  "model_unknown",
  "resource_unknown",
] as const;

/**
 * The catalog key for one failure, or the general one where this build has never heard of it.
 *
 * The narrowing is here and it is deliberate: an identifier is not text, and an application
 * that drew one because it had nothing better would be putting a subprocess's vocabulary in
 * front of a person (`.agents/rules/language.md`). The code itself is already in the
 * records — the Rust side writes it the moment the failure arrives — so nothing is lost by
 * keeping it off the screen.
 */
function reasonFor(code: string) {
  const known = SAYS.find((said) => said === code);
  return known === undefined ? ("ai.error.unknown" as const) : (`ai.error.${known}` as const);
}

/** What `Failure` is given. */
interface FailureProps {
  /** The identifier the last run failed with. */
  code: string;
}

/** Says that a run failed, and why, in words from the catalogs. */
function Failure({ code }: FailureProps) {
  const { t } = useTranslation();

  return (
    <Alert variant="destructive">
      <AlertTitle>{t("ai.error.heading")}</AlertTitle>
      <AlertDescription>{t(reasonFor(code))}</AlertDescription>
    </Alert>
  );
}

export default Failure;
