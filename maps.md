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
| `0x009C` | 6 bytes | **Personality counter array (live).** Decrements on each personality deletion. G=`09 09 09 09 0A 0A` (10 personalities), AA=`08 08 08 08 09 09`, AAAAA-AA=`07 07 07 07 08 08`. **Cleared (zeroed) by wipe-all** (WP4). A second copy lives at `0x60840` in BKP/WP/AA-BC saves and survives wipe-all — but in A-G saves the bytes at `0x60840` are unrelated content (`61 29 0D 0A 2D 2D` ≈ `"a)..--"`), so the mirror is not a stable cross-era invariant. See "Two-copy patch/personality model" below |
| `~0x0110` | bytes | **Personality slot permutation/order array.** On deletion, deleted slot is zeroed and following entries shift. Mirrored at `~0x60800` |
| `~0x001E` | 80 × `u16 LE` (= 160 B) | **DMX-address table**, indexed by `dimmer_id`. Entry = DMX channel, `0` = unpatched |
| `~0x0040` | sorted `u16 LE[]` | **Sorted DMX-channel list** (ascending). Lists every patched DMX start address. AC→BA: insert `C3 00` (=195) at position 4. AAAAA-AA→AB: removed `BF..C8` (=191..200) when fixtures 111–120 deleted |
| `~0x00AC` | bytes | **Per-personality patched-fixture counter** (one byte per personality slot). AC→BA `+1` at offset `0xAC` when PARINER got first patch |
| `~0x0498` | sorted `u16 LE[]` | **Sorted fixture_id list** (ascending). AB→AC removed `15..1E` (=21..30 = fixtures 101–110); AAAAA-AA→AB removed `1F..28` (=31..40 = fixtures 111–120) |
| `~0x4028` | `u16 LE[]` | **Per-fixture record-pointer table.** AB→AC removed 10 pointers `0x0641, 0x0691, 0x06E1, … (stride 0x50)`. AC→BA reused `0x0641` for the new PARINER patch. Pointers point into a per-fixture record region with **80-byte stride** |
| `~0x0338` | sorted `u16[]` | **Global active-dimmer list** (sorted by `dimmer_id`) |
| `0x4076` | `u16 LE` | Pointer / offset (value `0x1271` once first dimmer is patched) |
| `0x60878` | 60 × `u16 LE` | **Fixture→DMX patch table — persistent / "applied-config" copy.** Indexed by fixture handle (1..60). Verified across AA: PARINER 1–4 = `1,7,13,19`; FRESNEL 5–8 = `30,36,42,48`; CENTCM250 9+ = `54,63,…`; CNTCM72 = `115,129,143`; DIMMER 101–120 = `181..200`. **Survives wipe-all** (WP4 keeps it byte-identical). **Empty (all-zero) in the A-G era** — only populated from AA onward. The matching live-patch data lives at `0x1E + fixture_id*2` (the pre-AA "DMX-address table") and IS cleared by wipe-all. So `0x1E` and `0x60878` are **two copies with different lifetimes**, not a "canonical vs legacy mirror" relationship — see "Two-copy patch/personality model" below |
| `0x61000` | 0x800 × N | **Personality library** — compiled R20 fixture profiles, 2 KiB per slot. ASCII device name at slot start. **Deletion does NOT clear the slot's name** — bookkeeping lives in counters/permutation arrays elsewhere. See "Personality slot map" below |
| `file_size − 0xE00` (= `record_region_end`) | 0x600 | **Trailer block 1 — staged "next save" header.** When a next save name is queued in the firmware, this block contains: magic `"KINGKONG1024SHOWDATA"` + the **next** save's slot name at `+0x0E..+0x1E` (NOT the current file's name) + a partial copy of the patch state at the time the current save was committed (DMX-address table head at `+0x20`, `u16` lists at `+0x40`/`+0x60`, per-personality patched-fixture counters at `+0x98`, fixture/personality bitmap arrays at `+0x16C..+0x1D0`, group definitions at `+0x330..+0x46F`, fixture-handle list at `+0x4B4`). **All-zero when no next save is queued.** Verified end-to-end via DM3→DM4: DM3's block 1 advertises `"DM4"` because DM4 was queued next; DM4's block 1 is fully zero because no DM5 was queued at that moment. The earlier WP2→WP3 changes in this region were the same mechanism: WP2's block 1 staged `"WP3"` + WP2's patch state, WP3's staged `"WP4"` + WP3's (post-fixtures-wipe) patch state |
| `file_size − 0x800` (e.g. `0x161000`) | 0x800 | **Trailer block 2 — reserved.** All-zero in every file we have (A-G, AA-BC, BKP, WP1-5, DM1-4). Purpose unknown; possibly a second buffer for a future feature, or scratch space the firmware reserves but doesn't write |
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

