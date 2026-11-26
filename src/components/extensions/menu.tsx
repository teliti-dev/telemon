import { ChevronDownIcon, PuzzleIcon } from "lucide-react"

import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { extensionPages, useExtensionState, type ExtPageEntry } from "@/lib/extensions/registry"
import { useT } from "@/lib/i18n"
import { navigatePath, useRoute } from "@/lib/route"
import { cn } from "@/lib/utils"

/** Halaman extension yang aktif dan boleh dibuka akun ini; kosong bila tidak ada. Memperbarui diri saat extension dimuat ulang. */
export function useExtensionMenuPages(): ExtPageEntry[] {
  useExtensionState()
  return extensionPages()
}

/** Teks menu: nama extension, ditambah judul halaman bila extension punya lebih dari satu halaman. */
export function pageLabel(p: ExtPageEntry, all: ExtPageEntry[]) {
  return all.filter((x) => x.extId === p.extId).length > 1 ? `${p.extName} · ${p.title}` : p.extName
}

/**
 * Menu "Extension" di menu atas: satu entri per halaman extension yang aktif (mis. Terminal, Router health).
 * Tidak tampil bila belum ada halaman extension aktif untuk akun ini.
 */
export function ExtensionsMenu() {
  const t = useT()
  const pages = useExtensionMenuPages()
  const { page, segments } = useRoute()
  if (pages.length === 0) return null
  const active = page === "ext"
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-current={active ? "page" : undefined}
          className={cn(
            "inline-flex h-9 items-center gap-1.5 border-b-2 px-2 text-sm font-medium transition-colors outline-none focus-visible:bg-muted",
            active ? "border-brand text-brand" : "border-transparent text-muted-foreground hover:text-foreground"
          )}
        >
          <PuzzleIcon className="size-4" />
          {t("Extension")}
          <ChevronDownIcon className="size-3.5" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-56">
        {pages.map((p) => (
          <DropdownMenuItem key={`${p.extId}/${p.id}`} className={cn(active && segments[0] === p.extId && (segments[1] ?? p.id) === p.id && "text-brand")} onSelect={() => navigatePath("ext", [p.extId, p.id])}>
            <PuzzleIcon /> {pageLabel(p, pages)}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
