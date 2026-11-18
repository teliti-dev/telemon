import { ArrowRightIcon } from "lucide-react"

import { useT } from "@/lib/i18n"

export interface Change {
  label: string
  /** Kosong untuk data baru. */
  from?: string
  to: string
}

/** Ringkasan sebelum menulis ke router: apa yang akan berubah. */
export function ReviewChanges({ changes }: { changes: Change[] }) {
  const t = useT()
  return (
    <ul className="divide-y rounded-lg border text-[13px]">
      {changes.map((c) => (
        <li key={c.label} className="flex items-baseline justify-between gap-3 px-3 py-1.5">
          <span className="shrink-0 text-muted-foreground">{t(c.label) /* i18n-ignore: etiket sudah berupa kunci (tk) */}</span>
          <span className="flex min-w-0 items-center gap-1.5 text-right font-mono">
            {c.from !== undefined && (
              <>
                <span className="truncate text-muted-foreground line-through">{c.from}</span>
                <ArrowRightIcon className="size-3 shrink-0 text-muted-foreground" />
              </>
            )}
            <span className="truncate">{c.to}</span>
          </span>
        </li>
      ))}
    </ul>
  )
}
