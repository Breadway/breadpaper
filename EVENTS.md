# breadpaper — bread event integration

breadpaper is a one-shot CLI wallpaper setter: it works exactly the same
with or without `breadd` running. When breadd *is* present, each successful
`breadpaper set` (or the bare-path shorthand) publishes one event into the
shared bread automation fabric. See the parent `bread` repo's
`Documentation.md` — specifically its "Namespaces" and "Integrating a
bread\* app" sections — for the general convention this follows.

App id: **`paper`**. Transport: `bread-utils`'s `bread_client` module
(feature `bread-client`) — the CLI links it directly and uses
`BreadClient::connect("paper")` + `emit` only. v0.7.1 has no `command()`
helper, and breadpaper has no long-running process that could hold a
`subscribe` open.

There is no `breadpaper` daemon and no `watch` subcommand. A `bread-emit
bread.command.paper.set` (or any other `bread.command.paper.*`) with no
subscriber is a silent no-op — that is the documented bread convention,
not a breadpaper bug. Modules that want to change the wallpaper should
shell out:

```lua
bread.exec("breadpaper set /path/to/image.png")
```

The one-shot process still emits `bread.paper.changed` on success, so a
workflow can `bread.wait("bread.paper.changed", …)` for the real outcome
instead of assuming the exec finished the set.

## Events published (`bread.paper.*`)

| Event | Data | When |
|-------|------|------|
| `bread.paper.changed` | `{ "path": "<wallpaper>" }` | After a successful `set` (awww + wal + `bread-theme reload`). `path` is the canonical absolute path that was applied. Not emitted on `get`, and not emitted if any of the three steps fail. |

## Commands honored (`bread.command.paper.*`)

None, because there is nobody listening.

| Verb | Data | Status |
|------|------|--------|
| `set` | `{ "path": "..." }` | **Not subscribed.** The same work is `bread.exec("breadpaper set …")`. A future `breadpaper watch` (or a service-mode of this binary) could honor `bread.command.paper.set` and emit `bread.paper.set.done` / `.failed`; that is deliberately not added here — a long-running process whose only job is to re-exec the existing one-shot CLI is not worth the extra surface. |

### Not implemented: slideshow / library / random / next

breadpaper is not a wallpaper library, a slideshow daemon, or a picker.
Browsing `~/Pictures/Backgrounds` lives in bos-settings. Do not invent
`bread.command.paper.next` / `.random` / `.cycle` (or matching events)
ahead of a real product feature.

## Fail-safe behavior

- If breadd isn't installed or isn't running, `emit` is a silent no-op
  (`BreadClient::emit` never blocks or errors the caller) — breadpaper
  still sets the wallpaper, generates the palette, and reloads themes.
- There is no command subscription to reconnect, because there is no
  long-running subscriber.
