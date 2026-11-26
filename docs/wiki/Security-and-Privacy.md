[← Telemon documentation](Home.md)

# Security and privacy

What Telemon protects, and what stays your responsibility.

## What Telemon does

| Area | Protection |
|---|---|
| **Router passwords** | Stored in your operating system keyring, never in the database or in files |
| **PPPoE passwords** | Encrypted in the database (ChaCha20-Poly1305). Shown only to people with the Edit permission and hidden by default. Never written to logs |
| **Router connection** | TLS to the router with **certificate fingerprint pinning** |
| **Sign-in** | Hashed passwords (argon2id), two-factor authentication (TOTP) with recovery codes, rate limits and lockout |
| **Browser access** | Every request is checked on the server against the account's role. Cross-site requests are rejected |
| **Public access** | Two-factor forced for all accounts, writing off by default, and the tunnel turns itself off |
| **Updates** | Signed; the database is backed up before installing |
| **Downloads** | `cloudflared` is downloaded from its official release and checked against a pinned checksum |
| **Extensions** | Packages are signed; every permission is a toggle the server enforces; changes are audited |
| **AI assistant** | Read-only, masked IP and MAC by default, never sees passwords, only reads what the asker's role may see |

## What is up to you

- **Secure your network.** If you share to a browser or open public access, you are exposing a sign-in page. Use strong passwords, keep two-factor on, and use a firewall.
- **Give the API account only the rights it needs.** `read` for monitoring; `write` only if you edit from Telemon.
- **Keep backups.** Of Telemon's data folder and of your routers' configuration. Telemon does not back up router configuration.
- **Customer data.** Names, IP and MAC addresses and PPPoE credentials can be personal data. Handle them in line with the rules that apply to you.
- **The AI provider you choose** receives the data you ask about.
- **Trust only extensions you understand.** The SSH terminal runs with the full rights of the router account.

## Known limits

- Telemon has not been run against a real MikroTik router yet.
- The Windows installer is not code-signed, so SmartScreen warns.
- A session that was already signed in without two-factor before public access was turned on is not cut off; two-factor applies at the next sign-in.
- Extension user-interface code runs with the user's session and is not sandboxed (the reason only official extensions are allowed for now).

## Reporting a security problem

Open a [GitHub issue](https://github.com/teliti-dev/telemon/issues) that says you have a security report and ask for a private way to send it. **Do not put secrets, customer data or exploit details in the issue.**

See also the [Disclaimer and license](Disclaimer-and-License.md).
