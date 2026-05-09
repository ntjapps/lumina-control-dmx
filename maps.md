# Mini Pearl 512A `.KKD` Format Map

This document tracks reverse-engineering progress on the Mini Pearl 512A show
file format. The console saves to USB as `A.KKD` / `B.KKD` / `C.KKD` … (slot
letter rotates per save). Files are nominally **1,433,088 bytes** plus
**0x1000 (4096) bytes per recorded scene**.

> **Important value range:** dimmer levels are stored as raw bytes 0–255 (DMX
> 8-bit). On the console, "100% playback" refers to a master fader/grand-master
> applied during playback — it is not the same as a stored level of 255. The
> level we see in a scene record is the literal value the user set when
> recording (e.g. 100 = `0x64`).

---

## File-level structure (high level)

| Offset | Size | What |
|---|---|---|
| `0x0000` | 20 bytes | ASCII header ending in `"SHOWDATA"` |
| `0x0014..0x001D` | 10 bytes | **Slot name** (ASCII, NUL-padded). Console rotates A→B→…→Z→AA→AB→…→AAAAA-AA, etc. Matches USB filename. Verified G=`"G"`, AA=`"AA"`, AAAAA-AA=`"AAAAA-AA"` |
| `0x009C` | 6 bytes | **Personality counter array.** Decrements on each personality deletion. G=`09 09 09 09 0A 0A` (10 personalities), AA=`08 08 08 08 09 09`, AAAAA-AA=`07 07 07 07 08 08`. Mirrored at `0x60840` |
| `~0x0110` | bytes | **Personality slot permutation/order array.** On deletion, deleted slot is zeroed and following entries shift. Mirrored at `~0x60800` |
| `~0x001E` | 80 × `u16 LE` (= 160 B) | **DMX-address table**, indexed by `dimmer_id`. Entry = DMX channel, `0` = unpatched |
| `~0x0040` | sorted `u16 LE[]` | **Sorted DMX-channel list** (ascending). Lists every patched DMX start address. AC→BA: insert `C3 00` (=195) at position 4. AAAAA-AA→AB: removed `BF..C8` (=191..200) when fixtures 111–120 deleted |
| `~0x00AC` | bytes | **Per-personality patched-fixture counter** (one byte per personality slot). AC→BA `+1` at offset `0xAC` when PARINER got first patch |
| `~0x0498` | sorted `u16 LE[]` | **Sorted fixture_id list** (ascending). AB→AC removed `15..1E` (=21..30 = fixtures 101–110); AAAAA-AA→AB removed `1F..28` (=31..40 = fixtures 111–120) |
| `~0x4028` | `u16 LE[]` | **Per-fixture record-pointer table.** AB→AC removed 10 pointers `0x0641, 0x0691, 0x06E1, … (stride 0x50)`. AC→BA reused `0x0641` for the new PARINER patch. Pointers point into a per-fixture record region with **80-byte stride** |
| `~0x0338` | sorted `u16[]` | **Global active-dimmer list** (sorted by `dimmer_id`) |
| `0x4076` | `u16 LE` | Pointer / offset (value `0x1271` once first dimmer is patched) |
| `0x60878` | 60 × `u16 LE` | **Fixture→DMX patch table** (canonical, AA-era). Indexed by fixture handle (1..60). Verified across AA: PARINER 1–4 = `1,7,13,19`; FRESNEL 5–8 = `30,36,42,48`; CENTCM250 9+ = `54,63,…`; CNTCM72 = `115,129,143`; DIMMER 101–120 = `181..200`. The earlier `0x1E` table appears to be a legacy mirror |
| `0x61000` | 0x800 × N | **Personality library** — compiled R20 fixture profiles, 2 KiB per slot. ASCII device name at slot start. **Deletion does NOT clear the slot's name** — bookkeeping lives in counters/permutation arrays elsewhere. See "Personality slot map" below |
| `file_size − 0x800` (e.g. `0x161000`) | 0x800 | **Trailer / "next save" header.** Magic `"KINGKONG1024SHOWDATA"` followed by next slot name + a partial copy of the patch table. Empty when no further save is queued |
| `0xC4000` | 0x1000 × N | **Scene region** — each recorded scene occupies one 4096-byte slot, appended in record order |
| `0xC4000 + N*0x1000` | rest | "Library / fixture-profile" region (recognisable by patterns `64 00 60 EA …`, `E0 00 FF 00 …`, `B8 00 11 …`). Pushed forward by 0x1000 each time a scene is recorded |

