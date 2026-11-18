import { AccountsCard } from "@/components/accounts-card"
import { RolesCard } from "@/components/roles-card"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useUpdateGate } from "@/hooks/use-write-gate"

/** Admin: akun jarak jauh dan peran (izin per menu). Mengubah butuh izin Ubah pada menu Admin. */
export function AdminPage() {
  const reason = useUpdateGate("admin")()
  const readOnly = !!reason
  return (
    <div className="grid gap-3">
      {readOnly && (
        <Alert>
          <AlertDescription>{reason}. Anda hanya bisa melihat akun dan peran.</AlertDescription>
        </Alert>
      )}
      <Tabs defaultValue="akun" className="gap-3">
        <TabsList>
          <TabsTrigger value="akun">Akun</TabsTrigger>
          <TabsTrigger value="peran">Peran</TabsTrigger>
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
