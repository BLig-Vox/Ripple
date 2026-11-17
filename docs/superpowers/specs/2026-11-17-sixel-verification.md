# Sixel and links verification

Root cause: Ripple had no DCS bitmap decoder and reported no Sixel capability.
Chafa therefore used symbol art; explicit Sixel output was ignored.

## Observed on Bot Screen (Ubuntu, X11)

- Chafa 1.14.0 with `-f sixels` rendered the real Ripple PNG as a bitmap.
- The bitmap remained visible after the shell prompt returned after correcting
  advance-down cursor placement.
- Entering an alternate screen hid the primary image; leaving restored it.
- CSI 2J erased the visible image.
- Ctrl + right-click on a named OSC 8 link opened https://example.org/ in Paisley.
- Ctrl + right-click on a visible URL opened https://example.com/ in Paisley.
- Independent source review found resize, reverse-scroll, scale-change and URL
  delimiter issues; these were fixed and re-reviewed with no important findings.
- The release build passed under a 2,000,000,000-byte cgroup cap, zero build swap,
  one Cargo job and one linker thread at lower scheduling priority. The build
  supervisor requires at least 4 GiB available system memory throughout.

No automated tests were added or run. Auto-probing with newer Chafa, Wayland,
non-square Sixel pixel aspect, shared palette updates and graphics protocols
other than 7-bit Sixel have not been verified. Older Chafa needs `-f sixels`.
