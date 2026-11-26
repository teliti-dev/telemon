# Troubleshooting and FAQ

## Install and start

**Windows says "Windows protected your PC".**
The installer is not code-signed yet. Click **More info**, then **Run anyway**.

**macOS?**
It is not built or published for now.

**The AppImage will not start on Linux.**
Make it executable (`chmod +x`). If the window is blank, try running it from a terminal and read the messages.

## Routers

**A router shows Offline.**
Check that the router is reachable from this computer, that the **API service** is enabled, and that the host and port are right (8729 for TLS, 8728 without). Check the account's group has `read` and `api`.

**Telemon refuses to connect after I changed the router certificate.**
Telemon pins the fingerprint. Open the router, choose **Check certificate**, compare the fingerprint and trust it again.

**There are no events.**
Events come from the router's session list and log. Wait a polling interval (30 seconds by default). Check that PPPoE logging is on at the router.

**I cannot add or edit a user.**
Writing is off by default. It needs the router's **Allow editing users and PPP profiles** switch, an API account with `write`, an online router and the Edit permission. See [Changing things on the router](Changing-Things-on-the-Router).

## Sharing and accounts

**Public access will not turn on.**
The LAN server must be on, you must be in the desktop app, and the computer needs internet access (including `github.com`) the first time, to download `cloudflared`. Public access is not available on macOS.

**The public address changed.**
That is how Quick Tunnels work: the address is new every time you turn public access on.

**I lost my two-factor device.**
Use a **recovery code**. If you have none, an administrator can **reset two-factor** for your account in the Admin menu.

**I locked myself out.**
After too many failed sign-ins an account is locked for a few minutes (5 by default). Wait, or ask an administrator.

## Data

**Where is my data?**
In your user data folder. See [Settings, updates and backups](Settings-Updates-and-Backups#where-your-data-lives).

**Will I lose data if I update or reinstall?**
No. The data folder is separate from the program. Back it up anyway before testing a pre-release.

**Can I go back to an older version?**
There is no automatic downgrade. Restore a backup from the `backups` folder.

## Extensions

**"No official key pinned".**
This build does not trust any official signing key yet, so extensions cannot be installed from the store or from a file.

**An extension will not load.**
It may be *limited* because a required permission is off, or *incompatible* with your Telemon version. Open its page to see why.

## AI

**The assistant is not available.**
Add a provider with an API key in **Settings > AI**, make it the default, and make sure your role has the **AI chat** permission.

## Still stuck?

Open an issue at [github.com/teliti-dev/telemon/issues](https://github.com/teliti-dev/telemon/issues). Do **not** include passwords, API keys or customer data.
