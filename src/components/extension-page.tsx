import { PuzzleIcon } from "lucide-react"

import { ExtensionBoundary } from "@/components/extension-boundary"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { extensionPages, findExtensionPage, useExtensionState } from "@/lib/extensions/registry"
import { useT } from "@/lib/i18n"
import { pathHref } from "@/lib/route"

/** Halaman milik extension (`#/ext/<id>[/<halaman>]`): dasbor extension atau halaman biasa (ADR 0028). */
export function ExtensionPage({ extId, pageId }: { extId: string; pageId?: string }) {
  const t = useT()
  const state = useExtensionState()
  const page = findExtensionPage(extId, pageId)
  const siblings = extensionPages().filter((p) => p.extId === extId)

  if (!state.ready) return null
  if (!page) {
    return (
      <Alert className="max-w-lg">
        <PuzzleIcon />
        <AlertTitle>{t("Halaman extension tidak ditemukan")}</AlertTitle>
        <AlertDescription>{t("Extension mati, tidak terpasang, atau peran Anda tidak punya akses ke sana.")}</AlertDescription>
      </Alert>
    )
  }
  const Comp = page.Component
  return (
    <div className="grid gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="text-lg font-semibold">{page.title}</h2>
        <span className="text-xs text-muted-foreground">{page.extName}</span>
        {siblings.length > 1 && (
          <nav className="ml-auto flex flex-wrap gap-2 text-sm">
            {siblings.map((p) => (
              <a key={p.id} href={pathHref("ext", [extId, p.id])} className={p.id === page.id ? "text-brand" : "text-muted-foreground hover:text-foreground"}>
                {p.title}
              </a>
            ))}
          </nav>
        )}
      </div>
      <ExtensionBoundary key={`${extId}/${page.id}`} name={page.extName} fallback={(n) => <p className="text-sm text-muted-foreground">{t("Extension {name} gagal ditampilkan", { name: n })}</p>}>
        <Comp pageId={page.id} />
      </ExtensionBoundary>
    </div>
  )
}
