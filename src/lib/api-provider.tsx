import * as React from "react"
import { Loader2Icon } from "lucide-react"

import { detectAccess } from "./access"
import { ApiContext, type ApiValue } from "./api-context"
import { HttpApi } from "./data/http"
import { MockApi } from "./data/mock"
import { t } from "./i18n"

const MOCK_KEY = "telemon.api.mock"

/** `?mock` (atau `?mock=0`) memaksa data contoh (atau kembali ke server) dan diingat selama tab terbuka. */
function forcedMock(): boolean {
  try {
    const q = new URLSearchParams(window.location.search)
    if (q.has("mock")) {
      if (q.get("mock") === "0") sessionStorage.removeItem(MOCK_KEY)
      else sessionStorage.setItem(MOCK_KEY, "1")
    }
    return sessionStorage.getItem(MOCK_KEY) === "1"
  } catch {
    return false
  }
}

/**
 * Memilih sumber data:
 * - aplikasi desktop (Tauri): webview memuat UI dari server loopback aplikasi (ADR 0021); `/api/health` melaporkan
 *   `desktop: true`, jadi adapter HTTP dipakai sebagai pemilik (tanpa login);
 * - browser yang dilayani server Telemon (`/api/health` menjawab): adapter HTTP (data nyata, wajib login);
 * - selain itu (mis. `bun run dev` tanpa server): data contoh.
 */
async function createApi(): Promise<ApiValue> {
  if (!forcedMock()) {
    try {
      const res = await fetch("/api/health", { cache: "no-store" })
      const body = res.ok ? ((await res.json()) as { ok?: boolean; desktop?: boolean }) : null
      // `desktop: true` hanya bila server loopback aplikasi desktop mengenali token peluncuran ini.
      if (body?.ok === true) return { api: new HttpApi(), access: body.desktop === true ? "desktop" : "remote" }
    } catch {
      /* tidak ada server: pakai data contoh */
    }
  }
  return { api: new MockApi(), access: detectAccess() }
}

export function ApiProvider({ children }: { children: React.ReactNode }) {
  const [value, setValue] = React.useState<ApiValue | null>(null)
  React.useEffect(() => {
    let cancelled = false
    void createApi().then((v) => !cancelled && setValue(v))
    return () => {
      cancelled = true
    }
  }, [])
  if (!value) {
    return (
      <div className="flex min-h-svh items-center justify-center text-muted-foreground" role="status" aria-label={t("Memuat")}>
        <Loader2Icon className="size-5 animate-spin" />
      </div>
    )
  }
  return <ApiContext.Provider value={value}>{children}</ApiContext.Provider>
}
