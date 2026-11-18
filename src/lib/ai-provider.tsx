import * as React from "react"

import { useIsDesktop } from "@/hooks/use-media-query"
import { useAuth } from "@/lib/auth-context"
import { t } from "@/lib/i18n"
import { useStore } from "@/lib/store-context"
import type { AiChat, AiChatSummary, AiMessage, AiStreamEvent } from "@/lib/types"

import { AiChatContext, type AiView } from "./ai-context"

const uid = () => (typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : Math.random().toString(36).slice(2))

interface Meta {
  id: string
  title: string
  pinned: boolean
  createdAt: number
}

const freshMeta = (): Meta => ({ id: uid(), title: "", pinned: false, createdAt: Date.now() })

function apply(m: AiMessage, ev: AiStreamEvent): AiMessage {
  switch (ev.type) {
    case "step":
      return { ...m, steps: [...(m.steps ?? []), ev.text] }
    case "text":
      return { ...m, text: m.text + ev.text }
    case "widget":
      return { ...m, widgets: [...(m.widgets ?? []), ev.widget] }
    case "actions":
      return { ...m, actions: ev.actions }
    case "followups":
      return { ...m, followUps: ev.items }
    case "sources":
      return { ...m, sources: ev.items }
  }
}

const upsert = (list: AiChatSummary[], s: AiChatSummary) =>
  [...list.filter((x) => x.id !== s.id), s].sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updatedAt - a.updatedAt)

/**
 * Status AI chat. Percakapan disimpan lewat api (data Telemon per akun), bukan di browser,
 * sehingga riwayat tersedia lagi setelah halaman dimuat ulang dan mengikuti retensi.
 */
