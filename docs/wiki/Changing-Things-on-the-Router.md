# Changing things on the router

Telemon can write **PPP secrets** and **PPP profiles** back to a router. This is **off by default**.

> [!WARNING]
> Writing changes a live router. Try it on a **lab router** first, and keep your own backup of the router configuration. Telemon does not back it up for you.

## What it can and cannot do

| Can | Cannot |
|---|---|
| Add and edit PPP secrets | Change anything outside `/ppp/secret` and `/ppp/profile` (firewall, interfaces, ...) |
| Add and edit PPP profiles | **Delete** a secret or profile on the router |
| Disable a secret (this is what *delete* does) | |

When you delete a user in Telemon, the secret is **disabled** and its session is cut. A profile is only marked in Telemon, and Telemon refuses to remove one that is still in use.

## Turning writing on

1. Give the router's API account the **`write`** policy.
2. In the router's settings, turn on **Allow editing users and PPP profiles**.
3. The router must be **online**.

## The review step

Every change opens a **review dialog** that lists exactly what will be sent to the router. Nothing is written until you confirm. Each applied change is recorded in the **change history**.

## Who may write

- The **desktop app**, which is not limited by roles.
- **Browser accounts** whose role has the **Edit** permission. Those accounts must use **two-factor authentication**.
- Writing over **public access** is off by default. It can only be turned on from the desktop app.

The server checks the permission on every request. The interface only hides and explains.
