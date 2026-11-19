# Telemon

Aplikasi desktop (Tauri, Windows/Linux), bisa juga dibuka dari browser/HP lewat tombol "Serve ke browser", untuk memantau user PPPoE di banyak router Mikrotik lewat RouterOS API: status user (active, inactive, disabled) dan event (gagal auth, authenticated, connected, disconnected).

- Dokumentasi: repo `enginering-docs`, folder `telemon/docs/`
- Menjalankan: `bun install`, lalu `bun run dev` (UI saja) atau `bun run tauri dev` (aplikasi penuh)
- Tes: core `cd crates/telemon-core && cargo test` (95), server `cd crates/telemon-http && cargo test` (25), UI `bun run typecheck && bun run lint && bun run i18n:check`
- Mencoba UI dengan server demo (router tiruan): `bun run build`, lalu `cd crates/telemon-demo && cargo build --release && ./target/release/telemon-demo --static ../../dist`
- Rilis: buat Release di GitHub (tag `vX.Y.Z`); workflow `.github/workflows/release.yml` membangun Linux (`.deb`, `.AppImage`) dan Windows (`.exe` NSIS) di runner Linux lalu mengunggahnya. Lisensi AGPL-3.0.
