import * as React from "react"
import { Loader2Icon, TriangleAlertIcon } from "lucide-react"

import { AuthGate } from "@/components/auth-gate"
import { BottomNav } from "@/components/bottom-nav"
import { SiteHeader } from "@/components/site-header"
import { UserDetailSheet } from "@/components/user-detail-sheet"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import { ApiProvider } from "@/lib/api-provider"
import { useRoute, type Page } from "@/lib/route"
import { StoreProvider } from "@/lib/store"
import { useStore } from "@/lib/store-context"
import { AccountPage } from "@/pages/account"
import { EventsPage } from "@/pages/events"
import { OverviewPage } from "@/pages/overview"
import { ProfilesPage } from "@/pages/profiles"
import { RoutersPage } from "@/pages/routers"
import { SharePage } from "@/pages/share"
import { UsersPage } from "@/pages/users"

const TITLES: Record<Page, string> = {
  ringkasan: "Ringkasan",
  user: "Users",
  profile: "Profile",
  event: "Event",
  router: "Router",
  berbagi: "Berbagi",
  akun: "Akun saya",
}

function Content() {
  const { page, params, hash } = useRoute()
  const { status, error, reload } = useStore()
  const [openUser, setOpenUser] = React.useState<string | null>(null)

  React.useEffect(() => {
    document.title = `${TITLES[page]} · Telemon`
  }, [page])

  let body: React.ReactNode
  if (status === "loading") {
    body = (
      <div className="flex items-center gap-2 py-16 text-sm text-muted-foreground">
        <Loader2Icon className="size-4 animate-spin" /> Memuat data router…
      </div>
    )
  } else if (status === "error") {
    body = (
      <Alert variant="destructive" className="max-w-lg">
        <TriangleAlertIcon />
        <AlertTitle>Gagal memuat data</AlertTitle>
        <AlertDescription>
          <p>{error}</p>
          <Button variant="outline" size="sm" className="mt-2" onClick={reload}>
            Coba lagi
          </Button>
        </AlertDescription>
      </Alert>
    )
  } else {
    // key = hash: tautan dengan filter baru (mis. dari Ringkasan) mereset state halaman.
    body = {
      ringkasan: <OverviewPage onOpenUser={setOpenUser} />,
      user: <UsersPage key={hash} params={params} onOpenUser={setOpenUser} />,
      profile: <ProfilesPage key={hash} params={params} />,
      event: <EventsPage key={hash} params={params} onOpenUser={setOpenUser} />,
      router: <RoutersPage />,
      berbagi: <SharePage />,
      akun: <AccountPage />,
    }[page]
  }

  return (
    <div className="flex min-h-svh flex-col">
      <SiteHeader title={TITLES[page]} />
      <main className="mx-auto w-full max-w-screen-2xl flex-1 p-3 pb-[calc(4.5rem+env(safe-area-inset-bottom))] md:p-4">
        {body}
      </main>
      <BottomNav />
      <UserDetailSheet userId={openUser} onOpenChange={(o) => !o && setOpenUser(null)} />
    </div>
  )
}

export function App() {
  return (
    <TooltipProvider>
      <ApiProvider>
        <AuthGate>
          <StoreProvider>
            <Content />
            <Toaster position="top-center" />
          </StoreProvider>
        </AuthGate>
      </ApiProvider>
    </TooltipProvider>
  )
}

export default App
