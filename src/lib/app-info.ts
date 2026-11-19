/** Informasi aplikasi yang tampil di halaman Tentang. */
export const APP_VERSION: string = typeof __APP_VERSION__ === "string" ? __APP_VERSION__ : "dev"
export const ORG_NAME = "Teliti"
export const ORG_URL = "https://teliti.net/"
export const COPYRIGHT = `© 2026 ${ORG_NAME}`
export const REPO_URL = "https://github.com/teliti-dev/telemon"
export const ISSUES_URL = `${REPO_URL}/issues`
export const RELEASES_PAGE_URL = `${REPO_URL}/releases`
export const LICENSE_URL = `${REPO_URL}/blob/master/LICENSE`

/** Komponen pihak ketiga utama dan lisensinya. */
export const THIRD_PARTY: { name: string; license: string }[] = [
  { name: "mikrotik-rs", license: "AGPL-3.0" },
  { name: "Tauri", license: "MIT / Apache-2.0" },
  { name: "Axum, Tokio", license: "MIT" },
  { name: "SQLite", license: "Public domain" },
  { name: "cloudflared (Cloudflare)", license: "Apache-2.0" },
  { name: "React, Vite", license: "MIT" },
  { name: "shadcn/ui, Radix UI", license: "MIT" },
  { name: "Lucide", license: "ISC" },
]
