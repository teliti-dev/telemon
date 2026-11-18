import { SparklesIcon } from "lucide-react"

import { AiChatPanel } from "@/components/ai-chat-panel"
import { Button } from "@/components/ui/button"
import { Sheet, SheetContent, SheetDescription, SheetTitle } from "@/components/ui/sheet"
import { useIsDesktop } from "@/hooks/use-media-query"
import { useAiChat } from "@/lib/ai-context"
import { useT } from "@/lib/i18n"

/** Lebar panel AI di desktop; halaman diberi ruang sebesar ini (lihat App). */
export const AI_DOCK_WIDTH = "26rem"

/**
 * Penempatan AI chat:
 * - Desktop/tablet: tombol bulat melayang di kanan bawah; chat terbuka sebagai panel yang
 *   menempel di sisi kanan (bukan jendela melayang) dan mendorong isi halaman.
 * - HP: dibuka dari tombol AI di ujung kanan menu bawah, sebagai layar penuh.
 */
export function AiChatDock() {
  const t = useT()
  const ai = useAiChat()
  const isDesktop = useIsDesktop()

  if (!ai.available && !ai.open) return null

  if (isDesktop) {
    return ai.open ? (
      <aside aria-label={t("AI chat")} className="fixed inset-y-0 right-0 z-40 w-[26rem] border-l bg-background">
        <AiChatPanel onClose={() => ai.setOpen(false)} />
      </aside>
    ) : (
      <Button
        size="icon"
        className="fixed right-5 bottom-5 z-30 size-12 rounded-full shadow-lg md:size-12"
        aria-label={t("Buka AI chat")}
        title={t("Tanya AI")}
        onClick={() => ai.setOpen(true)}
      >
        <SparklesIcon className="size-5" />
      </Button>
    )
  }

  return (
    <Sheet open={ai.open} onOpenChange={ai.setOpen}>
      <SheetContent
        showCloseButton={false}
        className="gap-0 p-0 data-[side=right]:w-full pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)]"
      >
        <SheetTitle className="sr-only">{t("Tanya AI")}</SheetTitle>
        <SheetDescription className="sr-only">{t("Hanya membaca data Telemon; tidak mengubah apa pun.")}</SheetDescription>
        <AiChatPanel onClose={() => ai.setOpen(false)} />
      </SheetContent>
    </Sheet>
  )
}
