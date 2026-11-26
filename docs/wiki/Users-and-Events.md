[← Telemon documentation](Home.md)

# Users and events

## User status

Each PPPoE secret on a router has one status:

| Status | Meaning |
|---|---|
| **Active** | The user is connected right now (listed in `/ppp/active`) |
| **Inactive** | The secret exists but the user is not connected |
| **Disabled** | The secret is disabled on the router |
| **Auth failed** | Recent failed authentication for this user |

## Events

Telemon records what happens from the router's session list and log:

| Event | Meaning |
|---|---|
| `auth failed` | A login attempt was rejected |
| `authenticated` | A login succeeded |
| `connected` | A session came up |
| `disconnected` | A session ended |

The **Events** page lists them with filters and search. The Overview also shows the most frequent auth failures.

## Finding things

- **Users** and **Events** have filters (status, router, sort) and a search box. On a phone the filters become compact dropdowns.
- Press <kbd>Ctrl</kbd>+<kbd>K</kbd> (<kbd>⌘</kbd>+<kbd>K</kbd> on a Mac) to open the **command palette**. *Menu* is searched by default; turn on *Routers*, *Users* and *Other* with the switches under the palette or in **Appearance**.

## User detail and local notes

Open a user to see its detail and history. Telemon keeps **local notes** per user (customer name, phone, labels, notes). They are stored in Telemon, **not** on the router.

The PPPoE password is shown in a user's detail **only to people with the Edit permission**, and it is hidden by default.

## Importing from the router

**Import from MikroTik** copies users and PPP profiles from the router into Telemon. The router **wins**: where the two differ, the router's data overwrites Telemon's copy, and anything missing from the router is marked as deleted in Telemon.

## Data retention

Old events are removed after a while to keep the database small. The defaults are in [Settings, updates and backups](Settings-Updates-and-Backups.md#defaults).
