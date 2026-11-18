import {
  ActivityIcon,
  GaugeIcon,
  RadioTowerIcon,
  Share2Icon,
  UsersIcon,
  ZapIcon,
} from "lucide-react"

import { Badge } from "@/components/ui/badge"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
  useSidebar,
} from "@/components/ui/sidebar"
import { useNow } from "@/hooks/use-now"
import { href, useRoute, type Page } from "@/lib/route"
import { useStore } from "@/lib/store-context"

const NAV: { page: Page; label: string; icon: typeof GaugeIcon }[] = [
  { page: "ringkasan", label: "Ringkasan", icon: GaugeIcon },
  { page: "user", label: "Users", icon: UsersIcon },
  { page: "event", label: "Event", icon: ActivityIcon },
  { page: "router", label: "Router", icon: RadioTowerIcon },
  { page: "berbagi", label: "Berbagi", icon: Share2Icon },
]

export function AppSidebar() {
  const { page } = useRoute()
  const { routers, events, api } = useStore()
  const { setOpenMobile } = useSidebar()
  const now = useNow(30_000)

  const authFailed = events.filter(
    (e) => e.type === "auth_failed" && now - e.at < 3600_000
  ).length
  const routerProblems = routers.filter((r) => r.status !== "online").length
  const close = () => setOpenMobile(false)

  const badges: Partial<Record<Page, number>> = {
    event: authFailed,
    router: routerProblems,
  }

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" asChild>
              <a href={href("ringkasan")} onClick={close}>
                <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
                  <ZapIcon className="size-4" />
                </div>
                <div className="grid flex-1 text-left leading-tight">
                  <span className="truncate font-semibold">Telemon</span>
                  <span className="truncate text-xs text-muted-foreground">
                    Monitoring PPPoE
                  </span>
                </div>
              </a>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {NAV.map((item) => (
                <SidebarMenuItem key={item.page}>
                  <SidebarMenuButton
                    asChild
                    isActive={page === item.page || (item.page === "user" && page === "profile")}
                    tooltip={item.label}
                  >
                    <a href={href(item.page)} onClick={close}>
                      <item.icon />
                      <span>{item.label}</span>
                    </a>
                  </SidebarMenuButton>
                  {badges[item.page] ? (
                    <SidebarMenuBadge
                      title={
                        item.page === "event"
                          ? "Gagal auth dalam 1 jam"
                          : "Router bermasalah"
                      }
                    >
                      {badges[item.page]}
                    </SidebarMenuBadge>
                  ) : null}
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

      </SidebarContent>

      <SidebarFooter className="group-data-[collapsible=icon]:hidden">
        <div className="flex items-center justify-between px-2 text-xs text-muted-foreground">
          <span>v0.1.0</span>
          {api.kind === "mock" && <Badge variant="secondary">Data contoh</Badge>}
        </div>
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  )
}
