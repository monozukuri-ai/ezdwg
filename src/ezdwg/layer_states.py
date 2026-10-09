"""
High-level layer-state API for ezdwg (Phase 2).

Builds on raw.decode_layer_state_names / decode_layer_state_xrecords.

XRECORD group schema (explicit constants — not inferred from one file)
----------------------------------------------------------------------
State-level (once, before first layer block):
  GROUP_MASK (91)          LayerStateMasks bitfield (prefer over 90)
  GROUP_DESCRIPTION (301)  description string
  GROUP_CURRENT_VP (290)   boolean (current-vp / related)
  GROUP_VIEWPORT_CODE (302) filter / viewport code string (optional)

Per-layer block (repeats; starts at GROUP_LAYER_HANDLE (330) or GROUP_LAYER_NAME (8)):
  GROUP_LAYER_HANDLE (330) soft-pointer to LAYER object
  GROUP_LAYER_NAME (8)     layer name string (classic .las; optional on AC1032)
  GROUP_FLAGS (90)         per-layer state flags (AutoLISP layerstate-addlayers):
                             1=Off, 2=Frozen, 4=Locked, 8=NoPlot, 16=FrozenInNewViewports
                             (distinct from state-level LayerStateMasks / group 91)
  GROUP_COLOR (62)         ACI color
  GROUP_TRUE_COLOR (420)   24-bit true color (optional; not exercised by multi-page AC1032 maps)
  GROUP_LINEWEIGHT (370)   lineweight enum index
  GROUP_LINETYPE (331)     soft-pointer (linetype / related)
  GROUP_PLOTSTYLE (1)      plot-style / extra name string (e.g. "Farbe_2")
  GROUP_TRANSPARENCY (440) transparency raw (AcCmTransparency family)

Layer *names* on multi-page AC1032 maps are resolved via 330 → decode_layer_names (group 8 is
absent there; group 1 holds plot-style names, not layer names).

Fixture constants (multi-page AC1032 maps / LibreDWG) — pinned in tests
-------------------------------------------------------
- LAS_DICT_ENTRIES = 12 dictionary keys under ACAD_LAYERSTATES
- LAS_GROUPS_PER_LIVE_STATE = 2454 XDATA groups per live state (exact parse)
- LAS_BLOCKS_PER_LIVE_STATE ≈ 350 per-layer blocks
- LAS_FULL_MASK = 2047 (0x7FF) — all six live states use the full mask
"""

from __future__ import annotations

import re
from dataclasses import dataclass, replace
from enum import IntFlag
from typing import Any, Iterator, Mapping, Optional

from . import raw
from .layers import _transparency_from_raw

# ---------------------------------------------------------------------------
# Explicit DXF group-code constants (layer-state XRECORD schema)
# ---------------------------------------------------------------------------
GROUP_LAYER_NAME = 8
GROUP_PLOTSTYLE = 1
GROUP_COLOR = 62
GROUP_FLAGS = 90
GROUP_MASK = 91
GROUP_DESCRIPTION = 301
GROUP_CURRENT_VP = 290
GROUP_VIEWPORT_CODE = 302
GROUP_LAYER_HANDLE = 330
GROUP_LINETYPE = 331
GROUP_LINEWEIGHT = 370
GROUP_TRUE_COLOR = 420
GROUP_TRANSPARENCY = 440

# AutoLISP layerstate-addlayers *state* bits (group 90 per layer)
FLAG_OFF = 1
FLAG_FROZEN = 2
FLAG_LOCKED = 4
FLAG_NO_PLOT = 8
FLAG_FROZEN_IN_NEW_VIEWPORTS = 16

# Pinned multi-page AC1032 maps fixture expectations (tests assert these exactly)
LAS_DICT_ENTRIES = 12
LAS_GROUPS_PER_LIVE_STATE = 2454
LAS_MIN_BLOCKS_PER_LIVE_STATE = 300
LAS_FULL_MASK = 0x7FF  # 2047


