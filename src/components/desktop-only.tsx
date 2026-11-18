import { ShieldAlertIcon } from "lucide-react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"

export function DesktopOnly({ what }: { what: string }) {
  return (
    <Alert className="max-w-lg">
      <ShieldAlertIcon />
      <AlertTitle>Hanya di aplikasi desktop</AlertTitle>
      <AlertDescription>{what} hanya bisa diubah dari aplikasi Telemon di komputer.</AlertDescription>
    </Alert>
  )
}
