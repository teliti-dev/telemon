[← Telemon documentation](Home.md)

# Extensions

Extensions add features to Telemon: Overview widgets, pages, router actions, bottom-panel tabs, scheduled jobs and more. Open them from **Settings > Extensions**.

> [!NOTE]
> In this version only **official** extensions (signed by Teliti) can be installed. Support for other publishers is planned.

## Installing

Choose **Install extension**, then:

- **From the store**: browse, search and install official extensions. Dependencies are installed too.
- **Upload a file**: pick a `.tmx` file from your computer.

Before anything is installed you see the **publisher**, the **license**, the **permissions** it asks for and any extension it needs. Telemon checks the **signature**, the size and the checksum automatically; what is left for you is to decide whether the publisher, license and permissions make sense.

An extension starts **off**. Turn it on from its page.

## Permissions

Every permission has a **toggle**. Sensitive ones start **off**, and a new permission added by an update also starts off. If a **required** permission is off, the extension is *limited* and is not loaded. Turning on a sensitive permission asks you to confirm. Every change is written to the audit history.

## Access per role

Each extension has an **Access** page: for each role you choose *no access*, *view* or *edit*, plus any specific actions the extension declares.

## Licenses

**Each extension has its own license**, chosen by its publisher: free, open source, freemium, paid, trial or custom terms. The license shown on the extension's page comes from its publisher.

> [!IMPORTANT]
> Telemon does **not** sell, verify or enforce extension licenses, and is not responsible for extensions made by others.
>
> Developers can license their extensions as they like. See [Writing extensions](Writing-Extensions.md#is-my-extension-bound-by-telemons-agpl).

## Official extensions

| Extension | What it does |
|---|---|
| **Router health** | CPU, memory, uptime and version of every router, with a short history sampled every minute |
| **Router terminal (SSH)** | An SSH terminal to a router, opened from the router's action menu in a bottom panel |

> [!WARNING]
> The terminal runs with the **full rights of the router account**, and there is no read-only mode. Give the Terminal action only to roles you trust. The router's SSH host key is pinned, and every session is recorded in the audit history, including what was typed.

Neither extension has been tried against a real MikroTik router yet.

## On a server

Extensions are **on** in the desktop app. On a server they are **off** until the owner sets `TELEMON_EXTENSIONS=1`.

## Removing

Removing an extension keeps its data unless you choose to delete it. An extension that another one depends on cannot be removed first.
