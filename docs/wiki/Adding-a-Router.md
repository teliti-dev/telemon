[← Telemon documentation](Home.md)

# Adding a router

Telemon reads each MikroTik router over the **RouterOS API**.

## On the router

1. **Create a dedicated API account.** Do not reuse your admin account.
2. Put it in a group that has the **`read`** policy (and `api`). That is enough to monitor.
3. Add the **`write`** policy only if you want to edit users and PPP profiles from Telemon. See [Changing things on the router](Changing-Things-on-the-Router.md).
4. Make sure the **API service** is enabled and reachable from the computer that runs Telemon:

| Service | Default port | Use |
|---|---|---|
| `api-ssl` (TLS) | **8729** | Recommended |
| `api` (no TLS) | 8728 | Only on a network you trust |

## In Telemon

Open **Routers** and add a router:

| Field | What to enter |
|---|---|
| Name | Any label, for example `RB-North` |
| Host / IP | The router's address |
| Port | 8729 (TLS) or 8728 |
| API username and password | The dedicated account. The password is stored in the operating system keyring |
| Allow editing users and PPP profiles | Leave **off** unless the account has `write` rights |

### Trusting the TLS certificate

A router with its own certificate has to be trusted once:

1. Choose **Check certificate**.
2. **Compare the fingerprint** Telemon shows with the one on the router.
3. If they match, choose **I trust this certificate**.

Telemon pins that fingerprint. If the router's certificate changes later, Telemon refuses to connect until you check it again.

## After adding

Telemon starts polling right away. The router card shows **Online** or **Offline**, and its users and events fill in after the first poll.

## Removing a router

Removing a router stops monitoring it and removes its users and events from Telemon. **The router's own configuration is not changed.**
