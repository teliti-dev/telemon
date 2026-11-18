import * as React from "react"

import type { Access } from "./access"
import type { TelemonApi } from "./data/api"

export interface ApiValue {
  api: TelemonApi
  /** Desktop (tanpa login, boleh menulis ke router) atau jarak jauh (login, izin mengikuti peran). */
  access: Access
}

export const ApiContext = React.createContext<ApiValue | undefined>(undefined)

export function useApi() {
  const v = React.useContext(ApiContext)
  if (!v) throw new Error("useApi harus dipakai di dalam ApiProvider")
  return v
}
