import { navFor } from "@/components/nav-items"
import { useAuth } from "@/lib/auth-context"
import { useStore } from "@/lib/store-context"

/** Menu yang boleh dilihat akun ini (aplikasi desktop: semua). */
export function useNavItems() {
  const { access } = useStore()
  const { session } = useAuth()
  return navFor(access, session?.permissions)
}
