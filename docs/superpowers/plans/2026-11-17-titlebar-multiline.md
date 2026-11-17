# Implementation plan

1. Isolate keyboard encoding in src/keyboard.rs. Preserve legacy controls,
   application cursor sequences, paste and Unicode Text events; encode modified
   Enter and negotiated disambiguation explicitly.
2. Add bounded per-screen enhancement modes and CSI negotiation to TerminalGrid.
3. Add egui-lucide/egui_extras SVG support; implement src/window_chrome.rs and
   disable system decorations. Keep hit regions outside terminal layout.
4. Build with the 3GB/swap0/jobs1 supervisor and Ubuntu-compatible linker.
5. Inspect native appearance, window controls, resize/drag and actual keyboard
   bytes on Bot Screen; inspect a Codex draft if a local CLI session is available.
6. Read-only independent code review, address findings, document results and
   remaining verification limits.
7. User subsequently requested publication: bump to 0.1.3, build the release
   under the same memory cap, install on Bot Screen, publish to crates.io and
   create the GitHub release through the Vox account in Bot Screen.