### Mini Pearl 512A spec (per user)

- **One DMX universe — 512 DMX channels** (the "512" in the name).
- **3 fixture pages × 20 fixtures = 60 fixture handles.** Display numbers are
  `1–20`, `101–120`, `201–220` (gaps between banks).
- **10 playback pages × 12 faders per page = 120 scene slots.** Pages and
  fader-within-page are 0-indexed in the file.

### Key derived rules

- **`fixture_id` (internal, 1..60) from display number:**
  - `1..20` → `1..20`
  - `101..120` → `21..40`
  - `201..220` → `41..60`
  - Equivalently: `bank = (fixture−1) ÷ 100`; `within_bank = (fixture−1) mod 100`;
    `fixture_id = bank × 20 + within_bank + 1`.
  - Verified: fixture **220** → `fixture_id = 60 = 0x3C`; fixture **219** → `59 = 0x3B`.
  - (Earlier "fixture − 160" formula coincidentally worked for the third bank
    only.)
- **DMX-address table base ≈ `0x1E`, stride 2 bytes (LE u16):**
  - `address_offset(fixture_id) = 0x1E + fixture_id × 2`
  - Verified: fixture_id 60 → `0x96`, 59 → `0x94`. Both held value `5` (the
    DMX address used in the test patches).
- **Scene-index table: base `0x70C00`, stride `0x400` (= 1024 bytes), indexed
  by `linear = page × 12 + scene_num` (both 0-indexed, scene_num runs 0..11):**
  - `index_offset(page, scene_num) = 0x70C00 + (page × 12 + scene_num) × 0x400`
  - Verified: 5/11 → `0x82800`, 5/10 → `0x82400`, 5/9 → `0x82000`, 5/8 → `0x81C00`.
  - Total table size = 120 × 0x400 = `0x1E000` bytes (spans `0x70C00..0x8EC00`).

---

## Personality slot map (verified G.KKD)

R20 fixture personalities are compiled into 2 KiB slots starting at `0x61000`.
ASCII device name (≤11 chars) sits at the slot's start. **File slot index is
the canonical `personality_id`** — the console's display order may differ from
file slot order (it appears to reflect load order).

| File slot | File offset | Device | Console display # |
|---|---|---|---|
| 0 | `0x61000` | `PARINER` | 1 |
| 1 | `0x61800` | `PAROUTER` | 3 |
| 2 | `0x62000` | `CNTCM72` | 2 |
| 3 | `0x62800` | `STD` | 4 |
| 4 | `0x63000` | `N-PAR18-6` | 5 |
| 5 | `0x63800` | `N-PAR18` | 6 |
| 6 | `0x64000` | `ADJUK186` | 7 |
| 7 | `0x64800` | `N-PIN` | 8 |
| 8 | `0x65000` | `FRESNEL` | 9 |
| 9 | `0x65800` | `CENTCM250` | 10 |

Open: which byte in `.KKD` stores per-fixture `personality_id` (i.e. fixture 220 → 0=PARINER). Likely a parallel array adjacent to the DMX-address table at `0x1E`.

---

## Scene record (4096 bytes per scene)

Each recorded scene occupies one **4096-byte (`0x1000`) record**, inserted into
the file at a scene-specific position. Recording N scenes grows the file by
`N × 0x1000` bytes; everything past the insertion point is shifted forward.

