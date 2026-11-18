import { toast } from "sonner"

import { t } from "@/lib/i18n"

export function copyText(text: string, label = t("Disalin")) {
  navigator.clipboard
    .writeText(text)
    .then(() => toast.success(label))
    .catch(() => toast.error(t("Gagal menyalin; salin manual")))
}
