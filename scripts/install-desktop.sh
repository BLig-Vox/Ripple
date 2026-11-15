#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

build_profile=release
build_args=(--release)
if [[ "${1:-}" == --debug ]]; then
    build_profile=debug
    build_args=()
elif [[ $# -gt 0 ]]; then
    printf 'Usage: %s [--debug]\n' "$0" >&2
    exit 2
fi
cargo build --locked "${build_args[@]}"

app_bin_dir="$HOME/.local/bin"
app_data_dir="${XDG_DATA_HOME:-$HOME/.local/share}"
task_target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
install -Dm755 "$task_target_dir/$build_profile/ripple" "$app_bin_dir/ripple"
install -Dm644 assets/branding/ripple-logo.png "$app_data_dir/pixmaps/ripple.png"

# Absolute paths let launchers work even when ~/.local/bin isn't in the desktop PATH.
python3 - "$repo_root/assets/ripple.desktop" "$app_data_dir/applications/ripple.desktop" "$app_bin_dir/ripple" "$app_data_dir/pixmaps/ripple.png" <<'PY'
import pathlib
import sys

template, destination, executable, icon = sys.argv[1:]
def exec_quote(value):
    value = value.replace("\\", "\\\\").replace('"', '\\"').replace("`", "\\`").replace("$", "\\$")
    return '"' + value.replace("%", "%%") + '"'

content = pathlib.Path(template).read_text()
content = content.replace("Exec=ripple\n", "Exec=" + exec_quote(executable) + "\n", 1)
content = content.replace("TryExec=ripple\n", "TryExec=" + executable + "\n")
content = content.replace("Icon=ripple\n", "Icon=" + icon + "\n")
output = pathlib.Path(destination)
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(content)
PY

if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$app_data_dir/applications/ripple.desktop"
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$app_data_dir/applications"
fi
if command -v xdg-user-dir >/dev/null 2>&1; then
    app_desktop_dir="$(xdg-user-dir DESKTOP)"
else
    app_desktop_dir="$HOME/Desktop"
fi
install -Dm755 "$app_data_dir/applications/ripple.desktop" "$app_desktop_dir/ripple.desktop"
if command -v gio >/dev/null 2>&1; then
    gio set "$app_desktop_dir/ripple.desktop" metadata::trusted true 2>/dev/null || true
fi
printf 'Installed Ripple and launcher: %s\n' "$app_desktop_dir/ripple.desktop"