Confirmed in-scene offsets (offsets relative to `scene_base`):

| In-scene offset | Bytes / value | Meaning |
|---|---|---|
| `+0x000` | `00 00 60 EA 00 00 60 EA 00 00 60 EA 00` | Scene header — three `(u16 ?, u16 0xEA60)` pairs + 1 byte. `0xEA60 = 60000` is a constant (probably default fade/wait time, ~60s) |
| `+0x1B3` | `0x07` (when scene has ≥1 dimmer) | Flag — possibly "scene populated" |
| `+0x1EF` | `0x01` (when scene has ≥1 dimmer) | Flag/count — likely "active dimmer count" |
| **`+0x3EF`** | **1 byte = literal DMX level (0–255)** for the channel patched to fixture 220 (DMX channel **5**) | **Level** — verified across 3 scenes with 3 different stored levels (see below). Almost certainly part of a **512-byte level array indexed by DMX channel**, with `level_base ≈ +0x3EA` so `+0x3EA + 5 = +0x3EF` (Mini Pearl 512A is single-universe / 512 channels — storing the array DMX-channel-indexed lets the console ship it straight to the wire on playback) |

### Level field — verified

| File | Scene | Recorded level | scene_base | byte at scene_base + 0x3EF |
|---|---|---|---|---|
| D | 5/11 | 0 | `0xC4000` | `0x00` |
| E | 5/10 | 100 | `0xC5000` | `0x64` |
| F | 5/9  | 255 | `0xEB000` | `0xFF` |
| G | 5/8  | 50  | `0xEC000` | `0x32` |

**Leading hypothesis (DMX-channel-indexed):**
`level_byte = scene_base + 0x3EA + (dmx_channel)` (1-indexed channels, 512
total → 512-byte array spanning `+0x3EA..+0x5EA`). With our test fixture at
DMX 5 this gives `+0x3EA + 5 = +0x3EF` ✓.

**Alternative (fixture-id-indexed):** `level_byte = scene_base + 0x3B3 + fixture_id`.
Also fits the data we have so far because we only ever patched fixture_id 60.

To disambiguate in one save: patch a second fixture at a clearly different
DMX address (e.g. DMX 100), record a scene with both at known levels, and see
which model predicts the second level byte's position correctly.

> Stored values are raw DMX bytes (0–255). On the console "100% playback" is a
> master / grand-master scaling at playback time — it is independent of what
> level was stored.

### Scene record locator — solved

The scene records form a **flat array** at file offset **`0x93000`**, stride
**`0x1000`** (4096 bytes), indexed by an 8-bit **slot ID** that lives in the
scene-index entry at `+0x16`.

```
index_entry  = 0x70C00 + (page * 12 + scene_num) * 0x400
slot_id      = byte at (index_entry + 0x16)
scene_record = 0x93000 + slot_id * 0x1000
```

Verified against all four test saves:

| Scene | slot_id at index_entry+0x16 | Computed scene_record | Observed |
|---|---|---|---|
| 5/11 | `0x31` (49) | `0x93000 + 49 * 0x1000 = 0xC4000` | `0xC4000` ✓ |
| 5/10 | `0x32` (50) | `0xC5000` | `0xC5000` ✓ |
| 5/9  | `0x58` (88) | `0xEB000` | `0xEB000` ✓ |
| 5/8  | `0x59` (89) | `0xEC000` | `0xEC000` ✓ |

The reason scene 5/9 took slot 88 (not 51) when only two prior scenes existed
is still unclear — likely B's untouched layout reserves only certain slots as
"free for scene records" and the firmware allocates in a fixed order from
that free-list. More test recordings (e.g. patches across different pages)
would let us derive the slot-allocation order.

### Per-scene index table at `~0x82000` (stride `0x400`)

A separate 1024-byte-per-scene index table grows **backward** in the file as
new scenes are added:

