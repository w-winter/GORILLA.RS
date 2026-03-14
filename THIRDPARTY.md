# Third-party and reference material

## Attribution and provenance

This repository is a Rust port of IBM/Microsoft QBASIC `GORILLA.BAS`.

- The original `GORILLA.BAS` was published by Microsoft Corporation in 1991
- Microsoft retains copyright in the original QBASIC source and related original materials
- The dual MIT/Apache-2.0 license in this repository applies only to original code and repository-authored material
- No claim is made over Microsoft's original source, artwork, or other third-party reference material

See [`NOTICE`](NOTICE) for the short-form attribution statement carried with release archives.

## Embedded reference-derived data

The source tree contains small embedded data sets used for fidelity work:

- QBASIC-style `PLAY` sequence strings in `src/play_ref.rs`
- ASCII checkpoint and sprite reconstructions, palette-indexed render data in `src/render_ref.rs`

These were authored for this repository during fidelity work and remain reference-derived.

## Vendored third-party assets

The web build vendors Macroquad's JavaScript loader at:

- `web/vendor/mq_js_bundle.js`

That file was copied from the `macroquad` 0.4.14 crate (`js/mq_js_bundle.js`) and is covered by Macroquad's upstream `MIT OR Apache-2.0` license.

The runtime UI text uses a DOS/VGA-style font:

- `assets/fonts/Ac437_IBM_VGA_9x16.ttf`
- `assets/fonts/CC-BY-SA-4.0.txt`

That font comes from The Ultimate Oldschool PC Font Pack by VileR (`int10h.org`) and is distributed under `CC BY-SA 4.0`.

## Fidelity tooling

The comparison and extraction scripts in `tools/` are repository-authored.  They operate on Rust-generated outputs plus user-supplied reference logs; the repository does not bundle Microsoft's original BASIC source or any emulator captures.
