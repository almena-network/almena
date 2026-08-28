/**
 * A field for more than one line of text. Vendored from shadcn/ui, the registry every element
 * in this interface comes from.
 *
 * Everything below this header is the registry's own source, with **one** change, so that
 * re-running `pnpm dlx shadcn@latest add textarea --overwrite` produces a diff a reviewer can
 * read. Nothing else this project decides is written in here: the colours it names live in
 * `src/styles/tokens.css`, and a screen draws only the few of its variants this project has
 * given a meaning, never everything the registry ships.
 *
 * The change is `md:text-sm` becoming `expanded:text-sm`. There is one breakpoint in this
 * project and it is `expanded:` at 600 points — Tailwind's own five are cleared from the theme
 * in `src/styles/tokens.css`, so `md:` here draws **nothing at all** and the field would
 * sit at 15 points everywhere. The registry's intent is kept exactly: the larger size on a
 * narrow viewport, which is what stops a phone zooming the page when somebody taps into a
 * field, and the body size once there is room.
 *
 * It is the one element in this set a person types into, which makes it the one place the
 * document-wide refusal to let text be selected is lifted, because a caret is a selection. That
 * exception is already written, as an element rule in `src/styles/base.css`, so there is
 * nothing to do here.
 */

import * as React from "react"

import { cn } from "@/lib/cn"

function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "flex field-sizing-content min-h-16 w-full rounded-md border border-input bg-transparent px-3 py-2 text-base shadow-xs transition-[color,box-shadow] outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-destructive/20 expanded:text-sm dark:bg-input/30 dark:aria-invalid:ring-destructive/40",
        className
      )}
      {...props}
    />
  )
}

export { Textarea }
