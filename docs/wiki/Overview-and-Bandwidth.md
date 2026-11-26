# Overview and bandwidth

## The Overview page

**Overview** is a grid of **widgets**: counters, router cards, latest events, events per hour, users per profile, frequent auth failures and more.

- **One shared layout.** Every account sees the same layout. It is stored on the server side of Telemon, not in your browser.
- **Editing.** Accounts with the Overview **Edit** permission can press **Edit layout** to drag, resize, add and remove widgets. Editing is available on medium and wider screens; phones show the widgets in one column.
- **Defaults.** An empty layout falls back to the default arrangement.

## Bandwidth

Telemon can collect each router's **interface counters** and turn them into rates.

| Setting | Default |
|---|---|
| Collect bandwidth | On |
| Interval | 60 seconds (10 to 3600) |
| Keep samples | 7 days (1 to 365) |

Three widgets use it:

1. **A chart per router and interface.** Pick the router, interface and time range in the widget's settings.
2. **Total PPPoE traffic** across the routers.
3. **Average per active user.**

> [!NOTE]
> Dynamic PPPoE interfaces are **not** stored one by one. They are summed into one virtual interface called `@pppoe` per router, together with the number of sessions. Only `@pppoe` can be added up across routers; adding physical interfaces from several routers would count traffic twice.
>
> For PPPoE, the router's *tx* is the customer's **download**.

The first sample appears one interval after a router is added.

> [!WARNING]
> Bandwidth collection has not been tried against a real MikroTik router yet.
