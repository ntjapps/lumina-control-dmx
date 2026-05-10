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

## Reverse-engineering workflow (how to operate in this repo)

This repo's RE workflow is built on a small set of **Rust subcommands in `src/`**, run via `cargo run -- …`. Use them — do **not** reach for Python or other scripting languages. If a probe doesn't exist, add a Rust subcommand for it.

### Tools available

| Subcommand | Module | When to use |
|---|---|---|
| `dump <drive> <out.img>` | `src/dump.rs` | Raw-read a USB stick on Windows (`\\.\X:`). Admin shell required. Refuses `C:` |
| `carve <image> [name] [out]` | `src/carve.rs` | Parse FAT, locate the named file's directory entry, read `size` bytes from start cluster. Bypasses the malformed cluster chain. Defaults to `C.KKD` |
| `diff <a.kkd> <b.kkd>` | `src/diff.rs` | Byte-diff two carved files. Prints contiguous differing runs (gap-merged ≤ 8 B) with hex+ASCII context. Tolerates size mismatches |
| `find <file> "<hex bytes>"` | `src/find.rs` | Scan a file for a hex byte pattern. Useful to locate a known value (e.g. a DMX address, a level byte) anywhere in the file |
| `inspect <kkd> [kkd ...]` | `src/inspect.rs` | Side-by-side dump of all known offsets across multiple files. Add new field probes here as the map grows — this replaces ad-hoc scripts |

### Standard mapping loop

When the user provides a new save (or a new wipe stage), the loop is:

1. **Carve** the new file from `dumps/dump.imp` with `carve`. If the file isn't named `C.KKD`, use the explicit-name form.
2. **Inspect** the new file alongside its peers to see how known fields look. This catches the "is this exactly what I expect?" question cheaply.
3. **Diff** against the prior stage to see what specifically changed. The diff output's hex offsets tell you which region of `maps.md` to consult or update.
4. **Cross-reference** findings against `maps.md`'s field table and the wipe-test series before drawing conclusions. A field that appears at offset X in one save may live at X+δ in another era — check before claiming a "mirror."
5. **Update `maps.md`** with: new test-saves row, refinements to existing field entries, retracted hypotheses where the new data contradicts them. The "Wipe-test diff series" section is the model for documenting a new staged-wipe capture.
6. **Extend `inspect.rs`** if the new findings reveal a field worth probing across all saves on every future inspection. Don't write throwaway scripts.

### Conventions

- USB images: `dumps/dump.imp` (gitignored). Each new dump generally **adds** files (BKP, WP1..WPn) rather than replacing the prior set, so prior carved files remain valid evidence.
- Carved files: `dumps/<NAME>.KKD` (gitignored).
- `maps.md` is the single source of truth for the format. Findings that contradict it require updating it — not just noting in chat.
- Memory in `~/.claude/projects/.../memory/` complements `maps.md`: keep cross-conversation invariants there (the Rust-only rule, dump-file naming meanings, etc.). Don't duplicate `maps.md` content into memory.

## Architecture notes (to populate as code lands)

- USB raw-read layer (Windows: `CreateFileW(\\.\X:)` + `ReadFile`; cross-platform abstraction can come later).
- `C.KKD` parser/serializer — keep read and write symmetric so round-trip fidelity can be tested byte-for-byte.
- Show data model — fixtures, patch (DMX address → fixture/channel), scenes, chases, playback state. Keep this independent of the on-disk format.
- DMX output / visualizer — a 512-channel universe, ~44 Hz refresh; visualizer can render in 2D/3D from fixture positions.
