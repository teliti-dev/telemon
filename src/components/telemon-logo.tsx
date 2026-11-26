import { cn } from "@/lib/utils"

/**
 * Logo Telemon: telapak kaki dengan garis denyut (memantau) dan titik Live hijau, berwarna biru brand.
 * Sumber: `public/telemon.svg` (juga favicon dan sumber ikon aplikasi Tauri).
 */
export function TelemonLogo({ className }: { className?: string }) {
  return <img src="/telemon.svg" alt="" aria-hidden="true" draggable={false} className={cn("size-8 shrink-0 select-none", className)} />
}