### Playback-fader index entry — generalised (scenes & chases) — VERIFIED

The 1024-byte index entry at `0x70C00 + (page*12 + slot)*0x400` is a
**playback-fader entry** that supports both single-step (scene) and
multi-step (chase) recordings. Layout verified across D/E/F/G/BC:

```
+0x00  u16 LE          part_count                 (1 = scene, ≥2 = chase)
+0x02  u16 + u16 LE    timing 1: <val>, 0xEA60    (0xEA60 = 60000 const)
+0x06  u16 + u16 LE    timing 2: <val>, 0xEA60
+0x0A  u16 + u16 LE    timing 3: <val>, 0xEA60
+0x0E..0x15            bookkeeping (zeros in all observed)
+0x16  u16 LE          part_count (duplicate / mirror of +0x00)
+0x18  u16 LE × N      slot_id array (one per chase step / scene)
```

Each `slot_id` references a 4 KiB record at `0x93000 + slot_id * 0x1000` —
the same flat record region used for scenes.

The `+0x02 / +0x06 / +0x0A` first words are zero for D/E/F/G (default scene
timings) and `0x0064` (= 100) for the BC chase — likely fade/wait/etc.

### Chase example — BC.KKD (page 5 / no 7, 2 parts)

Index entry at `0x81800`:
```
0x00: 02 00              part_count = 2
0x02: 64 00 60 EA        timing 1
0x06: 64 00 60 EA        timing 2
0x0A: 64 00 60 EA        timing 3
0x18: 5A 00              part 1 slot_id = 90 → record at 0xED000
0x1A: 5B 00              part 2 slot_id = 91 → record at 0xEE000
```
Recorded levels: part 1 has `0xFF` (255), part 2 has `0x9C` (156) at the
same level-array position — consistent with each part being a full level
frame.

### Scene/chase record locator (final)

```
index_entry  = 0x70C00 + (page * 12 + slot_num) * 0x400
part_count   = u16 LE at (index_entry + 0x00)        // also mirrored at +0x16
slot_ids[i]  = u16 LE at (index_entry + 0x18 + i*2)  // i = 0..part_count-1
record[i]    = 0x93000 + slot_ids[i] * 0x1000
```

Verified raw bytes:

| Save | Index entry | Bytes 0x00..0x1B (key fields highlighted) |
|------|-------------|-------------------------------------------|
| D 5/11 | `0x82800` | `01 00` 00 00 60 EA 00 00 60 EA 00 00 60 EA 00 00 00 00 00 00 00 00 `01 00` `31 00` |
| E 5/10 | `0x82400` | `01 00` … `01 00` `32 00` |
| F 5/9  | `0x82000` | `01 00` … `01 00` `58 00` |
| G 5/8  | `0x81C00` | `01 00` … `01 00` `59 00` |
| BC 5/7 | `0x81800` | `02 00` `64 00` 60 EA `64 00` 60 EA `64 00` 60 EA 01 00 00 00 00 00 00 00 `02 00` `5A 00` `5B 00` |

Slot-id resolution (`+0x18` u16 LE) → record offset (`0x93000 + slot_id*0x1000`):

