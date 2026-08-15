# breadpaper

Wallpaper setter for the bread desktop. One command sets the wallpaper
via [awww](https://github.com/heywoodlh/awww), generates a palette with
[pywal](https://github.com/dylanaraps/pywal) (`wal`), and runs
`bread-theme reload`.

It is not a wallpaper library, a slideshow daemon, or a GUI. Browsing
`~/Pictures/Backgrounds` and picking an image lives in
[bos-settings](https://git.breadway.dev/Breadway/bos-settings).

## Dependencies

Must be on `$PATH` for `set`:

- `awww` — Wayland wallpaper (`awww img`)
- `wal` — palette generation (`python-pywal`)
- `bread-theme` — theme reload (bakery package, not `breadd`)

`get` only reads the path pywal stored at `~/.cache/wal/wal`.

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
breadpaper <path>          # shorthand for `set`
breadpaper set <path>      # awww + wal + bread-theme reload
breadpaper get             # print the current wallpaper path
```

Supported formats: `png`, `jpg`, `jpeg`, `webp`, `gif`, `bmp`.

## License

MIT — see [LICENSE](LICENSE).
