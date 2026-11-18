import { ZapIcon } from "lucide-react"

import { isNavActive, NAV } from "@/components/nav-items"
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
} from "@/components/ui/sidebar"
import { useNavBadges } from "@/hooks/use-nav-badges"
import { href, useRoute } from "@/lib/route"
import { useStore } from "@/lib/store-context"

/** Sidebar desktop. Di HP navigasi pindah ke menu bawah (BottomNav). */
export function AppSidebar() {
  const { page } = useRoute()
  const { api } = useStore()
  const badges = useNavBadges()

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" asChild>
              <a href={href("ringkasan")}>
                <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
                  <ZapIcon className="size-4" />
                </div>
                <div className="grid flex-1 text-left leading-tight">
                  <span className="truncate font-semibold">Telemon</span>
                  <span className="truncate text-xs text-muted-foreground">Monitoring PPPoE</span>
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
                  <SidebarMenuButton asChild isActive={isNavActive(item.page, page)} tooltip={item.label}>
                    <a href={href(item.page)}>
                      <item.icon />
                      <span>{item.label}</span>
                    </a>
                  </SidebarMenuButton>
                  {badges[item.page] ? (
                    <SidebarMenuBadge
                      title={item.page === "event" ? "Gagal auth dalam 1 jam" : "Router bermasalah"}
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
