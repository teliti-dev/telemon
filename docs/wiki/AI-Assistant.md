# AI assistant

An optional assistant that answers questions about your users and events in plain language. It is **off until you add a provider**, and it is **read-only**: it cannot change anything on a router.

> [!IMPORTANT]
> When you use it, the data you ask about is **sent to the AI provider you choose**. Choose a provider you are allowed to share customer data with.

## Setting it up

1. Open **Settings > AI**.
2. Add a **provider** and its **API key**. The key is write-only: Telemon never shows it again.
3. Pick a **default provider**. The assistant is available once the default provider has a key and AI chat is enabled (it is enabled by default).

Supported providers: **OpenAI**, **DeepSeek**, **Gemini**, **9Router** (OpenAI-compatible APIs) and **Anthropic**. A custom base URL can only be set from the desktop app.

## Using it

- Open the chat with the floating button, or press <kbd>Alt</kbd>+<kbd>A</kbd>. Type `/` for quick commands.
- On a phone the assistant is a page.
- Conversations are saved **per account** (not in the browser) and are removed after the retention period (90 days by default).

## What it can answer

- Look up users, routers and events.
- **Event statistics** per hour or day, the users with most auth failures and disconnects.
- **Bandwidth summary**: now, average, peak, per user, change against the previous period, per router.
- **Router health**: downtime and availability.
- **Overview changes.** It can *propose* widget changes (add, remove, change a widget's router, interface or range). A proposal appears as a card, and **nothing changes until you press Apply**, which uses your own permission.

## Privacy and limits

- It only reads data the **asker's role** may see.
- **IP and MAC addresses are masked by default.** You can change this in Settings.
- It **never sees passwords**.
- Answers can be wrong. Check them before you act.
- Each role needs the **AI chat** permission.

## Cost control

**Settings > AI** has a dashboard with tokens per day, cost, per-model and per-account usage, and a **monthly budget** with an alert percentage (80% by default) and an optional hard stop.
