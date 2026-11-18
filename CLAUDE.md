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
  - `bun install`, `bun run dev` (UI saja di browser), `bun run build`, `bun run lint`, `bun run typecheck` (`tsc -b`).
  - Komponen shadcn: `bunx --bun shadcn@latest add <komponen>`.
- `src-tauri/`: core Rust (poller RouterOS, SQLite, command/event Tauri). `bun run tauri dev`, `bun run tauri build`.
- Mode **"Serve ke browser"**: tombol di UI menyalakan server Axum di proses yang sama (bawaan mati; LAN port 7878; login akun wajib; hanya baca) yang menyajikan UI yang sama + API JSON dan WebSocket. UI memakai satu lapisan data dengan adapter Tauri dan HTTP; setiap fitur data harus tersedia di keduanya.
- Mode **"Publik"**: Cloudflare Quick Tunnel lewat sidecar `cloudflared` (versi dikunci + checksum) ke server mode serve. Wajib login akun + 2FA, hanya baca, rate limit login, mati otomatis (bawaan 8 jam). Event live via **WebSocket + cadangan polling, jangan SSE** (Quick Tunnel tidak mendukung SSE). Lihat ADR 0003.
- Olympus tidak punya toolchain Rust dan webkit2gtk: build Tauri dilakukan di mesin lain atau CI. Di olympus hanya UI (`bun run dev/build/lint`).
- **Edit user = catatan lokal** (`UserMeta`: nama pelanggan, telepon, label, catatan; disimpan di SQLite Telemon, kunci `routerId:username`). Tidak menulis ke router. Di mode serve/publik (adapter HTTP) catatan hanya bisa dibaca.
- **Mikrotik source of truth** (ADR 0004). Import (`importFromRouter`) menimpa salinan Telemon dengan data router; yang hilang di router ditandai soft delete. Tulis ke router hanya `/ppp/secret` dan `/ppp/profile`: langsung saat simpan setelah dialog tinjau, hanya untuk router `canWrite` yang online, dan hanya dari aplikasi desktop (adapter HTTP/serve/publik menolak semua tulis dan import). Jangan pernah memanggil `remove` pada secret/profile router: hapus = soft delete (secret di-disable, sesi diputus; profile hanya ditandai di Telemon dan ditolak bila masih dipakai). Password write-only: tidak diimpor, tidak disimpan. Fitur tulis lain butuh ADR baru.
- Kredensial router di keyring OS, bukan di SQLite atau berkas.

