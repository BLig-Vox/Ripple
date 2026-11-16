# Changelog

## 0.1.2

- Bundle JetBrainsMono Nerd Font Mono for consistent text and terminal icon glyphs without a separate font installation.
- Measure the terminal grid from the active font and place glyphs on cell boundaries, keeping text, colored backgrounds, selection and cursor aligned.
- Preserve the application background when clearing, scrolling, inserting/deleting cells and growing the grid. This fixes black gaps in Neovim themes.
- Parse colon-form RGB and indexed SGR colors as well as semicolon-form colors.
- Advertise truecolor support through COLORTERM.

Ripple remains an early terminal prototype with incomplete VT and Unicode support. This patch does not add full shaping, wide/combining character handling or every text style.
