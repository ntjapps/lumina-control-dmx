# lumina_control_dmx

PC-side editor, visualizer, and mixer for the **Avolites Mini Pearl 512A** DMX
lighting console. The console saves shows to USB as `*.KKD` files; this project
reverse-engineers that format so fixtures, patches, scenes, and chases can be
edited on a PC and written back.

## Project status

Pre-alpha. The scaffold provides the USB-extraction path and primitives for
diffing/searching `.KKD` files. Most of the on-disk format is decoded
**enough to read** patches, scenes, and chases; round-trip writing is not yet
implemented.

See **[`maps.md`](maps.md)** for the living format map, and
[`memory/`](https://github.com/ntjapps/lumina-control-dmx) (private to my
Claude memory) for higher-level notes.

## Why a custom tool

The console writes `.KKD` to FAT16 USB sticks with a **malformed cluster
chain**. Windows lists the file (~1.4 MB) but copying it fails with
`0x80070570 — "the file or directory is corrupted and unreadable."` The bytes
are intact on the stick; only the FAT metadata lies. Do **not** run `chkdsk` —
that destroys the data.

This tool:
1. Raw-reads the volume bytes via `\\.\X:` (admin required on Windows) into
   a single `.img`.
2. Parses the FAT16 boot sector and root directory to locate `*.KKD`.
3. Reads `entry.size` bytes contiguously from the file's start cluster,
   bypassing the broken chain.

## Build

Standard Rust crate (edition 2024).

```
cargo build --release
```

Binary lands at `target/release/lumina_control_dmx.exe`.

## Subcommands

```
lumina_control_dmx dump  <drive-letter> <output.img>
lumina_control_dmx carve <image>        [filename] [output]
lumina_control_dmx diff  <a.kkd>        <b.kkd>
lumina_control_dmx find  <file>         "<hex pattern>"
```

- `dump` — raw read `\\.\X:` → image. Refuses `C:` as a guard. **Run terminal
  as Administrator**. Replace `X` with the actual drive letter; check it
  carefully.
- `carve` — find a file in the image's root directory and extract its bytes
  contiguously from the start cluster (ignores the bad cluster chain).
- `diff` — byte-diff two `.KKD`s with run grouping + ASCII context. Driving
  tool for format discovery.
- `find` — locate a hex-byte pattern.

## Reverse-engineering workflow

The format is being decoded by **single-variable controlled saves** on the
console (record one scene at a time, patch one fixture at a time, etc.) and
diffing successive carved `.KKD` files. Test save log and per-region
findings live in [`maps.md`](maps.md).

## Specs (per the console)

- One DMX universe — **512 channels**, refresh ~44 Hz.
- **60 fixture handles** in 3 banks of 20 (display numbers `1–20`, `101–120`,
  `201–220`).
- **120 playback faders** (10 pages × 12 faders per page; 0-indexed in the
  file). Each fader can hold a scene (1 step) or chase (N steps).
- Personalities are **Avolites R20** text files dropped on the same USB stick
  alongside `*.KKD`.

## Repo layout

```
src/
  main.rs       CLI dispatch
  dump.rs       Raw \\.\X: read (Windows)
  carve.rs      FAT16 boot/root-dir parser + contiguous read by cluster
  diff.rs       Byte-diff with run grouping
  find.rs       Hex-pattern search
maps.md         Living format map
dumps/          Carved .KKD samples + R20 personalities (gitignored)
```

## Status — what's decoded

- File header, slot name (10-byte ASCII).
- Personality library (`0x61000`, stride `0x800`).
- Fixture→DMX patch table (`0x60878`, 60 × u16 LE).
- Sorted DMX-channel and fixture-id index lists.
- Playback-fader index table (`0x70C00`, stride `0x400`); unified scene/chase
  layout.
- Scene/chase record region (`0x93000`, stride `0x1000`), located via
  slot-id array in the index entry.
- Trailer with `KINGKONG1024` magic at end of file.

What's still unmapped: see "Coverage" section in [`maps.md`](maps.md).
