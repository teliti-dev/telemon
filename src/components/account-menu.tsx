import { LogOutIcon, UserRoundIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { useAuth } from "@/lib/auth-context"
import { navigate } from "@/lib/route"

/** Menu akun untuk akses jarak jauh (browser). Tidak tampil di aplikasi desktop. */
export function AccountMenu() {
  const { session, logout } = useAuth()
  if (!session) return null

  const initials =
    session.displayName
      .split(/\s+/)
      .filter(Boolean)
      .map((w) => w[0])
      .join("")
      .slice(0, 2)
      .toUpperCase() || session.username[0].toUpperCase()

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="sm" className="gap-1.5 px-2" aria-label="Profile">
          <span className="flex size-6 items-center justify-center rounded-full bg-primary text-[11px] font-semibold text-primary-foreground">
            {initials}
          </span>
          <span className="hidden sm:inline">Profile</span>
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-52">
        <DropdownMenuLabel className="grid gap-0.5 font-normal">
          <span className="truncate font-medium">{session.displayName}</span>
          <span className="truncate font-mono text-xs text-muted-foreground">{session.username}</span>
        </DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuItem onSelect={() => navigate("akun")}>
          <UserRoundIcon /> Akun saya
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => void logout()}>
          <LogOutIcon /> Keluar
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
