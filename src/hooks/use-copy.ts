import { toast } from "sonner"

export function copyText(text: string, label = "Disalin") {
  navigator.clipboard
    .writeText(text)
    .then(() => toast.success(label))
    .catch(() => toast.error("Gagal menyalin; salin manual"))
}
