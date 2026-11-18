import { AccountsCard } from "@/components/accounts-card"
import { RolesCard } from "@/components/roles-card"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useT } from "@/lib/i18n"
import { useUpdateGate } from "@/hooks/use-write-gate"
import { useRoute } from "@/lib/route"

/** Admin: akun jarak jauh dan peran (izin per menu). Mengubah butuh izin Ubah pada menu Admin. */
export function AdminPage() {
  const t = useT()
  const reason = useUpdateGate("admin")()
  const readOnly = !!reason
  const { params } = useRoute()
  return (
    <div className="grid gap-3">
      {readOnly && (
        <Alert>
          <AlertDescription>{t("{reason}. Anda hanya bisa melihat akun dan peran.", { reason })}</AlertDescription>
        </Alert>
      )}
      <Tabs defaultValue={params.get("tab") === "peran" ? "peran" : "akun"} className="gap-3">
        <TabsList>
          <TabsTrigger value="akun">{t("Akun")}</TabsTrigger>
          <TabsTrigger value="peran">{t("Peran")}</TabsTrigger>
        </TabsList>
        <TabsContent value="akun">
          <AccountsCard readOnly={readOnly} />
        </TabsContent>
        <TabsContent value="peran">
          <RolesCard readOnly={readOnly} />
        </TabsContent>
      </Tabs>
    </div>
  )
}
