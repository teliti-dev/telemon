import * as React from "react"
import {
  CopyIcon,
  DownloadIcon,
  HistoryIcon,
  MessageSquarePlusIcon,
  RefreshCwIcon,
  SendIcon,
  ShieldCheckIcon,
  SparklesIcon,
  SquareIcon,
  ThumbsDownIcon,
  ThumbsUpIcon,
  XIcon,
} from "lucide-react"
import { toast } from "sonner"

import { AiHistory } from "@/components/ai-history"
import { AiMarkdown } from "@/components/ai-markdown"
import { AiWidgetView } from "@/components/ai-widgets"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { Textarea } from "@/components/ui/textarea"
import { copyText } from "@/hooks/use-copy"
import { useIsDesktop } from "@/hooks/use-media-query"
import { useAiChat } from "@/lib/ai-context"
import { AI_SUGGESTIONS } from "@/lib/data/ai-engine"
import { tk, useT } from "@/lib/i18n"
import { navigate, type Page } from "@/lib/route"
import type { AiLink, AiMessage } from "@/lib/types"
import { cn } from "@/lib/utils"

type Command =
  | { cmd: string; label: string; question: string }
  | { cmd: string; label: string; action: "new" | "history" | "export" }

/** Perintah cepat: ketik "/" di kolom pertanyaan. */
const COMMANDS: Command[] = [
  { cmd: "/summary", label: tk("Ringkasan jaringan"), question: tk("Berapa user aktif sekarang?") },
  { cmd: "/routers", label: tk("Router bermasalah"), question: tk("Router mana yang bermasalah?") },
  { cmd: "/auth", label: tk("Gagal auth teratas"), question: tk("Siapa yang sering gagal auth?") },
  { cmd: "/drops", label: tk("Paling sering terputus"), question: tk("Siapa yang sering putus?") },
  { cmd: "/inactive", label: tk("User tidak aktif"), question: tk("Tampilkan user tidak aktif") },
  { cmd: "/profiles", label: tk("Pemakaian profil PPP"), question: tk("Profil PPP mana yang paling banyak dipakai?") },
  { cmd: "/events", label: tk("Event terbaru"), question: tk("Tampilkan event terbaru") },
  { cmd: "/stats", label: tk("Statistik event"), question: tk("Statistik event 24 jam terakhir") },
  { cmd: "/bandwidth", label: tk("Bandwidth pelanggan"), question: tk("Berapa bandwidth pelanggan sekarang?") },
  { cmd: "/health", label: tk("Kesehatan router"), question: tk("Bagaimana kesehatan router?") },
  { cmd: "/help", label: tk("Bantuan"), question: tk("Bantuan") },
  { cmd: "/new", label: tk("Percakapan baru"), action: "new" },
  { cmd: "/history", label: tk("Riwayat percakapan"), action: "history" },
  { cmd: "/export", label: tk("Ekspor percakapan"), action: "export" },
]

function Kbd({ children }: { children: React.ReactNode }) {
  return <kbd className="rounded border bg-muted px-1 py-0.5 font-mono text-[10px] text-foreground">{children}</kbd>
}

