import * as React from "react"

import { AddAccountForm, ChangeRoleForm, ResetPasswordForm } from "@/components/accounts-card"
import { FormLayoutProvider } from "@/components/form-layout"
import { AiProviderForm } from "@/components/ai-provider-dialog"
import { ImportBody } from "@/components/import-dialog"
import { PageHeader } from "@/components/page-header"
import { ProfileForm } from "@/components/profile-form-dialog"
import { RoleForm } from "@/components/role-dialog"
import { RouterForm } from "@/components/router-dialog"
import { PageSkeleton } from "@/components/skeletons"
import { UserForm } from "@/components/user-form-dialog"
import { Button } from "@/components/ui/button"
import { useApi } from "@/lib/api-context"
import { useT } from "@/lib/i18n"
import { navigate, navigatePath } from "@/lib/route"
import { useStore } from "@/lib/store-context"
import type { AccountSummary, AiProvider, Role } from "@/lib/types"

/**
 * Halaman bertingkat untuk HP: formulir yang di desktop berupa dialog menjadi halaman penuh
 * dengan alamat sendiri. Formulirnya sama (components/*-dialog.tsx); yang berbeda hanya
 * pembungkus (header halaman, tombol bawah yang menempel) lewat FormLayoutProvider.
 */

function Wrap({ title, subtitle, onBack, children }: { title: React.ReactNode; subtitle?: React.ReactNode; onBack: () => void; children: React.ReactNode }) {
  return (
    <div className="mx-auto max-w-xl">
      <PageHeader title={title} subtitle={subtitle} onBack={onBack} />
      <FormLayoutProvider layout="page">{children}</FormLayoutProvider>
    </div>
  )
}

function NotFound({ message, onBack }: { message: string; onBack: () => void }) {
  return (
    <div className="mx-auto max-w-xl">
      <PageHeader title={message} onBack={onBack} />
    </div>
  )
}

const goBackOr = (fallback: () => void) => () => (window.history.length > 1 ? window.history.back() : fallback())

export function UserFormPage({ mode, userId, router }: { mode: "create" | "edit"; userId?: string; router?: string }) {
  const t = useT()
  const { users } = useStore()
  const user = mode === "edit" ? users.find((u) => u.id === userId) : undefined
  const back = () => (mode === "edit" && userId ? navigatePath("user", [userId]) : navigate("user"))
  if (mode === "edit" && !user) return <NotFound message={t("User tidak ditemukan")} onBack={() => navigate("user")} />
  return (
    <Wrap title={user ? t("Edit {user}", { user: user.username }) : t("Tambah user")} subtitle={user ? undefined : undefined} onBack={back}>
      <UserForm mode={mode} user={user} defaultRouterId={router} onClose={back} />
    </Wrap>
  )
}

export function ProfileFormPage({ mode, profileId, router }: { mode: "create" | "edit"; profileId?: string; router?: string }) {
  const t = useT()
  const { profiles } = useStore()
  const profile = mode === "edit" ? profiles.find((p) => p.id === profileId) : undefined
  const back = () => navigate("profile")
  if (mode === "edit" && !profile) return <NotFound message={t("Profil PPP tidak ditemukan")} onBack={back} />
  return (
    <Wrap title={profile ? t("Edit profil PPP {name}", { name: profile.name }) : t("Tambah profil PPP")} onBack={back}>
      <ProfileForm mode={mode} profile={profile} defaultRouterId={router} onClose={back} />
    </Wrap>
  )
}

export function RouterFormPage({ routerId }: { routerId?: string }) {
  const t = useT()
  const { routers } = useStore()
  const router = routerId ? routers.find((r) => r.id === routerId) : undefined
  const back = () => navigate("router")
  if (routerId && !router) return <NotFound message={t("Router tidak ditemukan")} onBack={back} />
  return (
    <Wrap title={router ? t("Edit {name}", { name: router.name }) : t("Tambah router")} onBack={back}>
      <RouterForm router={router} onClose={back} />
    </Wrap>
  )
}

export function ImportPage({ router }: { router?: string }) {
  const t = useT()
  const back = goBackOr(() => navigate("router"))
  return (
    <Wrap title={t("Import dari Mikrotik")} onBack={back}>
      <ImportBody defaultRouterId={router} onClose={back} />
    </Wrap>
  )
}

