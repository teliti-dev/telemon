import { ArrowLeftIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { useT } from "@/lib/i18n"

/** Header halaman bertingkat (detail, formulir): tombol kembali, judul, dan aksi di kanan. */
export function PageHeader({
  title,
  subtitle,
  onBack,
  actions,
}: {
  title: React.ReactNode
  subtitle?: React.ReactNode
  onBack: () => void
  actions?: React.ReactNode
}) {
  const t = useT()
  return (
    <div className="mb-3 flex items-center gap-2">
      <Button variant="ghost" size="icon" onClick={onBack} aria-label={t("Kembali")} title={t("Kembali")} className="-ml-2 shrink-0">
        <ArrowLeftIcon />
      </Button>
      <div className="min-w-0 flex-1">
        <h2 className="truncate text-base font-semibold">{title}</h2>
        {subtitle ? <p className="truncate text-xs text-muted-foreground">{subtitle}</p> : null}
      </div>
      {actions}
    </div>
  )
}