| Save | slot_id | record offset |
|---|---|---|
| D 5/11 | 49 | `0xC4000` ✓ |
| E 5/10 | 50 | `0xC5000` ✓ |
| F 5/9  | 88 | `0xEB000` ✓ |
| G 5/8  | 89 | `0xEC000` ✓ |
| BC 5/7 part 1 | 90 | `0xED000` ✓ |
| BC 5/7 part 2 | 91 | `0xEE000` ✓ |

The reason F (scene 5/9) jumped to slot 88 instead of 51 is still unclear —
likely B's pre-existing layout reserves only certain slots as free for
record allocation, and the firmware allocates in a fixed order from that
free-list.

### Wipe-test diff series (BKP → WP1 → WP2 → WP3 → WP4 → WP5)

Carved from `dumps/dump.imp` (second USB dump, wipe-test session). Each save was taken immediately after a single wipe operation on the console:

| Pair | Wipe action | Δ size | Runs | Bytes differ | Where the diff lives |
|---|---|---|---|---|---|
| BKP→WP1 | wipe playbacks | −0xD0000 | 233 | 4304 (in common prefix) + 852 KB tail dropped | playback-fader index entries (`0x70C00..0x8EC00`) clear their slot-id arrays; whole slot-record region (`0x93000..file_end`) gone |
| WP1→WP2 | wipe palettes | 0 | 2 | **2** (1 = show-name char) | BKP had no palettes — wipe was a no-op. Palette storage location remains undetermined; need a fresh capture with palettes deliberately populated |
| WP2→WP3 | wipe fixtures + groups | 0 | 11 | 364 | **All in trailer block 1 at `0x93000..0x934BF`** — the head-of-file live tables are NOT touched by this stage. Trailer block 1 holds: copy of magic+name, copy of DMX-address table at +0x20 (`01 00 07 00 0D 00 13 00 …`), `u16` lists at +0x40 / +0x60, per-personality patched-fixture counters at +0x98 (`01 01 01 01 07 07 07 07 08 08 04 00 …`), fixture/personality bitmap arrays at +0x16C..+0x1D0 (`01 00 00 00 01 00 …`), group definitions at +0x330..+0x46F (`3E 00 3F 00 40 00 01 00 00 00 3D 00 42 00 …` — 4 fixture handles + group_id pattern), and a fixture-handle list at +0x4B4 (`9D 9E 9F 15 00 9C`) |
| WP3→WP4 | wipe all | 0 | 520 | 5683 | Clears head-of-file live tables: `0x1E` (DMX-address table → all zero), `0x4028` (record-pointer table → all zero except first entry `41 06`), `0x498` (sorted fixture-id list → all zero), `0x9C` (personality counter → `00 00 00 00 00 00`). **Preserves**: `0x60840` mirror, `0x60878` patch-table copy, personality library (`0x61000`+) — confirmed on the console UI |
| WP4→WP5 | wipe personality data | 0 | 230 | 6700 | Clears **exactly** the three fields wipe-all spared: `0x60840` mirror → zero, `0x60878` patch table → zero, personality library (`0x61000`+) → zero. The personality library is its own console-managed datum, separate from the "show" — and "wipe personality data" is the dedicated console command that erases it (the bigger destructive action; not part of "wipe all") |

### Patch + palette test series (WP5 → DM1 → DM2 → DM3 → DM4)

Drives the post-wipe state by patching, repatching, and saving palettes. From the same dump as WP5.

