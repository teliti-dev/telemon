# Accounts and roles

The **desktop app needs no login**. Anyone using a **browser** (LAN or public) signs in with an account, and what they can see and do depends on their **role**.

## Signing in

- **Password plus two-factor authentication (TOTP)**, with recovery codes. Passwords are stored hashed (argon2id).
- **Lockout.** After 5 failed sign-ins an account is locked for 5 minutes (both adjustable in Settings).
- **Session length.** 12 hours by default.
- **My account** is where you change your name and password, set up two-factor and see your sessions.

## Roles and permissions

A role has a permission **per menu**:

| Level | Meaning |
|---|---|
| **No access** | The menu is hidden and the server refuses the request |
| **View** | Read only |
| **Edit** | Read and change (includes View) |

The menus are Overview, Users, PPP profiles, Events, Routers, Admin and Settings. A separate **AI chat** permission controls the assistant.

- A new role starts with **no access**. A new account has **no role** until you pick one.
- Telemon ships with default roles (such as an administrator and a monitor). You can create, change and delete your own roles, as long as no account uses them.
- A role with **Edit** permission requires **two-factor authentication**.

## Managing people (Admin menu)

- **Accounts:** add an account with a role, change the role, reset a password, reset two-factor, disable or delete, and switch on *require two-factor* for everyone.
- **Roles:** a table of permissions per menu.

## Protection against privilege escalation

When an account manages other accounts through a browser, the server enforces that it **cannot**:

- create or change a role, or give a role, with permissions **higher than its own**;
- change **its own** account (role, disable, reset, delete) from Admin;
- change or delete the role **it is using**;
- manage an account whose permissions are **higher than its own**.

## Extension access

Each [extension](Extensions) also has its own **Access** page, where you decide per role what the extension may do.
