# Ripple GTK-style window controls and multiline input

User requests a GTK-like title bar using Lucide Rust icons, plus Shift+Enter
multiline support in Codex. Retain the existing egui/wgpu architecture and terminal
font/renderer. This is visual styling, not a migration to GTK widgets.

- Compact 38-point charcoal header, small Ripple identity and three consistent
  right-hand controls. Lucide Minus, Square/Copy and X, 14-point strokes, generous
  pointer targets and tooltips. Neutral hover states, quiet separator.
- Client decorations replace the system title bar. Blank header drag and double
  click maximize; all edges/corners resize. Native minimize/maximize/close commands.
- Shift+Enter sends CSI13;2u, distinct from ordinary Enter CR. Maintain legacy
  Ctrl+C and shell behavior unless the child explicitly requests enhanced input.
- Support the Kitty disambiguation flag only; query/set/push/pop are bounded,
  unsupported flags masked, primary/alternate keyboard modes isolated. Do not
  advertise complete Kitty keyboard-protocol support.
- Inspect rendered controls and key delivery on Bot Screen. Compilation uses the
  existing hard decimal3GB supervisor, swap0 and one Cargo job. No automated tests
  are added or run without an explicit request, per workspace instructions.
