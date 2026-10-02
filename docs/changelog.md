# Changelog

## Unreleased

### Added
- Layouts. `Document.layouts()` returns the layout table: the model tab and
  the paper-space sheets with their name, tab order, block record, paper size,
  margins, units and rotation. It marks the sheet that was current when the
  file was saved: the entities of that sheet are the ones stored without an
  owner handle (`entity_placement()` gives `(1, None)`), the entities of every
  other sheet name their block record. The raw function is
  `decode_layout_objects`. Every version is covered, R14 files that carry
  layout objects included; the layouts of the ACadSharp sample drawing read
  the same from all seven saves and match its DXF export. In 483 real-world
  drawings, 1,479 of 1,481 layout objects decode, and the current sheet is
  identified in 472 of the 476 drawings that have layouts.
- Paper-space viewports. A `VIEWPORT` entity carries its window on the sheet
  (`center`, `width`, `height`) and, from R2000 on, what it shows
  (`view_center`, `view_height`, `view_target`, `view_direction`,
  `view_twist_angle`), its `status_flags`, the layers that are frozen in it
  (`frozen_layers`, `frozen_layer_handles`) and its clip boundary. The raw
  function is `decode_viewport_details`. Until now a viewport had a handle, a
  layer and a color only. R13/R14 keep the view in the extended data of the
  entity, which is not read: their viewports have a window but no view. The
  viewports of the ACadSharp sample drawing match its DXF export in every
  version, and all 1,716 viewports of the real-world drawings decode.
- R13/R14 (`AC1012`, `AC1014`) decode `POLYLINE_3D`, `POLYLINE_MESH`,
  `POLYLINE_PFACE` and their vertices and faces. The 3D polyline and the
  polyface mesh of the R14 save of the ACadSharp sample drawing decode to the
  same values as in its R2004 save, and the 82 3D polylines of the real-world
  R13/R14 drawings have their vertices.
- Layer state and lineweights. `Document.layers()` reports `off`, `frozen`,
  `locked`, `plot` and `lineweight` for every layer, and every entity carries
  `lineweight` and `invisible` in `Entity.dxf`. The raw functions are
  `decode_layer_states` and `decode_entity_lineweights`. Lineweights use the
  values of DXF group 370 (hundredths of a millimetre, `-1` BYLAYER, `-2`
  BYBLOCK, `-3` default). Checked against DXF exports of the same drawings:
  the state and lineweight of 1,659 layers and the lineweight and visibility
  of 34,444 entities agree. R13/R14 files have neither lineweights nor a plot
  flag.
- R13/R14 (`AC1012`, `AC1014`) decode `INSERT`, `MINSERT`, `MTEXT`, `HATCH`,
  `SOLID`, `TRACE`, `3DFACE`, `SPLINE`, `ATTRIB`, `ATTDEF`, `LEADER`,
  `TOLERANCE`, `MLINE`, `SHAPE` and every `DIMENSION` type, with the layouts
  these versions have (plain doubles where R2000 introduced flagged and
  compressed forms, no fields that later versions added). They also report
  where each entity lives (`Document.entity_placement()`), the names of their
  blocks, and the dimension style and anonymous block of a dimension. Until
  now such a file yielded lines, arcs, circles, ellipses, points, lightweight
  polylines and single-line text only, all of them in model space, and
  dimensions without any of their points. The R14 save of the ACadSharp sample
  drawing decodes to the same values as its R2004 save for all of these types.
- Dimension style sizes: `Document.dimstyles()` (`dimscale`, `dimtxt`,
  `dimasz` by style name) and the raw function `decode_dimstyles`.
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
- A spline-fit 3D polyline is returned along its fitted vertices
  (`decode_polyline_3d_with_vertices`, `POLYLINE_3D` points). The control
  points of its frame, which the file stores in front of them, used to be part
  of the path.
- The members of a polyline in an R13-R2000 file (`vertex_handles`,
  `face_handles`, `seqend_handle`, and the `owner_handle` of its vertices) are
  resolved from the vertices that follow it. These versions list no owned
  objects, and the members stayed empty.
- `TEXT` and `MTEXT` of R2007 and later are read from where the format keeps
  them. These versions store every string of an object in its string stream,
  but the decoders looked for the text in the data stream and settled on the
  most plausible candidate. In drawings compared against an exact read, 373 of
  6,665 `TEXT` entities came out with another text or justification (two-digit
  numbers read as other characters, a middle-aligned text reported as baseline)
  and 286 of 2,572 `MTEXT` entities with another text. The search only remains
  as a fallback for records that do not have the specified layout.
- `ATTRIB` and `ATTDEF` of R2007 and later were almost never decoded: 0 of 57
  attributes in R2007 drawings, 0 of 159 in R2010, 7 of 2,986 in R2013 and 0 of
  1,510 in R2018. They are read with their specified layout now (text, tag and
  prompt from the string stream, the attribute fields from the end of the data
  stream), including the multi-line attributes of R2018, whose text comes
  without formatting codes. All but 3 of those attributes decode.
- `ATTDEF` of R2000 and R2004: the prompt was empty and a definition with an
  empty default value lost its tag and flags (so a constant definition was not
  recognizable). These versions have no lock position flag; reading one shifted
  the prompt by a bit.
- R2010+ entities with a saved graphic of 256 bytes or more: the size of the
  graphic (a BLL) was assembled with its bytes in the wrong order, so the
  common entity data was looked for at a shifted position. Such entities
  (multileaders, tables, entities of add-on applications) reported a wrong
  lineweight, visibility, linetype or layer.
- Layer names of R2010+ files: a layer whose color comes from a color book was
  named after the book. The name is the first string of the string stream.
- `TOLERANCE` was not decoded in any version: two fields that only R13/R14
  store were read from every file, and R2007+ keep the text in the string
  stream. Later versions take the text height from the dimension style, and
  so does `Entity.dxf["height"]` when the entity stores none (the raw rows
  keep the stored value, 0).
- Dimension text overrides of R2007 and later (`text`) were always empty; they
  are read from the string stream.
- `INSERT` and `MINSERT` of R2000: the count of owned attributes only exists
  from R2004 on. Reading it shifted the array counts of a `MINSERT` with
  attributes.
- R13/R14 `POINT` entities could come out at a wrong location (the layout was
  searched for instead of read), and R13/R14 `POLYLINE_2D` read its thickness
  and extrusion in the R2000 form, which shifted the elevation.
- Lineweight index 28 is BYLAYER, like 29. Some writers store it for every
  entity.
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
  drawings agree); the scan only remains as a fallback.
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