class LayerStateMasks(IntFlag):
    """Bits matching Autodesk LayerStateMasks / AcDbLayerStateManager."""

    ON = 0x0001
    FROZEN = 0x0002
    LOCKED = 0x0004
    PLOT = 0x0008
    NEW_VIEWPORT = 0x0010
    COLOR = 0x0020
    LINE_TYPE = 0x0040
    LINE_WEIGHT = 0x0080
    PLOT_STYLE = 0x0100
    CURRENT_VIEWPORT = 0x0200
    TRANSPARENCY = 0x0400


_HANDLE_VALUE_RE = re.compile(r"value=(\d+)")


def _parse_handle_value(val: Any) -> Optional[int]:
    if isinstance(val, int):
        return val
    if isinstance(val, str):
        m = _HANDLE_VALUE_RE.search(val)
        if m:
            return int(m.group(1))
    return None


@dataclass(frozen=True)
class LayerStateEntry:
    """One layer's snapshot inside a named state."""

    name: Optional[str]
    layer_handle: Optional[int]
    flags: Optional[int]
    on: Optional[bool]
    frozen: Optional[bool]
    locked: Optional[bool]
    plot: Optional[bool]
    frozen_in_new_viewports: Optional[bool]
    color: Optional[int]
    true_color: Optional[int]
    linetype_handle: Optional[int]
    linetype: Optional[str]
    lineweight_index: Optional[int]
    transparency: Optional[float]
    plot_style: Optional[str]
    raw_groups: tuple[tuple[int, Any], ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "layer_handle": self.layer_handle,
            "flags": self.flags,
            "on": self.on,
            "frozen": self.frozen,
            "locked": self.locked,
            "plot": self.plot,
            "frozen_in_new_viewports": self.frozen_in_new_viewports,
            "color": self.color,
            "true_color": self.true_color,
            "linetype_handle": self.linetype_handle,
            "linetype": self.linetype,
            "lineweight_index": self.lineweight_index,
            "transparency": self.transparency,
            "plot_style": self.plot_style,
        }



@dataclass(frozen=True)
class LayerStateDiffEntry:
    """One layer compared between a saved state and the live drawing."""

    layer_name: Optional[str]
    layer_handle: Optional[int]
    status: str  # matched | changed | missing_in_drawing | missing_in_state
    changes: dict[str, tuple[Any, Any]]  # prop -> (state_value, live_value)

    def to_dict(self) -> dict[str, Any]:
        return {
            "layer_name": self.layer_name,
            "layer_handle": self.layer_handle,
            "status": self.status,
            "changes": {k: list(v) for k, v in self.changes.items()},
        }


@dataclass(frozen=True)
class LayerStateDiff:
    """Result of LayerState.diff(document)."""

    state_name: str
    mask: LayerStateMasks
    entries: tuple[LayerStateDiffEntry, ...]
    summary: dict[str, int]

    def to_dict(self) -> dict[str, Any]:
        return {
            "state_name": self.state_name,
            "mask": int(self.mask),
            "summary": dict(self.summary),
            "entries": [e.to_dict() for e in self.entries],
        }


# Property keys compared under each LayerStateMasks bit.
_DIFF_PROP_MASK: tuple[tuple[str, LayerStateMasks], ...] = (
    ("on", LayerStateMasks.ON),
    ("frozen", LayerStateMasks.FROZEN),
    ("locked", LayerStateMasks.LOCKED),
    ("plot", LayerStateMasks.PLOT),
    ("frozen_in_new_viewports", LayerStateMasks.NEW_VIEWPORT),
    ("color", LayerStateMasks.COLOR),
    ("true_color", LayerStateMasks.COLOR),
    ("lineweight_index", LayerStateMasks.LINE_WEIGHT),
    ("linetype_handle", LayerStateMasks.LINE_TYPE),
    ("linetype", LayerStateMasks.LINE_TYPE),
    ("transparency", LayerStateMasks.TRANSPARENCY),
    ("plot_style", LayerStateMasks.PLOT_STYLE),
    # flags always useful when any visibility bit is masked
    ("flags", LayerStateMasks.ON | LayerStateMasks.FROZEN | LayerStateMasks.LOCKED | LayerStateMasks.PLOT | LayerStateMasks.NEW_VIEWPORT),
)


