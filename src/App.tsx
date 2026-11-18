import * as React from "react"
import { Loader2Icon, ShieldAlertIcon, TriangleAlertIcon } from "lucide-react"

import { AuthGate } from "@/components/auth-gate"
import { BottomNav } from "@/components/bottom-nav"
import { SiteHeader } from "@/components/site-header"
import { UserDetailSheet } from "@/components/user-detail-sheet"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import { ApiProvider } from "@/lib/api-provider"
import { PAGE_CONTAINER } from "@/lib/layout"
import { useNavItems } from "@/hooks/use-nav-items"
import { useAuth } from "@/lib/auth-context"
import { pageDenied } from "@/lib/permissions"
import { navigate, useRoute, type Page } from "@/lib/route"
import { StoreProvider } from "@/lib/store"
import { useStore } from "@/lib/store-context"
import { AccountPage } from "@/pages/account"
import { AdminPage } from "@/pages/admin"
import { EventsPage } from "@/pages/events"
import { OverviewPage } from "@/pages/overview"
import { ProfilesPage } from "@/pages/profiles"
import { RoutersPage } from "@/pages/routers"
import { SettingsPage } from "@/pages/settings"
import { SharePage } from "@/pages/share"
import { UsersPage } from "@/pages/users"

const TITLES: Record<Page, string> = {
  ringkasan: "Ringkasan",
  user: "Users",
  profile: "Profil PPP",
  event: "Event",
  router: "Router",
  berbagi: "Berbagi",
  admin: "Admin",
  pengaturan: "Pengaturan",
  akun: "Akun saya",
}

function NoAccess({ reason }: { reason: string }) {
  const first = useNavItems()[0]
  return (
    <Alert className="max-w-lg">
      <ShieldAlertIcon />
      <AlertTitle>Tidak ada akses</AlertTitle>
      <AlertDescription>
        <p>{reason}</p>
        {first && (
          <Button variant="outline" size="sm" className="mt-2" onClick={() => navigate(first.page)}>
            Buka {first.label}
          </Button>
        )}
      </AlertDescription>
    </Alert>
  )
}

function Content() {
  const { page, params, hash } = useRoute()
  const { status, error, reload, access } = useStore()
  const { session } = useAuth()
  const denied = pageDenied(page, access, session?.permissions)
  const [openUser, setOpenUser] = React.useState<string | null>(null)

  React.useEffect(() => {
    document.title = `${TITLES[page]} · Telemon`
  }, [page])

  let body: React.ReactNode
  if (denied) {
    body = <NoAccess reason={denied} />
  } else if (status === "loading") {
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
      admin: <AdminPage />,
      pengaturan: <SettingsPage />,
      akun: <AccountPage />,
    }[page]
  }

  return (
    <div className="flex min-h-svh flex-col">
      <SiteHeader title={TITLES[page]} />
      <main className={`${PAGE_CONTAINER} flex-1 py-4 pb-[calc(4.5rem+env(safe-area-inset-bottom))] md:py-6`}>
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
