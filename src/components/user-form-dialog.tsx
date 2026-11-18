import * as React from "react"
import { DicesIcon, TriangleAlertIcon } from "lucide-react"
import { toast } from "sonner"

import { ReviewChanges, type Change } from "@/components/review-changes"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { useWriteGate } from "@/hooks/use-write-gate"
import { useT } from "@/lib/i18n"
import { randomPassword } from "@/lib/random"
import { useStore } from "@/lib/store-context"
import type { PppUser, UserPatch } from "@/lib/types"

const dash = (v?: string) => (v && v.trim() ? v.trim() : "-")

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: "create" | "edit"
  user?: PppUser
  /** Router awal saat menambah user. */
  defaultRouterId?: string
}

export function UserFormDialog({ open, onOpenChange, ...rest }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[92svh] overflow-y-auto sm:max-w-md">
        {/* Dipasang hanya saat dialog terbuka agar state form selalu bersih. */}
        {open && <UserForm onClose={() => onOpenChange(false)} {...rest} />}
      </DialogContent>
    </Dialog>
  )
}

function UserForm({
  mode,
  user,
  defaultRouterId,
  onClose,
}: Omit<Props, "open" | "onOpenChange"> & { onClose: () => void }) {
  const t = useT()
  const { routers, profiles, routerById, api } = useStore()
  const gate = useWriteGate("user")
  const editing = mode === "edit" && !!user

  const writable = routers.filter((r) => r.canWrite)
  const initialRouter =
    user?.routerId ??
    writable.find((r) => r.id === defaultRouterId && r.status === "online")?.id ??
    writable.find((r) => r.status === "online")?.id ??
    writable[0]?.id ??
    ""
  const firstProfile = (rid: string) =>
    profiles.find((p) => p.routerId === rid && !p.deletedAt)?.name ?? ""

  const [routerId, setRouterId] = React.useState(initialRouter)
  const [username, setUsername] = React.useState(user?.username ?? "")
  const [password, setPassword] = React.useState("")
  const [profile, setProfile] = React.useState(user?.profile ?? firstProfile(initialRouter))
  const [staticAddress, setStaticAddress] = React.useState(user?.staticAddress ?? "")
  const [comment, setComment] = React.useState(user?.comment ?? "")
  const [disabled, setDisabled] = React.useState(user ? user.status === "disabled" : false)
  const [step, setStep] = React.useState<"form" | "review">("form")
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  const router = routerById.get(routerId)
  const blocked = gate(routerId)
  const routerProfiles = profiles.filter((p) => p.routerId === routerId && !p.deletedAt)
  const profileOptions =
    profile && !routerProfiles.some((p) => p.name === profile)
      ? [...routerProfiles.map((p) => p.name), profile]
      : routerProfiles.map((p) => p.name)

  // Perubahan terhadap kondisi sekarang (edit) atau seluruh isian (tambah).
  const patch: UserPatch = {}
  const changes: Change[] = []
  if (editing && user) {
    if (profile !== user.profile) {
      patch.profile = profile
      changes.push({ label: t("Profil"), from: user.profile, to: profile })
    }
    if (staticAddress.trim() !== (user.staticAddress ?? "")) {
      patch.staticAddress = staticAddress.trim()
      changes.push({ label: t("IP statis"), from: dash(user.staticAddress), to: dash(staticAddress) })
    }
    if (comment.trim() !== (user.comment ?? "")) {
      patch.comment = comment.trim()
      changes.push({ label: t("Komentar"), from: dash(user.comment), to: dash(comment) })
    }
    if (disabled !== (user.status === "disabled")) {
      patch.disabled = disabled
      changes.push({
        label: t("Secret"),
        from: user.status === "disabled" ? t("Nonaktif") : t("Aktif"),
        to: disabled ? t("Nonaktif") : t("Aktif"),
      })
    }
    if (password) {
      patch.password = password
      changes.push({ label: t("Password"), to: t("diganti") })
    }
  } else {
    changes.push({ label: t("Router"), to: router?.name ?? "-" })
    changes.push({ label: t("Username"), to: username.trim() })
    changes.push({ label: t("Password"), to: t("diatur") })
    changes.push({ label: t("Profil"), to: profile || "-" })
    if (staticAddress.trim()) changes.push({ label: t("IP statis"), to: staticAddress.trim() })
    if (comment.trim()) changes.push({ label: t("Komentar"), to: comment.trim() })
    if (disabled) changes.push({ label: t("Secret"), to: t("Nonaktif") })
  }

  const valid = editing
    ? changes.length > 0 && !!profile && (!password || password.length >= 6)
    : /^[\w.@-]{2,40}$/.test(username.trim()) && password.length >= 6 && !!profile && !!routerId

  async function submit() {
    setBusy(true)
    setError(null)
    try {
      if (editing && user) {
        await api.updateUser(user.id, patch)
        toast.success(t("{user} diperbarui di {router}", { user: user.username, router: router?.name ?? "" }))
      } else {
        const u = await api.createUser({
          routerId,
          username: username.trim(),
          password,
          profile,
          staticAddress: staticAddress.trim() || undefined,
          comment: comment.trim() || undefined,
          disabled,
        })
        toast.success(t("{user} dibuat di {router}", { user: u.username, router: router?.name ?? "" }))
      }
      onClose()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }

  if (step === "review") {
    return (
      <>
        <DialogHeader>
          <DialogTitle>{t("Tulis ke {router}?", { router: router?.name ?? "" })}</DialogTitle>
          <DialogDescription>
            {t("Perubahan langsung dikirim ke router dan berlaku untuk pelanggan.")}
          </DialogDescription>
        </DialogHeader>
        <ReviewChanges changes={changes} />
        {error && (
          <Alert variant="destructive">
            <TriangleAlertIcon />
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={() => setStep("form")} disabled={busy}>
            {t("Kembali")}
          </Button>
          <Button onClick={submit} disabled={busy}>
            {busy ? t("Menulis ke router…") : t("Tulis ke router")}
          </Button>
        </DialogFooter>
      </>
    )
  }

  return (
    <form
      className="grid gap-3"
      onSubmit={(e) => {
        e.preventDefault()
        if (valid && !blocked) setStep("review")
      }}
    >
      <DialogHeader>
        <DialogTitle>{editing ? t("Edit {user}", { user: user?.username ?? "" }) : t("Tambah user")}</DialogTitle>
        <DialogDescription>
          {editing
            ? t("Mengubah secret PPPoE di router.")
            : t("Membuat secret PPPoE baru di router.")}
        </DialogDescription>
      </DialogHeader>

      {blocked && (
        <Alert variant="destructive">
          <TriangleAlertIcon />
          <AlertDescription>{blocked}.</AlertDescription>
        </Alert>
      )}

      <div className="grid gap-1.5">
        <Label htmlFor="u-router">{t("Router")}</Label>
        <Select
          value={routerId}
          disabled={editing}
          onValueChange={(v) => {
            setRouterId(v)
            setProfile(firstProfile(v))
          }}
        >
          <SelectTrigger id="u-router" className="w-full">
            <SelectValue placeholder={t("Pilih router")} />
          </SelectTrigger>
          <SelectContent>
            {(editing ? routers : writable).map((r) => (
              <SelectItem key={r.id} value={r.id} disabled={r.status !== "online"}>
                {r.name}
                {r.status !== "online" && ` (${r.status})`}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {!editing && writable.length === 0 && (
          <p className="text-xs text-destructive">
            {t("Belum ada router dengan hak write. Aktifkan di halaman Router.")}
          </p>
        )}
      </div>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="u-name">{t("Username")}</Label>
          <Input
            id="u-name"
            value={username}
            disabled={editing}
            onChange={(e) => setUsername(e.target.value)}
            autoComplete="off"
            autoCapitalize="off"
            spellCheck={false}
            placeholder="budi.rt01"
            className="font-mono"
            autoFocus={!editing}
          />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="u-pass">
            {t("Password")}{" "}
            {editing && <span className="font-normal text-muted-foreground">{t("(kosong = tetap)")}</span>}
          </Label>
          <div className="flex gap-1.5">
            <Input
              id="u-pass"
              type="text"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="off"
              spellCheck={false}
              className="font-mono"
              aria-invalid={password.length > 0 && password.length < 6}
            />
            <Button
              type="button"
              variant="outline"
              size="icon"
              aria-label={t("Buat password acak")}
              title={t("Buat password acak")}
              onClick={() => setPassword(randomPassword())}
            >
              <DicesIcon />
            </Button>
          </div>
        </div>
      </div>

      <div className="grid gap-1.5">
        <Label htmlFor="u-profile">{t("Profil")}</Label>
        <Select value={profile} onValueChange={setProfile} disabled={profileOptions.length === 0}>
          <SelectTrigger id="u-profile" className="w-full">
            <SelectValue placeholder={t("Pilih profil PPP")} />
          </SelectTrigger>
          <SelectContent>
            {profileOptions.map((n) => (
              <SelectItem key={n} value={n}>
                {n}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="u-ip">{t("IP statis")}</Label>
          <Input
            id="u-ip"
            value={staticAddress}
            onChange={(e) => setStaticAddress(e.target.value)}
            inputMode="decimal"
            placeholder={t("opsional")}
            className="font-mono"
          />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="u-comment">{t("Komentar")}</Label>
          <Input
            id="u-comment"
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            maxLength={120}
            placeholder={t("opsional")}
          />
        </div>
      </div>

      <div className="flex items-center justify-between gap-4 rounded-lg border p-2.5">
        <div className="grid gap-0.5">
          <Label htmlFor="u-disabled">{t("Nonaktifkan secret")}</Label>
          <p className="text-xs text-muted-foreground">{t("User tidak bisa login dan sesi aktif diputus.")}</p>
        </div>
        <Switch id="u-disabled" checked={disabled} onCheckedChange={setDisabled} />
      </div>

      <DialogFooter>
        <Button type="button" variant="outline" onClick={onClose}>
          {t("Batal")}
        </Button>
        <Button type="submit" disabled={!valid || !!blocked}>
          {t("Tinjau")}
        </Button>
      </DialogFooter>
      {!valid && editing && changes.length === 0 && (
        <p className="-mt-1 text-right text-xs text-muted-foreground">{t("Belum ada perubahan.")}</p>
      )}
    </form>
  )
}