def _live_prop(layer: Any, prop: str, *, plotstyle_names: Optional[Mapping[int, str]] = None) -> Any:
    if prop == "linetype_handle":
        return getattr(layer, "ltype_handle", None)
    if prop == "linetype":
        return getattr(layer, "linetype", None)
    if prop == "plot_style":
        # Resolve live plotstyle_handle → name for comparison with state strings.
        h = getattr(layer, "plotstyle_handle", None)
        if h is None or plotstyle_names is None:
            return None
        return plotstyle_names.get(int(h))
    if prop == "flags":
        # Reconstruct a Layer-like flag word is not available on Layer;
        # live side of flags compare is skipped (None).
        return None
    return getattr(layer, prop, None)


def _state_prop(entry: "LayerStateEntry", prop: str) -> Any:
    return getattr(entry, prop, None)


def _normalize_lineweight_index(v: Any) -> Any:
    """Map DWG default sentinels to one value so -3 and 31 do not false-diff."""
    if v is None:
        return None
    try:
        iv = int(v)
    except (TypeError, ValueError):
        return v
    # Common DEFAULT markers in layer / state payloads
    if iv in (-3, 31):
        return -3
    return iv


def _values_differ(a: Any, b: Any, *, prop: str = "") -> bool:
    if a is None and b is None:
        return False
    if a is None or b is None:
        # One side unknown — not a hard change
        return False
    if prop == "lineweight_index":
        a = _normalize_lineweight_index(a)
        b = _normalize_lineweight_index(b)
    if isinstance(a, float) or isinstance(b, float):
        try:
            return abs(float(a) - float(b)) > 1e-6
        except (TypeError, ValueError):
            return a != b
    return a != b