/** Memuat akun dan peran untuk halaman Admin bertingkat. */
function useAdminData() {
  const { api } = useApi()
  const [data, setData] = React.useState<{ accounts: AccountSummary[]; roles: Role[] } | null>(null)
  const [error, setError] = React.useState<string | null>(null)
  React.useEffect(() => {
    let cancelled = false
    Promise.all([api.listAccounts(), api.listRoles()])
      .then(([accounts, roles]) => !cancelled && setData({ accounts, roles }))
      .catch((e: unknown) => !cancelled && setError(e instanceof Error ? e.message : String(e)))
    return () => {
      cancelled = true
    }
  }, [api])
  return { data, error }
}

const backToAccounts = () => navigate("admin", { tab: "akun" })
const backToRoles = () => navigate("admin", { tab: "peran" })

export function AccountFormPage({ kind, accountId }: { kind: "new" | "role" | "password"; accountId?: string }) {
  const t = useT()
  const { data, error } = useAdminData()
  if (error) return <NotFound message={error} onBack={backToAccounts} />
  if (!data) return <PageSkeleton page="admin" />
  const account = accountId ? data.accounts.find((a) => a.id === accountId) : undefined
  if (kind !== "new" && !account) return <NotFound message={t("Akun tidak ditemukan")} onBack={backToAccounts} />
  const title = kind === "new" ? t("Tambah akun") : kind === "role" ? t("Ubah peran {user}", { user: account!.username }) : t("Reset password {user}", { user: account!.username })
  return (
    <Wrap title={title} onBack={backToAccounts}>
      {kind === "new" && <AddAccountForm roles={data.roles} onDone={() => undefined} onClose={backToAccounts} />}
      {kind === "role" && <ChangeRoleForm account={account!} roles={data.roles} onDone={() => undefined} onClose={backToAccounts} />}
      {kind === "password" && <ResetPasswordForm account={account!} onClose={backToAccounts} />}
    </Wrap>
  )
}

export function RoleFormPage({ roleId }: { roleId?: string }) {
  const t = useT()
  const { data, error } = useAdminData()
  if (error) return <NotFound message={error} onBack={backToRoles} />
  if (!data) return <PageSkeleton page="admin" />
  const role = roleId ? data.roles.find((r) => r.id === roleId) : undefined
  if (roleId && !role) return <NotFound message={t("Peran tidak ditemukan")} onBack={backToRoles} />
  return (
    <Wrap title={role ? t("Edit peran {name}", { name: role.name }) : t("Tambah peran")} onBack={backToRoles}>
      <RoleForm role={role} onDone={() => undefined} onClose={backToRoles} />
    </Wrap>
  )
}

/** Tombol kembali kecil untuk halaman yang belum ditemukan. */
export function BackButton({ onBack, label }: { onBack: () => void; label: string }) {
  return (
    <Button variant="outline" size="sm" onClick={onBack}>
      {label}
    </Button>
  )
}

export function AiProviderFormPage({ providerId }: { providerId?: string }) {
  const t = useT()
  const { api } = useApi()
  const [state, setState] = React.useState<{ provider?: AiProvider; loading: boolean }>({ loading: !!providerId })
  const back = () => navigatePath("pengaturan", ["ai"])
  React.useEffect(() => {
    if (!providerId) return
    let cancelled = false
    api
      .getAiDashboard(7)
      .then((d) => !cancelled && setState({ provider: d.providers.find((p) => p.id === providerId), loading: false }))
      .catch(() => !cancelled && setState({ loading: false }))
    return () => {
      cancelled = true
    }
  }, [api, providerId])
  if (state.loading) return <PageSkeleton page="pengaturan" />
  if (providerId && !state.provider) return <NotFound message={t("Provider tidak ditemukan")} onBack={back} />
  return (
    <Wrap title={state.provider ? t("Edit {name}", { name: state.provider.name }) : t("Tambah provider AI")} onBack={back}>
      <AiProviderForm provider={state.provider} onClose={back} />
    </Wrap>
  )
}
