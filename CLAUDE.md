# Aturan repo

Dokumentasi repo ini ada di repo `enginering-docs`, folder `telemon/docs/` (scope `teliti/telemon`). Di workspace: `../docs/`.

1. Pekerjaan belum selesai sebelum dokumennya diperbarui: ubah/buat dokumen, naikkan `updated`, catat di `worklog/`, buat ADR untuk keputusan baru.
2. Jalankan `python3 ../../tools/docs-kit/docs.py index --docs telemon/docs` lalu `check --all` (0 error) sebelum commit docs.
3. Jangan mencampur scope: dokumen proyek ini hanya di `telemon/docs`.
4. Rahasia tidak ditulis di kode, commit, maupun dokumen; hanya lokasinya (Vaultwarden).
5. Pesan commit dan PR tanpa atribusi Claude.

## Struktur dan perintah
Aplikasi desktop **Tauri v2** (Windows dan Linux, tanpa deploy). Lihat ADR 0001-0003 di `telemon/docs/decisions`.
- Akar repo: UI React (Vite + TypeScript + shadcn/ui), dikelola dengan **bun**. Fokus tahap awal: UI dengan data contoh, poller menyusul. **Wajib responsif mobile** karena bisa dibuka dari HP lewat mode serve (tabel jadi kartu di lebar kecil, sidebar jadi sheet).
  - `bun install`, `bun run dev` (UI saja di browser), `bun run build`, `bun run lint`.
  - Komponen shadcn: `bunx --bun shadcn@latest add <komponen>`.
- `src-tauri/`: core Rust (poller RouterOS, SQLite, command/event Tauri). `bun run tauri dev`, `bun run tauri build`.
- Mode **"Serve ke browser"**: tombol di UI menyalakan server Axum di proses yang sama (bawaan mati; LAN port 7878; token akses wajib; hanya baca) yang menyajikan UI yang sama + API JSON dan WebSocket. UI memakai satu lapisan data dengan adapter Tauri dan HTTP; setiap fitur data harus tersedia di keduanya.
- Mode **"Publik"**: Cloudflare Quick Tunnel lewat sidecar `cloudflared` (versi dikunci + checksum) ke server mode serve. Wajib PIN (hash), hanya baca, rate limit login, mati otomatis (bawaan 8 jam). Event live via **WebSocket + cadangan polling, jangan SSE** (Quick Tunnel tidak mendukung SSE). Lihat ADR 0003.
- Olympus tidak punya toolchain Rust dan webkit2gtk: build Tauri dilakukan di mesin lain atau CI. Di olympus hanya UI (`bun run dev/build/lint`).
- **Edit user = catatan lokal** (`UserMeta`: nama pelanggan, telepon, label, catatan; disimpan di SQLite Telemon, kunci `routerId:username`). Tidak menulis ke router. Di mode serve/publik (adapter HTTP) catatan hanya bisa dibaca.
- Telemon hanya **membaca** router (akun API grup `read`); jangan menambah fitur yang menulis ke router tanpa ADR. Kredensial router di keyring OS, bukan di SQLite atau berkas.

## Struktur UI (`src/`)
- `lib/data/api.ts`: kontrak `TelemonApi` (snapshot + live update + aksi). Adapter: `mock.ts` (data contoh, dipakai sekarang); adapter Tauri dan HTTP menyusul.
- `lib/store.tsx` + `lib/store-context.ts`: state global (router, user, event, share) dari adapter.
- `lib/route.ts`: routing hash (`#/user?status=active`), sama di Tauri dan mode serve.
- `pages/`: ringkasan, user, event, router, berbagi. `components/status.tsx`: label/ikon status dan event.
- Warna status (`--status-good|warning|serious|critical` di `index.css`) hanya untuk titik/ikon, selalu dengan label; teks memakai token teks.
- Dev dari HP/laptop lain: `bun run dev --host`.