export function AiChatProvider({ children }: { children: React.ReactNode }) {
  const { api, access, share } = useStore()
  const { session } = useAuth()
  const isDesktop = useIsDesktop()
  const [open, setOpen] = React.useState(false)
  const [view, setView] = React.useState<AiView>("chat")
  const [messages, setMessages] = React.useState<AiMessage[]>([])
  const [meta, setMeta] = React.useState<Meta>(freshMeta)
  const [busy, setBusy] = React.useState(false)
  const [history, setHistory] = React.useState<AiChatSummary[]>([])
  const [historyLoading, setHistoryLoading] = React.useState(false)
  const messagesRef = React.useRef<AiMessage[]>([])
  const metaRef = React.useRef<Meta>(meta)
  const busyRef = React.useRef(false)
  const abortRef = React.useRef<AbortController | null>(null)

  const commit = React.useCallback((next: AiMessage[]) => {
    messagesRef.current = next
    setMessages(next)
  }, [])
  const commitMeta = React.useCallback((next: Meta) => {
    metaRef.current = next
    setMeta(next)
  }, [])

  const featureOn = share?.features.ai !== false
  const permitted = access === "desktop" || (session?.permissions.ai ?? "none") !== "none"
  const available = featureOn && permitted
  const unavailableReason = !featureOn
    ? t("AI dimatikan oleh pemilik Telemon.")
    : !permitted
      ? t("Peran Anda tidak punya akses ke AI chat.")
      : null

  const persist = React.useCallback(
    (msgs: AiMessage[]) => {
      const m = metaRef.current
      if (msgs.length === 0) return
      const chat: AiChat = { id: m.id, title: m.title || t("Percakapan baru"), pinned: m.pinned, createdAt: m.createdAt, updatedAt: Date.now(), messages: msgs }
      api
        .saveAiChat(chat)
        .then((s) => setHistory((h) => upsert(h, s)))
        .catch(() => undefined)
    },
    [api]
  )

  const run = React.useCallback(
    async (base: AiMessage[], q: string) => {
      const user: AiMessage = { id: uid(), role: "user", text: q, at: Date.now() }
      const reply: AiMessage = { id: uid(), role: "assistant", text: "", at: Date.now() }
      const history_ = [...base, user]
      let local = reply
      commit([...history_, reply])
      if (!metaRef.current.title) commitMeta({ ...metaRef.current, title: q.slice(0, 48) })
      busyRef.current = true
      setBusy(true)
      const ctrl = new AbortController()
      abortRef.current = ctrl
      const patch = (next: AiMessage) => {
        local = next
        commit(messagesRef.current.map((x) => (x.id === reply.id ? next : x)))
      }
      try {
        for await (const ev of api.askAi(history_, { signal: ctrl.signal })) patch(apply(local, ev))
      } catch (e) {
        if (!ctrl.signal.aborted) {
          const error = e instanceof Error ? e.message : String(e)
          patch({ ...local, error: true, text: t("Gagal mendapat jawaban: {error}", { error }) })
        }
      } finally {
        busyRef.current = false
        setBusy(false)
        abortRef.current = null
        persist(messagesRef.current)
      }
    },
    [api, commit, commitMeta, persist]
  )

  const send = React.useCallback(
    (text: string) => {
      const q = text.trim()
      if (!q || busyRef.current) return
      setView("chat")
      void run(messagesRef.current, q)
    },
    [run]
  )

  const stop = React.useCallback(() => abortRef.current?.abort(), [])

  const newChat = React.useCallback(() => {
    abortRef.current?.abort()
    commit([])
    commitMeta(freshMeta())
    setView("chat")
  }, [commit, commitMeta])

  const ask = React.useCallback(
    (text: string) => {
      if (busyRef.current) abortRef.current?.abort()
      commit([])
      commitMeta(freshMeta())
      setOpen(true)
      setView("chat")
      queueMicrotask(() => void run([], text.trim()))
    },
    [commit, commitMeta, run]
  )

  const regenerate = React.useCallback(() => {
    if (busyRef.current) return
    const msgs = messagesRef.current
    let i = msgs.length - 1
    while (i >= 0 && msgs[i].role !== "user") i--
    if (i < 0) return
    void run(msgs.slice(0, i), msgs[i].text)
  }, [run])

  const setFeedback = React.useCallback(
    (messageId: string, value: "up" | "down" | undefined) => {
      commit(messagesRef.current.map((m) => (m.id === messageId ? { ...m, feedback: value } : m)))
      persist(messagesRef.current)
    },
    [commit, persist]
  )

  const exportMarkdown = React.useCallback(() => {
    const body = messagesRef.current
      .map((m) => `**${m.role === "user" ? t("Anda") : t("AI")}:** ${m.text}`)
      .join("\n\n")
    return `# ${metaRef.current.title || t("Percakapan baru")}\n\n${body}\n`
  }, [])

  const loadHistory = React.useCallback(() => {
    setHistoryLoading(true)
    api
      .listAiChats()
      .then(setHistory)
      .catch(() => undefined)
      .finally(() => setHistoryLoading(false))
  }, [api])

  const openChat = React.useCallback(
    async (id: string) => {
      abortRef.current?.abort()
      const c = await api.getAiChat(id)
      commit(c.messages)
      commitMeta({ id: c.id, title: c.title, pinned: c.pinned, createdAt: c.createdAt })
      setView("chat")
    },
    [api, commit, commitMeta]
  )

  const saveMeta = React.useCallback(
    async (id: string, change: Partial<Pick<AiChat, "title" | "pinned">>) => {
      const c = await api.getAiChat(id)
      const s = await api.saveAiChat({ ...c, ...change })
      setHistory((h) => upsert(h, s))
      if (metaRef.current.id === id) commitMeta({ ...metaRef.current, ...change })
    },
    [api, commitMeta]
  )

  const renameChat = React.useCallback((id: string, title: string) => saveMeta(id, { title: title.trim() || t("Percakapan baru") }), [saveMeta])
  const togglePin = React.useCallback(
    (id: string) => saveMeta(id, { pinned: !history.find((h) => h.id === id)?.pinned }),
    [saveMeta, history]
  )
  const deleteChat = React.useCallback(
    async (id: string) => {
      await api.deleteAiChat(id)
      setHistory((h) => h.filter((x) => x.id !== id))
      if (metaRef.current.id === id) newChat()
    },
    [api, newChat]
  )

  // Pintasan keyboard: Alt+A membuka/menutup AI chat.
  React.useEffect(() => {
    if (!available) return
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey && !e.ctrlKey && !e.metaKey && e.code === "KeyA") {
        e.preventDefault()
        setOpen((o) => !o)
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [available])

  const value = React.useMemo(
    () => ({
      available,
      unavailableReason,
      masked: share?.features.aiMask !== false,
      open,
      setOpen,
      docked: open && available && isDesktop,
      view,
      setView,
      title: meta.title,
      messages,
      busy,
      send,
      stop,
      newChat,
      ask,
      regenerate,
      setFeedback,
      exportMarkdown,
      history,
      historyLoading,
      loadHistory,
      openChat,
      renameChat,
      togglePin,
      deleteChat,
      currentId: meta.id,
    }),
    [available, unavailableReason, share?.features.aiMask, open, isDesktop, view, meta, messages, busy, send, stop, newChat, ask, regenerate, setFeedback, exportMarkdown, history, historyLoading, loadHistory, openChat, renameChat, togglePin, deleteChat]
  )

  return <AiChatContext.Provider value={value}>{children}</AiChatContext.Provider>
}
