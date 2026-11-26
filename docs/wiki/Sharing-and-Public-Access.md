# Sharing and public access

The desktop app is one program, but its dashboard can be opened from other devices.

```
Desktop app (no login)
   ├── LAN server ───────────────▶ browsers and phones on your network (login + 2FA)
   └── Cloudflare Quick Tunnel ──▶ a public address (2FA forced, turns itself off)
```

Both are **off by default** and are controlled from the **Sharing** page, which exists only in the desktop app.

## Share to a browser (LAN)

1. Open **Sharing** and turn on the LAN server.
2. Open the address it shows (the default port is **7878**) from a laptop or phone on the same network.
3. Sign in with an account. See [Accounts and roles](Accounts-and-Roles).

Browsers always need an account. There is no anonymous access.

## Public access

Public access opens Telemon to the **internet** through a **Cloudflare Quick Tunnel**. You do not need a Cloudflare account.

- **First use:** Telemon downloads the official `cloudflared` program once, checks its checksum, and keeps it in its data folder. This needs internet access, including to `github.com`.
- **Requirements:** the LAN server must be on, and you start it from the desktop app.
- **Duration:** you choose 1, 4, 8 or 24 hours. The tunnel then turns itself off.
- **Two-factor authentication is forced for every account** while public access is on.
- **The address is random** and changes every time you turn it on. Only the desktop app shows it.
- If the tunnel dies, Telemon shows an error instead of a dead address.

> [!IMPORTANT]
> Anyone with the address can reach the **sign-in page**. Use strong passwords and keep two-factor on. Writing to routers over public access is off by default.

> [!NOTE]
> Public access is not available on macOS, and there is no way to install `cloudflared` by hand for isolated networks yet.

## What the browser can do

A browser sees the same dashboard, limited by the account's **role**. See [Accounts and roles](Accounts-and-Roles). Live events arrive over a WebSocket, with polling as a fallback.
