import * as React from "react"
import { Loader2Icon, TriangleAlertIcon } from "lucide-react"

import { AppSidebar } from "@/components/app-sidebar"
import { SiteHeader } from "@/components/site-header"
import { UserDetailSheet } from "@/components/user-detail-sheet"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useRoute, type Page } from "@/lib/route"
import { StoreProvider } from "@/lib/store"
import { useStore } from "@/lib/store-context"
import { EventsPage } from "@/pages/events"
import { OverviewPage } from "@/pages/overview"
import { RoutersPage } from "@/pages/routers"
import { SharePage } from "@/pages/share"
import { UsersPage } from "@/pages/users"

const TITLES: Record<Page, string> = {
  ringkasan: "Ringkasan",
  user: "User PPPoE",
  event: "Event",
  router: "Router",
  berbagi: "Berbagi",
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
      event: <EventsPage key={hash} params={params} onOpenUser={setOpenUser} />,
      router: <RoutersPage />,
      berbagi: <SharePage />,
    }[page]
  }

  return (
    <>
      <SiteHeader title={TITLES[page]} />
      <main className="flex-1 p-4 md:p-6">{body}</main>
      <UserDetailSheet userId={openUser} onOpenChange={(o) => !o && setOpenUser(null)} />
    </>
  )
}

export function App() {
  return (
    <TooltipProvider>
      <StoreProvider>
        <SidebarProvider>
          <AppSidebar />
          <SidebarInset className="min-w-0">
            <Content />
          </SidebarInset>
        </SidebarProvider>
        <Toaster position="top-center" />
      </StoreProvider>
    </TooltipProvider>
  )
}

export default App
