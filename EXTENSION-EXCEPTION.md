# Telemon Extension Exception

**Additional permission under section 7 of the GNU Affero General Public License, version 3**, granted by Teliti for the Telemon source code in this repository.

Telemon is licensed under the AGPL-3.0 ([LICENSE](LICENSE)). This exception supplements that license. It lets **extension developers choose the license of their own extension**: free, open source, freemium, paid, proprietary or anything else.

## The permission

Teliti gives you **additional permission** to write, use, convey and license an **Extension** under **terms of your choice**, including proprietary and paid terms and terms that do not make the Extension's source available, provided that:

1. **It only uses the Extension Interface.** The Extension talks to Telemon only through the interface that Telemon publishes for extensions, called the *Extension Interface* here:
   - the manifest format and the `.tmx` package format;
   - the user-interface contract in `src/lib/extensions/sdk.ts` (the object passed to `activate(sdk)`), and the HTTP routes and WebSocket sessions Telemon exposes to extensions;
   - the WebAssembly host interface (`telemon.call`, ABI version 1) and the `telemon-ext-guest` crate, which is separately licensed under MIT or Apache-2.0.
2. **It contains no Telemon code.** The Extension does not copy or include source or object code from Telemon, other than the definitions of the Extension Interface themselves (for example, using the type definitions in `sdk.ts` at build time).
3. **It is not a changed Telemon.** Code that modifies Telemon's own files, or that makes Telemon itself behave differently outside the Extension Interface, is **not** an Extension. A modified Telemon stays under the AGPL-3.0 in full, including its network clause (section 13).

## What this does not do

- It does not change the license of Telemon or of any modified version of Telemon.
- It does not cover third-party components that Telemon includes under their own licenses (for example `mikrotik-rs`, AGPL-3.0). An Extension must not include such components.
- It does not make Telemon responsible for your Extension. See [DISCLAIMER.md](DISCLAIMER.md).
- It gives you no right to use the Teliti or Telemon names or logo to suggest that the Extension is official.

## Removing this permission

As section 7 of the AGPL provides, when you convey a copy of Telemon (or part of it) you may remove this additional permission from that copy.

## Official extensions

The official extensions in the [`telemon-extensions`](https://github.com/teliti-dev/telemon-extensions) repository are published by Teliti under their own license, stated in each extension's manifest.

---

*Teliti is the copyright holder of the Telemon source code in this repository, so it can grant this permission. This text is not legal advice.*