/** Isi AI chat: percakapan, riwayat, perintah cepat, dan kolom ketik. Dipakai panel desktop dan layar penuh HP. */
export function AiChatPanel({ onClose }: { onClose: () => void }) {
  const t = useT()
  const ai = useAiChat()
  const isDesktop = useIsDesktop()
  const [draft, setDraft] = React.useState("")
  const [cmdIndex, setCmdIndex] = React.useState(0)
  const endRef = React.useRef<HTMLDivElement>(null)
  const inputRef = React.useRef<HTMLTextAreaElement>(null)

  React.useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" })
  }, [ai.messages, ai.view])

  const lastAssistant = [...ai.messages].reverse().find((m) => m.role === "assistant")
  const lastUserText = [...ai.messages].reverse().find((m) => m.role === "user")?.text

  const grow = () => {
    const el = inputRef.current
    if (!el) return
    el.style.height = "auto"
    el.style.height = `${Math.min(el.scrollHeight, 128)}px`
  }

  const palette = draft.startsWith("/") && !draft.includes(" ") ? COMMANDS.filter((c) => c.cmd.startsWith(draft.toLowerCase())) : []
  const showPalette = palette.length > 0

  const go = (to: AiLink) => {
    navigate(to.page as Page, to.params)
    if (!isDesktop) onClose()
  }

  const doExport = () => {
    const blob = new Blob([ai.exportMarkdown()], { type: "text/markdown;charset=utf-8" })
    const url = URL.createObjectURL(blob)
    const a = document.createElement("a")
    a.href = url
    a.download = `${(ai.title || "telemon-chat").replace(/[^\w.-]+/g, "-").slice(0, 40)}.md`
    a.click()
    URL.revokeObjectURL(url)
    toast.success(t("Percakapan diekspor"))
  }

  const reset = () => {
    setDraft("")
    setCmdIndex(0)
    requestAnimationFrame(grow)
  }

  const runCommand = (c: Command) => {
    reset()
    if ("question" in c) ai.send(t(c.question /* i18n-ignore: kunci tk */))
    else if (c.action === "new") ai.newChat()
    else if (c.action === "history") ai.setView("history")
    else if (ai.messages.length > 0) doExport()
  }

  const submit = () => {
    const q = draft.trim()
    if (showPalette) {
      runCommand(palette[Math.min(cmdIndex, palette.length - 1)])
      return
    }
    if (!q || ai.busy) return
    ai.send(q)
    reset()
  }

  return (
    <div
      className="flex h-full min-h-0 flex-col"
      onKeyDown={(e) => {
        if (e.key === "Escape" && !showPalette) onClose()
      }}
    >
      <header className="flex items-center gap-1 border-b px-3 py-2.5">
        <span className="mr-1 flex size-8 shrink-0 items-center justify-center rounded-lg bg-brand/10 text-brand">
          <SparklesIcon className="size-4" />
        </span>
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-sm font-semibold">{ai.view === "history" ? t("Riwayat percakapan") : ai.title || t("Tanya AI")}</h2>
          <p className="truncate text-xs text-muted-foreground">{t("Hanya membaca data Telemon; tidak mengubah apa pun.")}</p>
        </div>
        <Button
          variant={ai.view === "history" ? "secondary" : "ghost"}
          size="icon"
          onClick={() => ai.setView(ai.view === "history" ? "chat" : "history")}
          aria-label={t("Riwayat percakapan")}
          title={t("Riwayat percakapan")}
        >
          <HistoryIcon />
        </Button>
        <Button variant="ghost" size="icon" onClick={ai.newChat} aria-label={t("Percakapan baru")} title={t("Percakapan baru")}>
          <MessageSquarePlusIcon />
        </Button>
        <Button variant="ghost" size="icon" onClick={doExport} disabled={ai.messages.length === 0} aria-label={t("Ekspor percakapan")} title={t("Ekspor percakapan")}>
          <DownloadIcon />
        </Button>
        <Button variant="ghost" size="icon" onClick={onClose} aria-label={t("Tutup AI chat")} title={`${t("Tutup AI chat")} (Esc)`}>
          <XIcon />
        </Button>
      </header>

      {ai.view === "history" ? (
        <div className="min-h-0 flex-1">
          <AiHistory />
        </div>
      ) : (
        <>
          <div className="min-h-0 flex-1 overflow-y-auto px-3 py-3" aria-live="polite">
            {ai.messages.length === 0 ? (
              <div className="grid justify-items-center gap-3 px-2 pt-6 text-center">
                <span className="flex size-12 items-center justify-center rounded-2xl bg-brand/10 text-brand">
                  <SparklesIcon className="size-6" />
                </span>
                <div>
                  <p className="font-medium">{t("Apa yang ingin Anda ketahui?")}</p>
                  <p className="text-sm text-muted-foreground">{t("Pilih contoh di bawah atau ketik pertanyaan sendiri.")}</p>
                </div>
                <div className="flex flex-wrap justify-center gap-2">
                  {AI_SUGGESTIONS.map((s) => (
                    <Button
                      key={s}
                      variant="outline"
                      size="sm"
                      className="h-auto rounded-full py-1.5 whitespace-normal"
                      onClick={() => ai.send(t(s /* i18n-ignore: kunci tk */))}
                      disabled={!ai.available || ai.busy}
                    >
                      {t(s /* i18n-ignore: kunci tk */)}
                    </Button>
                  ))}
                </div>
                <ul className="mt-2 grid gap-1 text-left text-xs text-muted-foreground">
                  <li>
                    <Kbd>/</Kbd> {t("perintah cepat")}
                  </li>
                  <li>
                    {/* i18n-ignore: nama tombol keyboard */}
                    <Kbd>Enter</Kbd> {t("kirim")}, <Kbd>Shift</Kbd>+<Kbd>Enter</Kbd> {t("baris baru")}
                  </li>
                  <li>
                    <Kbd>↑</Kbd> {t("ulangi pertanyaan terakhir")}
                  </li>
                  <li>
                    {/* i18n-ignore: nama tombol keyboard */}
                    <Kbd>Alt</Kbd>+<Kbd>A</Kbd> {t("buka atau tutup AI chat")}, <Kbd>Esc</Kbd> {t("tutup")}
                  </li>
                </ul>
              </div>
            ) : (
              <ol className="grid gap-4">
                {ai.messages.map((m) => (
                  <MessageItem key={m.id} m={m} isLast={m === lastAssistant} onNavigate={go} />
                ))}
              </ol>
            )}
            <div ref={endRef} />
          </div>

          <footer className="relative border-t p-3">
            {showPalette && (
              <ul
                role="listbox"
                aria-label={t("Perintah cepat")}
                className="absolute inset-x-3 bottom-full mb-1 max-h-60 overflow-y-auto rounded-xl border bg-popover p-1 shadow-md"
              >
                {palette.map((c, i) => (
                  <li key={c.cmd} role="option" aria-selected={i === cmdIndex}>
                    <button
                      type="button"
                      onMouseEnter={() => setCmdIndex(i)}
                      onClick={() => runCommand(c)}
                      className={cn("flex w-full items-center gap-3 rounded-lg px-2 py-1.5 text-left text-sm", i === cmdIndex && "bg-muted")}
                    >
                      <span className="w-20 shrink-0 font-mono text-xs text-brand">{c.cmd}</span>
                      <span className="truncate">{t(c.label /* i18n-ignore: kunci tk */)}</span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
            {!ai.available ? (
              <p className="text-sm text-muted-foreground">{ai.unavailableReason}</p>
            ) : (
              <form
                className="flex items-end gap-2"
                onSubmit={(e) => {
                  e.preventDefault()
                  submit()
                }}
              >
                <Textarea
                  ref={inputRef}
                  rows={1}
                  value={draft}
                  onChange={(e) => {
                    setDraft(e.target.value)
                    setCmdIndex(0)
                    grow()
                  }}
                  onKeyDown={(e) => {
                    if (e.nativeEvent.isComposing) return
                    if (showPalette) {
                      if (e.key === "ArrowDown") {
                        e.preventDefault()
                        setCmdIndex((i) => (i + 1) % palette.length)
                        return
                      }
                      if (e.key === "ArrowUp") {
                        e.preventDefault()
                        setCmdIndex((i) => (i - 1 + palette.length) % palette.length)
                        return
                      }
                      if (e.key === "Escape") {
                        e.preventDefault()
                        e.stopPropagation()
                        reset()
                        return
                      }
                      if (e.key === "Tab") {
                        e.preventDefault()
                        setDraft(palette[Math.min(cmdIndex, palette.length - 1)].cmd)
                        return
                      }
                    }
                    if (e.key === "ArrowUp" && draft === "" && lastUserText) {
                      e.preventDefault()
                      setDraft(lastUserText)
                      requestAnimationFrame(grow)
                      return
                    }
                    if (e.key === "Enter" && !e.shiftKey) {
                      e.preventDefault()
                      submit()
                    }
                  }}
                  placeholder={t("Ketik pertanyaan atau / untuk perintah…")}
                  aria-label={t("Ketik pertanyaan atau / untuk perintah…")}
                  className="max-h-32 min-h-9 resize-none py-2 text-base md:text-sm"
                />
                {ai.busy ? (
                  <Button type="button" size="icon" variant="outline" onClick={ai.stop} aria-label={t("Berhenti")} title={t("Berhenti")}>
                    <SquareIcon />
                  </Button>
                ) : (
                  <Button type="submit" size="icon" disabled={!draft.trim()} aria-label={t("Kirim")} title={t("Kirim")}>
                    <SendIcon />
                  </Button>
                )}
              </form>
            )}
            <p className="mt-2 flex items-center justify-center gap-1.5 text-center text-[11px] text-muted-foreground">
              {ai.masked && <ShieldCheckIcon className="size-3 shrink-0" aria-hidden />}
              {t("AI bisa keliru. Periksa data penting sebelum bertindak.")}
              {ai.masked && ` ${t("IP dan MAC disamarkan.")}`}
            </p>
          </footer>
        </>
      )}
    </div>
  )
}

function MessageItem({ m, isLast, onNavigate }: { m: AiMessage; isLast: boolean; onNavigate: (to: AiLink) => void }) {
  const t = useT()
  const ai = useAiChat()

  if (m.role === "user") {
    return (
      <li className="flex justify-end">
        <div className="max-w-[88%] rounded-2xl rounded-br-md bg-primary px-3 py-2 text-primary-foreground">
          <span className="sr-only">{t("Anda")}: </span>
          <p className="text-sm break-words whitespace-pre-wrap">{m.text}</p>
        </div>
      </li>
    )
  }

  const pending = ai.busy && isLast
  const thinking = pending && m.text === ""
  return (
    <li className="grid gap-2">
      <div className={cn("max-w-[94%] rounded-2xl rounded-bl-md px-3 py-2", m.error ? "bg-destructive/10 text-destructive" : "bg-muted")}>
        <span className="sr-only">{t("AI")}: </span>
        {m.steps && m.steps.length > 0 && (
          <details open={pending} className="mb-1.5 text-xs text-muted-foreground">
            <summary className="cursor-pointer select-none">{pending ? m.steps[m.steps.length - 1] : t("{n} langkah", { n: m.steps.length })}</summary>
            <ol className="mt-1 list-decimal space-y-0.5 pl-5">
              {m.steps.map((s) => (
                <li key={s}>{s}</li>
              ))}
            </ol>
          </details>
        )}
        {thinking ? (
          <div className="grid w-44 gap-1.5 py-1" role="status">
            <Skeleton className="h-3 w-full" />
            <Skeleton className="h-3 w-3/5" />
            <span className="sr-only">{t("Menjawab…")}</span>
          </div>
        ) : (
          <AiMarkdown text={m.text} />
        )}
      </div>

      {m.widgets && m.widgets.length > 0 && (
        <div className="grid max-w-[94%] gap-2">
          {m.widgets.map((w, i) => (
            <AiWidgetView key={i} widget={w} onNavigate={onNavigate} onPatch={(p) => ai.patchWidget(m.id, i, p)} />
          ))}
        </div>
      )}

      {m.actions && m.actions.length > 0 && !pending && (
        <div className="flex max-w-[94%] flex-wrap gap-1.5">
          {m.actions.map((a) => (
            <Button
              key={a.id}
              size="sm"
              variant="outline"
              onClick={() => {
                if (a.to) onNavigate(a.to)
                else if (a.copy) copyText(a.copy, t("Disalin"))
              }}
            >
              {a.copy && !a.to ? <CopyIcon /> : null}
              {a.label}
            </Button>
          ))}
        </div>
      )}

      {m.sources && m.sources.length > 0 && !pending && (
        <p className="flex max-w-[94%] flex-wrap items-center gap-1 text-[11px] text-muted-foreground">
          {t("Sumber:")}
          {m.sources.map((s) => (
            <span key={s} className="rounded-full border px-1.5 py-0.5">
              {s}
            </span>
          ))}
        </p>
      )}

      {!pending && !m.error && m.text && (
        <div className="flex items-center gap-0.5 text-muted-foreground">
          <Button size="icon-sm" variant="ghost" onClick={() => copyText(m.text, t("Disalin"))} aria-label={t("Salin jawaban")} title={t("Salin jawaban")}>
            <CopyIcon />
          </Button>
          {isLast && (
            <Button size="icon-sm" variant="ghost" onClick={ai.regenerate} aria-label={t("Ulangi jawaban")} title={t("Ulangi jawaban")}>
              <RefreshCwIcon />
            </Button>
          )}
          <Button
            size="icon-sm"
            variant="ghost"
            className={cn(m.feedback === "up" && "text-brand")}
            aria-pressed={m.feedback === "up"}
            onClick={() => ai.setFeedback(m.id, m.feedback === "up" ? undefined : "up")}
            aria-label={t("Jawaban membantu")}
            title={t("Jawaban membantu")}
          >
            <ThumbsUpIcon />
          </Button>
          <Button
            size="icon-sm"
            variant="ghost"
            className={cn(m.feedback === "down" && "text-brand")}
            aria-pressed={m.feedback === "down"}
            onClick={() => ai.setFeedback(m.id, m.feedback === "down" ? undefined : "down")}
            aria-label={t("Jawaban kurang tepat")}
            title={t("Jawaban kurang tepat")}
          >
            <ThumbsDownIcon />
          </Button>
        </div>
      )}

      {isLast && !pending && m.followUps && m.followUps.length > 0 && (
        <div className="flex max-w-[94%] flex-wrap gap-1.5">
          {m.followUps.map((f) => (
            <Button key={f} size="sm" variant="outline" className="h-auto rounded-full py-1 whitespace-normal" onClick={() => ai.send(f)}>
              {f}
            </Button>
          ))}
        </div>
      )}
    </li>
  )
}