@dataclass(frozen=True)
class LayerState:
    name: str
    handle: int
    mask: LayerStateMasks
    description: Optional[str]
    entries: tuple[LayerStateEntry, ...]
    raw_header_groups: tuple[tuple[int, Any], ...]
    #: Group 290 when present (viewport-related flag from XRECORD header).
    current_viewport: Optional[bool] = None
    #: Group 302 when present (filter / viewport code string).
    viewport_code: Optional[str] = None

    def get(self, layer_name: str) -> Optional[LayerStateEntry]:
        for e in self.entries:
            if e.name == layer_name:
                return e
        return None

    def diff(
        self,
        document: Any,
        *,
        properties: Optional[LayerStateMasks] = None,
        include_missing_in_state: bool = False,
    ) -> LayerStateDiff:
        """Compare this state to ``document.layers``.

        Parameters
        ----------
        document:
            An ezdwg ``Document`` (needs ``.layers``).
        properties:
            Mask of properties to compare; default ``self.mask``.
        include_missing_in_state:
            If True, live layers with no state entry are listed as
            ``missing_in_state`` (default off).
        """
        mask = properties if properties is not None else self.mask
        layers = document.layers
        diff_entries: list[LayerStateDiffEntry] = []
        seen_names: set[str] = set()
        # Handle → name for live plot-style comparison (CTB often only "Normal").
        _doc_path = getattr(document, "decode_path", None) or getattr(document, "path", None)
        plotstyle_names = _plotstyle_handle_to_name_map(_doc_path)

        for entry in self.entries:
            if entry.name is None:
                diff_entries.append(
                    LayerStateDiffEntry(
                        layer_name=None,
                        layer_handle=entry.layer_handle,
                        status="missing_in_drawing",
                        changes={},
                    )
                )
                continue

            seen_names.add(entry.name)
            live = layers.get(entry.name)
            if live is None:
                diff_entries.append(
                    LayerStateDiffEntry(
                        layer_name=entry.name,
                        layer_handle=entry.layer_handle,
                        status="missing_in_drawing",
                        changes={},
                    )
                )
                continue

            changes: dict[str, tuple[Any, Any]] = {}
            for prop, bit in _DIFF_PROP_MASK:
                if not (mask & bit):
                    continue
                if prop == "flags":
                    # Raw state flags only — live Layer has no matching word.
                    continue
                sv = _state_prop(entry, prop)
                lv = _live_prop(live, prop, plotstyle_names=plotstyle_names)
                if _values_differ(sv, lv, prop=prop):
                    changes[prop] = (sv, lv)

            status = "changed" if changes else "matched"
            diff_entries.append(
                LayerStateDiffEntry(
                    layer_name=entry.name,
                    layer_handle=entry.layer_handle,
                    status=status,
                    changes=changes,
                )
            )

        if include_missing_in_state:
            for live in layers:
                if live.name not in seen_names:
                    diff_entries.append(
                        LayerStateDiffEntry(
                            layer_name=live.name,
                            layer_handle=getattr(live, "handle", None),
                            status="missing_in_state",
                            changes={},
                        )
                    )

        summary = {
            "matched": 0,
            "changed": 0,
            "missing_in_drawing": 0,
            "missing_in_state": 0,
        }
        for de in diff_entries:
            summary[de.status] = summary.get(de.status, 0) + 1

        return LayerStateDiff(
            state_name=self.name,
            mask=mask,
            entries=tuple(diff_entries),
            summary=summary,
        )

    def apply(
        self,
        document: Any,
        *,
        properties: Optional[LayerStateMasks] = None,
        skip_missing: bool = True,
        dry_run: bool = False,
    ) -> LayerStateDiff:
        """Restore masked properties onto ``document.layers`` (in memory only).

        Does **not** write DWG/DXF bytes.

        Parameters
        ----------
        document:
            Open ``Document`` whose ``layers`` table is updated.
        properties:
            Mask of properties to restore; default ``self.mask``.
        skip_missing:
            If True (default), entries without a live layer are skipped.
        dry_run:
            If True, do not mutate; return the current ``diff`` under the
            same mask (preview of what would change).

        Returns
        -------
        LayerStateDiff
            After a real apply, the post-apply diff (ideally few/no
            ``changed`` rows for properties that had state values).
        """
        mask = properties if properties is not None else self.mask
        if dry_run:
            return self.diff(document, properties=mask)

        from .layers import Layer  # local import: avoid cycles at module load
        from . import lineweight as _lineweight

        layers = document.layers
        plotstyle_by_name: Optional[dict[str, int]] = None
        for entry in self.entries:
            if entry.name is None:
                if skip_missing:
                    continue
                continue
            live = layers.get(entry.name)
            if live is None:
                if skip_missing:
                    continue
                continue

            updates: dict[str, Any] = {}
            if (mask & LayerStateMasks.ON) and entry.on is not None:
                updates["on"] = entry.on
            if (mask & LayerStateMasks.FROZEN) and entry.frozen is not None:
                updates["frozen"] = entry.frozen
            if (mask & LayerStateMasks.LOCKED) and entry.locked is not None:
                updates["locked"] = entry.locked
            if (mask & LayerStateMasks.PLOT) and entry.plot is not None:
                updates["plot"] = entry.plot
            if (mask & LayerStateMasks.COLOR) and entry.color is not None:
                updates["color"] = abs(int(entry.color))
            if (mask & LayerStateMasks.COLOR) and entry.true_color is not None:
                tc = int(entry.true_color)
                updates["true_color"] = tc
                updates["rgb"] = ((tc >> 16) & 0xFF, (tc >> 8) & 0xFF, tc & 0xFF)
            if (mask & LayerStateMasks.NEW_VIEWPORT) and entry.frozen_in_new_viewports is not None:
                updates["frozen_in_new_viewports"] = entry.frozen_in_new_viewports
            if (mask & LayerStateMasks.LINE_TYPE) and entry.linetype_handle is not None:
                updates["ltype_handle"] = entry.linetype_handle
            if (mask & LayerStateMasks.TRANSPARENCY) and entry.transparency is not None:
                updates["transparency"] = entry.transparency
            if (mask & LayerStateMasks.LINE_WEIGHT) and entry.lineweight_index is not None:
                idx = int(entry.lineweight_index)
                # Normalize default sentinels to table index 31 when needed
                if idx == -3:
                    idx = 31
                updates["lineweight_index"] = idx
                try:
                    lw, oob = _lineweight._lookup(idx, None)
                    updates["lineweight"] = lw
                    updates["lineweight_index_out_of_range"] = oob
                except Exception:
                    pass
            if (mask & LayerStateMasks.PLOT_STYLE) and entry.plot_style is not None:
                # Resolve state plot-style string → live handle; skip if unknown.
                if plotstyle_by_name is None:
                    plotstyle_by_name = _plotstyle_name_to_handle_map(
                        getattr(document, "decode_path", None) or getattr(document, "path", None)
                    )
                h = plotstyle_by_name.get(entry.plot_style)
                if h is None:
                    # Case-insensitive fallback (AutoCAD names are usually exact)
                    h = next(
                        (
                            v
                            for k, v in plotstyle_by_name.items()
                            if k.lower() == entry.plot_style.lower()
                        ),
                        None,
                    )
                if h is not None and h != 0:
                    updates["plotstyle_handle"] = int(h)

            if not updates:
                continue
            new_layer = replace(live, **updates)
            layers.replace_layer(new_layer)

        return self.diff(document, properties=mask)

    def resolution_stats(self) -> dict[str, int]:
        """Count named vs handle-only entries (live LAYER table only)."""
        total = len(self.entries)
        with_handle = sum(1 for e in self.entries if e.layer_handle is not None)
        named = sum(1 for e in self.entries if e.name is not None)
        return {
            "entries": total,
            "with_layer_handle": with_handle,
            "named": named,
            "unnamed": total - named,
        }

    def to_dict(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "handle": self.handle,
            "mask": int(self.mask),
            "description": self.description,
            "current_viewport": self.current_viewport,
            "viewport_code": self.viewport_code,
            "entries": [e.to_dict() for e in self.entries],
        }


