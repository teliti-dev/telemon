import { resolve } from "node:path"
import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  // Tauri butuh port tetap; layar tidak dibersihkan agar log Rust tetap terlihat.
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  resolve: {
    alias: {
      "@": resolve(import.meta.dirname, "./src"),
    },
  },
})
