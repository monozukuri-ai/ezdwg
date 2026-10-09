# Changelog

## Unreleased

### Added
- Structured `Document.layers` (`Layer` / `LayerTable`) with callable legacy
  `doc.layers()` access, lineweight conversion, and EED transparency/description.
- Layer-state discovery, masked in-memory diff/apply, property/nested filter
  discovery, and viewport layer override metadata. These metadata changes are
  not written to DWG/DXF or used by plotting.
- Shared native layer decode cache with file metadata invalidation and
  `clear_decode_caches()` support.
- Native `AC1021` (`R2007`) read path in the high-level API (`ezdwg.read`) without compatibility conversion.
- Native `AC1024` (`R2010`) read path in the high-level API (`ezdwg.read`) for `LINE`, `ARC`, and `LWPOLYLINE`.
- Native `AC1027` (`R2013`) read path in the high-level API (`ezdwg.read`) for `LINE`, `ARC`, and `LWPOLYLINE`.
- AC1021 regression suite covering:
  - Rust object/entity decode checks for `LINE`, `ARC`, `LWPOLYLINE`.
  - Python high-level and raw API checks with paired sample files.
  - CLI `inspect` verification for native `decode_version: AC1021`.
- AC1024 regression suite covering high-level and raw geometry checks against paired DXF samples for:
  - `LINE`
  - `ARC`
  - `LWPOLYLINE`
- AC1027 regression suite covering high-level and raw geometry checks against paired DXF samples for:
  - `LINE`
  - `ARC`
  - `LWPOLYLINE`
- R2007+/R2010+/R2013+ regression coverage for:
  - `POINT`
  - `CIRCLE`
  - `ELLIPSE`
- TEXT/MTEXT regression coverage for `R2000`/`R2004` sample pairs.

### Changed
- Removed the external DWG compatibility-conversion path from `ezdwg.read`; AC10xx versions in scope now use native decode paths.
- R2007/R2010/R2013 entity decoding now uses version-aware common header paths for:
  - `LINE`
  - `ARC`
  - `LWPOLYLINE`
  - `POINT`
  - `CIRCLE`
  - `ELLIPSE`
  - `TEXT`
  - `MTEXT`
  - `DIMENSION` (linear/radius/diameter)
  to account for `material flags`, `shadow flags`, R2010 visual-style bits, and the R2013+ ds-binary-data flag.

### Fixed
- Object-map page offsets, body-handle identity, 64-bit handle references, and
  string-stream size metadata. Layer-state lineweights and viewport colors/
  transparency now use decoded units, including true-color black and ACI restore.
- `POLYLINE_2D` / `POLYLINE_3D` / `POLYLINE_PFACE` / `POLYLINE_MESH`: the "Owned Object
  Count" BL is only read for R2004+ layouts (ODA spec 20.4.16 / 20.4.17 / 20.4.33 / 20.4.34).
  R2000 files used to read the count from unrelated bits, which silently dropped most 2D
  polylines (6 of 334 decoded in one R2000 drawing) and, when the garbage value was large,
  aborted the whole process with `memory allocation of N bytes failed`.
- Handle-reference counts (owned objects, reactors) are validated against the remaining
  handle stream before `Vec::with_capacity`, so corrupted R2004+ objects fail with a
  `Format` error for that entity instead of aborting the process.
- R2007 (`AC1021`) data pages: the Reed-Solomon block count is now derived from the
  compressed size padded to the 8-byte CRC block (ODA 5.4). A bare
  `ceil(compressed / 251)` was one block short whenever that padding crossed a
  251-byte boundary, so the de-interleave stride was wrong and decompression
  failed with "back-reference offset exceeds decompressed prefix" (seen on the
  `AcDb:Handles` section of real files). A page-size based stride is tried as a
  fallback before giving up.

- HATCH on R2007+ (`AC1021`+): the pattern/gradient names live in the object's
  string stream and consume no bits in the data stream. The decoder used to read
  them inline, which shifted every following field; most R2007+ hatches only
  survived through the polyline-scan fallback and some produced empty
  boundaries. Candidate scoring now penalizes empty/degenerate paths and a
  zero extrusion, so a misaligned candidate can no longer outrank the real one.
- HATCH spline boundary edges (edge type 4, including the R2010+ fit-point
  block) are decoded and sampled instead of failing the entity.

- DIMENSION on R2007 (`AC1021`) now tries the string-stream layout first (no
  version byte, user text not in the data stream) — the same class of bug as
  the HATCH one; the R2000-style inline-text variants remain a fallback. Real
  R2007 drawings no longer yield dimensions with absurd/non-planar values.
- DIMENSION plausibility scoring now penalizes garbage magnitudes
  (`0 < |v| < 1e-30`, the signature of doubles read at the wrong bit offset)
  and the R2000/R2004 spec layout is tried first, so a mis-aligned candidate can
  no longer win a tie against the correct one (real AC1018 files had 31/108
  dimensions with unit-vector-like definition points and spurious Z values).
- DIAMETER/RADIUS dimensions on R2000-R2007 decode their own specific data
  (`15-pt / 10-pt / leader length`) instead of being delegated to the LINEAR
  layout.
- ANG2LN / ANG3PT / ALIGNED / ORDINATE dimensions decode their own type-specific
  tail (ODA 20.4.23-20.4.27: ANG2LN `2RD 16-pt, 3BD 13/14/15/10`, ANG3PT
  `3BD 10/13/14/15`, ALIGNED without the dimension rotation, ORDINATE
  `3BD 10/13/14, RC flags`) instead of the LINEAR tail. With the LINEAR tail
  every point after the 12-pt was read at the wrong bit offset (for ANG2LN the
  leading 16 raw bytes of the 16-pt shifted everything), the candidate was
  rejected as implausible and the entity surfaced as an all-zero placeholder row.
  Verified against the paired ACadSharp DXF samples for AC1015-AC1032.
- Dimension rows gained a 12th element `(point15, point16)` — DXF codes 15/16
  (angular vertex / second line start, and the 2-line angular arc point;
  RADIUS/DIAMETER expose their 15-pt there too). The high-level document maps
  them to `defpoint4` / `defpoint5`; the first 11 elements are unchanged.
- R13 (`AC1012`) files are accepted and decoded through the R13/R14 path.
- The R13/R14 common entity header is parsed with the ODA layout first
  (`RL bitsize, BB entmode, BL numreactors, B isbylayerlt, B nolinks, BS color,
  BD ltscale, BS invisibility` — no xdictionary flag, no ltype/plotstyle flag
  pair, no lineweight byte); the previous guessed layouts remain as fallbacks.
  Real R13 drawings that decoded to garbage coordinates now decode cleanly.

### Notes
- This release keeps API signatures stable (`ezdwg.read`, `ezdwg.raw`, entity decode functions).
- ARC angles remain radians in `ezdwg.raw` and degrees in the high-level API.
- AC1021/AC1024/AC1027 style-handle and layer-color resolution for LINE/ARC/LWPOLYLINE is currently best-effort on some files.
