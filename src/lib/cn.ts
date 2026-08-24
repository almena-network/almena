/**
 * Joining class names, which every element in `src/components/ui/` does.
 *
 * It is one function and it is not a drawer of helpers: `clsx` resolves the conditions and
 * `tailwind-merge` resolves the conflicts, so that a class passed in by a caller beats the
 * one the element was built with instead of landing beside it and losing to source order.
 *
 * The file is named after the function rather than called `utils`, which is what shadcn's
 * scaffold names it — see `.agents/rules/code.md`, and `components.json`,
 * whose `aliases.utils` is what points the generated imports here.
 */

import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Resolves a list of class names into the one string an element is drawn with.
 *
 * @param inputs - Class names, arrays or condition objects, in the order they should apply.
 * @returns The merged class string, with later Tailwind utilities beating earlier ones of the
 *   same kind.
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