| Pair | Console action | Runs | Bytes | What it touches |
|---|---|---|---|---|
| WP5→DM1 | patch all 60 fixtures with built-in Dimmer, DMX 1..60 | 2909 | 3301 | `0x1E + fixture_id*2` populated for fid 1..60 (`01 00 02 00 03 00 …`); `0x4028` template populated; many small bitmap-style updates scattered file-wide. Personality library, `0x60840`, `0x60878` UNCHANGED — built-in personalities don't go to the library |
| DM1→DM2 | unpatch 58, keep f1@DMX 1 + f2@DMX 512 | 2906 | 3253 | `0x1E + 0x02` reverts `01 00 02 00 03 00 …` → `01 00 00 02 00 00 …`; `0x4028` reverts to all-zero. The 60-fixture state and the 2-fixture state both touch ~2900 widely-scattered bytes — strongly suggests the patch state is encoded as **bitmap arrays / per-fixture flag bytes** in many places, not just in the canonical tables |
| DM2→DM3 | save palette to fixture 1 | 3 | 3 | name byte (×2 — head + trailer) + **`0x6C00`: `00 → 01`** (single byte flag, palette-saved for fixture 1) |
| DM3→DM4 | save palette to fixture 220 (`fid 60`) + failed group attempt | 5 | 40 | name byte + **`0x5F400`: `00 → 01`** (palette flag for `fid 60`) + trailer block 1 cleared (no next save queued). **No bytes from the failed group attempt** — failure means nothing written |

### Built-in vs library personalities (verified)

Patching with the **built-in Dimmer** personality leaves the personality-library bundle empty:
- `0x9C` personality counter stays `00 00 00 00 00 00`
- `0x60840` mirror stays zero
- `0x60878` persistent patch table stays zero
- `0x61000+` personality library stays zero

Yet the live `0x1E + fixture_id*2` DMX-address table IS populated correctly. So:
- **The "Dimmer" personality is firmware-resident, not stored in the file.**
- **`0x60878` is only populated for fixtures patched with R20-imported personalities** (PARINER, FRESNEL, CENTCM250, etc. — the AA-era state). For built-in personalities the "persistent" copy doesn't exist because there's no library entry to anchor it.
- **For round-trip writeback:** `0x60878` is written *only when* the fixture's personality has a library slot. Built-in dimmers never produce a `0x60878` entry.

### Palette flag — observed but mapping not pinned

Two data points so far:

| Fixture (display) | `fixture_id` | Palette flag offset | Stride from previous |
|---|---|---|---|
| 1 | 1 | `0x6C00` | — |
| 220 | 60 | `0x5F400` | `−0xD000` over 59 ids = non-integer per-id stride |

