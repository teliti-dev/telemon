import * as React from "react"

import {
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { cn } from "@/lib/utils"

/**
 * Formulir dipakai di dua tempat: dialog (desktop) dan halaman penuh (HP). Bagian formulir yang
 * berbeda antar keduanya (judul, tombol bawah) memakai komponen di sini, bukan Dialog* langsung.
 * Bawaan "dialog"; halaman membungkus formulir dengan <FormLayoutProvider layout="page">.
 */
export type FormLayout = "dialog" | "page"

const FormLayoutContext = React.createContext<FormLayout>("dialog")

export function FormLayoutProvider({ layout, children }: { layout: FormLayout; children: React.ReactNode }) {
  return <FormLayoutContext.Provider value={layout}>{children}</FormLayoutContext.Provider>
}

export const useFormLayout = () => React.useContext(FormLayoutContext)

/** Dialog: judul dan keterangan. Halaman: hanya keterangan (judul ada di header halaman). */
export function FormHeader({ title, description }: { title: React.ReactNode; description?: React.ReactNode }) {
  const layout = useFormLayout()
  if (layout === "page") {
    return description ? <p className="text-sm text-muted-foreground">{description}</p> : null
  }
  return (
    <DialogHeader>
      <DialogTitle>{title}</DialogTitle>
      {description ? <DialogDescription>{description}</DialogDescription> : null}
    </DialogHeader>
  )
}

/** Dialog: footer biasa. Halaman: bilah tombol menempel di atas menu bawah. */
export function FormFooter({ children, className }: { children: React.ReactNode; className?: string }) {
  const layout = useFormLayout()
  if (layout === "page") {
    return (
      <div
        className={cn(
          "sticky bottom-[calc(3.5rem+env(safe-area-inset-bottom))] z-10 -mx-4 flex flex-col-reverse gap-2 border-t bg-background/95 px-4 py-3 backdrop-blur sm:flex-row sm:justify-end md:bottom-0",
          className
        )}
      >
        {children}
      </div>
    )
  }
  return <DialogFooter className={className}>{children}</DialogFooter>
}
