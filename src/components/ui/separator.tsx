/**
 * The rule between two things. Vendored from shadcn/ui —
 * `.agents/rules/interface-elements.md`.
 *
 * Everything below this header is the registry's own source, left exactly as it arrived so
 * that re-running `pnpm dlx shadcn@latest add separator --overwrite` produces a diff a reviewer
 * can read. Nothing this project decides is written in here: the colours it names live in
 * `src/styles/tokens.css`, and which of its variants a screen may draw is in the rule.
 */

import * as React from "react"
import { Separator as SeparatorPrimitive } from "radix-ui"

import { cn } from "@/lib/cn"

function Separator({
  className,
  orientation = "horizontal",
  decorative = true,
  ...props
}: React.ComponentProps<typeof SeparatorPrimitive.Root>) {
  return (
    <SeparatorPrimitive.Root
      data-slot="separator"
      decorative={decorative}
      orientation={orientation}
      className={cn(
        "shrink-0 bg-border data-[orientation=horizontal]:h-px data-[orientation=horizontal]:w-full data-[orientation=vertical]:h-full data-[orientation=vertical]:w-px",
        className
      )}
      {...props}
    />
  )
}

export { Separator }
