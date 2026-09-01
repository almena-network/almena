/**
 * The card, and the seven slots it is built out of. Vendored from shadcn/ui, the registry
 * every element in this interface comes from.
 *
 * Everything below this header is the registry's own source, with **one** change, so that
 * re-running `pnpm dlx shadcn@latest add card --overwrite` produces a diff a reviewer can
 * read. Nothing else this project decides is written in here: the colours it names live in
 * `src/styles/tokens.css`, and a screen draws only the few of its variants this project has
 * given a meaning, never everything the registry ships.
 *
 * The change is `text-base` on `CardTitle`. shadcn/ui's own body size is 16 points and a
 * title at that size is told from its description by weight alone; this project's body is 13
 * (`src/styles/tokens.css`), so a title that inherited it would be a heading nobody could
 * find. It is made here rather than passed in by every screen, because a card's title being
 * one size is the whole reason there is one card.
 */

import * as React from "react"

import { cn } from "@/lib/cn"

function Card({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card"
      className={cn(
        "flex flex-col gap-6 rounded-xl border bg-card py-6 text-card-foreground shadow-sm",
        className
      )}
      {...props}
    />
  )
}

function CardHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-header"
      className={cn(
        "@container/card-header grid auto-rows-min grid-rows-[auto_auto] items-start gap-2 px-6 has-data-[slot=card-action]:grid-cols-[1fr_auto] [.border-b]:pb-6",
        className
      )}
      {...props}
    />
  )
}

function CardTitle({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-title"
      className={cn("text-base leading-none font-semibold", className)}
      {...props}
    />
  )
}

function CardAction({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-action"
      className={cn(
        "col-start-2 row-span-2 row-start-1 self-start justify-self-end",
        className
      )}
      {...props}
    />
  )
}

function CardContent({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-content"
      className={cn("px-6", className)}
      {...props}
    />
  )
}

function CardFooter({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-footer"
      className={cn("flex items-center px-6 [.border-t]:pt-6", className)}
      {...props}
    />
  )
}

export {
  Card,
  CardHeader,
  CardFooter,
  CardTitle,
  CardAction,
  CardContent,
}
