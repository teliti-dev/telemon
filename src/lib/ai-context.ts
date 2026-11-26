import * as React from "react"

import type { AiChatSummary, AiMessage, AiWidget } from "./types"

export type AiView = "chat" | "history"

export interface AiChatValue {
  /** AI menyala dan peran punya akses; bila false tombol AI disembunyikan. */
  available: boolean
  unavailableReason: string | null
  /** Data sensitif (IP, MAC) disamarkan di jawaban. */
  masked: boolean
  open: boolean
  setOpen: (open: boolean) => void
  /** Panel menempel di kanan layar lebar (mendorong isi halaman). */
  docked: boolean
  view: AiView
  setView: (v: AiView) => void

  // Percakapan yang sedang dibuka
  title: string
  messages: AiMessage[]
  busy: boolean
  send: (text: string) => void
  stop: () => void
  /** Ubah satu widget di jawaban (mis. usulan jadi `applied`) dan simpan ke riwayat. */
  patchWidget: (messageId: string, index: number, patch: Partial<AiWidget>) => void
  newChat: () => void
  /** Buka panel, mulai percakapan baru, dan langsung bertanya (pintasan dari halaman lain). */
  ask: (text: string) => void
  regenerate: () => void
  setFeedback: (messageId: string, value: "up" | "down" | undefined) => void
  exportMarkdown: () => string

  // Riwayat
  history: AiChatSummary[]
  historyLoading: boolean
  loadHistory: () => void
  openChat: (id: string) => Promise<void>
  renameChat: (id: string, title: string) => Promise<void>
  togglePin: (id: string) => Promise<void>
  deleteChat: (id: string) => Promise<void>
  currentId: string
}

export const AiChatContext = React.createContext<AiChatValue | undefined>(undefined)

export function useAiChat() {
  const v = React.useContext(AiChatContext)
  // i18n-ignore: pesan untuk developer
  if (!v) throw new Error("useAiChat harus dipakai di dalam AiChatProvider")
  return v
}