## Struktur UI (`src/`)
- `lib/data/api.ts`: kontrak `TelemonApi` (snapshot + live update + aksi). Adapter: `mock.ts` (data contoh, dipakai sekarang); adapter Tauri dan HTTP menyusul.
- `lib/store.tsx` + `lib/store-context.ts`: state global (router, user, event, share) dari adapter.
- `lib/route.ts`: routing hash (`#/user?status=active`), sama di Tauri dan mode serve.
- `pages/`: ringkasan, user, profile, event, router, berbagi. Users dan Profile satu menu sidebar (segmen di atas halaman); sidebar tetap 5 menu. `components/status.tsx`: label/ikon status dan event.
- Penanda **terpilih** (menu aktif atas/bawah, toggle, tab, pilihan di drawer) hanya mengubah warna ikon dan teks ke `text-brand` (biru, token `--brand`, kontras 6,8:1 terang dan 7,2:1 gelap), **tanpa mengubah latar**. Jangan memakai warna status untuk pilihan.
- Drawer/sheet **penuh layar di HP**: beri `data-[side=right]:w-full` (bukan `w-full` biasa, yang kalah oleh `data-[side=right]:w-3/4` bawaan) dan padding area aman; di layar ≥ 640 px lebarnya dibatasi `max-w-sm` bawaan.
- Filter di HP dipadatkan jadi dropdown dalam satu baris (`components/filter-select.tsx`: `FilterSelect`, `FilterMulti`); chip hanya di desktop. Tombol dropdown menampilkan nama filter (redup) saat bawaan dan nilainya (brand) saat aktif.
- Warna status (`--status-good|warning|serious|critical` di `index.css`) hanya untuk titik/ikon, selalu dengan label; teks memakai token teks.
- Header dua baris (`components/site-header.tsx`): logo + Live + Appearance + Profile, lalu menu utama di bawahnya (garis bawah pada menu aktif). Lebar dan jarak kiri-kanan konten satu sumber di `lib/layout.ts` (`PAGE_CONTAINER`, maks 7xl); pakai itu untuk elemen halaman baru.
- Navigasi: **menu atas** di desktop dan **menu bawah** di HP (< 768px, `components/bottom-nav.tsx`); tidak ada sidebar. Daftar menu dan angka lencana satu sumber (`components/nav-items.ts`, `hooks/use-nav-badges.ts`); Berbagi (hanya desktop), Admin, dan Pengaturan tampil di kanan menu atas sesuai izin; di HP masuk "Lainnya" pada menu bawah. Halaman memberi ruang bawah untuk menu itu (`pb-[calc(4.5rem+env(safe-area-inset-bottom))]`).
- Kepadatan: kontrol (`Button`, `Input`, `Select`, `Toggle`, `Tabs`) 36 px di HP dan 28 px di md ke atas. Pakai varian `md:` bila menambah kontrol baru, jangan mengecilkan di HP (area ketuk). **Jangan mengubah font-size root** (`html`): hampir semua ukuran Tailwind berbasis `rem`, jadi jarak, tinggi kontrol, dan ikon ikut menyusut (pernah membuat UI terlalu kecil). Tema dan ukuran tampilan (90-125%, `lib/ui-scale.ts`) diatur pengguna lewat drawer **Appearance** di kanan (`components/appearance-drawer.tsx`).
- **Dua mode akses** (`lib/access.ts`, ADR 0005): `desktop` (Tauri; tanpa login, boleh menulis ke router) dan `remote` (browser lewat serve/publik; wajib login, hanya baca). Pakai `useStore().access`, jangan `api.kind`, untuk membedakan keduanya. `components/auth-gate.tsx` memasang login di mode `remote`.
- Akun jarak jauh: login + 2FA TOTP + kode pemulihan, halaman Akun saya (`pages/account.tsx`). **Peran kustom dengan izin per menu** (ADR 0006): tiap menu (Ringkasan, Users, Profil PPP, Event, Router, Admin, Pengaturan) dicentang **Lihat** dan/atau **Ubah** (bawaan tanpa akses; Ubah mencakup Lihat; menu tanpa fitur ubah hanya punya Lihat); `lib/permissions.ts` (daftar menu, label, `pageDenied`), `hooks/use-write-gate.ts` (`useUpdateGate(resource)`, `useWriteGate(resource)` untuk izin + syarat router). Jangan membandingkan `access` langsung untuk izin tulis; pakai gate itu. Server harus menegakkan izin di setiap rute; UI hanya menyembunyikan dan menjelaskan.
- Istilah: **Profil PPP** = profile di router (`/ppp/profile`); **Akun saya** = profil sendiri akun jarak jauh (tanpa izin). Jangan menyebut keduanya "Profile".
- **Jarak jauh boleh edit dan tambah sesuai peran**, termasuk Router (tambah/edit/hapus), Admin, dan Pengaturan; yang tetap **hanya desktop**: Berbagi (serve/publik) dan izin tulis lewat publik. Pengaman anti-kenaikan izin (izin tidak melebihi milik sendiri, tidak mengubah akun/peran sendiri, tidak mengelola akun berizin lebih tinggi) ditegakkan server dan ada di adapter contoh. Akun berperan Ubah wajib 2FA; tulis lewat publik mati bawaan.
- Nilai bawaan pengaturan dan peran satu sumber di `lib/defaults.ts` (`DEFAULT_SETTINGS`, `DEFAULT_ROLES`); tabelnya ada di ADR 0006. Ubah di sana, bukan di UI.
- Adapter contoh (`lib/data/mock.ts`) memuat akun dan kode demo untuk mencoba alur; **jangan ikut build rilis**. Di mode contoh, drawer Appearance memuat pilihan pratinjau desktop/jarak jauh. Tidak ada saklar desktop/mobile di header: tata letak mengikuti lebar layar.
- Dev dari HP/laptop lain: `bun run dev --host`.
