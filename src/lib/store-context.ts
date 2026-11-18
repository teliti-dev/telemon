import * as React from "react"

import type { TelemonApi } from "./data/api"
import type { PppEvent, PppProfile, PppUser, RouterInfo, ShareState, UserMeta } from "./types"

export interface StoreState {
  status: "loading" | "ready" | "error"
  error?: string
  routers: RouterInfo[]
  /** Semua user, termasuk yang sudah di-soft delete. */
  users: PppUser[]
  profiles: PppProfile[]
  events: PppEvent[]
  metas: Record<string, UserMeta>
  share?: ShareState
}

export interface StoreValue extends StoreState {
  api: TelemonApi
  /** User yang belum di-soft delete. */
  liveUsers: PppUser[]
  routerById: Map<string, RouterInfo>
  reload: () => void
}

export const StoreContext = React.createContext<StoreValue | undefined>(
  undefined
)

export function useStore() {
  const v = React.useContext(StoreContext)
  if (!v) throw new Error("useStore harus dipakai di dalam StoreProvider")
  return v
}
