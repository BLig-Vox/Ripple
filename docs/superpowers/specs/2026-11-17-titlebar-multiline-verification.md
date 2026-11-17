# Title bar and multiline verification

## Root cause and fix

The previous keyboard handler sent carriage return for Enter without checking
modifiers, so Codex could not distinguish Shift+Enter. Modified Enter now uses
CSI-u. Applications can query and enable the Kitty disambiguation flag; mode
stacks are bounded and separate for the primary and alternate screens. Plain
Enter, legacy control characters, text/IME and bracketed paste remain separate.

The header is client decoration rendered in egui, using egui-lucide 0.2.0.
Window operations are forwarded to winit rather than implemented in terminal
content. PNG and SVG loaders are registered once.

## Observed on Ubuntu Bot Screen

- Header, logo and Lucide controls render with no system title bar above them.
- Maximize, restore and minimize respond to the header buttons.
- Codex CLI 0.169.0 launched with a temporary local provider and no credentials.
  Typing `First line`, Shift+Enter, `Second line` produced a two-line draft;
  nothing was submitted to a model.
- Ubuntu-compatible debug builds succeeded under a hard 3,000,000,000-byte
  cgroup limit, swap limit zero and one Cargo job.
- Independent read-only review found F3 and shifted-punctuation encoding issues;
  these were corrected and re-reviewed with no remaining important findings.
- No automated tests were added or run for this request.

## Remaining verification limits

The Bot Screen session is X11. Wayland, custom drag/resize gestures and non-US
keyboard layouts were not verified. Shifted ASCII punctuation mapping follows
the US layout because egui does not retain the unshifted logical character.
Only the disambiguation flag is supported, not the complete Kitty protocol.
