import { AccountsCard } from "@/components/accounts-card"
import { DesktopOnly } from "@/components/desktop-only"
import { RolesCard } from "@/components/roles-card"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useStore } from "@/lib/store-context"

/** Admin: akun jarak jauh dan peran (izin per menu). Hanya di aplikasi desktop. */
export function AdminPage() {
  const { access } = useStore()
  if (access !== "desktop") return <DesktopOnly what="Pengelolaan akun dan peran" />
  return (
    <Tabs defaultValue="akun" className="gap-3">
      <TabsList>
        <TabsTrigger value="akun">Akun</TabsTrigger>
        <TabsTrigger value="peran">Peran</TabsTrigger>
      </TabsList>
      <TabsContent value="akun">
        <AccountsCard />
      </TabsContent>
      <TabsContent value="peran">
        <RolesCard />
      </TabsContent>
    </Tabs>
  )
}
