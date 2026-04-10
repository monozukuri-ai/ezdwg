# Roadmap

ezdwg already covers a meaningful set of DWG versions, high-level entities, and raw decode helpers. The next step is not just adding more entity-specific decoders. The bigger goal is to turn ezdwg into a stronger DWG parser that can reconstruct the drawing database, resolve references consistently, and expose more of the DWG object model through stable Python APIs.

This roadmap focuses on that shift.

## Current Position

Today, ezdwg is strongest in these areas:

- version detection and object-header listing across supported DWG versions
- high-level read access for core geometry, text, dimensions, and several advanced entity types
- raw decode helpers for many entity families
- DWG to DXF conversion for practical extraction workflows
- a native AC1015 writer for a limited subset of entities

The main gaps are not basic file opening or simple entity iteration. The larger gaps are:

- object-table and dictionary decoding
- general handle-reference resolution
- paperspace and layout semantics beyond modelspace-first workflows
- style resolution beyond the current best-effort layer color path
- proxy, custom-class, and 3D object fidelity
- robust diagnostics and partial recovery for imperfect files

## Guiding Principles

- Prefer verifiable decode paths over heuristics.
- Add raw decode visibility before collapsing data into high-level abstractions.
- Treat handle references and ownership as shared infrastructure, not per-entity special cases.
- Keep unknown or partially decoded content inspectable instead of dropping it.
- Back every major capability with fixture-based regression coverage across DWG versions.

## Phase 1: Object Database Foundations

Build out the DWG object model so the parser can expose more than modelspace entities.

### Goals

- add raw decoders for `DICTIONARY`, `XRECORD`, `BLOCK_HEADER`, `LAYOUT`, `LAYER`, `LTYPE`, `DIMSTYLE`, and `APPID`
- expose stable Python APIs for object lookup and table access
- centralize owner and pointer resolution instead of repeating it in entity-specific code

### Deliverables

- `Document.by_handle(handle)` for direct object lookup
- `Document.objects()` for iterating decoded objects
- `Document.layouts()`, `Document.blocks()`, and `Document.layers()`
- shared handle graph helpers in the Rust core and Python layer

### Exit Criteria

- block table, layer table, and layout dictionary entries can be decoded and queried without using raw record bytes directly
- object relationships can be traversed from both entities and objects through a common API

## Phase 2: Layout, Block, and Reference Semantics

Make layouts and block references first-class parts of the high-level API.

### Goals

- support paperspace layouts as naturally as modelspace
- resolve nested `INSERT` and `MINSERT` trees consistently
- attach `ATTRIB` and `ATTDEF` data to block-reference workflows
- stabilize anonymous block and dynamic block name handling

### Deliverables

- layout-aware query APIs beyond `Document.modelspace()`
- block-definition iteration scoped by block record
- high-level block-reference helpers with transform metadata
- clearer XREF and unresolved-reference reporting

### Exit Criteria

- users can inspect modelspace, paperspace, and named block contents without dropping to raw APIs
- nested block references produce stable, testable semantics across supported versions

## Phase 3: Style and Annotation Fidelity

Improve semantic correctness for appearance and annotation data.

### Goals

- resolve linetype, lineweight, true color, transparency, and plot-style-related metadata
- expose text style and dimstyle resolution
- improve MTEXT, DIMENSION, HATCH, LEADER, TOLERANCE, and MLINE fidelity
- preserve annotative and orientation-related data where available

### Deliverables

- richer resolved style dictionaries on high-level entities
- explicit APIs for layer, text-style, and dimstyle lookup
- fewer best-effort cases in R2007+ style resolution

### Exit Criteria

- entity appearance is reconstructed from the correct table/object sources rather than inferred from partial fields
- annotation entities round-trip more faithfully through DXF export and inspection tools

## Phase 4: Proxy, Custom Class, and 3D Support

Strengthen coverage for data that does not fit simple 2D entity decoding.

### Goals

- read class metadata needed for custom objects and proxy content
- expose `ACAD_PROXY_ENTITY` and `ACAD_PROXY_OBJECT` in a structured way
- improve `BODY`, `REGION`, and `3DSOLID` handling through ACIS-related decode infrastructure
- use proxy graphics as an explicit fallback path when semantic decode is incomplete

### Deliverables

- class-table inspection APIs
- structured proxy-object diagnostics
- ACIS chain inspection helpers promoted from diagnostic-only usage toward reusable APIs

### Exit Criteria

- unsupported custom content remains visible and inspectable
- 3D and proxy-heavy files degrade gracefully instead of appearing silently incomplete

## Phase 5: Robustness and Diagnostics

Make the parser more reliable on imperfect real-world files.

### Goals

- distinguish strict and lenient decode modes
- improve `ezdwg inspect` so it can report unknown types, broken refs, truncated records, and partial-decoding outcomes
- preserve enough raw context for debugging unsupported objects

### Deliverables

- structured diagnostics for unknown handles, unknown type codes, and unresolved references
- partial-recovery behavior for files that contain damaged or unsupported objects
- clearer CLI output for object and entity coverage gaps

### Exit Criteria

- inspection output is useful for triaging parser bugs and sample-file gaps
- unsupported content can be reported precisely enough to guide the next implementation step

## Phase 6: Writer and Round-Trip Expansion

Expand write support only after read-side semantics are strong enough.

### Goals

- broaden native writer support beyond the current AC1015 subset
- preserve more metadata during DWG to DWG and DWG to DXF conversions
- avoid destructive loss of unknown objects where preservation is feasible

### Deliverables

- clearer internal IR boundaries between read-side decode and write-side serialization
- targeted writer coverage for the entity types whose high-level semantics are already stable

### Exit Criteria

- write support grows on top of reliable read semantics instead of baking in reader limitations

## Recommended Near-Term Priorities

If work is staged over the next few milestones, the best order is:

1. object database foundations
2. shared handle-reference resolution
3. layouts and block semantics
4. style and annotation fidelity
5. proxy and 3D support
6. writer expansion

This order keeps the project focused on the biggest missing capability: reading DWG as a structured database rather than only as a collection of decoded modelspace entities.
