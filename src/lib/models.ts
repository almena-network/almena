/**
 * The models Almena knows how to ask the agent for.
 *
 * **This list is what Almena can name, and not what this computer can serve.** Nothing has
 * asked the local server what it holds — there is no discovery here, and the Settings card
 * says so on the screen rather than leaving somebody to find out by choosing one that is not
 * there. What comes back then is a failure with `model_unknown` on it, which is the agent
 * telling the difference between *that model is not here* and *the agent is broken*.
 *
 * It lives on this side for the reason every other vocabulary does: `preferences.rs` stores a
 * string and hands it back, and adding a model must not mean editing Rust
 * (`.agents/rules/modularity-and-reuse.md`).
 *
 * The names are the ones an OpenAI-compatible server is asked by, so they are identifiers
 * rather than text a person reads — which is why they are not in the catalogs. What *is* in
 * the catalogs is everything the card says around them.
 */

/**
 * Every model Almena will ask for, in the order the Settings screen lists them.
 *
 * Nothing is a default: where nobody has chosen, the agent is told nothing at all and uses its
 * own — which is a fact this side deliberately does not know, so that there is one place a
 * default lives and it is not two.
 */
export const MODELS = [
  "google/gemma-4-e4b",
  "qwen/qwen3-8b",
  "meta/llama-3.2-3b",
  "mistral/mistral-7b",
] as const;

/** One of the models Almena knows how to ask for. */
export type Model = (typeof MODELS)[number];

/**
 * Whether a stored value is still one of the models this build knows.
 *
 * A build that dropped a name somebody had chosen would otherwise draw a control with nothing
 * selected and no explanation. This is what lets the card fall back to *nothing chosen*, which
 * is a state it already draws.
 *
 * @param value - Whatever was stored, which is any string at all.
 */
export function isModel(value: string | null): value is Model {
  return value !== null && (MODELS as readonly string[]).includes(value);
}
