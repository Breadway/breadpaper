# breadpaper — bread event integration

breadpaper is a wallpaper setter: it works exactly the same with or
without `breadd` running. When breadd *is* present, a successful
`breadpaper set` (or the bare-path shorthand) publishes `bread.paper.changed`
into the shared bread automation fabric, and `breadpaper listen` honors
`bread.command.paper.set`. See the parent `bread` repo's
`Documentation.md` — specifically its "Namespaces" and "Integrating a
bread\* app" sections — for the general convention this follows.

App id: **`paper`**. Transport: `bread-utils`'s `bread_client` module
(feature `bread-client`) — the CLI links it directly. One-shot
`set`/`get` use `BreadClient::connect("paper")` + `emit` only. The
long-running `listen` subcommand holds a `subscribe` open.

`breadpaper listen` is fail-silent if breadd is down: `subscribe`
reconnects with backoff and simply delivers nothing until the daemon
comes back. The one-shot `set`/`get` path does not require `listen`.
Modules that want to change the wallpaper without a listener can still
shell out:

```lua
bread.exec("breadpaper set /path/to/image.png")
```

A workflow that publishes the command instead should wait for the
confirmation, not assume the emit finished the set:

```lua
bread.emit("bread.command.paper.set", { path = "/path/to/image.png" })
bread.wait("bread.paper.set.done", { timeout = 10000 })
```

## Events published (`bread.paper.*`)

| Event | Data | When |
|-------|------|------|
| `bread.paper.changed` | `{ "path": "<wallpaper>" }` | After a successful `set` (awww + wal + `bread-theme reload`), including when `listen` honors `bread.command.paper.set`. `path` is the canonical absolute path that was applied. Not emitted on `get`, and not emitted if any of the three steps fail. |
| `bread.paper.set.done` | `{ "path": "<wallpaper>" }` | `bread.command.paper.set` was received and `set()` succeeded. `path` is the canonical absolute path that was applied. Not emitted by the one-shot CLI `set` — that path only publishes `changed`. |
| `bread.paper.set.failed` | `{ "error": "<message>", "path"?: "<requested>" }` | `bread.command.paper.set` was received but `set()` failed, or `data.path` was missing/not a string. `path` is the requested (not canonical) path when one was supplied. |

## Commands honored (`bread.command.paper.*`)

Honored only while `breadpaper listen` is running. A
`bread-emit bread.command.paper.set` with no listener is a silent no-op
— that is the documented bread convention, not a breadpaper bug.

| Verb | Data | Effect |
|------|------|--------|
| `set` | `{ "path": "..." }` | Calls the existing `set()` (awww + wal + `bread-theme reload`). Emits `bread.paper.set.done` / `.failed`. A successful set also emits `bread.paper.changed`. |

### Not implemented: slideshow / library / random / next

breadpaper is not a wallpaper library, a slideshow daemon, or a picker.
Browsing `~/Pictures/Backgrounds` lives in bos-settings. Do not invent
`bread.command.paper.next` / `.random` / `.cycle` (or matching events)
ahead of a real product feature. Unrecognized verbs are ignored.

## Fail-safe behavior

- If breadd isn't installed or isn't running, `emit` is a silent no-op
  (`BreadClient::emit` never blocks or errors the caller) — breadpaper
  still sets the wallpaper, generates the palette, and reloads themes.
- `breadpaper listen` does not exit if breadd is down. The command
  subscription reconnects automatically (`BreadClient::subscribe`'s
  background thread has its own backoff loop); no restart of `listen`
  is needed once breadd returns.
