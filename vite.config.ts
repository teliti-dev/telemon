import { execSync } from "node:child_process"
import { readFileSync } from "node:fs"
import { resolve } from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

const devTarget = process.env.VITE_TELEMON_URL ?? (process.env.TAURI_ENV_PLATFORM ? "http://127.0.0.1:47831" : undefined)

const pkg = JSON.parse(readFileSync(resolve(import.meta.dirname, "package.json"), "utf8")) as { version: string }

/**
 * Versi untuk halaman Tentang. Build rilis (tag) memakai package.json apa adanya (disamakan dengan tag oleh workflow).
 * Build lain diberi penanda commit supaya jelas asalnya: di Actions (manual) `0.0.0-ci+<sha>`, di lokal
 * `<versi>+<jumlah commit sejak tag>.<sha>` (kosong bila tepat di tag).
 */
function appVersion(): string {
  const git = (cmd: string) => {
    try {
      return execSync(`git ${cmd}`, { cwd: import.meta.dirname, stdio: ["ignore", "pipe", "ignore"] }).toString().trim()
    } catch {
      return ""
    }
  }
  if (process.env.GITHUB_ACTIONS) {
    const sha = (process.env.GITHUB_SHA ?? "").slice(0, 7)
    return pkg.version.endsWith("-ci") && sha ? `${pkg.version}+${sha}` : pkg.version
  }
  const m = /-(\d+)-g([0-9a-f]+)(-dirty)?$/.exec(git("describe --tags --long --dirty"))
  if (!m) return pkg.version
  return m[1] === "0" && !m[3] ? pkg.version : `${pkg.version}+${m[1]}.${m[2]}${m[3] ? ".dirty" : ""}`
}

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  // Versi aplikasi untuk halaman Tentang; workflow rilis menyamakan package.json dengan tag sebelum build.
  define: { __APP_VERSION__: JSON.stringify(appVersion()) },
  // Tauri butuh port tetap; layar tidak dibersihkan agar log Rust tetap terlihat.
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    // Dev terhadap server Telemon (mis. server demo): VITE_TELEMON_URL=http://127.0.0.1:7878 bun run dev.
    // Di bawah `bun run tauri dev` (TAURI_ENV_PLATFORM terisi) otomatis ke server loopback aplikasi desktop.
    proxy: devTarget ? { "/api": { target: devTarget, ws: true }, "/desktop-entry": { target: devTarget }, "/ext": { target: devTarget } } : undefined,
  },
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },
})
