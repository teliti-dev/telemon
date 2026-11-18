import * as React from "react"
import { CheckIcon, PencilIcon, PinIcon, PinOffIcon, SearchIcon, Trash2Icon, XIcon } from "lucide-react"
import { toast } from "sonner"

import { SkeletonRows } from "@/components/skeletons"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { useAiChat } from "@/lib/ai-context"
import { formatRelative } from "@/lib/format"
import { useT } from "@/lib/i18n"
import type { AiChatSummary } from "@/lib/types"
import { cn } from "@/lib/utils"

const DAY = 86400_000

/** Riwayat percakapan: cari, kelompokkan menurut waktu, sematkan, ganti nama, dan hapus. */
export function AiHistory() {
  const t = useT()
  const ai = useAiChat()
  const [query, setQuery] = React.useState("")
  const [editing, setEditing] = React.useState<string | null>(null)
  const [draftTitle, setDraftTitle] = React.useState("")
  const [confirming, setConfirming] = React.useState<string | null>(null)
  const [now] = React.useState(() => Date.now())

  React.useEffect(() => {
    ai.loadHistory()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const q = query.trim().toLowerCase()
  const list = ai.history.filter((c) => !q || c.title.toLowerCase().includes(q) || c.preview.toLowerCase().includes(q))

  const startOfToday = new Date(now).setHours(0, 0, 0, 0)
  const groups: { title: string; items: AiChatSummary[] }[] = [
    { title: t("Disematkan"), items: list.filter((c) => c.pinned) },
    { title: t("Hari ini"), items: list.filter((c) => !c.pinned && c.updatedAt >= startOfToday) },
    { title: t("Kemarin"), items: list.filter((c) => !c.pinned && c.updatedAt < startOfToday && c.updatedAt >= startOfToday - DAY) },
    { title: t("7 hari terakhir"), items: list.filter((c) => !c.pinned && c.updatedAt < startOfToday - DAY && c.updatedAt >= startOfToday - 7 * DAY) },
    { title: t("Lebih lama"), items: list.filter((c) => !c.pinned && c.updatedAt < startOfToday - 7 * DAY) },
  ].filter((g) => g.items.length > 0)

  const fail = (e: unknown) => toast.error(e instanceof Error ? e.message : String(e))

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="border-b p-3">
        <div className="relative">
          <SearchIcon className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("Cari percakapan…")}
            aria-label={t("Cari percakapan…")}
            className="pl-8"
          />
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {ai.historyLoading && ai.history.length === 0 ? (
          <SkeletonRows count={5} />
        ) : groups.length === 0 ? (
          <p className="p-6 text-center text-sm text-muted-foreground">{q ? t("Tidak ada percakapan yang cocok.") : t("Belum ada riwayat percakapan.")}</p>
        ) : (
          groups.map((g) => (
            <section key={g.title} className="py-1">
              <h3 className="px-3 pt-2 pb-1 text-xs font-medium text-muted-foreground">{g.title}</h3>
              <ul>
                {g.items.map((c) => (
                  <li key={c.id} className="group border-b last:border-b-0">
                    {editing === c.id ? (
                      <form
                        className="flex items-center gap-1.5 px-3 py-2"
                        onSubmit={(e) => {
                          e.preventDefault()
                          ai.renameChat(c.id, draftTitle).then(() => setEditing(null), fail)
                        }}
                      >
                        <Input value={draftTitle} onChange={(e) => setDraftTitle(e.target.value)} maxLength={80} autoFocus onKeyDown={(e) => e.key === "Escape" && setEditing(null)} aria-label={t("Nama percakapan")} />
                        <Button type="submit" size="icon" variant="outline" aria-label={t("Simpan")}>
                          <CheckIcon />
                        </Button>
                        <Button type="button" size="icon" variant="ghost" onClick={() => setEditing(null)} aria-label={t("Batal")}>
                          <XIcon />
                        </Button>
                      </form>
                    ) : (
                      <div className="flex items-start gap-1 px-1 py-1.5">
                        <button
                          type="button"
                          onClick={() => ai.openChat(c.id).catch(fail)}
                          className="min-w-0 flex-1 rounded-md px-2 py-1 text-left outline-none hover:bg-muted/60 focus-visible:bg-muted"
                        >
                          <div className="flex items-center gap-1.5">
                            {c.pinned && <PinIcon className="size-3 shrink-0 text-muted-foreground" />}
                            <span className={cn("truncate text-sm font-medium", c.id === ai.currentId && "text-brand")}>{c.title}</span>
                          </div>
                          {c.preview && <div className="truncate text-xs text-muted-foreground">{c.preview}</div>}
                          <div className="text-[11px] text-muted-foreground">
                            {formatRelative(c.updatedAt, now)} · {t("{n} pesan", { n: c.messageCount })}
                          </div>
                        </button>
                        {confirming === c.id ? (
                          <div className="flex shrink-0 items-center gap-1 pt-1">
                            <Button size="sm" variant="destructive" onClick={() => ai.deleteChat(c.id).then(() => setConfirming(null), fail)}>
                              {t("Hapus")}
                            </Button>
                            <Button size="sm" variant="ghost" onClick={() => setConfirming(null)}>
                              {t("Batal")}
                            </Button>
                          </div>
                        ) : (
                          <div className="flex shrink-0 items-center md:opacity-0 md:transition-opacity md:group-focus-within:opacity-100 md:group-hover:opacity-100">
                            <Button size="icon-sm" variant="ghost" onClick={() => ai.togglePin(c.id).catch(fail)} aria-label={c.pinned ? t("Lepas sematan") : t("Sematkan")} title={c.pinned ? t("Lepas sematan") : t("Sematkan")}>
                              {c.pinned ? <PinOffIcon /> : <PinIcon />}
                            </Button>
                            <Button
                              size="icon-sm"
                              variant="ghost"
                              onClick={() => {
                                setEditing(c.id)
                                setDraftTitle(c.title)
                              }}
                              aria-label={t("Ganti nama")}
                              title={t("Ganti nama")}
                            >
                              <PencilIcon />
                            </Button>
                            <Button size="icon-sm" variant="ghost" onClick={() => setConfirming(c.id)} aria-label={t("Hapus percakapan")} title={t("Hapus percakapan")}>
                              <Trash2Icon />
                            </Button>
                          </div>
                        )}
                      </div>
                    )}
                  </li>
                ))}
              </ul>
            </section>
          ))
        )}
      </div>
    </div>
  )
}