@dataclass(frozen=True)
class LayerStateTable:
    """All named states in a drawing (immutable snapshot)."""

    states: tuple[LayerState, ...]

    def names(self) -> list[str]:
        return [s.name for s in self.states]

    def get(self, name: str) -> Optional[LayerState]:
        for s in self.states:
            if s.name == name:
                return s
        return None

    def resolution_stats(self) -> dict[str, int]:
        """Aggregate name-resolution stats across all states."""
        entries = with_handle = named = 0
        for s in self.states:
            st = s.resolution_stats()
            entries += st["entries"]
            with_handle += st["with_layer_handle"]
            named += st["named"]
        return {
            "states": len(self.states),
            "entries": entries,
            "with_layer_handle": with_handle,
            "named": named,
            "unnamed": entries - named,
        }

    def __len__(self) -> int:
        return len(self.states)

    def __iter__(self) -> Iterator[LayerState]:
        return iter(self.states)

    def __getitem__(self, index: int) -> LayerState:
        return self.states[index]


def _decode_flags(
    flags: Optional[int],
) -> tuple[Optional[bool], Optional[bool], Optional[bool], Optional[bool], Optional[bool]]:
    """Decode per-layer group 90 (AutoLISP layerstate-addlayers *state* bits).

    Official *state* integer (sum of bits), Autodesk OARX Help
    ``layerstate-addlayers``:

      FLAG_OFF (1)                    — layer off
      FLAG_FROZEN (2)                 — frozen
      FLAG_LOCKED (4)                 — locked
      FLAG_NO_PLOT (8)                — no plot
      FLAG_FROZEN_IN_NEW_VIEWPORTS (16) — frozen in new viewports

    Returns ``(on, frozen, locked, plot, frozen_in_new_viewports)``.
    When *flags* is None, all five values are None.

    Visibility is flags-only: negative ACI is a LAYER-table / DXF convention
    and is **not** used here (never observed in state group 62 on ``multi-page AC1032 maps``).

    Note: state-level ``LayerStateMasks`` (group 91) uses different bit
    meanings (which properties to restore), not this encoding.
    """
    if flags is None:
        return None, None, None, None, None

    on = not bool(flags & FLAG_OFF)
    frozen = bool(flags & FLAG_FROZEN)
    locked = bool(flags & FLAG_LOCKED)
    plot = not bool(flags & FLAG_NO_PLOT)
    frozen_new_vp = bool(flags & FLAG_FROZEN_IN_NEW_VIEWPORTS)
    return on, frozen, locked, plot, frozen_new_vp



