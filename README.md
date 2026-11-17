# Ripple

A Rust terminal emulator with a GPU-rendered interface and a simple workflow.

![Ripple running on Linux](assets/screenshots/ripple-linux.png)

## Download

Get the Linux x86_64 build from [GitHub Releases](https://github.com/BLig-Vox/Ripple/releases). Version 0.1.4 is an early prerelease. The binary is built on Ubuntu 24.04 and requires glibc 2.39 or newer, an X11 or Wayland desktop, and a working Vulkan or EGL/OpenGL graphics driver. Other Linux distributions have not been verified.

```sh
sha256sum -c SHA256SUMS
tar -xzf ripple-v0.1.4-linux-x86_64.tar.gz
cd ripple-v0.1.4-linux-x86_64
./ripple
```

To put the binary on your PATH:

```sh
install -Dm755 ripple "$HOME/.local/bin/ripple"
```

For application menu and desktop integration, use the source installer below.

## Current state

The first Linux prototype uses `eframe` with its wgpu renderer and a custom wgpu pass for cell backgrounds and the cursor. It launches the default shell through a PTY, parses a starter ANSI/VT subset, and provides bounded scrollback, drag selection, and clipboard shortcuts. It opens at 80% of the active monitor's dimensions and centers itself on that monitor.

This is an early prototype, not yet a feature-complete terminal. Current limitations include single-window operation, incomplete VT compatibility, no shaped GPU glyph atlas, and partial terminal graphics support.

## Arch Linux packaging

The source package recipe is in [packaging/arch](packaging/arch/README.md).
An Arch-built `vox-ripple` package and checksum are available on the
[v0.1.0 release](https://github.com/BLig-Vox/Ripple/releases/tag/v0.1.0).
It has **not been published to the AUR yet**; new account registration is temporarily closed.

## Cargo installation

Linux prerequisites: Rust 1.95 or newer, a C build toolchain, `curl`, Python 3,
and the graphics libraries/driver required by winit/wgpu. Run this as your normal
user. It installs Ripple, adds the application menu entry and logo, creates a
desktop shortcut, then clears Cargo's downloaded crate archives and extracted
crate sources. Run it when no other Cargo builds are active.

```bash
(
set -eu

# 1. Install the terminal.
ripple_cargo_home="${CARGO_HOME:-$HOME/.cargo}"
cargo install vox-ripple --version 0.1.4 --locked --root "$ripple_cargo_home"
ripple_data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
ripple_desktop_dir="$(xdg-user-dir DESKTOP 2>/dev/null || printf '%s/Desktop' "$HOME")"
ripple_tmp="$(mktemp -d)"
trap 'rm -rf -- "$ripple_tmp"' EXIT

# 2. Install the logo and launchers without rebuilding Ripple.
curl -fL https://raw.githubusercontent.com/BLig-Vox/Ripple/v0.1.4/assets/branding/ripple-logo.png \
  -o "$ripple_tmp/ripple.png"
install -Dm644 "$ripple_tmp/ripple.png" "$ripple_data_home/pixmaps/ripple.png"

python3 - "$ripple_cargo_home/bin/ripple" "$ripple_data_home" "$ripple_desktop_dir" <<'PYTHON'
import pathlib
import sys

binary, data, desktop = map(pathlib.Path, sys.argv[1:])
if not binary.is_file():
    raise SystemExit(f"Installed executable not found: {binary}")

def value(text):
    return str(text).replace("\\", "\\\\").replace("\n", "\\n").replace("\t", "\\t").replace("\r", "\\r")

def command(path):
    text = str(path).replace("\\", "\\\\").replace('"', '\\"')
    text = text.replace("$", "\\$").replace("`", "\\`").replace("%", "%%")
    return value('"' + text + '"')

entry = ("[Desktop Entry]\nType=Application\nName=Ripple\n"
         "Comment=A simple Rust terminal emulator\n"
         f"Exec={command(binary)}\nTryExec={value(binary)}\n"
         f"Icon={value(data / 'pixmaps/ripple.png')}\n"
         "Terminal=false\nCategories=System;TerminalEmulator;\n"
         "StartupNotify=true\nStartupWMClass=ripple\n")
for destination in (data / "applications/ripple.desktop", desktop / "ripple.desktop"):
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(entry)
    destination.chmod(0o755)
PYTHON

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$ripple_data_home/applications"
fi
if command -v gio >/dev/null 2>&1; then
    gio set "$ripple_desktop_dir/ripple.desktop" metadata::trusted true 2>/dev/null || true
fi

# 3. Clear downloaded crate caches; keep installed programs and Cargo settings.
rm -rf -- "$ripple_cargo_home/registry/cache" "$ripple_cargo_home/registry/src"
printf 'Installed Ripple with its logo and desktop launchers.\n'
)
```

Launch **Ripple** from your application menu, or run
`"${CARGO_HOME:-$HOME/.cargo}/bin/ripple"`. Some desktop environments require
right-clicking the desktop shortcut and choosing **Allow Launching**. Hidden
desktop icons stay hidden.

Cache cleanup affects all downloaded registry crates, so future Cargo builds may
need to download them again. It keeps the registry index, Git caches, installed
binaries, Rust toolchains, credentials, and installation records. It does not
remove existing project `target` directories or a custom `CARGO_TARGET_DIR`.

For installation directly from the existing GitHub release:

```sh
cargo install --git https://github.com/BLig-Vox/Ripple --tag v0.1.4 --locked --bin ripple
```

## Build and run

Install a recent stable Rust toolchain and the Linux development libraries required by winit/wgpu. Then run:

```sh
cargo run --release
```

The window uses wgpu through eframe. Vulkan or a supported desktop graphics backend must be available.

## Appearance and shortcuts

Ripple opens as one plain terminal surface with a compact GTK-style title bar, Lucide window controls and 8 pixels of terminal padding. There are no permanent toolbars or status badges. JetBrainsMono Nerd Font Mono is bundled for consistent terminal symbols; its license is in `assets/fonts`. Text, selection, backgrounds and the cursor use the same measured cell width. Drag to select text, use Ctrl+Shift+C to copy, Ctrl+Shift+V to paste, and the mouse wheel for scrollback.

## Clipboard images

Shift+Enter sends a distinct modified Enter sequence, allowing compatible applications such as Codex to insert a newline while Enter submits. Ripple supports the disambiguation flag of the Kitty keyboard protocol; other enhancement flags are not advertised.

Ctrl+Shift+V accepts text or an image. Image clipboard data is saved as a private
temporary PNG and its file path is pasted into the terminal. Applications that
accept image paths, including Codex CLI, can attach that file. Ripple does not
send binary image bytes to the shell. Ctrl+V remains a raw control key for the
foreground application.

Images stay readable until the Ripple window closes normally, then the temporary
files are removed. Image pastes are limited to 32 files and 64 MiB of saved PNGs
per window, with a 16-megapixel limit per image. The temporary path is local to
the machine running Ripple. Successful paste also clears an earlier clipboard
error message.

## Memory and session lifecycle

PTY output uses a bounded 32-chunk queue (8 KiB per chunk). Scrollback is limited to 2,000 lines and 8 MiB of retained cell storage, whichever is reached first. The visible grid is limited to 512 columns and 256 rows. GPU vertex buffers grow only to the visible grid's needs and are owned by the renderer.

Session shutdown cancels the reader, joins its thread, and terminates and reaps the shell. Reader waits and queue sends check cancellation every 50 ms. Regression tests repeatedly open and close sessions with both idle and saturated output and check child processes, thread counts, and file descriptors.

```sh
cargo test -- --test-threads=1
python3 scripts/memory-soak.py <ripple-pid> 60
```

The sampler reports Linux RSS, thread counts, and descriptor counts. RSS includes graphics-driver caches and allocator retention; a short stable run does not prove the absence of every possible leak.

## Desktop launcher

Run `./scripts/install-desktop.sh` to build a release binary and install Ripple for the current user. The installer adds an application menu entry, a desktop shortcut, and the logo. Use `--debug` for a development build. Rust, Python 3, and the Linux build prerequisites above must be installed.

## Full-screen applications

Ripple recognizes the alternate-screen sequences used by Neovim (`?1049h` / `?1049l`). The shell screen is restored on exit, and alternate-screen output never enters shell scrollback. Scrolling regions, reverse index, line insertion/deletion, cursor visibility, application cursor keys, basic terminal replies, truecolor, and bracketed paste are supported. Wheel events go to applications that enable mouse reporting; otherwise alternate-screen wheel input sends cursor keys. This remains a limited VT implementation.

## License

Ripple is licensed under the [MIT License](LICENSE.md).

## Images and links

Ripple supports **Sixel bitmap images**. Recent Chafa versions with terminal
probing can select Sixel after querying Ripple. For older Chafa (including
Ubuntu 24.04's 1.14), select the format explicitly:

```sh
chafa -f sixels image.png
```

Images scroll with terminal output. Erasing/overwriting a covered area discards
the affected image; shrinking the terminal discards images crossing removed
rows/columns. Alternate screens have separate images. Input is limited to 4 MiB,
each image to 2048×2048 pixels, and retained images to 32 images/32 MiB per screen.
Only 7-bit Sixel is implemented; Kitty/iTerm graphics, shared palette updates and
non-square pixel aspect scaling are not implemented.

**Ctrl + right-click** an OSC 8 hyperlink or a visible `http://` / `https://` URL
to open it in the system's default browser. Hold Ctrl to see the link cursor.
Only HTTP/HTTPS is allowed; text is passed as an argument to `gio open` or
`xdg-open`, never a shell. Plain URL detection requires a complete URL within
one terminal row; OSC 8 links also work across wrapped labels.
