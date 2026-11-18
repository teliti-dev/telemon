import * as React from "react"
import { Loader2Icon } from "lucide-react"

import { useApi } from "@/lib/api-context"
import { AuthContext } from "@/lib/auth-context"
import type { SessionInfo } from "@/lib/types"
import { LoginPage } from "@/pages/login"

type State =
  | { status: "loading" }
  | { status: "anon" }
  | { status: "authed"; session: SessionInfo | null }

/**
 * Aplikasi desktop langsung masuk (tanpa login). Akses jarak jauh (browser lewat
 * Serve LAN atau Publik) wajib login, dengan 2FA bila diwajibkan.
 */
export function AuthGate({ children }: { children: React.ReactNode }) {
  const { api, access } = useApi()
  const [state, setState] = React.useState<State>(
    access === "desktop" ? { status: "authed", session: null } : { status: "loading" }
  )

  React.useEffect(() => {
    if (access === "desktop") return
    let cancelled = false
    api
      .session()
      .then((s) => !cancelled && setState(s ? { status: "authed", session: s } : { status: "anon" }))
      .catch(() => !cancelled && setState({ status: "anon" }))
    return () => {
      cancelled = true
    }
  }, [api, access])

  const value = React.useMemo(
    () => ({
      session: state.status === "authed" ? state.session : null,
      setSession: (s: SessionInfo) => setState({ status: "authed", session: s }),
      logout: async () => {
        await api.logout().catch(() => undefined)
        setState({ status: "anon" })
      },
    }),
    [state, api]
  )

  if (state.status === "loading") {
    return (
      <div className="flex min-h-svh items-center justify-center text-sm text-muted-foreground">
        <Loader2Icon className="mr-2 size-4 animate-spin" /> Memeriksa sesi…
      </div>
    )
  }
  if (state.status === "anon") {
    return <LoginPage onAuthed={value.setSession} />
  }
  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>
}
