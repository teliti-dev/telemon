import * as React from "react"
import { ShieldAlertIcon, TriangleAlertIcon } from "lucide-react"

import { AiChatDock, AI_DOCK_WIDTH } from "@/components/ai-chat-dock"
import { AuthGate } from "@/components/auth-gate"
import { BottomNav } from "@/components/bottom-nav"
import { PageSkeleton } from "@/components/skeletons"
import { SiteHeader } from "@/components/site-header"
import { UpdateBanner } from "@/components/update-banner"
import { UserDetailSheet } from "@/components/user-detail-sheet"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Toaster } from "@/components/ui/sonner"
import { TooltipProvider } from "@/components/ui/tooltip"
import { useAiChat } from "@/lib/ai-context"
import { AiChatProvider } from "@/lib/ai-provider"
import { ExtensionsLoader } from "@/lib/extensions/loader"
import { ApiProvider } from "@/lib/api-provider"
import { PAGE_CONTAINER } from "@/lib/layout"
import { useNavItems } from "@/hooks/use-nav-items"
import { tk, useLang, useT } from "@/lib/i18n"
import { useAuth } from "@/lib/auth-context"
import { pageDenied } from "@/lib/permissions"
import { useIsDesktop } from "@/hooks/use-media-query"
import { navigate, navigatePath, useRoute, type Page } from "@/lib/route"
import { StoreProvider } from "@/lib/store"
import { useStore } from "@/lib/store-context"
import { AboutPage } from "@/pages/about"
import { AccountPage } from "@/pages/account"
import { AiChatPage } from "@/components/ai-chat-dock"
import { AppearancePage } from "@/pages/appearance"
import { AdminPage } from "@/pages/admin"
import { EventsPage } from "@/pages/events"
import { OverviewPage } from "@/pages/overview"
import { ProfilesPage } from "@/pages/profiles"
import { RoutersPage } from "@/pages/routers"
import { SettingsPage } from "@/pages/settings"
import { SettingsExtensionsPage } from "@/pages/settings-extensions"
import { SettingsAiPage } from "@/pages/settings-ai"
import { SharePage } from "@/pages/share"
import { AccountFormPage, AiProviderFormPage, ImportPage, ProfileFormPage, RoleFormPage, RouterFormPage, UserFormPage } from "@/pages/subpages"
import { UsersPage } from "@/pages/users"
import { UserDetailPage } from "@/components/user-detail-sheet"

const TITLES: Record<Page, string> = {
  ringkasan: tk("Ringkasan"),
  user: tk("Users"),
  profile: tk("Profil PPP"),
  event: tk("Event"),
  router: tk("Router"),
  berbagi: tk("Berbagi"),
  admin: tk("Admin"),
  pengaturan: tk("Pengaturan"),
  tentang: tk("Tentang"),
  akun: tk("Akun saya"),
  import: tk("Import dari Mikrotik"),
  appearance: tk("Appearance"),
  ai: tk("Tanya AI"),
}

function NoAccess({ reason }: { reason: string }) {
  const t = useT()
  const first = useNavItems()[0]
  return (
    <Alert className="max-w-lg">
      <ShieldAlertIcon />
      <AlertTitle>{t("Tidak ada akses")}</AlertTitle>
      <AlertDescription>
        <p>{reason}</p>
        {first && (
          <Button variant="outline" size="sm" className="mt-2" onClick={() => navigate(first.page)}>
            {t("Buka {label}", { label: t(first.label) /* i18n-ignore: label bertanda tk() */ })}
          </Button>
        )}
      </AlertDescription>
    </Alert>
  )
}

