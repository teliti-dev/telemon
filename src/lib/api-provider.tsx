import * as React from "react"

import { detectAccess } from "./access"
import { ApiContext } from "./api-context"
import type { TelemonApi } from "./data/api"
import { MockApi } from "./data/mock"

/** Adapter Tauri dan HTTP belum ada; sementara semua lingkungan memakai data contoh. */
function createApi(): TelemonApi {
  return new MockApi()
}

export function ApiProvider({ children }: { children: React.ReactNode }) {
  const [value] = React.useState(() => ({ api: createApi(), access: detectAccess() }))
  return <ApiContext.Provider value={value}>{children}</ApiContext.Provider>
}
