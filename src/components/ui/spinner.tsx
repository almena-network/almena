/**
 * What is drawn while something is in flight. Vendored from shadcn/ui —
 * `.agents/rules/interface.md`.
 *
 * It ships with an English `aria-label`, which is the one thing about it this project cannot
 * accept: a string a person hears is user-facing text and comes from a catalog
 * (`.agents/rules/language.md`). The label is therefore passed in at every call site,
 * which overrides it — rather than edited here, where an update would silently put the English
 * back.
 *
 * Everything below this header is the registry's own source, left exactly as it arrived so
 * that re-running `pnpm dlx shadcn@latest add spinner --overwrite` produces a diff a reviewer
 * can read. Nothing this project decides is written in here: the colours it names live in
 * `src/styles/tokens.css`, and which of its variants a screen may draw is in the rule.
 */

import { Loader2Icon } from "lucide-react"

import { cn } from "@/lib/cn"

function Spinner({ className, ...props }: React.ComponentProps<"svg">) {
  return (
    <Loader2Icon
      role="status"
      aria-label="Loading"
      className={cn("size-4 animate-spin", className)}
      {...props}
    />
  )
}

export { Spinner }