function Content() {
  const t = useT()
  const lang = useLang()
  const { page, segments, params, hash } = useRoute()
  const isDesktop = useIsDesktop()
  const { status, error, reload, access } = useStore()
  const { session } = useAuth()
  const ai = useAiChat()
  const denied = pageDenied(page, access, session?.permissions)
  const [openUser, setOpenUser] = React.useState<string | null>(null)

  React.useEffect(() => {
    document.title = `${t(TITLES[page]) /* i18n-ignore: kunci tk */} · Telemon`
  }, [page, lang, t])

  const openUserFn = (id: string) => (isDesktop ? setOpenUser(id) : navigatePath("user", [id]))

  // Di HP, formulir dan detail yang di desktop berupa dialog/drawer menjadi halaman bertingkat.
  function renderPage(): React.ReactNode {
    const [s0, s1, s2] = segments
    if (page === "appearance") return <AppearancePage />
    if (page === "ai") return isDesktop ? <OverviewPage onOpenUser={openUserFn} /> : <AiChatPage />
    if (page === "import") return <ImportPage router={params.get("router") ?? undefined} />
    if (page === "user") {
      if (s0 === "new") return <UserFormPage mode="create" router={params.get("router") ?? undefined} />
      if (s0 && s1 === "edit") return <UserFormPage mode="edit" userId={s0} />
      if (s0) return <UserDetailPage userId={s0} onBack={() => navigate("user")} />
      return <UsersPage key={hash} params={params} onOpenUser={openUserFn} />
    }
    if (page === "profile") {
      if (s0 === "new") return <ProfileFormPage mode="create" router={params.get("router") ?? undefined} />
      if (s0 && s1 === "edit") return <ProfileFormPage mode="edit" profileId={s0} />
      return <ProfilesPage key={hash} params={params} />
    }
    if (page === "router") {
      if (s0 === "new") return <RouterFormPage />
      if (s0 && s1 === "edit") return <RouterFormPage routerId={s0} />
      return <RoutersPage />
    }
    if (page === "admin") {
      if (s0 === "account" && s1 === "new") return <AccountFormPage kind="new" />
      if (s0 === "account" && s1 && s2 === "role") return <AccountFormPage kind="role" accountId={s1} />
      if (s0 === "account" && s1 && s2 === "password") return <AccountFormPage kind="password" accountId={s1} />
      if (s0 === "role" && s1 === "new") return <RoleFormPage />
      if (s0 === "role" && s1 && s2 === "edit") return <RoleFormPage roleId={s1} />
      return <AdminPage />
    }
    switch (page) {
      case "ringkasan": return <OverviewPage onOpenUser={openUserFn} />
      case "event": return <EventsPage key={hash} params={params} onOpenUser={openUserFn} />
      case "berbagi": return <SharePage />
      case "tentang": return <AboutPage />
      case "pengaturan":
        if (s0 === "ai") return <SettingsAiPage />
        if (s0 === "extension") return <SettingsExtensionsPage />
        if (s0 === "ai-provider" && s1 === "new") return <AiProviderFormPage />
        if (s0 === "ai-provider" && s1 && s2 === "edit") return <AiProviderFormPage providerId={s1} />
        return <SettingsPage />
      default: return <AccountPage />
    }
  }

  let body: React.ReactNode
  if (denied) {
    body = <NoAccess reason={denied} />
  } else if (status === "loading") {
    body = <PageSkeleton page={page} />
  } else if (status === "error") {
    body = (
      <Alert variant="destructive" className="max-w-lg">
        <TriangleAlertIcon />
        <AlertTitle>{t("Gagal memuat data")}</AlertTitle>
        <AlertDescription>
          <p>{error}</p>
          <Button variant="outline" size="sm" className="mt-2" onClick={reload}>
            {t("Coba lagi")}
          </Button>
        </AlertDescription>
      </Alert>
    )
  } else {
    // key = hash: tautan dengan filter baru (mis. dari Ringkasan) mereset state halaman.
    body = renderPage()
  }

  return (
    <div
      className="flex min-h-svh flex-col transition-[padding] duration-200 motion-reduce:transition-none"
      style={ai.docked ? { paddingRight: AI_DOCK_WIDTH } : undefined}
    >
      <SiteHeader title={t(TITLES[page]) /* i18n-ignore: kunci tk */} />
      <UpdateBanner />
      <main className={`${PAGE_CONTAINER} flex-1 py-4 pb-[calc(4.5rem+env(safe-area-inset-bottom))] md:py-6`}>
        {body}
      </main>
      <BottomNav />
      <AiChatDock />
      {isDesktop && <UserDetailSheet userId={openUser} onOpenChange={(o) => !o && setOpenUser(null)} />}
    </div>
  )
}

export function App() {
  return (
    <TooltipProvider>
      <ApiProvider>
        <AuthGate>
          <StoreProvider>
            <AiChatProvider>
              <ExtensionsLoader />
              <Content />
              <Toaster position="top-center" />
            </AiChatProvider>
          </StoreProvider>
        </AuthGate>
      </ApiProvider>
    </TooltipProvider>
  )
}

export default App