| Save | Index entry offset | Byte at +0x16 |
|---|---|---|
| D (scene 5/11) | `0x82800` | `0x31` |
| E (scene 5/10) | `0x82400` | `0x32` |
| F (scene 5/9)  | `0x82000` | `0x58` |
| G (scene 5/8)  | `0x81C00` | `0x59` |

Each entry begins with `01 00 00 00 60 EA 00 00 60 EA 00 00 60 EA 00 00`
(matches scene-header magic). The semantics of the byte at `+0x16` aren't
clear yet — values 0x31, 0x32, 0x58 don't directly map to scene number, level,
or page.

### Earlier wrong hypotheses (retracted)

- The periodic `0x3C` insertions at `0x1270`, `0x5670`, `0x7E70`, etc. seen in
  A↔B were **not** per-cue blocks — they're part of fixed-position
  header/library structures already present in A.KKD.
- Scenes are **not** appended in record order — they go to scene-specific
  slots.

---

## Test saves

Saves done so far. Each one is a single controlled state on the console.

| File | Size (bytes) | State on console |
|---|---|---|
| `A.KKD` | 1,433,088 | Fresh / empty show, no dimmer patches, no recorded scenes |
| `B.KKD` | 1,433,088 | A + 1 dimmer: **fixture 220**, DMX address **5**, default level |
| `C.KKD` | 1,433,088 | A + 1 dimmer: **fixture 219**, DMX address **5**, default level (replaces B's patch) |
| `D.KKD` | 1,437,184 (+0x1000) | B's patch (fixture 220) + recorded scene **page 5 / no 11**, all 0 / black |
| `E.KKD` | 1,441,280 (+0x1000) | D + recorded scene **page 5 / no 10**, dimmer at level **100** |
| `F.KKD` | 1,445,376 (+0x1000) | E + recorded scene **page 5 / no 9**, dimmer at level **255** |
| `G.KKD` | 1,449,472 (+0x1000) | F + recorded scene **page 5 / no 8**, dimmer at level **50** |

---

## Diff log

### A vs B — patch one dimmer (fixture 220 / DMX 5)

- 68 differing runs, only **69 bytes total** changed.
- `0x14`: `'A' → 'B'` (slot letter).
- `0x96`: `00 → 05` — DMX-address table entry for `dimmer_id = 60`.
- `0x338`: `00 00 → 3C 00` — dimmer 60 inserted into global active list.
- `0x4076`: `00 00 → 71 12` — pointer = `0x1271`.
- Many single-byte `0x3C` writes at offsets `0xBE0`, `0xC30`, `0x1270`,
  `0x5670`, `0x7E70`, `0x9670`, `0xAE70`, …
  (these are the fixed-header structures, **not scene blocks**).

### B vs C — change patch from fixture 220 → 219 (DMX still 5)

- 134 differing runs, **205 bytes** total.
- `0x14`: `'B' → 'C'`.
- `0x94..0x96`: `00 00 05 → 05 00 00` — `5` moves from `0x96` (dimmer 60) to
  `0x94` (dimmer 59), as predicted by `address_offset(id) = 0x1E + id*2`.
- `0x338`: `3C 00 → 3B 00` — global active list entry shifts from
  `dimmer_id 60` to `59`.
- All the `0x3C`-at-fixed-offsets in A↔B are now `0x3B`, shifted ~80 bytes
  earlier within their containing structure.

### A vs C — empty show vs single dimmer fixture 219 / DMX 5

(Implied by composing A↔B and B↔C diffs — same shape as A↔B but with `0x3B`
instead of `0x3C` and DMX `5` written at `0x94` instead of `0x96`.)

### B vs D — record one all-zero scene at page 5 / no 11

- 587 differing runs, ~7388 bytes.
- File grows from 1,433,088 → 1,437,184 (`+0x1000`).
- `0x14`: `'B' → 'D'`.
- New scene record at `D[0xC4000..0xC5000]` — header bytes
  `00 00 60 EA 00 00 60 EA 00 00 60 EA 00`, all level positions zero.
- All bytes that were at `B[0xC4000..]` are shifted to `D[0xC5000..]`
  (verified e.g. `B[0xC43F1]` = `E0 00 FF 00 …` matches `D[0xC53F1]`).
- A scattered handful of single-byte changes outside the scene region (e.g. at
  `0x82800` and `0x8F031`) — likely "scene count" / "last-modified" / index
  fields. Not yet decoded.

### C vs D — useful only for cross-checking; mixes 2 changes

- D's state has the dimmer back at fixture 220, **and** a recorded scene.
- The 3B → 3C reversions confirm `dimmer_id = fixture# − 160` again.
- Everything else matches B↔D.

### D vs E — record a second scene at page 5 / no 10 with dimmer at level 100

- 589 differing runs, ~7785 bytes.
- File grows from 1,437,184 → 1,441,280 (`+0x1000`).
- `0x14`: `'D' → 'E'`.
- New scene record inserted at offset `0xC5000` in D's coords (immediately
  after the existing scene 5/11 at `0xC4000`).
