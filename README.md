<p align="center"><img src="public/telemon.svg" alt="Telemon" width="96" height="96"></p>

# Telemon

Monitor PPPoE users and events across many MikroTik routers, from a desktop app or any browser.

Telemon reads each router over the RouterOS API and shows who is connected, who is inactive or disabled, and what just happened (auth failed, authenticated, connected, disconnected). The MikroTik router stays the source of truth: Telemon imports from it, and when you allow it, writes PPP secrets and profiles back with a review step.

![Overview](docs/screenshots/overview.png)

> Screenshots come from the bundled demo server with simulated routers and made-up users. No real data.

## Status

Pre-release (`v0.1.0-rc.x`). The core, server and UI are tested against a simulated RouterOS. **It has not been run against a real MikroTik router yet (this includes bandwidth charts, Router health and the SSH terminal).** The Linux `.deb` has been smoke-tested in a clean Ubuntu 24.04 container (installs, starts, serves the UI in the real webview); the Windows build has been tried on a real PC by the maintainer and works; a real desktop keyring on Linux has not been tried. The macOS build crashes at launch on at least one Apple Silicon Mac (under investigation). Treat it as a release candidate and keep write access off until you have tried it on a lab router.

## Features

- **Multi-router monitoring.** Users with status (active, inactive, disabled, auth failed), live events, per-router health, filters and search.
- **Widget overview.** The Overview page is a grid of widgets (counters, router cards, latest events, events per hour, users per profile and more). Accounts with the Overview *Edit* permission can drag, resize, add and remove widgets; the layout is shared by every account, and an empty layout falls back to the default. Editing is on screens medium and wider; phones show the widgets in one column.
- **Bandwidth charts.** Telemon reads each router's interface counters on an interval you set (default 60 s, kept 7 days) and turns them into rates. Three widgets use it: a chart per router and interface, the total across all routers' PPPoE sessions, and the average per active user. Pick the router, interface and range in the widget's settings. PPPoE sessions are summed into one `@pppoe` series per router, so totals and per-user averages need no per-session history. The first sample appears one interval after a router is added.
- **Desktop app** (Windows, Linux and macOS, Tauri v2) with a local SQLite database. No server to deploy.
- **Share to a browser.** One switch turns the desktop app into a LAN server so you can open the same UI from a laptop or phone.
- **Public access.** An optional Cloudflare Quick Tunnel exposes the server to the internet. Telemon downloads a pinned, checksum-verified `cloudflared` for you. While public access is on, two-factor authentication is forced and the tunnel turns off by itself after a timer.
- **Accounts and roles** for browser access: login with TOTP two-factor authentication and recovery codes, custom roles with per-menu permissions (no access, view, edit), and protection against privilege escalation. The desktop app itself needs no login.
- **Write back to the router** (desktop only, per router, off by default): add, edit and soft-delete PPP secrets and profiles after a review dialog, with a change history. Telemon never deletes a secret or profile on the router; it disables or marks them.
- **Secure storage.** Router credentials and PPPoE passwords are encrypted (ChaCha20-Poly1305). The database key lives in the OS keyring, with a `0600` file fallback when no keyring is available. TLS to the router supports certificate fingerprint pinning.
- **Signed in-app updates** with a stable and a test channel. The database is backed up before an update installs.
- **AI assistant** (read-only, off until you add a provider): ask questions about your users and events in plain language. It uses OpenAI-compatible APIs (OpenAI, DeepSeek, Gemini, 9Router) or Anthropic, only reads data the asker's role may see, masks IPs and MACs by default, never sees passwords, and has a monthly budget with a usage dashboard. Besides lookups it can compute **event statistics** (per hour or day, most auth failures and disconnects), a **bandwidth summary** (now, average, peak, per user, change versus the previous period, per router) and **router health** (downtime and availability). It can also **propose Overview widget changes** (add, remove, change a widget's router, interface or range): a proposal appears as a card and nothing changes until you press Apply, which uses your own permission. Customer data is sent to the provider you choose.
- **Extensions (official only for now).** Signed `.tmx` packages (Ed25519) add features to the app: Overview widgets, pages, router actions, bottom-panel tabs, scheduled workers, API routes and a sandboxed WASM backend, with their own database tables through declarative migrations. Install from the built-in store (a signed catalogue), or from a file or link; dependencies and Telemon version ranges are checked before install. Every permission has a toggle the core enforces, each extension has its own Access page per role (none, view, edit plus declared actions), and changes are audited. The first extensions are **Router health** and **Router terminal (SSH)**; the terminal opens from a router's action menu in a bottom panel, pins the router's SSH host key and needs the Router *Edit* permission. On by default in the desktop app; off on a server until `TELEMON_EXTENSIONS=1`. Extension packages live in [`telemon-extensions`](https://github.com/teliti-dev/telemon-extensions); this repo holds the host and the SDK contract (`src/lib/extensions/sdk.ts`). See ADR 0028 and 0029.
- **Command palette.** Press Ctrl/Cmd+K (or the Search button) to jump anywhere, Spotlight style: arrow keys move, Enter opens, Esc closes. Menu is searched by default; Routers, Users and Other (PPP profiles, extension pages) are switches under the palette or in Appearance, remembered per browser or app, and only offered when your role may see them.
- **Compact header and menu.** The header holds the logo and Appearance (plus your account in a browser) with the compact menu underneath: Overview, Users, Events, Routers and one **Settings** dropdown that groups Settings (General, AI, Extensions), Manage (Admin, Sharing) and About. In a browser the Search box sits in the middle of the header; in the desktop app press Ctrl/Cmd+K, and a green Live dot you can click shows status (routers online, active users, last event, sharing, version). On Windows, Linux and macOS the header doubles as the title bar; the desktop window has a minimum and maximum size. Phones keep the bottom menu.
- **Bilingual UI** (English and Indonesian), light and dark themes, responsive down to phone width.

| Users | User detail |
|---|---|
| ![Users](docs/screenshots/users.png) | ![User detail](docs/screenshots/user-detail.png) |

| Events | Settings |
|---|---|
| ![Events](docs/screenshots/events.png) | ![Settings](docs/screenshots/settings.png) |

| Command palette (Ctrl+K) | Extensions |
|---|---|
| ![Command palette](docs/screenshots/command-palette.png) | ![Extensions](docs/screenshots/extensions.png) |

**AI dashboard** (Settings): providers, tokens per day, balance and monthly budget.

![AI dashboard](docs/screenshots/ai-dashboard.png)

| Dark theme | Phone | Phone | Sign in |
|---|---|---|---|
| ![Dark](docs/screenshots/overview-dark.png) | ![Phone overview](docs/screenshots/mobile-overview.png) | ![Phone users](docs/screenshots/mobile-users.png) | ![Sign in](docs/screenshots/login.png) |

## Install

Download the file for your system from the [Releases](https://github.com/teliti-dev/telemon/releases) page and check it against `SHA256SUMS.txt`.

| System | File | Updates |
|---|---|---|
| Windows | `Telemon_<version>_x64-setup.exe` | In app |
| Linux | `Telemon_<version>_amd64.AppImage` | In app |
| Debian, Ubuntu | `Telemon_<version>_amd64.deb` | Notified only; install the new `.deb` yourself |
| macOS 11+ (Apple Silicon and Intel) | `Telemon_<version>_universal.dmg` | In app |

The Windows installer is not code-signed yet, so SmartScreen will warn on first run. The macOS app is signed ad-hoc but not notarized (no Apple Developer account yet): on first launch, right-click the app and choose **Open**, or run `xattr -cr /Applications/Telemon.app`. The macOS build has been compiled and packaged by CI but not yet run on a Mac. On macOS the automatic `cloudflared` download for public access is not available yet; LAN sharing works. Release files carry a minisign signature (`.sig`) that the in-app updater verifies.

Pre-releases are on the test channel. Stable releases will use the stable channel; there is none yet.

## How it works

```
 MikroTik routers ──RouterOS API──▶ telemon-core (poller, SQLite, accounts)
                                         │
                      loopback server (127.0.0.1, per-launch token)
                                         │
                          Desktop webview (no login)
                                         │
              optional: LAN server ──▶ browsers and phones (login + 2FA, per role)
                                         │
              optional: Cloudflare Quick Tunnel ──▶ public URL (2FA forced, auto-off)
```

The desktop window loads the same web UI over a loopback HTTP server that only answers requests carrying a token generated at launch. LAN and public access are separate listeners with account login.

## Development

Requirements: [bun](https://bun.sh), and a stable Rust toolchain for the crates.

```sh
bun install
bun run dev            # UI only, in the browser
bun run tauri dev      # full desktop app (needs the Tauri system dependencies)
```

Checks:

```sh
bun run typecheck && bun run lint && bun run i18n:check
(cd crates/telemon-core    && cargo test)   # 100 tests
(cd crates/telemon-http    && cargo test)   # 44 tests
(cd crates/telemon-ai      && cargo test)   # 21 tests
(cd crates/telemon-desktop && cargo test)   # 14 tests
```

Try the UI without a router, using the demo server (simulated routers, public demo password, **for trying the UI only**):

```sh
bun run build
cd crates/telemon-demo && cargo build --release
./target/release/telemon-demo --static ../../dist     # http://127.0.0.1:7878
```

Layout: `src/` React UI (Vite, TypeScript, shadcn/ui), `crates/telemon-core` RouterOS client, poller, storage and accounts, `crates/telemon-http` Axum server, `crates/telemon-ai` AI assistant (LLM clients, read-only tools), `crates/telemon-desktop` desktop wiring and `cloudflared` management, `src-tauri` the Tauri shell.

## Releasing

Create a GitHub Release with a tag `vX.Y.Z`. `.github/workflows/release.yml` builds Linux (`.deb`, `.AppImage`), Windows (NSIS `.exe`, cross-compiled on Linux) and macOS (universal `.dmg`, on a macOS runner), signs the updater artifacts, and uploads them with `SHA256SUMS.txt` and `latest.json`. Prereleases are published to the `channel-rc` test channel.

## Design notes

Code comments refer to architecture decisions by number (for example "ADR 0021"). Those decision records are maintained separately by the maintainers and are not part of this repository.

## License

[AGPL-3.0](LICENSE). Telemon uses the AGPL-licensed `mikrotik-rs` client.

---

Copyright © 2026 [Teliti](https://teliti.net/). See [NOTICE](NOTICE).
