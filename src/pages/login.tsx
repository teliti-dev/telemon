import * as React from "react"
import { Loader2Icon, MonitorIcon, ZapIcon } from "lucide-react"

import { RecoveryCodes } from "@/components/recovery-codes"
import { TotpSetup } from "@/components/totp-setup"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Toaster } from "@/components/ui/sonner"
import { setPreviewAccess } from "@/lib/access"
import { useApi } from "@/lib/api-context"
import type { SessionInfo } from "@/lib/types"

type Step =
  | { name: "credentials" }
  | { name: "totp"; challengeId: string; recovery: boolean }
  | { name: "setup"; challengeId: string; secret: string; otpauthUrl: string }
  | { name: "recovery"; codes: string[]; session: SessionInfo }

const errMsg = (e: unknown) => (e instanceof Error ? e.message : String(e))

export function LoginPage({ onAuthed }: { onAuthed: (s: SessionInfo) => void }) {
  const { api } = useApi()
  const [step, setStep] = React.useState<Step>({ name: "credentials" })
  const [username, setUsername] = React.useState("")
  const [password, setPassword] = React.useState("")
  const [code, setCode] = React.useState("")
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  const [saved, setSaved] = React.useState(false)

  async function run(fn: () => Promise<void>) {
    setBusy(true)
    setError(null)
    try {
      await fn()
    } catch (e) {
      setError(errMsg(e))
    } finally {
      setBusy(false)
    }
  }

  const submitCredentials = (e: React.FormEvent) => {
    e.preventDefault()
    void run(async () => {
      const r = await api.login(username, password)
      setPassword("")
      if (r.status === "ok") onAuthed(r.session)
      else if (r.status === "2fa_required") setStep({ name: "totp", challengeId: r.challengeId, recovery: false })
      else setStep({ name: "setup", challengeId: r.challengeId, secret: r.secret, otpauthUrl: r.otpauthUrl })
    })
  }

  const submitTotp = (e: React.FormEvent) => {
    e.preventDefault()
    if (step.name !== "totp") return
    void run(async () => {
      onAuthed(await api.verify2fa(step.challengeId, code, { recovery: step.recovery }))
    })
  }

  const titles: Record<Step["name"], { title: string; desc: string }> = {
    credentials: { title: "Masuk ke Telemon", desc: "Gunakan akun yang dibuat pemilik Telemon." },
    totp: { title: "Verifikasi dua langkah", desc: "Masukkan kode dari aplikasi authenticator." },
    setup: { title: "Aktifkan 2FA", desc: "Pemilik Telemon mewajibkan verifikasi dua langkah." },
    recovery: { title: "Simpan kode pemulihan", desc: "2FA sudah aktif." },
  }
  const t = titles[step.name]

  return (
    <div className="flex min-h-svh items-center justify-center bg-muted/30 p-4">
      <div className="grid w-full max-w-sm gap-3">
        <div className="flex items-center justify-center gap-2 font-semibold">
          <div className="flex size-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
            <ZapIcon className="size-4" />
          </div>
          Telemon
        </div>

        <Card>
          <CardHeader>
            <CardTitle>{t.title}</CardTitle>
            <CardDescription>{t.desc}</CardDescription>
          </CardHeader>
          <CardContent className="grid gap-3">
            {step.name === "credentials" && (
              <form onSubmit={submitCredentials} className="grid gap-3">
                <div className="grid gap-1.5">
                  <Label htmlFor="l-user">Username</Label>
                  <Input
                    id="l-user"
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    autoComplete="username"
                    autoCapitalize="off"
                    spellCheck={false}
                    autoFocus
                  />
                </div>
                <div className="grid gap-1.5">
                  <Label htmlFor="l-pass">Password</Label>
                  <Input
                    id="l-pass"
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    autoComplete="current-password"
                  />
                </div>
                {error && (
                  <Alert variant="destructive">
                    <AlertDescription>{error}</AlertDescription>
                  </Alert>
                )}
                <Button type="submit" disabled={busy || !username.trim() || !password}>
                  {busy && <Loader2Icon className="animate-spin" />}
                  Masuk
                </Button>
              </form>
            )}

            {step.name === "totp" && (
              <form onSubmit={submitTotp} className="grid gap-3">
                <div className="grid gap-1.5">
                  <Label htmlFor="l-code">{step.recovery ? "Kode pemulihan" : "Kode 6 digit"}</Label>
                  <Input
                    id="l-code"
                    value={code}
                    onChange={(e) =>
                      setCode(step.recovery ? e.target.value.trim().toLowerCase() : e.target.value.replace(/\D/g, "").slice(0, 6))
                    }
                    inputMode={step.recovery ? "text" : "numeric"}
                    autoComplete="one-time-code"
                    placeholder={step.recovery ? "xxxxx-xxxxx" : "000000"}
                    className={step.recovery ? "font-mono" : "font-mono text-base tracking-[0.3em]"}
                    autoFocus
                  />
                </div>
                {error && (
                  <Alert variant="destructive">
                    <AlertDescription>{error}</AlertDescription>
                  </Alert>
                )}
                <Button type="submit" disabled={busy || (step.recovery ? code.length < 11 : code.length !== 6)}>
                  {busy && <Loader2Icon className="animate-spin" />}
                  Verifikasi
                </Button>
                <div className="flex justify-between text-sm">
                  <button
                    type="button"
                    className="text-muted-foreground underline-offset-4 hover:underline"
                    onClick={() => {
                      setCode("")
                      setError(null)
                      setStep({ ...step, recovery: !step.recovery })
                    }}
                  >
                    {step.recovery ? "Pakai kode authenticator" : "Pakai kode pemulihan"}
                  </button>
                  <button
                    type="button"
                    className="text-muted-foreground underline-offset-4 hover:underline"
                    onClick={() => {
                      setCode("")
                      setError(null)
                      setStep({ name: "credentials" })
                    }}
                  >
                    Kembali
                  </button>
                </div>
              </form>
            )}

            {step.name === "setup" && (
              <TotpSetup
                secret={step.secret}
                otpauthUrl={step.otpauthUrl}
                submitLabel="Aktifkan"
                onSubmit={async (c) => {
                  const r = await api.complete2faSetup(step.challengeId, c)
                  setStep({ name: "recovery", codes: r.recoveryCodes, session: r.session })
                }}
                onCancel={() => setStep({ name: "credentials" })}
              />
            )}

            {step.name === "recovery" && (
              <>
                <RecoveryCodes codes={step.codes} />
                <label className="flex items-center gap-2 text-sm">
                  <input type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} className="size-4" />
                  Saya sudah menyimpan kode ini
                </label>
                <Button disabled={!saved} onClick={() => onAuthed(step.session)}>
                  Lanjut
                </Button>
              </>
            )}
          </CardContent>
        </Card>

        {api.kind === "mock" && (
          <Card className="gap-2 py-3 text-xs">
            <CardContent className="grid gap-2 px-3">
              <p className="font-medium">Mode data contoh</p>
              <ul className="grid gap-0.5 text-muted-foreground">
                <li>
                  <span className="font-mono text-foreground">admin</span> / <span className="font-mono text-foreground">demo-telemon</span>, kode 2FA{" "}
                  <span className="font-mono text-foreground">123456</span>
                </li>
                <li>
                  <span className="font-mono text-foreground">operator</span> / <span className="font-mono text-foreground">demo-telemon</span>, tanpa 2FA
                </li>
                <li>
                  <span className="font-mono text-foreground">tamu</span>: akun nonaktif
                </li>
              </ul>
              <Button variant="outline" size="sm" className="w-fit" onClick={() => setPreviewAccess("desktop")}>
                <MonitorIcon /> Lihat sebagai aplikasi desktop
              </Button>
            </CardContent>
          </Card>
        )}
      </div>
      <Toaster position="top-center" />
    </div>
  )
}
