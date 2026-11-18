import * as React from "react"
import { CopyIcon } from "lucide-react"
import { QRCodeSVG } from "qrcode.react"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { copyText } from "@/hooks/use-copy"
import { useT } from "@/lib/i18n"

/** QR + kode rahasia untuk aplikasi authenticator, lalu isian kode 6 digit untuk memastikan sudah terdaftar. */
export function TotpSetup({
  secret,
  otpauthUrl,
  submitLabel,
  onSubmit,
  onCancel,
}: {
  secret: string
  otpauthUrl: string
  submitLabel: string
  onSubmit: (code: string) => Promise<void>
  onCancel?: () => void
}) {
  const t = useT()
  const [code, setCode] = React.useState("")
  const [busy, setBusy] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await onSubmit(code)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      setBusy(false)
    }
  }

  return (
    <form onSubmit={submit} className="grid gap-3">
      <p className="text-sm text-muted-foreground">
        {t("Pindai QR dengan aplikasi authenticator (Google Authenticator, Authy, 1Password, dan sejenisnya), lalu masukkan kode 6 digit yang muncul.")}
      </p>
      <div className="flex flex-col items-start gap-3 sm:flex-row">
        <div className="w-fit shrink-0 rounded-lg border bg-white p-2">
          <QRCodeSVG value={otpauthUrl} size={132} marginSize={0} aria-label={t("QR kode 2FA")} />
        </div>
        <div className="grid min-w-0 flex-1 gap-1.5">
          <Label className="text-xs text-muted-foreground">{t("Atau masukkan kode ini secara manual")}</Label>
          <div className="flex gap-1.5">
            <Input readOnly value={secret} className="font-mono text-xs tracking-wider" onFocus={(e) => e.target.select()} />
            <Button type="button" variant="outline" size="icon" aria-label={t("Salin kode")} onClick={() => copyText(secret, t("Kode disalin"))}>
              <CopyIcon />
            </Button>
          </div>
        </div>
      </div>
      <div className="grid gap-1.5">
        <Label htmlFor="totp-code">{t("Kode 6 digit")}</Label>
        <Input
          id="totp-code"
          value={code}
          onChange={(e) => setCode(e.target.value.replace(/\D/g, "").slice(0, 6))}
          inputMode="numeric"
          autoComplete="one-time-code"
          placeholder="000000"
          className="font-mono text-base tracking-[0.3em]"
          autoFocus
        />
      </div>
      {error && (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
      <div className="flex justify-end gap-2">
        {onCancel && (
          <Button type="button" variant="outline" onClick={onCancel} disabled={busy}>
            {t("Batal")}
          </Button>
        )}
        <Button type="submit" disabled={code.length !== 6 || busy}>
          {busy ? t("Memeriksa…") : submitLabel}
        </Button>
      </div>
    </form>
  )
}
