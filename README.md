# breadpaper

Wallpaper manager for the bread desktop. Sets a wallpaper via [awww](https://github.com/heywoodlh/awww), generates a colour palette with [pywal](https://github.com/dylanaraps/pywal), and reloads bread themes in one command.

## Dependencies

Runtime:

- `awww` — Wayland wallpaper daemon
- `python-pywal` — colour palette generation
- `bread-theme` — theme reload (part of the bread ecosystem)

Build:

- Rust 1.85+ (edition 2024) / cargo

## Install

**From the bakery (recommended on bread OS):**

```
bakery install breadpaper
```

**From source:**

```
cargo build --release
install -Dm755 target/release/breadpaper ~/.local/bin/breadpaper
```

**Arch Linux (PKGBUILD):**

```
cd packaging/arch
makepkg -si
```

## Usage

```
breadpaper <path>          # set wallpaper (shorthand for `set`)
breadpaper set <path>      # set wallpaper, generate palette, reload themes
breadpaper get             # print the current wallpaper path
```

Supported formats: `png`, `jpg`, `jpeg`, `webp`, `gif`, `bmp`.

The current wallpaper path is stored by pywal at `~/.cache/wal/wal`.

## License

MIT — see [LICENSE](LICENSE).
