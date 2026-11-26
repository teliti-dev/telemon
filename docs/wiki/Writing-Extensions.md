[← Telemon documentation](Home.md)

# Writing extensions

This page is for developers who want to build an extension. Read [Extensions](Extensions.md) first for how users see them.

> [!NOTE]
> Right now Telemon installs only **official** packages, meaning packages signed with the Teliti key. You can build and test your own package, but the app will not install an unsigned or third-party-signed package yet.

## What is a package?

A package is a `.tmx` file, a zip with:

```
manifest.json          what the extension is and what it asks for
ui/index.js            optional: the user interface
backend.wasm           optional: a sandboxed backend
migrations/*.json      optional: database changes
signature.json         Ed25519 signature over every file
```

## The manifest

```json
{
  "id": "net.example.demo",
  "name": "Demo",
  "version": "1.0.0",
  "apiVersion": 1,
  "publisher": "Example Co",
  "description": "One sentence about what it does.",
  "ui": { "entry": "ui/index.js" },
  "contributes": {
    "widgets": [{ "type": "hello", "title": "Hello" }]
  },
  "permissions": [{ "id": "data.events", "required": true }],
  "telemon": ">=0.1.0",
  "license": {
    "kind": "freemium",
    "summary": "Free for up to 5 routers; Pro is paid per month.",
    "url": "https://example.com/pricing"
  }
}
```

| Field | Notes |
|---|---|
| `id` | Lowercase, reverse-domain style (`net.example.demo`) |
| `version` | `X.Y.Z` |
| `apiVersion` | `1` |
| `contributes` | `widgets`, `pages`, `routes`, `workers`, `sessions`, `routerActions`, `panels` |
| `permissions` | What the extension asks for. Users toggle each one; sensitive ones start off |
| `actions` | Finer actions that an admin can grant per role |
| `telemon` | Version range of Telemon you support, for example `>=0.1.0 <0.4.0` |
| `requires` | Other extensions you need, each with a version range (`optional` allowed) |

### Choosing your license

**You decide the license of your extension.** Add a `license` object:

| Key | Meaning |
|---|---|
| `kind` | `open-source`, `free` (free, closed source), `freemium`, `paid`, `trial` or `proprietary` (custom terms) |
| `spdx` | An SPDX identifier such as `MIT` or `AGPL-3.0-only`. **Required** for `open-source` |
| `url` | Link to the full terms or pricing. Must be `https://` |
| `summary` | One or two sentences for users (up to 200 characters) |

Telemon only **shows** this on the install preview, the store and the detail page. It does not sell, verify or enforce your license, and it adds no payment system.

> [!WARNING]
> Older Telemon builds reject a manifest with an unknown field. Packages that use `license` need a Telemon release that understands it.

## The user interface

`ui/index.js` exports an object with an `activate(sdk)` function. React belongs to the app and reaches you as `sdk.React`.

| Call | What it does |
|---|---|
| `sdk.registerWidget(...)` | A widget for the Overview |
| `sdk.registerPage(...)` | A page at `#/ext/<id>/<page>` |
| `sdk.registerRouterAction(...)` | An item in each router's action menu |
| `sdk.registerPanel(...)` and `sdk.panel.open(...)` | A tab in the bottom panel |
| `sdk.api.get/post` | Call your own backend routes |
| `sdk.sessionUrl(...)` | A WebSocket address for sessions such as the SSH terminal |
| `sdk.hasPermission(...)`, `sdk.can(...)`, `sdk.accessLevel` | Check what the current user may do |

The contract lives in `src/lib/extensions/sdk.ts` in the main repository.

> [!WARNING]
> Extension UI code runs **with the signed-in user's session** and is not sandboxed. That is why only trusted (official) extensions can be installed for now.

## The backend (WebAssembly)

A backend is a WASM module that answers API routes and runs scheduled jobs.

- Declare `backend.entry`, plus `contributes.routes` and `contributes.workers` (`everySeconds` from 10 to 86400).
- The module exports `memory`, `telemon_alloc` and `telemon_handle`, and may import only `telemon.call`. There is no WASI. Messages are JSON in both directions.
- **Limits per call:** a fuel budget, 64 MB of memory, 256 KB request, 1 MB answer and 500 host calls. A fresh instance runs for every call.
- **Host API:** time, log, a key-value store, your own database tables, router list, read-only router prints from an allow-list, and events. Reading router or event data needs the matching permission toggle **and** the calling user's own role permission.
- For Rust there is a helper crate, `telemon-ext-guest`, that compiles to `wasm32-unknown-unknown`.

## Database tables

Add `migrations/0001-name.json` files with declarative changes (`create_table`, `add_column`, `create_index`, `drop_table`, `drop_index`). Every table and index name must start with `x_<your id>__`. Telemon backs up the database before it applies a migration and keeps your data when the extension is removed unless the user chooses to delete it.

## Building and signing

The `tmx` tool in the repository does the packaging:

```sh
tmx keygen                                  # make a signing key
tmx pack ./my-extension -o my.tmx --key-env MY_KEY   # the key is read from the environment variable
```

Keep the private key secret and out of the repository.

## Examples

The official extensions live in the [`telemon-extensions`](https://github.com/teliti-dev/telemon-extensions) repository (a hello example, Router health and the SSH terminal).
