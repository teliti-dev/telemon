import { CopyIcon, TriangleAlertIcon } from "lucide-react"

import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { copyText } from "@/hooks/use-copy"
import { useT } from "@/lib/i18n"

/** Kode pemulihan 2FA: tiap kode hanya bisa dipakai sekali. Ditampilkan sekali saja. */
export function RecoveryCodes({ codes }: { codes: string[] }) {
  const t = useT()
  return (
    <div className="grid gap-3">
      <Alert>
        <TriangleAlertIcon />
        <AlertDescription>
          {t("Simpan kode ini di tempat aman (misalnya pengelola password). Kode hanya ditampilkan sekarang, dan tiap kode cuma bisa dipakai sekali bila Anda kehilangan akses ke aplikasi authenticator.")}
        </AlertDescription>
      </Alert>
      <ul className="grid grid-cols-2 gap-1.5 rounded-lg border p-3 font-mono text-[13px]">
        {codes.map((c) => (
          <li key={c}>{c}</li>
        ))}
      </ul>
      <Button
        type="button"
        variant="outline"
        className="w-fit"
        onClick={() => copyText(codes.join("\n"), t("Kode pemulihan disalin"))}
      >
        <CopyIcon /> {t("Salin semua")}
      </Button>
    </div>
  )
}
