# Getting started

This page takes you from download to your first data on screen.

## 1. Install

Download the file for your system from the [Releases](https://github.com/teliti-dev/telemon/releases) page and check it against `SHA256SUMS.txt`.

| System | File | Updates |
|---|---|---|
| **Windows** | `Telemon_<version>_x64-setup.exe` | In app |
| **Linux** | `Telemon_<version>_amd64.AppImage` | In app |
| **Debian, Ubuntu** | `Telemon_<version>_amd64.deb` | Notified only; install the new `.deb` yourself |

> [!NOTE]
> **macOS is not built or published** for now.

**Windows warning.** The installer is not code-signed yet, so Windows SmartScreen shows *Windows protected your PC*. Click **More info**, then **Run anyway**.

**Linux AppImage.** Make the file executable (`chmod +x`) and run it.

## 2. Open Telemon

The desktop app needs **no login**. It keeps its data on your computer (see [Settings, updates and backups](Settings-Updates-and-Backups#where-your-data-lives)).

## 3. Add your first router

Open **Routers** and add the router. The details are in [Adding a router](Adding-a-Router). In short:

1. Create a **dedicated API account** on the router (group `read` is enough for monitoring).
2. Enter the router's host, port (**8729 with TLS** is recommended, 8728 without) and the account.
3. If you use TLS, **check and trust the certificate**.

## 4. Look at your data

After the first poll the router shows up as online and its users and events appear:

- **Overview**: counters, router cards, latest events and charts.
- **Users**: every PPPoE secret with its status. See [Users and events](Users-and-Events).
- **Events**: what just happened, with filters and search.

## 5. Next steps

- Import users from the router, or look at one user's detail.
- Press <kbd>Ctrl</kbd>+<kbd>K</kbd> to search everywhere.
- [Share the dashboard](Sharing-and-Public-Access) to a phone or laptop.
- [Turn on the AI assistant](AI-Assistant) if you want one.

> [!TIP]
> Want to look around first? The project has a demo server with simulated routers and made-up users. See [Development](Development#try-the-ui-without-a-router).