def _header_viewport_meta(
    header: list[tuple[int, Any]],
) -> tuple[Optional[bool], Optional[str]]:
    """Extract groups 290 / 302 from state header (best-effort)."""
    current_vp: Optional[bool] = None
    vp_code: Optional[str] = None
    for code, val in header:
        if code == 290 and current_vp is None:
            if isinstance(val, bool):
                current_vp = val
            elif isinstance(val, int):
                current_vp = bool(val)
        elif code == GROUP_VIEWPORT_CODE and vp_code is None and isinstance(val, str):
            vp_code = val
    return current_vp, vp_code


def _split_state_groups(
    groups: list[tuple[int, Any]],
) -> tuple[list[tuple[int, Any]], list[list[tuple[int, Any]]]]:
    """Split raw groups into header + per-layer blocks.

    A per-layer block starts at GROUP_LAYER_HANDLE (330) or GROUP_LAYER_NAME (8).
    """
    header: list[tuple[int, Any]] = []
    blocks: list[list[tuple[int, Any]]] = []
    current: Optional[list[tuple[int, Any]]] = None
    for code, val in groups:
        if code in (GROUP_LAYER_HANDLE, GROUP_LAYER_NAME):
            current = [(code, val)]
            blocks.append(current)
            continue
        if current is None:
            header.append((code, val))
        else:
            current.append((code, val))
    return header, blocks


def _entry_from_block(
    block: list[tuple[int, Any]],
    layer_names: Mapping[int, str],
    linetype_names: Optional[Mapping[int, str]] = None,
) -> LayerStateEntry:
    layer_handle: Optional[int] = None
    name_from_dxf8: Optional[str] = None
    flags: Optional[int] = None
    color: Optional[int] = None
    true_color: Optional[int] = None
    lineweight_index: Optional[int] = None
    linetype_handle: Optional[int] = None
    plot_style: Optional[str] = None
    transparency: Optional[float] = None

    for code, val in block:
        if code == GROUP_LAYER_HANDLE and layer_handle is None:
            layer_handle = _parse_handle_value(val)
        elif code == GROUP_LAYER_NAME and name_from_dxf8 is None and isinstance(val, str):
            name_from_dxf8 = val
        elif code == GROUP_FLAGS and flags is None and isinstance(val, int):
            flags = val
        elif code == GROUP_COLOR and color is None and isinstance(val, int):
            color = val
        elif code == GROUP_TRUE_COLOR and true_color is None and isinstance(val, int):
            true_color = val
        elif code == GROUP_LINEWEIGHT and lineweight_index is None and isinstance(val, int):
            lineweight_index = val
        elif code == GROUP_LINETYPE and linetype_handle is None:
            linetype_handle = _parse_handle_value(val)
        elif code == GROUP_PLOTSTYLE and plot_style is None and isinstance(val, str):
            plot_style = val
        elif code == GROUP_TRANSPARENCY and transparency is None and isinstance(val, int):
            transparency = _transparency_from_raw(val)

    name = None
    if layer_handle is not None:
        name = layer_names.get(layer_handle)
    if name is None and name_from_dxf8:
        name = name_from_dxf8
    on, frozen, locked, plot, frozen_new_vp = _decode_flags(flags)
    abs_color = abs(color) if color is not None else None

    return LayerStateEntry(
        name=name,
        layer_handle=layer_handle,
        flags=flags,
        on=on,
        frozen=frozen,
        locked=locked,
        plot=plot,
        frozen_in_new_viewports=frozen_new_vp,
        color=abs_color,
        true_color=true_color,
        linetype_handle=linetype_handle,
        linetype=(
            linetype_names.get(linetype_handle)
            if linetype_names is not None and linetype_handle is not None
            else None
        ),
        lineweight_index=lineweight_index,
        transparency=transparency,
        plot_style=plot_style,
        raw_groups=tuple(block),
    )


