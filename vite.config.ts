import { resolve } from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

const devTarget = process.env.VITE_TELEMON_URL ?? (process.env.TAURI_ENV_PLATFORM ? "http://127.0.0.1:47831" : undefined)

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  // Tauri butuh port tetap; layar tidak dibersihkan agar log Rust tetap terlihat.
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    // Dev terhadap server Telemon (mis. server demo): VITE_TELEMON_URL=http://127.0.0.1:7878 bun run dev.
    // Di bawah `bun run tauri dev` (TAURI_ENV_PLATFORM terisi) otomatis ke server loopback aplikasi desktop.
    proxy: devTarget ? { "/api": { target: devTarget, ws: true }, "/desktop-entry": { target: devTarget } } : undefined,
  },
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },
})
