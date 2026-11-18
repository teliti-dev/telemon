import * as React from "react"

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Separator } from "@/components/ui/separator"
import { Sheet, SheetContent, SheetDescription, SheetTitle } from "@/components/ui/sheet"
import { useIsDesktop } from "@/hooks/use-media-query"
import { useT } from "@/lib/i18n"
import { cn } from "@/lib/utils"

/**
 * Menu aksi baris: dropdown di desktop, lembar bawah (modal) di HP dengan target ketuk besar.
 * API meniru DropdownMenu sehingga pemakai cukup mengganti nama komponen.
 */
const Ctx = React.createContext<{ open: boolean; setOpen: (o: boolean) => void } | null>(null)

export function ActionMenu({ children }: { children: React.ReactNode }) {
  const isDesktop = useIsDesktop()
  const [open, setOpen] = React.useState(false)
  if (isDesktop) return <DropdownMenu>{children}</DropdownMenu>
  return <Ctx.Provider value={{ open, setOpen }}>{children}</Ctx.Provider>
}

export function ActionMenuTrigger({ children }: { asChild?: boolean; children: React.ReactElement<{ onClick?: (e: React.MouseEvent) => void }> }) {
  const ctx = React.useContext(Ctx)
  if (!ctx) return <DropdownMenuTrigger asChild>{children}</DropdownMenuTrigger>
  return React.cloneElement(children, {
    onClick: (e: React.MouseEvent) => {
      children.props.onClick?.(e)
      ctx.setOpen(true)
    },
  })
}

export function ActionMenuContent({
  children,
  onClick,
}: {
  align?: "start" | "center" | "end"
  onClick?: (e: React.MouseEvent) => void
  children: React.ReactNode
}) {
  const t = useT()
  const ctx = React.useContext(Ctx)
  if (!ctx) return <DropdownMenuContent align="end" onClick={onClick}>{children}</DropdownMenuContent>
  return (
    <Sheet open={ctx.open} onOpenChange={ctx.setOpen}>
      <SheetContent side="bottom" showCloseButton={false} className="gap-0 rounded-t-2xl p-2 pb-[calc(0.5rem+env(safe-area-inset-bottom))]" onClick={onClick}>
        <SheetTitle className="sr-only">{t("Aksi")}</SheetTitle>
        <SheetDescription className="sr-only">{t("Pilih aksi")}</SheetDescription>
        <div className="mx-auto mb-1 h-1 w-9 rounded-full bg-muted" aria-hidden />
        <div className="grid">{children}</div>
      </SheetContent>
    </Sheet>
  )
}

export function ActionMenuItem({
  children,
  disabled,
  variant,
  onSelect,
}: {
  children: React.ReactNode
  disabled?: boolean
  variant?: "default" | "destructive"
  onSelect?: () => void
}) {
  const ctx = React.useContext(Ctx)
  if (!ctx) {
    return (
      <DropdownMenuItem disabled={disabled} variant={variant} onSelect={onSelect}>
        {children}
      </DropdownMenuItem>
    )
  }
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={() => {
        ctx.setOpen(false)
        onSelect?.()
      }}
      className={cn(
        "flex min-h-12 items-center gap-3 rounded-lg px-3 text-left text-sm outline-none active:bg-muted disabled:opacity-50 [&_svg]:size-4 [&_svg]:shrink-0",
        variant === "destructive" ? "text-destructive" : ""
      )}
    >
      {children}
    </button>
  )
}

export function ActionMenuSeparator() {
  const ctx = React.useContext(Ctx)
  return ctx ? <Separator className="my-1" /> : <DropdownMenuSeparator />
}
