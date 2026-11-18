# Telemon

Aplikasi desktop (Tauri, Windows/Linux), bisa juga dibuka dari browser/HP lewat tombol "Serve ke browser", untuk memantau user PPPoE di banyak router Mikrotik lewat RouterOS API: status user (active, inactive, disabled) dan event (gagal auth, authenticated, connected, disconnected).

- Dokumentasi: repo `enginering-docs`, folder `telemon/docs/`
- Menjalankan: `bun install`, lalu `bun run dev` (UI saja) atau `bun run tauri dev` (aplikasi penuh)
- Tes: belum ada
- Rilis: `bun run tauri build` (Windows `.msi`/`.exe`, Linux `.AppImage`/`.deb`)