- Inside that record, the only non-zero bytes are at `+0x1B3` (`0x07`),
  `+0x1EF` (`0x01`), and `+0x3EF` (`0x64`) — the latter is the **stored
  level value 100** for `dimmer_id = 60`.
- Scene-index entry added at `0x82400` with `+0x16 = 0x32`.
- All bytes that were at `D[0xC5000..]` are shifted to `E[0xC6000..]`.

### E vs F — record scene 5/9 with dimmer at level 255

- File grows by `+0x1000`.
- `0x14`: `'E' → 'F'`.
- **New scene record inserted at offset `0xEB000`** (in E's coords) — NOT
  adjacent to the previous scenes at `0xC4000`/`0xC5000`. This was the
  surprise that disproved the "scenes are appended sequentially" hypothesis.
- Inside that record, level byte `0xFF` (= 255) at `+0x3EF`. Other diffs at
  `+0x1B3 = 0x07` and `+0x1EF = 0x01` (same flag pattern as E).
- Scene-index entry added at `0x82000` with `+0x16 = 0x58`.
- E[0xEB000..] is shifted to F[0xEC000..] (`+0x1000`).

### F vs G — record scene 5/8 with dimmer at level 50

- File grows by `+0x1000`.
- `0x14`: `'F' → 'G'`.
- New scene record inserted at offset `0xEC000` (in F's coords) — adjacent to
  scene 5/9 (`+0x1000` later). Confirms scenes 5/9 and 5/8 cluster together.
- Inside that record, level byte `0x32` (= 50) at `+0x3EF`. Same flag pattern.
- Scene-index entry added at `0x81C00` with `+0x16 = 0x59`.
- F[0xEC000..] is shifted to G[0xED000..].

---

## Open questions / next experiments

| Goal | Test save |
|---|---|
| Pin down exact level-array stride/base | **F.KKD** = same as E but level = **50** (`0x32`) for the same dimmer. Diff E↔F should change exactly one byte (or one u16) inside scene 5/10's record |
| Find where chases live | **G.KKD** = build a 2-step chase using the patched dimmer; diff against E |
| Find non-dimmer fixture data | **H.KKD** = patch a non-dimmer fixture (e.g. a scanner) at a different DMX address |
| Find fixture label/name field | Save with a named fixture/scene; search for the ASCII string in the diff |

---

## Conventions used in this repo

- USB raw image: `dumps/dump.imp` (gitignored). Produced by `cargo run -- dump <drive> dumps/dump.imp` on Windows in an Administrator shell.
- Carved files: `dumps/{A,B,C,D,E}.KKD` (gitignored). Produced by
  `cargo run -- carve dumps/dump.imp <NAME>.KKD dumps/<NAME>.KKD`.
- Diffs: `cargo run -- diff dumps/X.KKD dumps/Y.KKD`.
