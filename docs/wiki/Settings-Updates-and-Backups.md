# Settings, updates and backups

## Where your data lives

Telemon keeps its database (`telemon.db`) and other files in your user **data folder**, separate from the program. Updating or reinstalling does **not** remove it.

| System | Folder |
|---|---|
| Windows | `%APPDATA%\dev.teliti.telemon` |
| Linux | `~/.local/share/dev.teliti.telemon` |

- **Secrets.** Router passwords and the database key are in your operating system's **keyring**. If there is no keyring (for example on some Linux desktops), Telemon uses a `secrets.json` file in the data folder, readable only by you.
- **The database and its key go together.** If you delete the data folder but keep the keyring entry, or the other way round, saved router credentials cannot be read again.
- **Back up the data folder** before you try a pre-release.

## Settings

Open **Settings** (it needs the matching permission). Each section has a *Default* button and there is *Restore all to defaults*.

### Defaults

| Section | Setting | Default |
|---|---|---|
| Data retention | Keep PPPoE events | 30 days (7 to 365, or forever) |
| | Maximum events | 200,000 |
| | Permanently delete soft-deleted items | after 90 days (or never) |
| | Keep change and sign-in history | 365 days |
| | Keep AI chat history | 90 days |
| Polling and sync | Read active sessions | every 30 seconds (10 to 600) |
| | Read router log | every 30 seconds (10 to 600) |
| | Automatic import | every 24 hours (off, or 1 hour to 7 days) |
| | Connection timeout | 10 seconds |
| | Retries | 2 |
| Remote access | Sign-in session length | 12 hours |
| | Failed sign-ins before lockout | 5 |
| | Lockout time | 5 minutes |
| | Write over public access | off |
| AI assistant | AI chat enabled / mask IP and MAC | on / on |
| Bandwidth | Collect / interval / keep | on / 60 s / 7 days |

Data past the retention limit is permanently deleted **from Telemon**. Data on the router is not changed. The Storage section shows size and counts and has a *Clean now* button.

## Updates

Telemon checks for **signed updates** and shows a banner and a card in Settings.

| System | How updates arrive |
|---|---|
| Windows | In the app |
| Linux AppImage | In the app |
| Linux `.deb` | You are notified; install the new `.deb` yourself |

- **Channels.** Pre-releases are on the **test channel**. Stable releases will use the stable channel; there is none yet. Choose the channel in Settings > Updates.
- **Verified.** The updater checks a signature on every download.
- **Backup first.** Before an update is installed, Telemon copies the database into `backups/` in the data folder. **If the backup fails, nothing is installed.** The three most recent backups are kept.
- **Migrations.** The first time a new version opens, it upgrades the database automatically, step by step, each step in one transaction.
- **No downgrade.** There is no automatic way back. To go back, restore a backup yourself.

> [!NOTE]
> Updating from one pre-release to the next has not been fully proven on every system yet. Copy your data folder first if the data matters.