So palette flags are NOT a simple `base + fixture_id*stride` array. Likely the byte sits at a fixture-specific offset inside a per-fixture record (the `0x4400..0x60100` striped region), at a relative position that depends on fixture personality or order. **Need a third palette save (e.g. on `fixture_id 30`) before claiming a formula.** Also unclear: whether the actual palette levels are stored elsewhere (and we're only seeing the "palette saved" flag), or whether the palette is only meaningful when attached to a playback (in which case full palette data would live in the slot-record region — currently truncated in WP5-derived saves).

### Failed group save — verified no-op

DM4 attempted to save a fixture group but the console reported failure. Diff DM3→DM4 contains zero bytes attributable to a group structure. Confirms **failed console operations write nothing to the file** — no half-applied state to clean up.

### Two-copy patch/personality model — verified

The wipe-test data shows the file maintains **two copies** of patch/personality bookkeeping with **different lifetimes**:

| Datum | Live copy | Persistent copy (personality-library group) | Cleared by wipe-all (WP4)? | Cleared by wipe personality data (WP5)? |
|---|---|---|---|---|
| Personality counter | `0x009C` (6 B) | `0x60840` (6 B) | live ✓ / persistent ✗ | live n/a / persistent ✓ |
| Fixture→DMX patch table | `0x001E` (`+fixture_id*2`) | `0x60878` (60 × `u16`) | live ✓ / persistent ✗ | live n/a / persistent ✓ |
| Personality library | n/a (no live copy) | `0x61000` (10 × `0x800`) | ✗ (preserved — UI-confirmed) | ✓ (zeroed) |

The persistent copies form a single **personality-library bundle**: `0x60840` + `0x60878` + `0x61000+` are written, mirrored, and erased together. They survive "wipe all" (which only wipes show data) and are erased by "wipe personality data" — verified end-to-end via WP4→WP5.

**Implication for write-back:** A correct round-trip must update **both** the live copies AND the persistent copies in lockstep when the personality library or its fixture mappings change. The persistent bundle acts as a "loaded personality library + how it's wired" snapshot the firmware re-applies on boot; the live copies reflect runtime show state that wipe-all resets.

**Cross-era caveat:** In **A-G**-era saves the offset `0x60840` does NOT contain a personality-counter mirror — it contains unrelated bytes (`61 29 0D 0A 2D 2D` ≈ `"a)..--"`). Either (a) some structure ahead of `0x60840` is differently sized between eras, shifting the mirror's location, or (b) the persistent copy at `0x60840` is only emitted by firmware/console states that have a real "applied config" to snapshot. Don't trust `0x60840` as a fixed mirror until tested cross-era.

Similarly `0x60878` is **all-zero in the A-G era** but populated from AA onward. The presence of the persistent copy correlates with the AA-era state where multiple fixture types are properly patched.

### Earlier wrong hypotheses (retracted)

- The periodic `0x3C` insertions at `0x1270`, `0x5670`, `0x7E70`, etc. seen in
  A↔B were **not** per-cue blocks — they're part of fixed-position
  header/library structures already present in A.KKD.
- Scenes are **not** appended in record order — they go to scene-specific
  slots.
- `0x60878` is **not** the canonical patch table while `0x1E` is a "legacy
  mirror." The wipe-test data shows the opposite lifetime: `0x1E` is the
  **live** copy (cleared by wipe-all), `0x60878` is the **persistent /
  applied-config** copy (survives wipe-all). Both are real, just played
  different roles.
- The trailer is **not** a single `0x800` block at `file_size − 0x800`. It is
  a **two-block** structure totalling `0xE00` bytes at the file's tail:
  `0x600` staged-next-save header (block 1, contains the **next** save's name
  + a snapshot of current patch state — verified via DM3→DM4) + `0x800`
  reserved zero region (block 2, always all-zero in our captures).
- Trailer block 1 is **not** a "patch/group/fixture mirror" of the current
  file. It's a **forward-pointer**: it advertises the *next* save's slot name.
  Earlier saves looked like a mirror because the snapshot data inside it
  happens to mirror live patch state at save time — but the magic+name field
  is the next save's name, not the current one's.

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
| `BKP.KKD` | 1,457,664 | Backup of a real, in-use show (10 personalities, multi-fixture patch, multiple recorded playbacks). Carved from the second dump (`dumps/dump.imp`, the wipe-test session) |
| `WP1.KKD` | 605,696 | BKP after **wipe playbacks** on the console. File shrinks by `0xD0000` — the entire slot-record region (`0x93000..file_end`) is dropped, leaving only `0x93000 + 0x600` head + `0x800` queued-save trailer |
| `WP2.KKD` | 605,696 | WP1 after **wipe palettes**. Diff vs WP1 = **2 bytes** (1 = show-name char, 1 = a flag/counter). Indicates BKP had no palettes populated — wipe was effectively a no-op |
| `WP3.KKD` | 605,696 | WP2 after **wipe fixtures and groups**. Diff vs WP2 = 364 B / 11 runs, **all clustered in `0x93000..0x934BF`** (trailer block 1). The head-of-file live tables at `0x1E` / `0x4028` / `0x498` / `0x60878` are NOT touched by this stage — fixtures+groups erases the trailer mirror only |
| `WP4.KKD` | 605,696 | WP3 after **wipe all**. Diff vs WP3 = 5683 B / 520 runs. Clears the head-of-file live tables: `0x1E` DMX-address table, `0x4028` per-fixture record-pointer table, `0x498` sorted fixture-id list, `0x9C` personality counter. **Preserves**: personality library at `0x61000` (`PARINER` etc.), the `0x60878` patch-table copy, and the `0x60840` personality counter mirror. **Console-UI confirmed: personality library is retained across "wipe all"** |
| `WP5.KKD` | 605,696 | WP4 after **wipe personality data** (separate console command from "wipe all"). Diff vs WP4 = 6700 B / 230 runs. Clears **exactly** the three fields wipe-all spared: `0x60840` mirror → zero, `0x60878` persistent patch table → zero, `0x61000+` personality library → zero (`PARINER` name gone). Confirms the two-copy model end-to-end: the persistent copies belong to the personality library, not the show |
| `DM1.KKD` | 605,696 | WP5 + **patch all 60 fixtures with built-in Dimmer, DMX 1..60 contiguous**. `0x1E` table populated `01 00 02 00 03 00 …` per `0x1E + fixture_id*2`. `0x4028` record-pointer table populated with full template (`41 06 91 06 …` stride `0x50`). Personality library at `0x61000` stays **empty** — the built-in "Dimmer" personality is in firmware, not in the file. By extension `0x60840` and `0x60878` (the personality-library bundle) also stay empty |
| `DM2.KKD` | 605,696 | DM1 → keep only fixture 1 (DMX 1) and fixture 2 (DMX **512**); unpatched the other 58. `0x1E`: fixture_id 1 → `01 00`, fixture_id 2 → `00 02` (= `0x0200` = 512). Confirms DMX address is stored as `u16 LE` and supports the full 1..512 range. **`0x4028` record-pointer table reverts to all-zero** when patch density falls below the 60-fixture full-load — meaning that table is only populated when the full set of records is allocated, not per-patch |
| `DM3.KKD` | 605,696 | DM2 + **save palette to fixture 1**. Diff vs DM2 = **3 bytes / 3 runs**: name byte + trailer name byte + **`0x6C00`: `00 → 01`** (palette presence flag for fixture 1). The actual palette levels are NOT visible in this diff — possibly stored in the slot-record region which has been wiped, or palette save without an associated playback only sets the flag |
| `DM4.KKD` | 605,696 | DM3 + **save palette to fixture 220** (= `fixture_id 60`); also attempted to save a fixture group, **operation failed on the console**. Diff vs DM3 = 40 B / 5 runs: name byte + **`0x5F400`: `00 → 01`** (palette flag for `fixture_id` 60) + clearing of trailer block 1 (no next save queued at DM4 time). **No bytes attributable to the group save** — confirms "operation failed → nothing written" |

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

- USB raw image: `dumps/dump.imp` (gitignored). Produced by `cargo run -- dump <drive> dumps/dump.imp` on Windows in an Administrator shell. Two dumps so far: the original (carving A-G, AA, AAAAA-AA, AB, AC, BA, BC) and the wipe-test session (carving BKP, WP1-WP4 — see "Wipe-test diff series").
- Carved files: `dumps/*.KKD` (gitignored). Produced by
  `cargo run -- carve dumps/dump.imp <NAME>.KKD dumps/<NAME>.KKD`.
- Diffs: `cargo run -- diff dumps/X.KKD dumps/Y.KKD`.
- Cross-file field inspection: `cargo run -- inspect dumps/A.KKD dumps/B.KKD …` — prints decoded values at all known offsets side-by-side. Use this whenever you'd otherwise reach for an ad-hoc script; extend `src/inspect.rs` to add new field probes.

---

## Coverage — every byte of A.KKD (baseline, 0x15DE00 = 1,433,088 bytes)

This walks the whole file end-to-end. Each row is a contiguous region with
its decode status. **Decoded** = field/structure understood. **Partial** =
shape known (offset, stride) but per-byte semantics unclear. **Unknown** =
populated bytes whose purpose is undetermined. **Zero** = always 0 in every
saved file we have, no decoded purpose.

| File range | Size | Status | What it is / what's unknown |
|---|---:|---|---|
| `0x000000..0x000014` | 20 B | **Decoded** | ASCII `"SHOWDATA"` header + 12 padding |
| `0x000014..0x00001E` | 10 B | **Decoded** | Slot name (rotates A→…→Z→AA→…) |
| `0x00001E..0x00003E` | 32 B | **Partial** | Start of fixture-related u16 LE table; positions roughly correspond to dimmer-bank 1 entries. Earlier "DMX-address table @ 0x1E indexed by fixture_id" verified for fixture_id 60 only — full extent and layout still unconfirmed |
| `0x00003E..0x00009C` | 94 B | **Unknown** | Sorted DMX-channel list lives somewhere in here (verified entries `BB..C8` at `~0x54..0x77` representing DMX 187–200). Exact base + stride + length not pinned |
| `0x00009C..0x0000A2` | 6 B | **Decoded** | Personality counter array (G=`09 09 09 09 0A 0A`, decrements per personality deletion) |
| `0x0000A2..0x0000AC` | 10 B | **Unknown** | Likely additional patch counters/flags |
| `0x0000AC..0x0000B6` | ~10 B | **Partial** | Per-personality patched-fixture counter (one byte per personality slot). Confirmed `+1` at `0xAC` when first PARINER patched. Width = 10 bytes assumed (one per personality) but not byte-walked |
| `0x0000B6..0x000110` | ~90 B | **Unknown** | Mostly populated. Headers/flags. Untouched by patch/scene operations in our diffs |
| `0x000110..0x000130` | ~32 B | **Partial** | Personality slot permutation/order array. Mirrored at `~0x60800`. Layout in flux |
| `0x000130..0x000338` | ~520 B | **Unknown** | Big undecoded block. Some bytes change with global state, no individual fields decoded |
| `0x000338..0x000400` | ~200 B | **Partial** | Global active-dimmer list (sorted u16 LE). Verified inserts/removes for fixture_id 60. Exact base + length not pinned |
| `0x000400..0x000498` | ~150 B | **Unknown** | |
| `0x000498..0x000500` | ~100 B | **Decoded** | Sorted fixture_id list (u16 LE). Verified across patch/delete cycles |
| `0x000500..0x004028` | ~14.5 KB | **Unknown** | Largely zero in A baseline. Some bytes populate when fixtures patched. No structure decoded |
| `0x004028..0x004078` | 80 B | **Decoded** | Per-fixture record-pointer table (u16 LE, indexed by fixture handle, stride 2). Pointers like `0x0641, 0x0691, …` step by `0x50` into a per-fixture record region |
| `0x004078..0x004400` | ~900 B | **Unknown** | |
| `0x004400..0x060100` | ~368 KB | **Partial — large striped pattern** | Periodic structure with **stride `0x1800` (6144 bytes)** — populated `+0x000..+0x900` (2304 bytes), zero `+0x900..+0x1800` (3840 bytes). ~60–61 blocks total. Almost certainly the **per-fixture record region** that `0x4028` pointers index into (60 fixtures × 0x1800 ≈ 90 KB; rest may be padding or extra blocks). **Personality_id, channel inversions, fade times, palette per fixture probably live here.** This is the single highest-value undecoded region |
| `0x060100..0x060800` | ~1.8 KB | **Unknown** | |
| `0x060800..0x060846` | ~70 B | **Decoded** | Mirror of `~0x110` slot permutation + `0x9C` counters |
| `0x060846..0x060878` | ~50 B | **Unknown** | |
| `0x060878..0x0608F0` | 120 B | **Decoded** | **Canonical fixture→DMX patch table** (60 × u16 LE, indexed by fixture handle). Verified end-to-end |
| `0x0608F0..0x060D00` | ~1 KB | **Partial** | Continuation of patch metadata: secondary lists at `0x60896` (e.g. `73 81 8F`), `0x608EE` (timing-like values), `0x60978`–`0x609C2` (per-fixture flag arrays), `0x60BB4`–`0x60C30` (`FF`-marker arrays), `0x60C30..0x60C2F` (free-list indicators), `0x60CB0`–`0x60D6E` (4-channel groupings per patched fixture). Each piece is **observed but not formally decoded** |
| `0x060D00..0x061000` | ~768 B | **Unknown** | |
| `0x061000..0x066000` | 20 KB | **Decoded** | **Personality library** — 10 × `0x800` (2 KiB) slots. ASCII device name at slot start; rest is the compiled R20 binary (parser not written; we read R20 source directly). Deletion does **not** clear the slot |
| `0x066000..0x070C00` | ~43 KB | **Unknown — large** | Always populated in baseline A.KKD with similar template-like content. Could be additional personality slots (firmware-shipped library), fixture-attribute defaults, palette tables, or boot/menu state. **Untouched by any patch/scene/chase save we have**, so single-variable diffs can't reach it |
| `0x070C00..0x08EC00` | 120 KB | **Decoded** | **Playback-fader index table.** 120 × `0x400` entries indexed by `page*12 + fader`. Per-entry layout decoded for `+0x00 part_count`, 3 timing pairs, slot-id array at `+0x18`. Bytes `+0x1B..+0x3FF` of each entry are **mostly zero/template, not decoded** — could hold per-fader name, attribute filter, follow links |
| `0x08EC00..0x093000` | ~17 KB | **Unknown — large** | Between index table end and record region start. Populated even at baseline. Possibly: palette pages, group definitions, fixture-aliases, menu state. **Untouched by saves**, dead-zone for diff-driven discovery |
| `0x093000..0x15C500` | ~870 KB | **Partial** | **Slot record region.** 128 slots × `0x1000` bytes. Each slot is **pre-populated with template data even when "unused"** (~1280 non-zero bytes at the start of every slot in A baseline). Decoded inside a recorded scene/chase: header at `+0x000`, flags at `+0x1B3` and `+0x1EF`, level byte at `+0x3EF` (DMX-channel-indexed level array hypothesis: `+0x3EA + dmx_channel`). **Everything else inside a 4 KiB record (≈ 4080 of 4096 bytes) is undecoded.** Includes: scene name, fade in/out, snap channels, HTP/LTP overrides, attribute filter, scene chase parameters per part, link to next, etc. |
| `file_end − 0xE00` ..  `file_end − 0x800` | 0x600 | **Partial — trailer block 1, staged next-save header** | When a next save is queued: magic `KINGKONG1024SHOWDATA<NEXT_save_name>` at `+0x00..+0x1E` followed by a snapshot of the current patch state — DMX-address table head (`+0x20`), `u16` lists (`+0x40`, `+0x60`), per-personality patched-fixture counters (`+0x98`), fixture/personality bitmap arrays (`+0x16C..+0x1D0`), group definitions (`+0x330..+0x46F`), fixture-handle list (`+0x4B4`). All-zero when no next save queued. Verified DM3 (next=DM4) → DM4 (no next) |
| `file_end − 0x800` .. `file_end` | 0x800 | **Reserved** | All-zero in every save captured (A-G, AA-BC, BKP, WP1-5, DM1-4). Purpose undetermined — possibly a second buffer slot |

### Roll-up

| Status | Approx. coverage |
|---|---:|
| **Decoded** (field-level understanding) | ~150 KB |
| **Partial** (shape/region known, contents unclear) | ~1 MB (mostly the slot records + striped per-fixture region + index entry interiors) |
| **Unknown** (populated bytes, no current hypothesis) | ~70 KB (the 0x66000..0x70C00 and 0x8EC00..0x93000 dead zones plus various small gaps) |
| **Always-zero in A baseline** | ~285 KB |

### Highest-value targets next

1. **`0x004400..0x060100` per-fixture record region (stride `0x1800`).** Deref a pointer from `0x4028` and walk one fixture's record. This is where `personality_id`, channel inversions, fade times, palette etc. almost certainly live. Diff a save where one fixture's personality is changed in place to localise `personality_id`.
2. **Inside the slot record (`0x1000` bytes) past the level array.** A scene with non-default fade-in / fade-out / snap settings should expose those fields. A scene with a name should expose the name field.
3. **`0x066000..0x070C00`** and **`0x08EC00..0x093000`** dead zones. These need diffs against operations we haven't tried (palette save, group create, console preferences). Hard to attack with the test saves we have.
4. **R20 → 2 KiB compiled personality slot.** Comparing the original R20 text with the bytes at `0x61000` could decode personality compilation. Useful only when round-trip writing is needed.
