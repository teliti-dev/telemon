# Development

Telemon is a **Tauri v2** desktop app with a Rust core and a React UI.

## Requirements

- [bun](https://bun.sh)
- A stable **Rust** toolchain
- For the full desktop app: the Tauri system dependencies (on Linux, WebKitGTK)

## Run it

```sh
bun install
bun run dev            # the UI only, in a browser
bun run tauri dev      # the full desktop app
```

## Check your changes

```sh
bun run typecheck && bun run lint && bun run i18n:check
(cd crates/telemon-core    && cargo test)
(cd crates/telemon-http    && cargo test)
(cd crates/telemon-ai      && cargo test)
(cd crates/telemon-ext     && cargo test)
(cd crates/telemon-desktop && cargo test)
```

`bun run i18n:check` must be clean: UI text is written in Indonesian as the key and translated in `src/lib/i18n/en/*.ts`.

## Try the UI without a router

The demo server uses simulated routers and a public demo password. It is **for trying the UI only**.

```sh
bun run build
cd crates/telemon-demo && cargo build --release
./target/release/telemon-demo --static ../../dist     # http://127.0.0.1:7878
```

## Project layout

| Path | What is in it |
|---|---|
| `src/` | React UI (Vite, TypeScript, shadcn/ui) |
| `crates/telemon-core` | RouterOS client, poller, storage, accounts |
| `crates/telemon-http` | Axum server |
| `crates/telemon-ai` | AI assistant |
| `crates/telemon-ext` | Extension packages, manifests, WASM runtime |
| `crates/telemon-ssh` | SSH terminal for extensions |
| `crates/telemon-desktop` | Desktop wiring and `cloudflared` management |
| `src-tauri` | The Tauri shell |

## How it fits together

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

## Rules of the repository

- Work on a **branch**; the default branch is `master`.
- A change in behaviour comes with **tests** and a README or comment update.
- UI text must be translated (Indonesian and English).
- The repository is public: **never commit secrets** or internal addresses.

## Releasing

Create a GitHub Release with a tag `vX.Y.Z`. The release workflow builds Linux (`.deb`, `.AppImage`) and Windows (NSIS `.exe`, cross-compiled on Linux), signs the update files and uploads them with `SHA256SUMS.txt` and `latest.json`. Pre-releases go to the test channel.

## Wiki

These pages live in `docs/wiki/` in the main repository and are copied to the GitHub wiki with `scripts/sync-wiki.sh`. Edit them there, in a pull request.
