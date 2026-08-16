# breadpaper

Wallpaper setter for the bread desktop. One command sets the wallpaper
via [awww](https://github.com/heywoodlh/awww), generates a palette with
[pywal](https://github.com/dylanaraps/pywal) (`wal`), and runs
`bread-theme reload`. Two monitors can keep different wallpapers (and
per-output bread-theme files); the last path per output is stored in
`~/.config/breadpaper/current.json`.

`set` / `get` / `apply` stay one-shot CLI. `breadpaper library` (alias
`browse`) opens a GTK picker over the wallpaper directories. It is not
a slideshow daemon.

## Dependencies

Must be on `$PATH` for `set`:

- `awww` — Wayland wallpaper (`awww img`)
- `wal` — palette generation (`python-pywal`)
- `bread-theme` — theme reload (bakery package, not `breadd`)

`library` also needs GTK4 (the window loads `bread-theme`'s shared
stylesheet and follows the monitor it sits on). `get` without
`--output` still reads the path pywal stored at `~/.cache/wal/wal`.
`get --output NAME` reads `~/.config/breadpaper/current.json`.

## Install

```
bakery install breadpaper
```

From source:

```
cargo build --release
install -Dm755 target/release/breadpaper ~/.local/bin/breadpaper
```

## Usage

```
breadpaper <path>                 # shorthand for `set` (all outputs)
breadpaper <path> --output NAME   # one output
breadpaper set <path>             # awww + wal + bread-theme reload (all)
breadpaper set <path> --output NAME
breadpaper get                    # print ~/.cache/wal/wal
breadpaper get --output NAME      # path from current.json
breadpaper apply                  # restore current.json
breadpaper library                # GTK picker (alias: browse)
breadpaper library --dir PATH     # also scan PATH (repeatable)
breadpaper listen                 # honor bread.command.paper.set / .library
```

Supported formats: `png`, `jpg`, `jpeg`, `webp`, `gif`, `bmp`.

## Library

`breadpaper library` scans these directories (missing ones are skipped):

1. `~/Pictures/Wallpapers`
2. `/usr/share/backgrounds/bos`

Override the list in `~/.config/breadpaper/config.toml`:

```toml
library_dirs = [
    "~/Pictures/Wallpapers",
    "/usr/share/backgrounds/bos",
]
```

`BREADPAPER_LIBRARY_DIRS` (colon-separated) overrides the file. `--dir`
appends extra roots for that invocation. Clicking a thumbnail applies
to the monitor the picker is on (`set --output`); if the output cannot
be resolved it falls back to all outputs.

## Bread events

After a successful `set`, breadpaper emits `bread.paper.changed` if
`breadd` is running (silent no-op if it isn't). `breadpaper listen` is
the optional long-running subscriber for `bread.command.paper.set` and
`bread.command.paper.library`; it does not start by itself. Lua modules
can still `bread.exec("breadpaper set …")` or
`bread.exec("breadpaper library")`. See [EVENTS.md](EVENTS.md).

## License

MIT — see [LICENSE](LICENSE).
