import * as React from "react"
import { TriangleAlertIcon } from "lucide-react"
import { toast } from "sonner"

import { ReviewChanges, type Change } from "@/components/review-changes"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { FormFooter, FormHeader } from "@/components/form-layout"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
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
import { useWriteGate } from "@/hooks/use-write-gate"
import { tk, useT } from "@/lib/i18n"
import { useStore } from "@/lib/store-context"
import type { OnlyOne, PppProfile, ProfilePatch } from "@/lib/types"

export const ONLY_ONE_LABEL: Record<OnlyOne, string> = {
  default: tk("Bawaan router"),
  yes: tk("Ya (satu sesi per user)"),
  no: tk("Tidak (boleh banyak sesi)"),
}

const dash = (v?: string) => (v && v.trim() ? v.trim() : "-")
const RATE = /^\d+[kKmMgG]?(\/\d+[kKmMgG]?)?$/

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: "create" | "edit"
  profile?: PppProfile
  defaultRouterId?: string
}

export function ProfileFormDialog({ open, onOpenChange, ...rest }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[92svh] overflow-y-auto sm:max-w-md">
        {open && <ProfileForm onClose={() => onOpenChange(false)} {...rest} />}
      </DialogContent>
    </Dialog>
  )
}

export function ProfileForm({
  mode,
  profile,
  defaultRouterId,
  onClose,
}: Omit<Props, "open" | "onOpenChange"> & { onClose: () => void }) {
  const t = useT()
  const { routers, routerById, api } = useStore()
  const gate = useWriteGate("profile")
  const editing = mode === "edit" && !!profile

  const writable = routers.filter((r) => r.canWrite)
  const initialRouter =
    profile?.routerId ??
    writable.find((r) => r.id === defaultRouterId && r.status === "online")?.id ??
    writable.find((r) => r.status === "online")?.id ??
    writable[0]?.id ??
    ""

  const [routerId, setRouterId] = React.useState(initialRouter)
  const [name, setName] = React.useState(profile?.name ?? "")
  const [rateLimit, setRateLimit] = React.useState(profile?.rateLimit ?? "")
  const [localAddress, setLocalAddress] = React.useState(profile?.localAddress ?? "")
  const [remoteAddress, setRemoteAddress] = React.useState(profile?.remoteAddress ?? "")
  const [onlyOne, setOnlyOne] = React.useState<OnlyOne>(profile?.onlyOne ?? "default")
  const [comment, setComment] = React.useState(profile?.comment ?? "")
  const [step, setStep] = React.useState<"form" | "review">("form")
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  const router = routerById.get(routerId)
  const blocked = gate(routerId)
  const rateOk = !rateLimit.trim() || RATE.test(rateLimit.trim())

  const patch: ProfilePatch = {}
  const changes: Change[] = []
  if (editing && profile) {
    if (rateLimit.trim() !== (profile.rateLimit ?? "")) {
      patch.rateLimit = rateLimit.trim()
      changes.push({ label: t("Rate limit"), from: dash(profile.rateLimit), to: dash(rateLimit) })
    }
    if (localAddress.trim() !== (profile.localAddress ?? "")) {
      patch.localAddress = localAddress.trim()
      changes.push({ label: t("Local address"), from: dash(profile.localAddress), to: dash(localAddress) })
    }
    if (remoteAddress.trim() !== (profile.remoteAddress ?? "")) {
      patch.remoteAddress = remoteAddress.trim()
      changes.push({ label: t("Remote address"), from: dash(profile.remoteAddress), to: dash(remoteAddress) })
    }
    if (onlyOne !== profile.onlyOne) {
      patch.onlyOne = onlyOne
      changes.push({ label: t("Only one"), from: t(ONLY_ONE_LABEL[profile.onlyOne]), to: t(ONLY_ONE_LABEL[onlyOne]) }) // i18n-ignore: etiket sudah berupa kunci (tk)
    }
    if (comment.trim() !== (profile.comment ?? "")) {
      patch.comment = comment.trim()
      changes.push({ label: t("Komentar"), from: dash(profile.comment), to: dash(comment) })
    }
  } else {
    changes.push({ label: t("Router"), to: router?.name ?? "-" })
    changes.push({ label: t("Nama"), to: name.trim() })
    changes.push({ label: t("Rate limit"), to: dash(rateLimit) })
    changes.push({ label: t("Local address"), to: dash(localAddress) })
    changes.push({ label: t("Remote address"), to: dash(remoteAddress) })
    changes.push({ label: t("Only one"), to: t(ONLY_ONE_LABEL[onlyOne]) }) // i18n-ignore: etiket sudah berupa kunci (tk)
    if (comment.trim()) changes.push({ label: t("Komentar"), to: comment.trim() })
  }

  const valid = editing
    ? changes.length > 0 && rateOk
    : /^[\w.-]{1,40}$/.test(name.trim()) && rateOk && !!routerId

  async function submit() {
    setBusy(true)
    setError(null)
    try {
      if (editing && profile) {
        await api.updateProfile(profile.id, patch)
        toast.success(t("Profil PPP {name} diperbarui di {router}", { name: profile.name, router: router?.name ?? "" }))
      } else {
        const p = await api.createProfile({
          routerId,
          name: name.trim(),
          rateLimit: rateLimit.trim() || undefined,
          localAddress: localAddress.trim() || undefined,
          remoteAddress: remoteAddress.trim() || undefined,
          onlyOne,
          comment: comment.trim() || undefined,
        })
        toast.success(t("Profil PPP {name} dibuat di {router}", { name: p.name, router: router?.name ?? "" }))
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
        <FormHeader title={t("Tulis ke {router}?", { router: router?.name ?? "" })} description={t("Perubahan profil PPP langsung dikirim ke router. Sesi yang sedang berjalan bisa terpengaruh, misalnya saat rate limit diubah.")} />
        <ReviewChanges changes={changes} />
        {error && (
          <Alert variant="destructive">
            <TriangleAlertIcon />
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
        <FormFooter>
          <Button variant="outline" onClick={() => setStep("form")} disabled={busy}>
            {t("Kembali")}
          </Button>
          <Button onClick={submit} disabled={busy}>
            {busy ? t("Menulis ke router…") : t("Tulis ke router")}
          </Button>
        </FormFooter>
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
      <FormHeader title={editing ? t("Edit profil PPP {name}", { name: profile?.name ?? "" }) : t("Tambah profil PPP")} description={editing ? t("Mengubah /ppp/profile di router.") : t("Membuat /ppp/profile baru di router.")} />

      {blocked && (
        <Alert variant="destructive">
          <TriangleAlertIcon />
          <AlertDescription>{blocked}.</AlertDescription>
        </Alert>
      )}

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="p-router">{t("Router")}</Label>
          <Select value={routerId} onValueChange={setRouterId} disabled={editing}>
            <SelectTrigger id="p-router" className="w-full">
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
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="p-name">{t("Nama")}</Label>
          <Input
            id="p-name"
            value={name}
            disabled={editing}
            onChange={(e) => setName(e.target.value)}
            autoComplete="off"
            spellCheck={false}
            // i18n-ignore: contoh nama profil
            placeholder="20Mbps"
            className="font-mono"
            autoFocus={!editing}
          />
        </div>
      </div>

      <div className="grid gap-1.5">
        <Label htmlFor="p-rate">{t("Rate limit (rx/tx)")}</Label>
        <Input
          id="p-rate"
          value={rateLimit}
          onChange={(e) => setRateLimit(e.target.value)}
          placeholder="10M/10M"
          spellCheck={false}
          className="font-mono"
          aria-invalid={!rateOk}
        />
        {!rateOk ? (
          <p className="text-xs text-destructive">{t("Format tidak valid. Contoh: 10M/10M atau 5M.")}</p>
        ) : (
          <p className="text-xs text-muted-foreground">{t("Unggah/unduh dari sisi router. Kosong = tanpa batas.")}</p>
        )}
      </div>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="p-local">{t("Local address")}</Label>
          <Input
            id="p-local"
            value={localAddress}
            onChange={(e) => setLocalAddress(e.target.value)}
            placeholder="100.64.1.1"
            className="font-mono"
          />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="p-remote">{t("Remote address (pool)")}</Label>
          <Input
            id="p-remote"
            value={remoteAddress}
            onChange={(e) => setRemoteAddress(e.target.value)}
            placeholder="pool-pppoe-1"
            className="font-mono"
          />
        </div>
      </div>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <div className="grid gap-1.5">
          <Label htmlFor="p-only">{t("Only one")}</Label>
          <Select value={onlyOne} onValueChange={(v) => setOnlyOne(v as OnlyOne)}>
            <SelectTrigger id="p-only" className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {(Object.keys(ONLY_ONE_LABEL) as OnlyOne[]).map((k) => (
                <SelectItem key={k} value={k}>
                  {t(ONLY_ONE_LABEL[k]) /* i18n-ignore: etiket sudah berupa kunci (tk) */}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="p-comment">{t("Komentar")}</Label>
          <Input
            id="p-comment"
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            maxLength={120}
            placeholder={t("opsional")}
          />
        </div>
      </div>

      <FormFooter>
        <Button type="button" variant="outline" onClick={onClose}>
          {t("Batal")}
        </Button>
        <Button type="submit" disabled={!valid || !!blocked}>
          {t("Tinjau")}
        </Button>
      </FormFooter>
      {!valid && editing && changes.length === 0 && (
        <p className="-mt-1 text-right text-xs text-muted-foreground">{t("Belum ada perubahan.")}</p>
      )}
    </form>
  )
}
