import { AppearanceBody } from "@/components/appearance-drawer"
import { PageHeader } from "@/components/page-header"
import { useT } from "@/lib/i18n"
import { navigate } from "@/lib/route"

/** Pengaturan tampilan sebagai halaman (dibuka dari tombol Appearance di HP). */
export function AppearancePage() {
  const t = useT()
  const back = () => (window.history.length > 1 ? window.history.back() : navigate("ringkasan"))
  return (
    <div className="mx-auto max-w-xl">
      <PageHeader title={t("Appearance")} subtitle={t("Tema, bahasa, dan ukuran tampilan. Pilihan disimpan di browser ini.")} onBack={back} />
      <AppearanceBody />
    </div>
  )
}
