/**
 * The name of a control, tied to it. Vendored from shadcn/ui —
 * `.agents/rules/interface-elements.md`.
 *
 * Everything below this header is the registry's own source, left exactly as it arrived so
 * that re-running `pnpm dlx shadcn@latest add label --overwrite` produces a diff a reviewer
 * can read. Nothing this project decides is written in here: the colours it names live in
 * `src/styles/tokens.css`, and which of its variants a screen may draw is in the rule.
 */

"use client"

import * as React from "react"
import { Label as LabelPrimitive } from "radix-ui"

import { cn } from "@/lib/cn"

function Label({
  className,
  ...props
}: React.ComponentProps<typeof LabelPrimitive.Root>) {
  return (
    <LabelPrimitive.Root
      data-slot="label"
      className={cn(
        "flex items-center gap-2 text-sm leading-none font-medium select-none group-data-[disabled=true]:pointer-events-none group-data-[disabled=true]:opacity-50 peer-disabled:cursor-not-allowed peer-disabled:opacity-50",
        className
      )}
      {...props}
    />
  )
}

export { Label }
