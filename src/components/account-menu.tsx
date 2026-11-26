import * as React from "react"
import { LogOutIcon, PuzzleIcon, SparklesIcon, UserRoundIcon } from "lucide-react"


import { isNavActive } from "@/components/nav-items"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { useIsDesktop } from "@/hooks/use-media-query"
import { useNavItems } from "@/hooks/use-nav-items"
import { useAuth } from "@/lib/auth-context"
import { useT } from "@/lib/i18n"
import { navigate, navigatePath, useRoute, type Page } from "@/lib/route"
import { cn } from "@/lib/utils"

/**
 * Menu profil.
 * - Desktop/tablet (md ke atas): dropdown berisi Akun saya dan Keluar, hanya untuk akses jarak jauh
 *   (aplikasi desktop tanpa login; menu Berbagi, Admin, dan Pengaturan sudah ada di menu atas).
 * - HP: modal (bukan dropdown) berisi Akun saya, Berbagi, Admin, Pengaturan, dan Keluar sesuai izin.
 */
export function AccountMenu() {
  const t = useT()
  const isDesktop = useIsDesktop()
  const { session, logout } = useAuth()
  const { page, segments } = useRoute()
  const manageItems = useNavItems().filter((i) => i.more)
  const [open, setOpen] = React.useState(false)
  const onAi = page === "pengaturan" && (segments[0] === "ai" || segments[0] === "ai-provider")
  const onExt = page === "pengaturan" && segments[0] === "extension"

  const initials = session
    ? session.displayName
        .split(/\s+/)
        .filter(Boolean)
        .map((w) => w[0])
        .join("")
        .slice(0, 2)
        .toUpperCase() || session.username[0].toUpperCase()
    : ""

  const avatar = session ? (
    <span className="flex size-6 items-center justify-center rounded-full bg-primary text-[11px] font-semibold text-primary-foreground">
      {initials}
    </span>
  ) : (
    <UserRoundIcon />
  )

  if (isDesktop) {
    if (!session) return null
    return (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="sm" className="gap-1.5 px-2" aria-label={t("Profile")}>
            {avatar}
            <span>{t("Profile")}</span>
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-52">
          <DropdownMenuLabel className="grid gap-0.5 font-normal">
            <span className="truncate font-medium">{session.displayName}</span>
            <span className="truncate font-mono text-xs text-muted-foreground">{session.username}</span>
          </DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={() => navigate("akun")}>
            <UserRoundIcon /> {t("Akun saya")}
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => void logout()}>
            <LogOutIcon /> {t("Keluar")}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    )
  }

  const go = (to: Page) => {
    setOpen(false)
    navigate(to)
  }

  const row = "h-12 w-full justify-start gap-3 px-3 text-base [&_svg:not([class*='size-'])]:size-5"

  return (
    <>
      <Button
        variant="ghost"
        size={session ? "sm" : "icon"}
        className={session ? "gap-1.5 px-1.5" : undefined}
        aria-label={t("Profile")}
        aria-haspopup="dialog"
        onClick={() => setOpen(true)}
      >
        {avatar}
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="gap-3 sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>{session ? session.displayName : t("Menu")}</DialogTitle>
            <DialogDescription>
              {session ? (
                <>
                  <span className="font-mono">{session.username}</span> · {session.roleName}
                </>
              ) : (
                t("Aplikasi desktop")
              )}
            </DialogDescription>
          </DialogHeader>
          <ul className="grid gap-1">
            {session && (
              <li>
                <Button variant="ghost" className={cn(row, page === "akun" && "text-brand")} onClick={() => go("akun")}>
                  <UserRoundIcon /> {t("Akun saya")}
                </Button>
              </li>
            )}
            {manageItems.map((item) => (
              <React.Fragment key={item.page}>
                <li>
                  <Button
                    variant="ghost"
                    className={cn(row, isNavActive(item.page, page) && !(item.page === "pengaturan" && (onAi || onExt)) && "text-brand")}
                    onClick={() => go(item.page)}
                  >
                    <item.icon /> {t(item.label /* i18n-ignore: label bertanda tk() */)}
                  </Button>
                </li>
                {item.page === "pengaturan" && (
                  <li>
                    <Button
                      variant="ghost"
                      className={cn(row, onAi && "text-brand")}
                      onClick={() => {
                        setOpen(false)
                        navigatePath("pengaturan", ["ai"])
                      }}
                    >
                      <SparklesIcon /> {t("Pengaturan AI")}
                    </Button>
                  </li>
                )}
                {item.page === "pengaturan" && (
                  <li>
                    <Button
                      variant="ghost"
                      className={cn(row, onExt && "text-brand")}
                      onClick={() => {
                        setOpen(false)
                        navigatePath("pengaturan", ["extension"])
                      }}
                    >
                      <PuzzleIcon /> {t("Extension")}
                    </Button>
                  </li>
                )}
              </React.Fragment>
            ))}
            {session && (
              <li className="mt-1 border-t pt-2">
                <Button
                  variant="ghost"
                  className={row}
                  onClick={() => {
                    setOpen(false)
                    void logout()
                  }}
                >
                  <LogOutIcon /> {t("Keluar")}
                </Button>
              </li>
            )}
          </ul>
        </DialogContent>
      </Dialog>
    </>
  )
}