def _build_layer_name_map(path: str) -> dict[int, str]:
    """Map LAYER object handle → name from the live layer table.

    Handles that appear in a layer-state XRECORD but are absent here were
    purged/renamed since the state was saved — entries keep ``name=None``.
    """
    out: dict[int, str] = {}
    try:
        for h, n in raw.decode_layer_names(path):
            out[int(h)] = str(n)
    except Exception:
        pass
    return out


def _build_linetype_name_map(path: str) -> dict[int, str]:
    """Best-effort handle → linetype name from the live layer table."""
    out: dict[int, str] = {}
    try:
        from .layers import build_layer_table
        for layer in build_layer_table(path):
            if layer.ltype_handle is not None and layer.linetype:
                out[int(layer.ltype_handle)] = str(layer.linetype)
    except Exception:
        pass
    return out


def _plotstyle_name_to_handle_map(path: Optional[str]) -> dict[str, int]:
    """Plot-style dictionary: name → object handle (0 entries if path missing)."""
    out: dict[str, int] = {}
    if not path:
        return out
    try:
        for name, handle in raw.decode_plotstyles(str(path)):
            if name and handle:
                out[str(name)] = int(handle)
    except Exception:
        pass
    return out


def _plotstyle_handle_to_name_map(path: Optional[str]) -> dict[int, str]:
    """Inverse of `_plotstyle_name_to_handle_map` (first name wins per handle)."""
    out: dict[int, str] = {}
    if not path:
        return out
    try:
        for name, handle in raw.decode_plotstyles(str(path)):
            if name and handle and int(handle) not in out:
                out[int(handle)] = str(name)
    except Exception:
        pass
    return out


def build_layer_state_table(path: str) -> LayerStateTable:
    """Decode all layer states in *path* into a LayerStateTable.

    One ``LayerState`` per dictionary name that has a decoded XRECORD body.
    Order follows ``raw.decode_layer_state_names`` (dictionary order). Names
    present only in the dictionary with no resolvable body are skipped (after
    fix-03 / feature-05 this does not occur on ``multi-page AC1032 maps``).
    """
    layer_names = _build_layer_name_map(path)
    linetype_names = _build_linetype_name_map(path)
    all_names = list(raw.decode_layer_state_names(path))
    rows = raw.decode_layer_state_xrecords(path)
    by_name: dict[str, Any] = {}
    for row in rows:
        by_name[str(row[0])] = row

    states: list[LayerState] = []
    for name in all_names:
        row = by_name.get(name)
        if row is None:
            continue
        _n, xh, mask_i, desc, groups, _objids, _xraw = row
        header, blocks = _split_state_groups(list(groups))
        entries = tuple(_entry_from_block(b, layer_names, linetype_names) for b in blocks)
        mask = LayerStateMasks(mask_i if mask_i is not None else 0)
        cur_vp, vp_code = _header_viewport_meta(header)
        states.append(
            LayerState(
                name=str(name),
                handle=int(xh),
                mask=mask,
                description=desc if isinstance(desc, str) else None,
                entries=entries,
                raw_header_groups=tuple(header),
                current_viewport=cur_vp,
                viewport_code=vp_code,
            )
        )
    return LayerStateTable(states=tuple(states))
