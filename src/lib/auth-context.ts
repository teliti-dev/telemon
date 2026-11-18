import * as React from "react"

import type { SessionInfo } from "./types"

export interface AuthValue {
  /** Null di aplikasi desktop (tanpa login). */
  session: SessionInfo | null
  setSession: (s: SessionInfo) => void
  logout: () => Promise<void>
}

export const AuthContext = React.createContext<AuthValue | undefined>(undefined)

export function useAuth() {
  const v = React.useContext(AuthContext)
  if (!v) throw new Error("useAuth harus dipakai di dalam AuthGate")
  return v
}
