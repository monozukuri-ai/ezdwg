# Changelog

## Unreleased

### Added
- Linetypes. `Document.linetypes()` returns the linetype table (name,
  description and dash pattern), `Document.layers()` the layer table with each
  layer's linetype, and every entity carries `linetype`, `linetype_handle` and
  `linetype_scale` in `Entity.dxf`. The raw functions are `decode_linetypes`,
  `decode_layer_linetypes` and `decode_entity_linetypes`. All versions from R13
  to R2018 are covered; checked against DXF exports of the same drawings
  (linetype patterns, layer linetypes and entity linetypes of every standard
  entity type agree).
- HATCH pattern definitions: `pattern_angle`, `pattern_scale`, `pattern_double`
  and `pattern_lines` (angle, base point, offset and dashes of each family of
  pattern lines) in `Entity.dxf` of pattern-filled hatches, and the
  `decode_hatch_patterns` raw function.
- Added `ezdwg plot` to display model-space drawings or save PNG, SVG, and PDF
  files, with entity filtering, image resolution, and title options. Requires
  the optional `plot` extra; file output works without a display. Falls back to
  an SVG preview in the browser when no interactive matplotlib backend is available.
- Added `ezdwg.clear_decode_caches()` so long-running batch converters can
  release per-file decoded tables and entity metadata after finishing a DWG.
- Header-variables decoding across `AC1015`-`AC1032`: `Document.header_variables()`
  ($INSUNITS, $LUNITS/$LUPREC, $AUNITS/$AUPREC, $LTSCALE, $TEXTSIZE, model-space
  $EXTMIN/$EXTMAX/$LIMMIN/$LIMMAX) plus the `Document.units` / `Document.insunits`
  conveniences and the `decode_header_variables` raw function. R14 has no
  `$INSUNITS` and yields `None` values. Validated against paired DXF headers for
  every supported version family.
- Bundled the MIT `LICENSE` file in wheels (`License-Expression` metadata via
  PEP 639 `license` / `license-files` in `pyproject.toml`).
- Completed high-level coordinate extraction and API documentation for `INSERT`/`MINSERT`, `HATCH`, and `SPLINE`.
- Added fixture-backed high-level `INSERT` regression coverage for block name and transform metadata.
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
- R2000 (`AC1015`) entities lost their layer: the common entity data was read
  with the R2004 meaning of one flag bit ("XDic Missing Flag", which R2000 does
  not have; the bit is "Nolinks" there), so the xdictionary handle was taken for
  the layer handle and about 95% of the entities reported layer handle 0. The
  layer, linetype and plotstyle handles of R2000 entities are now read from
  their real position, including entities that store previous/next links.
- Layer names were empty for R13, R14, R2000 and R2007 files, and layer colors
  and block names were unreadable for R2000: the table records of these versions
  are now read with their own layout (no "XDic Missing Flag" before R2004, the
  object size after the EED in R13/R14, names in the string stream in R2007).
- The layer handle of every entity now comes from the common entity data. The
  type-specific decoders reported a wrong layer for some `INSERT`, `POLYLINE`,
  `DIMENSION` and `POINT` entities (about 4% of the entities in drawings
  compared against their DXF export).
- `DIMENSION` entities lost their saved graphics in R2000, R2004 and R2007
  files: the dimension style and the anonymous block were read before the common
  entity handles instead of after them, so `anonymous_block_handle`,
  `anonymous_block_name` and the block-derived `char_height` never resolved.
  R2010+ files relied on a scan of the handle stream that missed some
  dimensions and could pick an arrowhead block for a dimension that has no
  block. Both handles are now read from their place in the handle stream (the
  block lines of all 485 dimensions compared against DXF exports of the same
  drawings agree); the scan only remains as a fallback. R13/R14 dimensions are
  still unresolved.
- Block names of R2010+ files: the name is now read from the string stream
  before the record data in front of it. That data was read with a layout these
  versions do not have, and when the read failed the name was left to a scan
  that only accepts ASCII names. Block headers named with Japanese characters
  only could come out unnamed (3 to 18 per drawing in the R2018 drawings
  compared against their DXF export), and two anonymous blocks could get the
  same number (`*D3` twice). Every block header of those drawings is named now,
  and anonymous blocks are numbered once, in handle order.
- Entities with a color book color (ENC flag `0x4000`) were misread: no RGB
  value follows the flags for them, and the color handle precedes the layer
  handle in the handle stream.
- R13/R14 entities with previous/next links: the links follow the layer and
  linetype handles in these versions.
- The R2000 writer stores the xdictionary handle that the format requires, so
  other readers find the layer handle where they expect it.
- Improve plot readability with smaller point markers, faint crosses for block
  references, thinner geometry and dimension strokes, light-background color
  contrast, and rectangular view bounds. The CLI hides coordinate axes and
  uses a larger canvas. Dimension ticks/overshoot use text height rather than
  measured length, and labels honor their attachment point.
- Plot text in drawing units instead of treating DWG text heights as matplotlib
  point sizes. Text now scales with geometry across zoom levels, figure sizes,
  and output DPI; automatic view bounds include the text outlines.
- Use saved anonymous-block text heights for dimensions where available, instead
  of sizing dimension labels as a percentage of their measured length. The
  high-level DIMENSION entity exposes resolved `char_height` and its source.
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
