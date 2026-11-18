import * as React from "react"

import type { TelemonApi } from "./data/api"
import type { PppEvent, PppUser, RouterInfo, ShareState, UserMeta } from "./types"

export interface StoreState {
  status: "loading" | "ready" | "error"
  error?: string
  routers: RouterInfo[]
  users: PppUser[]
  events: PppEvent[]
  metas: Record<string, UserMeta>
  share?: ShareState
}

export interface StoreValue extends StoreState {
  api: TelemonApi
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
