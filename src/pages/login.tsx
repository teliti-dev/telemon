import * as React from "react"
import { EyeIcon, EyeOffIcon, Loader2Icon, LockKeyholeIcon, MonitorIcon, ZapIcon } from "lucide-react"

import { RecoveryCodes } from "@/components/recovery-codes"
import { AppearanceDrawer } from "@/components/appearance-drawer"
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

const TITLES: Record<Step["name"], { title: string; desc: string }> = {
  credentials: { title: "Masuk", desc: "Gunakan akun yang dibuat pemilik Telemon." },
  totp: { title: "Verifikasi dua langkah", desc: "Masukkan kode dari aplikasi authenticator Anda." },
  setup: { title: "Aktifkan 2FA", desc: "Pemilik Telemon mewajibkan verifikasi dua langkah untuk akun ini." },
  recovery: { title: "Simpan kode pemulihan", desc: "2FA sudah aktif di akun Anda." },
}

export function LoginPage({ onAuthed }: { onAuthed: (s: SessionInfo) => void }) {
  const { api } = useApi()
  const [step, setStep] = React.useState<Step>({ name: "credentials" })
  const [username, setUsername] = React.useState("")
  const [password, setPassword] = React.useState("")
  const [showPassword, setShowPassword] = React.useState(false)
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
    if (busy) return
    void run(async () => {
      const r = await api.login(username, password)
      setPassword("")
      setShowPassword(false)
      if (r.status === "ok") onAuthed(r.session)
      else if (r.status === "2fa_required") setStep({ name: "totp", challengeId: r.challengeId, recovery: false })
      else setStep({ name: "setup", challengeId: r.challengeId, secret: r.secret, otpauthUrl: r.otpauthUrl })
    })
  }

  const verify = (value: string) => {
    if (step.name !== "totp" || busy) return
    void run(async () => {
      try {
        onAuthed(await api.verify2fa(step.challengeId, value, { recovery: step.recovery }))
      } catch (e) {
        setCode("")
        throw e
      }
    })
  }

  const goBack = () => {
    setCode("")
    setError(null)
    setStep({ name: "credentials" })
  }

  const t = TITLES[step.name]

  return (
    <div className="relative flex min-h-svh flex-col items-center justify-start bg-muted/30 px-5 pt-12 pb-8 sm:justify-center sm:pt-8">
      <div className="absolute top-3 right-3">
        <AppearanceDrawer iconOnly />
      </div>

      <div className="grid w-full max-w-sm gap-4">
        <div className="grid justify-items-center gap-2 text-center">
          <div className="flex size-12 items-center justify-center rounded-2xl bg-primary text-primary-foreground shadow-sm">
            <ZapIcon className="size-5" />
          </div>
          <div>
            <div className="text-lg font-semibold">Telemon</div>
            <div className="text-sm text-muted-foreground">Monitoring PPPoE multi-Mikrotik</div>
          </div>
        </div>

        <Card className="rounded-2xl shadow-sm [--card-spacing:--spacing(5)] sm:[--card-spacing:--spacing(6)]">
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
                    autoCorrect="off"
                    spellCheck={false}
                    className="h-10 md:h-10"
                    autoFocus
                  />
                </div>
                <div className="grid gap-1.5">
                  <Label htmlFor="l-pass">Password</Label>
                  <div className="relative">
                    <Input
                      id="l-pass"
                      type={showPassword ? "text" : "password"}
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      autoComplete="current-password"
                      className="h-10 pr-10 md:h-10"
                    />
                    <button
                      type="button"
                      onClick={() => setShowPassword((v) => !v)}
                      aria-label={showPassword ? "Sembunyikan password" : "Tampilkan password"}
                      aria-pressed={showPassword}
                      className="absolute inset-y-0 right-0 flex w-10 items-center justify-center rounded-r-lg text-muted-foreground outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
                    >
                      {showPassword ? <EyeOffIcon className="size-4" /> : <EyeIcon className="size-4" />}
                    </button>
                  </div>
                </div>
                {error && (
                  <Alert variant="destructive">
                    <AlertDescription>{error}</AlertDescription>
                  </Alert>
                )}
                <Button type="submit" className="h-10 md:h-10" disabled={busy || !username.trim() || !password}>
                  {busy && <Loader2Icon className="animate-spin" />}
                  {busy ? "Memeriksa…" : "Masuk"}
                </Button>
                <p className="text-center text-xs text-muted-foreground">
                  Lupa password? Minta pemilik Telemon mereset akun Anda dari aplikasi desktop.
                </p>
              </form>
            )}

            {step.name === "totp" && (
              <form
                onSubmit={(e) => {
                  e.preventDefault()
                  verify(code)
                }}
                className="grid gap-3"
              >
                <div className="grid gap-1.5">
                  <Label htmlFor="l-code">{step.recovery ? "Kode pemulihan" : "Kode 6 digit"}</Label>
                  <Input
                    id="l-code"
                    value={code}
                    onChange={(e) => {
                      const v = step.recovery
                        ? e.target.value.trim().toLowerCase()
                        : e.target.value.replace(/\D/g, "").slice(0, 6)
                      setCode(v)
                      // Kode authenticator langsung diperiksa begitu 6 digit terisi.
                      if (!step.recovery && v.length === 6) verify(v)
                    }}
                    inputMode={step.recovery ? "text" : "numeric"}
                    autoComplete="one-time-code"
                    autoCapitalize="off"
                    spellCheck={false}
                    placeholder={step.recovery ? "xxxxx-xxxxx" : "000000"}
                    className={step.recovery ? "h-10 font-mono md:h-10" : "h-12 text-center font-mono text-lg tracking-[0.4em] md:h-12"}
                    disabled={busy}
                    autoFocus
                  />
                </div>
                {error && (
                  <Alert variant="destructive">
                    <AlertDescription>{error}</AlertDescription>
                  </Alert>
                )}
                {(step.recovery || busy) && (
                  <Button type="submit" className="h-10 md:h-10" disabled={busy || code.length < 11}>
                    {busy && <Loader2Icon className="animate-spin" />}
                    {busy ? "Memeriksa…" : "Verifikasi"}
                  </Button>
                )}
                <div className="flex justify-between text-sm">
                  <button
                    type="button"
                    className="text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
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
                    className="text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
                    onClick={goBack}
                  >
                    Kembali
                  </button>
                </div>
                <p className="text-center text-xs text-muted-foreground">
                  Kehilangan aplikasi authenticator dan kode pemulihan? Minta pemilik Telemon mereset 2FA akun Anda.
                </p>
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
                onCancel={goBack}
              />
            )}

            {step.name === "recovery" && (
              <>
                <RecoveryCodes codes={step.codes} />
                <label className="flex items-center gap-2 text-sm">
                  <input type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} className="size-4" />
                  Saya sudah menyimpan kode ini
                </label>
                <Button className="h-10 md:h-10" disabled={!saved} onClick={() => onAuthed(step.session)}>
                  Lanjut
                </Button>
              </>
            )}
          </CardContent>
        </Card>

        <p className="flex items-center justify-center gap-1.5 text-xs text-muted-foreground">
          <LockKeyholeIcon className="size-3" /> Akses lewat browser hanya untuk melihat data
        </p>

        {api.kind === "mock" && (
          <details className="rounded-xl border bg-card text-xs">
            <summary className="cursor-pointer px-4 py-2.5 font-medium select-none">Mode data contoh: akun untuk mencoba</summary>
            <div className="grid gap-2 border-t px-4 py-3">
              <ul className="grid gap-1 text-muted-foreground">
                <li>
                  <span className="font-mono text-foreground">admin</span> / <span className="font-mono text-foreground">demo-telemon</span>, kode 2FA{" "}
                  <span className="font-mono text-foreground">123456</span>
                </li>
                <li>
                  <span className="font-mono text-foreground">operator</span> / <span className="font-mono text-foreground">demo-telemon</span>: peran Ubah,
                  wajib daftar 2FA saat masuk (kode <span className="font-mono text-foreground">123456</span>)
                </li>
                <li>
                  <span className="font-mono text-foreground">pantau</span> / <span className="font-mono text-foreground">demo-telemon</span>: peran Monitor, tanpa 2FA
                </li>
                <li>
                  <span className="font-mono text-foreground">tamu</span>: akun nonaktif
                </li>
              </ul>
              <Button variant="outline" size="sm" className="w-fit" onClick={() => setPreviewAccess("desktop")}>
                <MonitorIcon /> Lihat sebagai aplikasi desktop
              </Button>
            </div>
          </details>
        )}
      </div>
      <Toaster position="top-center" />
    </div>
  )
}
