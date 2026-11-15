# Memory and UI checks

Checked on the project's Ubuntu Bot Screen, 2026-11-16.

## Changes

- Plain charcoal terminal surface, native title bar, 8 px padding, neutral cursor and selection. Removed permanent branding, shell badge, status bar, dimensions label, and rounded terminal inset.
- Cancellable PTY reader with an owned join handle. Shutdown joins the reader and reaps the shell. Fallible setup obtains I/O handles before spawning the shell and cleans up if thread creation fails.
- History bounded by both 2,000 lines and 8 MiB of retained cell allocations; visible grid bounded to 512 columns by 256 rows. Output queue remains 32 chunks of up to 8 KiB.

## Evidence

The lifecycle regression failed before the fix because a reader thread remained alive after Drop. After the fix, all three tests pass:

- 12 session open/close cycles, alternating idle shells and fully saturated output queues; child processes disappear and thread/descriptor counts return to baseline after every close.
- 15,000 output lines over 30 width changes; history allocations stay within the byte and line limits, and clear-history releases retained rows.
- Excessive grid sizes are clamped before allocation.

`cargo build`, `cargo fmt -- --check`, and `git diff --check` also passed.

The native UI was inspected directly on the Bot Screen; shell output and clear-screen were exercised. Browser/Playwright review does not apply to this native app.

## Runtime memory samples

`python3 scripts/memory-soak.py <pid> <seconds>` sampled `/proc` every two seconds.

- First run: 60 seconds, 80,000 generated output lines and a window resize. RSS rose from 149,964 KiB to 163,704 KiB as history and graphics allocations warmed up. Threads remained 28 and descriptors remained 13.
- Second run: 40 seconds at the same window size, another 80,000 output lines. Every RSS sample was 163,704 KiB (about 160 MiB); threads remained 28 and descriptors remained 13.

These runs show stable retained memory under the tested repeated workload and successful session cleanup. They are not a proof that all application or graphics-driver paths are leak-free. The Bot Screen reports a DRI3/EGL warning, so its graphics memory figures should not be treated as hardware GPU benchmarks.

## Desktop integration and alternate screen follow-up

The logo is bundled as the native window icon. The user installer adds a validated `.desktop` file to the application menu and `/home/bot/Desktop` in the Bot Screen. Launching that desktop file with `gio launch` opened Ripple with the logo in its native title bar.

Two additional regression tests cover alternate-screen isolation/restoration across resize, and scrolling-region/reverse-index behavior. All five tests passed with `cargo test -- --test-threads=1` after these changes. The development binary was rebuilt and installed using `scripts/install-desktop.sh --debug`.

Interactive check: opened the README in `nvim --clean -n` inside the installed Ripple. Six wheel-down events scrolled the editor from line 1 to line 40 without exposing shell history. Quitting Neovim restored the shell screen, including a marker printed before editor startup. This validates the reported alternate-screen/scrollback problem; it does not establish complete Neovim or xterm compatibility. Control-sequence semantics were checked against the upstream XTerm control sequences documentation: https://invisible-island.net/xterm/ctlseqs/ctlseqs.html.
