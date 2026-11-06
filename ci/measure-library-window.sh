#!/bin/sh
# Headless layout measurement for the Wallpapers window (PAPER-19).
#
# Starts its own weston headless compositor with isolated HOME and XDG
# directories, renders one wallpaper and then three, and prints the measured
# window, tile and thumbnail allocations. It never touches the live desktop.
#
# Requirements: a built binary (cargo build), weston and ImageMagick (magick).
# This is a development/verification helper, not a CI test.
set -eu

repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin=${BREADPAPER_BIN:-$repo/target/debug/breadpaper}

for tool in weston magick; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "missing required tool: $tool" >&2
        exit 1
    }
done
[ -x "$bin" ] || {
    echo "missing breadpaper binary: $bin (run cargo build first)" >&2
    exit 1
}

scratch=$(mktemp -d "${TMPDIR:-/tmp}/paper19-measure.XXXXXX")
weston_pid=
cleanup() {
    if [ -n "$weston_pid" ]; then
        kill "$weston_pid" 2>/dev/null || true
        wait "$weston_pid" 2>/dev/null || true
    fi
    rm -rf "$scratch"
}
trap cleanup EXIT INT TERM

mkdir -p "$scratch/walls" "$scratch/one" "$scratch/home/Pictures/Backgrounds" \
    "$scratch/run" "$scratch/xdg/breadpaper"
chmod 700 "$scratch/run"
magick -size 400x266 gradient:'#3050a0-#a03050' "$scratch/walls/one.png"
magick -size 400x266 gradient:'#105030-#30a050' "$scratch/walls/two.png"
magick -size 400x266 gradient:'#503010-#a08030' "$scratch/walls/three.png"
cp "$scratch/walls/one.png" "$scratch/one/one.png"
cp "$scratch/walls/one.png" "$scratch/home/Pictures/Backgrounds/one.png"
cat >"$scratch/xdg/breadpaper/config.toml" <<'EOF'
library_dirs = ["~/Pictures/Backgrounds", "/no/such/breadpaper-wallpapers"]
EOF

export XDG_RUNTIME_DIR="$scratch/run"
export HOME="$scratch/home"
export XDG_CONFIG_HOME="$scratch/xdg"
socket="paper19-$$"

weston \
    --backend=headless \
    --width=1200 \
    --height=1000 \
    --fake-seat \
    --socket="$socket" \
    --idle-time=0 \
    --renderer=pixman \
    --log="$scratch/weston.log" \
    >"$scratch/weston.out" 2>&1 &
weston_pid=$!

i=0
while [ ! -S "$XDG_RUNTIME_DIR/$socket" ]; do
    i=$((i + 1))
    if [ "$i" -gt 100 ]; then
        echo "weston did not start; see $scratch/weston.log" >&2
        exit 1
    fi
    sleep 0.1
done

measure_case() {
    label=$1
    dir=$2
    echo "== $label =="
    if [ -n "$dir" ]; then
        WAYLAND_DISPLAY="$socket" \
            GDK_BACKEND=wayland \
            GSK_RENDERER=cairo \
            BREADPAPER_LIBRARY_DIRS="$dir" \
            BREADPAPER_UI_MEASURE=1 \
            "$bin" library
    else
        WAYLAND_DISPLAY="$socket" \
            GDK_BACKEND=wayland \
            GSK_RENDERER=cairo \
            BREADPAPER_UI_MEASURE=1 \
            "$bin" library
    fi
}

measure_case "one wallpaper" "$scratch/one"
measure_case "three wallpapers" "$scratch/walls"
measure_case "canonical folder with a missing entry" ""

# Click feedback and persistence. awww is stubbed: the point is the tile
# state, the status line and current.json, not the compositor call.
mkdir -p "$scratch/bin"
printf '#!/bin/sh\nexit 0\n' >"$scratch/bin/awww"
chmod +x "$scratch/bin/awww"
echo "== click applies and persists =="
PATH="$scratch/bin:$PATH" \
    WAYLAND_DISPLAY="$socket" \
    GDK_BACKEND=wayland \
    GSK_RENDERER=cairo \
    BREADPAPER_LIBRARY_DIRS="$scratch/walls" \
    BREADPAPER_UI_MEASURE=1 \
    BREADPAPER_UI_MEASURE_CLICK=1 \
    "$bin" library
echo "current.json:"
cat "$XDG_CONFIG_HOME/breadpaper/current.json"
