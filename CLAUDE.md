# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

`lumina_control_dmx` — a Rust program that acts as a **PC-side editor, visualizer, and mixer** for the **Mini Pearl 512A** DMX lighting console. The console saves shows to USB as a single file named `C.KKD` (always that name; saves overwrite). The end goal is to read, edit, visualize, and write back `C.KKD` so fixtures, positions, DMX patches, scenes, and chases can be configured on a PC instead of on the console's hardware UI.

### Known constraint: USB extraction

The console writes `C.KKD` with a malformed FAT cluster chain. Windows lists the file (~1.4 MB) but copying it fails with `0x80070570` *"the file or directory is corrupted and unreadable."* The bytes are on the stick — the filesystem metadata lies. **Do not try to fix the stick with chkdsk; that destroys the data.** Extract by raw block reads from `\\.\X:` (admin required) and carve the `C.KKD` payload from the resulting image.

## Phased plan

1. **Extract** — raw-dump the USB stick to `.img`, locate and carve `C.KKD` from it.
2. **Reverse-engineer** the `C.KKD` binary format (diff multiple saves with controlled, single-variable changes on the console).
3. **Editor + visualizer** — model fixtures, patches, scenes, chases; render DMX output; round-trip back to a valid `C.KKD` the console accepts.

## Commands

The project is not yet scaffolded. Once `cargo init` has been run:

- Build: `cargo build` / `cargo build --release`
- Run: `cargo run -- <args>`
- Test: `cargo test` (single: `cargo test <name>`)
- Lint: `cargo clippy --all-targets -- -D warnings`
- Format: `cargo fmt`

Raw USB reads on Windows require the terminal to be running **as Administrator** and the path `\\.\X:` (replace `X` with the actual drive letter — be very careful to pick the right one).

## Architecture notes (to populate as code lands)

- USB raw-read layer (Windows: `CreateFileW(\\.\X:)` + `ReadFile`; cross-platform abstraction can come later).
- `C.KKD` parser/serializer — keep read and write symmetric so round-trip fidelity can be tested byte-for-byte.
- Show data model — fixtures, patch (DMX address → fixture/channel), scenes, chases, playback state. Keep this independent of the on-disk format.
- DMX output / visualizer — a 512-channel universe, ~44 Hz refresh; visualizer can render in 2D/3D from fixture positions.
