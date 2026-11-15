# Ripple terminal design

## Goal

Build Ripple as a capable, GPU-rendered Rust terminal emulator with an uncluttered interface. The long-term ambition is mature terminal performance and features with a straightforward user experience. Ghostty and Kitty set the bar; this design does not claim feature parity in the first release.

## First launch experience

- Open a native Linux window at 80% of the active monitor's usable width and height, centered on that monitor.
- Use a dark, readable default theme and start a shell in the user's home directory.
- Keep the terminal as the main surface; make tabs and essential actions discoverable without a dense control panel.
- Keep the window resizable and let the terminal grid adapt to its current dimensions.

## Recommended architecture

- **App/window:** Rust with `eframe` and `egui` for the native window, app chrome, tabs, and settings.
- **Terminal viewport:** a dedicated `wgpu` renderer for cell backgrounds, glyphs, cursor, and selection. Keep this behind a renderer interface so rendering details do not leak into terminal state.
- **Terminal core:** separate modules for PTY lifecycle, terminal protocol parsing, screen/cell state, scrollback, input encoding, and rendering data. Keep UI state out of the core.
- **Initial platform:** Linux, matching the current Bot Screen. Keep platform-specific PTY/window integration behind small interfaces for future ports.
- **Dependency decisions:** confirm the current parser and PTY crates with a minimal integration spike before pinning them as architecture commitments.

`egui`/`eframe` provides the window and UI framework; `wgpu` is the GPU API used by the terminal renderer. The terminal's specialized rendering remains separate from egui's general UI painting. See the [eframe documentation](https://docs.rs/eframe/latest/eframe/) and [wgpu project](https://github.com/gfx-rs/wgpu).

## First usable milestone

The first milestone is a single-window terminal that:

1. Starts the user's default shell through a PTY and displays its output.
2. Supports basic ANSI colors and common cursor movement, line editing, and screen clearing.
3. Sends keyboard input and handles terminal resize correctly.
4. Provides a usable scrollback, text selection, copy, and paste.
5. Renders the terminal grid through `wgpu` and stays responsive while output is arriving.
6. Opens at the requested centered 80% geometry and remains usable at other sizes.

Tabs, configuration files, broad compatibility, ligatures, advanced graphics protocols, and extensive customization follow after this milestone. They are not prerequisites for the first executable.

## Engineering constraints

- Separate PTY/parser state changes from drawing so rendering can be optimized without changing terminal behavior.
- Avoid blocking the UI/render loop on shell I/O; pass data through bounded or otherwise controlled channels.
- Treat terminal escape sequences as untrusted input and keep parser behavior bounded.
- Start with a small supported compatibility target and add features based on explicit tests and real application behavior.
- Do not promise Ghostty or Kitty parity until compatibility and performance have been measured against named workloads.

## Open decisions for implementation

- Font shaping and glyph atlas strategy for the GPU renderer.
- Parser and PTY crate selection after checking current APIs and platform behavior.
- Whether the first shell launch offers a configurable shell picker or simply uses the user's default shell.
- How the window obtains usable monitor geometry and requests initial centering in the Linux desktop environment.

## Acceptance for the design

The user approves the Rust + egui/eframe app shell, dedicated wgpu terminal viewport, Linux-first scope, centered 80% startup window, and the staged feature approach before implementation begins.
