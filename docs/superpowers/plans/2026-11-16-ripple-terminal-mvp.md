# Ripple terminal MVP Implementation Plan

> **For agentic workers:** Use `superpowers:executing-plans` to implement this plan task-by-task. Keep implementation in this session.

**Goal:** Build the first Linux Ripple executable with a centered 80% window, a working shell PTY, basic terminal state, and a dedicated wgpu viewport.

**Architecture:** Use eframe/egui for the native application shell and window controls. Keep terminal state and PTY communication independent from UI state, and paint the terminal cells in a wgpu render path. Keep the first milestone single-window and Linux-first.

**Tech Stack:** Rust, eframe, egui, wgpu, a PTY crate and a VT parser selected after checking their current Linux APIs.

**Spec:** `docs/superpowers/specs/2026-11-16-ripple-terminal-design.md`

## Global Constraints

- Initial window is 80% of the active monitor's usable width and height, centered on that monitor.
- Linux is the first platform.
- Terminal drawing uses a dedicated wgpu renderer; egui owns app chrome.
- Keep PTY/parser state out of the UI and do not block the render loop on shell I/O.
- Keep the first milestone to one window; tabs and broad compatibility are later work.
- Do not claim Ghostty or Kitty feature parity for this milestone.

## Review Focus

- PTY output bursts must not freeze the UI; use a channel and bounded update batches.
- Terminal resize must update both the PTY dimensions and the visible cell grid.
- Escape sequences must not panic or cause unbounded allocation.
- Startup sizing and centering must use usable monitor geometry and account for decorations.
- GPU renderer failure or unsupported adapter should produce a clear startup error.

---

### Task 1: Native app shell and startup geometry

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/app.rs`
- Create: `src/window_geometry.rs`

**Interfaces:**
- `RippleApp::new(cc: &eframe::CreationContext<'_>) -> Self`
- `window_geometry::initial_viewport(...)` returns an eframe viewport configured for 80% usable monitor dimensions and centered position.

- [ ] Create the binary crate and pin compatible `eframe` dependencies with the wgpu renderer enabled.
- [ ] Implement a dark eframe window with a simple app title and a placeholder terminal surface.
- [ ] Compute initial geometry from the active monitor's usable area; retain normal resizability.
- [ ] Show a clear error if the requested renderer cannot initialize.

### Task 2: Terminal state and PTY session

**Files:**
- Create: `src/terminal/mod.rs`
- Create: `src/terminal/grid.rs`
- Create: `src/terminal/session.rs`
- Modify: `src/app.rs`

**Interfaces:**
- `TerminalGrid` owns rows, columns, cursor, colors, selection, and bounded scrollback.
- `TerminalSession` starts the user's default shell in a PTY, sends input and resize requests, and delivers output batches to the app without blocking it.

- [ ] Select and add Linux-compatible PTY and VT parser dependencies after checking current APIs.
- [ ] Start the shell in the user's home directory and route keyboard input through the PTY.
- [ ] Parse a small initial ANSI/VT subset into grid updates; handle malformed input without panic.
- [ ] Resize the PTY and grid together; retain a bounded scrollback buffer.

### Task 3: Dedicated GPU terminal viewport

**Files:**
- Create: `src/terminal/renderer.rs`
- Modify: `src/app.rs`
- Modify: `Cargo.toml`

**Interfaces:**
- `TerminalRenderer::paint(...)` consumes an immutable terminal snapshot and viewport dimensions.
- Renderer owns GPU resources; terminal state remains owned by `TerminalGrid`.

- [ ] Use eframe's wgpu paint callback/custom rendering path for terminal cells, glyphs, cursor, and selection.
- [ ] Start with a fixed monospace font and glyph atlas; document the font source and fallback behavior.
- [ ] Update rendering dimensions with the grid and avoid blocking on PTY output.

### Task 4: Essential terminal interactions and runnable first release

**Files:**
- Modify: `src/app.rs`
- Modify: `src/terminal/grid.rs`
- Modify: `src/terminal/session.rs`
- Modify: `README.md`

- [ ] Add copy/paste and text selection using the desktop clipboard.
- [ ] Add basic scrollback navigation and terminal resize handling.
- [ ] Document build prerequisites, run instructions, supported initial VT behavior, and current limitations.
- [ ] Run the application in the Bot Screen and inspect the startup window and interactive shell before calling the milestone complete.
