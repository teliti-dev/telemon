import { resolve } from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  // Tauri butuh port tetap; layar tidak dibersihkan agar log Rust tetap terlihat.
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    // Dev terhadap server Telemon (mis. server demo): VITE_TELEMON_URL=http://127.0.0.1:7878 bun run dev
    proxy: process.env.VITE_TELEMON_URL ? { "/api": { target: process.env.VITE_TELEMON_URL, ws: true } } : undefined,
  },
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },
})
