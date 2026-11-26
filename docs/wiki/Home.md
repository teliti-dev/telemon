# Telemon

**Telemon** monitors PPPoE users and events across many MikroTik routers, from a desktop app or any browser.

> [!WARNING]
> Telemon is in **pre-release**. It has been tested against a *simulated* RouterOS and has <ins>not</ins> been run against a real MikroTik router yet. Keep write access off until you have tried it on a lab router. See [Disclaimer and license](Disclaimer-and-License.md).

## Start here

| I want to... | Read |
|---|---|
| Install Telemon and see my first data | [Getting started](Getting-Started.md) |
| Connect a MikroTik router | [Adding a router](Adding-a-Router.md) |
| Understand statuses and events | [Users and events](Users-and-Events.md) |
| Build my Overview and see bandwidth | [Overview and bandwidth](Overview-and-Bandwidth.md) |
| Add or edit users on the router | [Changing things on the router](Changing-Things-on-the-Router.md) |
| Open Telemon from a phone or the internet | [Sharing and public access](Sharing-and-Public-Access.md) |
| Give other people access | [Accounts and roles](Accounts-and-Roles.md) |
| Ask questions in plain language | [AI assistant](AI-Assistant.md) |
| Add features with extensions | [Extensions](Extensions.md) |
| Make my own extension | [Writing extensions](Writing-Extensions.md) |
| Update, back up, or change settings | [Settings, updates and backups](Settings-Updates-and-Backups.md) |
| Know what is protected and what is not | [Security and privacy](Security-and-Privacy.md) |
| Fix a problem | [Troubleshooting and FAQ](Troubleshooting-and-FAQ.md) |
| Work on the code | [Development](Development.md) |

## What Telemon does

- Shows **who is connected**, who is inactive and who is disabled, for every router in one place.
- Records **events**: auth failed, authenticated, connected, disconnected.
- Keeps the **router as the source of truth**: Telemon imports from it, and writes back only when you allow it, after a review step.
- Runs as a **desktop app** with its own local database. One switch shares it to browsers on your network; another can open it to the internet.

## Links

- [Releases](https://github.com/teliti-dev/telemon/releases) (downloads)
- [Report a problem](https://github.com/teliti-dev/telemon/issues)
- [Source code](https://github.com/teliti-dev/telemon) (AGPL-3.0)
