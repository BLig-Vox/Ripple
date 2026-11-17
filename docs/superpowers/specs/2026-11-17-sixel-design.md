# Chafa image support

Chafa currently falls back to colored characters: Ripple has no graphics decoder
and reports no bitmap capability. Add bounded Sixel bitmap decoding and renderer
textures, capability and cell geometry replies, and PTY pixel dimensions.
Sixel is the common format Chafa can probe; Kitty/iTerm protocols are separate
future work. Keep text rendering and keyboard behavior unchanged.

Accept 7-bit DCS Sixel with RGB/HLS palettes, raster dimensions, repeats and
transparent backgrounds. Bound input to 4 MiB, images to 2048x2048 pixels, paint
work to 16 million pixel writes, retained images to 32/32 MiB per screen.
Images move with normal scrollback; alternate screens keep separate images.
Overlapping text/erase operations discard affected images; do not leave stale
textures after eviction. Unsupported or malformed payloads are discarded.

Build under a hard decimal 2 GB cap, one Cargo job, zero build swap. Inspect
actual Chafa output on Bot Screen and obtain independent source review. No new
automated tests are requested. Publish the patch to crates.io and GitHub through
Vox in Bot Screen, including explicit instructions for older Chafa versions that
cannot probe Sixel automatically.

Also include OSC 8 HTTP/HTTPS links and visible URL detection, opened only by
Ctrl+right-click. Bound OSC input, URL size, stored link IDs and the desktop
opener queue. Pass URLs as process arguments with no shell interpolation.
