# Changelog

## 0.1.5

- Replace the bundled application and desktop logo with the flat golden ripple emblem.
- Branding-only release; terminal behavior is unchanged.

## 0.1.4

- Render Chafa Sixel bitmap images with bounded decoding, scrollback retention and visible GPU texture caching.
- Report Sixel capability and terminal pixel geometry for compatible image tools.
- Open OSC 8 links and visible HTTP/HTTPS URLs with Ctrl + right-click.
- Bound OSC payloads, link storage and the desktop URL opener queue.

## 0.1.3

- Add a compact GTK-style title bar with Lucide minimize, maximize/restore and close icons, native window dragging and resize edges.
- Distinguish Shift+Enter from Enter so Codex can compose multiline prompts.
- Support bounded, per-screen Kitty keyboard disambiguation negotiation and modified navigation/function keys.

## 0.1.2

- Bundle JetBrainsMono Nerd Font Mono for consistent text and terminal icon glyphs without a separate font installation.
- Measure the terminal grid from the active font and place glyphs on cell boundaries, keeping text, colored backgrounds, selection and cursor aligned.
- Preserve the application background when clearing, scrolling, inserting/deleting cells and growing the grid. This fixes black gaps in Neovim themes.
- Parse colon-form RGB and indexed SGR colors as well as semicolon-form colors.
- Advertise truecolor support through COLORTERM.

Ripple remains an early terminal prototype with incomplete VT and Unicode support. This patch does not add full shaping, wide/combining character handling or every text style.
