"""
High-level layer-state API for ezdwg (Phase 2).

Builds on raw.decode_layer_state_names / decode_layer_state_xrecords.

XRECORD group schema (explicit constants — not inferred from one file)
----------------------------------------------------------------------
State-level (once, before first layer block):
  GROUP_MASK (91)          LayerStateMasks bitfield (prefer over 90)
  GROUP_DESCRIPTION (301)  description string
  GROUP_CURRENT_VP (290)   boolean — True means "saved with a viewport current"
                           (typically False on stock samples; see module docs)
  GROUP_VIEWPORT_CODE (302) opaque context string (opaque context; not necessarily a filter/layout name)

Per-layer block (repeats; starts at GROUP_LAYER_HANDLE (330) or GROUP_LAYER_NAME (8)):
  GROUP_LAYER_HANDLE (330) soft-pointer to LAYER object
  GROUP_LAYER_NAME (8)     layer name string (classic .las; optional on AC1032)
  GROUP_FLAGS (90)         per-layer state flags (AutoLISP layerstate-addlayers):
                             1=Off, 2=Frozen, 4=Locked, 8=NoPlot, 16=FrozenInNewViewports
                             (distinct from state-level LayerStateMasks / group 91)
  GROUP_COLOR (62)         ACI color
  GROUP_TRUE_COLOR (420)   24-bit true color (optional)
  GROUP_LINEWEIGHT (370)   hundredths of a millimetre or a DXF sentinel
  GROUP_LINETYPE (331)     soft-pointer (linetype / related)
  GROUP_PLOTSTYLE (1)      plot-style / extra name string (e.g. "Farbe_2")
  GROUP_TRANSPARENCY (440) transparency raw (AcCmTransparency family)

Layer *names* are resolved via 330 → decode_layer_names (group 8 is
absent there; group 1 holds plot-style names, not layer names).

Raw groups are retained for fields outside the structured API.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field, replace
from enum import IntFlag
from typing import Any, Iterator, Mapping, Optional

from . import raw
from .layers import _properties_from_raw_color, _transparency_from_raw
from .lineweight import STANDARD_TABLE

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
GROUP_LINEWEIGHT = 370  # hundredths of a millimetre, converted to an enum index
GROUP_TRUE_COLOR = 420
GROUP_TRANSPARENCY = 440

# AutoLISP layerstate-addlayers *state* bits (group 90 per layer)
FLAG_OFF = 1
FLAG_FROZEN = 2
FLAG_LOCKED = 4
FLAG_NO_PLOT = 8
FLAG_FROZEN_IN_NEW_VIEWPORTS = 16


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
    """Result of LayerState.diff(document) / apply.

    ``summary`` counts entry statuses. Extra keys may appear:

    - ``viewport_overrides_skipped`` (0 or 1): mask includes
      ``CURRENT_VIEWPORT`` but per-viewport freeze **and** LAYER xdict
      property overrides (VP Color/Ltype/…) are not restored; global layer
      properties were still compared/applied as usual.
    """

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
    (
        "flags",
        LayerStateMasks.ON
        | LayerStateMasks.FROZEN
        | LayerStateMasks.LOCKED
        | LayerStateMasks.PLOT
        | LayerStateMasks.NEW_VIEWPORT,
    ),
)


def _live_prop(
    layer: Any, prop: str, *, plotstyle_names: Optional[Mapping[int, str]] = None
) -> Any:
    if prop == "transparency":
        return getattr(layer, prop, None) or 0.0
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
    if prop == "true_color":
        return a != b
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


def _document_viewport_overrides(document: Any) -> "ViewportOverrideTable":
    """Return (and cache) a mutable ViewportOverrideTable on *document*."""
    existing = getattr(document, "viewport_overrides", None)
    if isinstance(existing, ViewportOverrideTable):
        return existing
    path = getattr(document, "decode_path", None) or getattr(document, "path", None)
    table = (
        build_viewport_override_table(str(path)) if path else ViewportOverrideTable()
    )
    try:
        object.__setattr__(document, "viewport_overrides", table)
    except Exception:
        try:
            document.viewport_overrides = table  # type: ignore[attr-defined]
        except Exception:
            pass
    return table


def _vp_live_prop(
    vp_table: "ViewportOverrideTable",
    layer: Any,
    layer_handle: int,
    viewport_handle: int,
    prop: str,
    *,
    plotstyle_names: Optional[Mapping[int, str]] = None,
) -> Any:
    """Effective value of *prop* for *layer* inside *viewport_handle*."""
    if prop == "frozen":
        return vp_table.is_frozen(viewport_handle, layer_handle)
    if prop == "color":
        v = vp_table.get_property(layer_handle, viewport_handle, "color")
        return (
            v
            if v is not None
            else _live_prop(layer, "color", plotstyle_names=plotstyle_names)
        )
    if prop == "true_color":
        props = vp_table.properties.get((layer_handle, viewport_handle), {})
        if "true_color" in props:
            return props["true_color"]
        return _live_prop(layer, "true_color", plotstyle_names=plotstyle_names)
    if prop == "lineweight_index":
        idx = vp_table.get_property(layer_handle, viewport_handle, "lineweight_index")
        if idx is not None:
            return idx
        v = vp_table.get_property(layer_handle, viewport_handle, "lineweight")
        return (
            _lineweight_index_from_dxf(v)
            if v is not None
            else _live_prop(layer, "lineweight_index", plotstyle_names=plotstyle_names)
        )
    if prop in ("linetype_handle", "linetype"):
        v = vp_table.get_property(layer_handle, viewport_handle, "linetype")
        if v is not None:
            return int(v) if prop == "linetype_handle" else None
        return _live_prop(layer, prop, plotstyle_names=plotstyle_names)
    if prop == "transparency":
        v = vp_table.get_property(layer_handle, viewport_handle, "transparency")
        return (
            v
            if v is not None
            else _live_prop(layer, "transparency", plotstyle_names=plotstyle_names)
        )
    if prop == "plot_style":
        v = vp_table.get_property(layer_handle, viewport_handle, "plot_style")
        if v is not None and plotstyle_names is not None:
            return plotstyle_names.get(int(v), v)
        if v is not None:
            return v
        return _live_prop(layer, "plot_style", plotstyle_names=plotstyle_names)
    return _live_prop(layer, prop, plotstyle_names=plotstyle_names)


@dataclass
class ViewportOverrideTable:
    """Mutable in-memory VP Freeze + property overrides (V2a/V2b + V3-C).

    Loaded from the live drawing; ``LayerState.apply(..., viewport=…)`` mutates
    this table only (does not write DWG bytes or global ``Layer`` rows for the
    VP-targeted channels).
    """

    #: viewport_handle → set of layer handles frozen in that VP
    frozen_by_vp: dict[int, set[int]] = field(default_factory=dict)
    #: (layer_handle, viewport_handle) → {property: value}
    properties: dict[tuple[int, int], dict[str, Any]] = field(default_factory=dict)

    def is_frozen(self, viewport_handle: int, layer_handle: int) -> bool:
        return int(layer_handle) in self.frozen_by_vp.get(int(viewport_handle), set())

    def get_property(self, layer_handle: int, viewport_handle: int, prop: str) -> Any:
        return self.properties.get((int(layer_handle), int(viewport_handle)), {}).get(
            prop
        )

    def set_frozen(self, viewport_handle: int, layer_handle: int, frozen: bool) -> None:
        vp = int(viewport_handle)
        lh = int(layer_handle)
        bucket = self.frozen_by_vp.setdefault(vp, set())
        if frozen:
            bucket.add(lh)
        else:
            bucket.discard(lh)

    def set_property(
        self, layer_handle: int, viewport_handle: int, prop: str, value: Any
    ) -> None:
        key = (int(layer_handle), int(viewport_handle))
        self.properties.setdefault(key, {})[prop] = value

    def stats(self) -> dict[str, int]:
        return {
            "viewports_with_freeze": sum(1 for s in self.frozen_by_vp.values() if s),
            "frozen_pairs": sum(len(s) for s in self.frozen_by_vp.values()),
            "property_pairs": len(self.properties),
            "property_cells": sum(len(d) for d in self.properties.values()),
        }


def build_viewport_override_table(path: str) -> ViewportOverrideTable:
    """Load live VP freeze lists + LAYER xdict property overrides."""
    table = ViewportOverrideTable()
    for vp_h, handles in read_viewport_frozen_layers(path).items():
        table.frozen_by_vp[int(vp_h)] = set(int(h) for h in handles)
    for row in read_layer_vp_overrides(path):
        prop, value = row["property"], row["value"]
        if prop == "color":
            values = _properties_from_raw_color(value)
        else:
            values = {
                prop: _transparency_from_raw(value) if prop == "transparency" else value
            }
        for key, normalized in values.items():
            table.set_property(
                row["layer_handle"], row["viewport_handle"], key, normalized
            )
    return table


@dataclass(frozen=True)
class LayerState:
    name: str
    handle: int
    mask: LayerStateMasks
    description: Optional[str]
    entries: tuple[LayerStateEntry, ...]
    raw_header_groups: tuple[tuple[int, Any], ...]
    #: Group 290 when present (viewport-related flag from XRECORD header).
    #: When False, restore is treated as global (not viewport-scoped). Calibrated as
    #: "saved while a viewport was current" only when True is observed on a
    #: fixture; unknown context strings are preserved without interpretation.
    current_viewport: Optional[bool] = None
    #: Group 302 when present — opaque context string (not a filter name or
    #: layout name). May be AEC layer-key style codes.
    viewport_code: Optional[str] = None

    def get(self, layer_name: str) -> Optional[LayerStateEntry]:
        for e in self.entries:
            if e.name == layer_name:
                return e
        return None

    @property
    def scope(self) -> str:
        """Best-effort save scope from header group 290 only.

        Returns
        -------
        ``\"viewport\"``
            Group 290 is True.
        ``\"global\"``
            Group 290 is False.
        ``\"unknown\"``
            Group 290 absent.

        Group 302 is **not** used for classification (opaque on current fixtures).
        """
        if self.current_viewport is True:
            return "viewport"
        if self.current_viewport is False:
            return "global"
        return "unknown"

    def diff(
        self,
        document: Any,
        *,
        properties: Optional[LayerStateMasks] = None,
        include_missing_in_state: bool = False,
        viewport: Optional[int] = None,
    ) -> LayerStateDiff:
        """Compare this state to ``document.layers`` (and optional VP overrides).

        Parameters
        ----------
        document:
            An ezdwg ``Document`` (needs ``.layers``).
        properties:
            Mask of properties to compare; default ``self.mask``.
        include_missing_in_state:
            If True, live layers with no state entry are listed as
            ``missing_in_state`` (default off).
        viewport:
            When set to a viewport entity handle, freeze / color / linetype /
            lineweight / transparency / plot_style are compared against
            ``document.viewport_overrides`` for that VP (V3-C). Other flags
            still compare to the global ``Layer`` table.
        """
        mask = properties if properties is not None else self.mask
        layers = document.layers
        diff_entries: list[LayerStateDiffEntry] = []
        seen_names: set[str] = set()
        # Handle → name for live plot-style comparison (CTB often only "Normal").
        _doc_path = getattr(document, "decode_path", None) or getattr(
            document, "path", None
        )
        plotstyle_names = _plotstyle_handle_to_name_map(_doc_path)
        vp_h = int(viewport) if viewport is not None else None
        vp_table: Optional[ViewportOverrideTable] = None
        if vp_h is not None:
            vp_table = _document_viewport_overrides(document)

        # Properties compared against VP override table when viewport= is set.
        _VP_PROPS = {
            "frozen",
            "color",
            "true_color",
            "lineweight_index",
            "linetype_handle",
            "linetype",
            "transparency",
            "plot_style",
        }

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
            layer_h = int(getattr(live, "handle", 0) or entry.layer_handle or 0)
            for prop, bit in _DIFF_PROP_MASK:
                if not (mask & bit):
                    continue
                if prop == "flags":
                    continue
                if (
                    prop == "true_color"
                    and entry.true_color is None
                    and entry.color is None
                ):
                    continue
                sv = _state_prop(entry, prop)
                if (
                    vp_table is not None
                    and vp_h is not None
                    and prop in _VP_PROPS
                    and layer_h
                ):
                    lv = _vp_live_prop(
                        vp_table,
                        live,
                        layer_h,
                        vp_h,
                        prop,
                        plotstyle_names=plotstyle_names,
                    )
                else:
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
        # V3-B/C: skipped only when CURRENT_VIEWPORT is masked and no target VP.
        summary["viewport_overrides_skipped"] = (
            1 if (mask & LayerStateMasks.CURRENT_VIEWPORT) and vp_h is None else 0
        )
        summary["viewport_overrides_applied"] = 1 if vp_h is not None else 0

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
        viewport: Optional[int] = None,
    ) -> LayerStateDiff:
        """Restore masked properties in memory (global layers and/or one VP).

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
        viewport:
            Viewport entity handle. When set (V3-C), freeze + color / linetype
            / lineweight / transparency / plot_style are written into
            ``document.viewport_overrides`` for that VP only. Global-only
            flags (on, locked, plot, frozen_in_new_viewports) still update
            ``document.layers``. When omitted and the mask includes
            ``CURRENT_VIEWPORT``, those VP channels are skipped
            (``viewport_overrides_skipped=1``); global properties are still restored.

        Returns
        -------
        LayerStateDiff
            Post-apply diff under the same mask and ``viewport`` argument.
        """
        mask = properties if properties is not None else self.mask
        if dry_run:
            return self.diff(document, properties=mask, viewport=viewport)

        from . import lineweight as _lineweight

        layers = document.layers
        if not skip_missing:
            missing = [
                e.name or e.layer_handle
                for e in self.entries
                if e.name is None or layers.get(e.name) is None
            ]
            if missing:
                raise KeyError(f"layers missing from drawing: {missing}")
        plotstyle_by_name: Optional[dict[str, int]] = None
        vp_h = int(viewport) if viewport is not None else None
        vp_table: Optional[ViewportOverrideTable] = None
        if vp_h is not None:
            vp_table = _document_viewport_overrides(document)

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

            layer_h = int(getattr(live, "handle", 0) or entry.layer_handle or 0)

            # --- V3-C: VP-targeted channels ---
            if vp_table is not None and vp_h is not None and layer_h:
                if (mask & LayerStateMasks.FROZEN) and entry.frozen is not None:
                    vp_table.set_frozen(vp_h, layer_h, bool(entry.frozen))
                if (mask & LayerStateMasks.COLOR) and entry.color is not None:
                    # Store ACI as positive; true color separate if present.
                    vp_table.set_property(layer_h, vp_h, "color", abs(int(entry.color)))
                if (mask & LayerStateMasks.COLOR) and (
                    entry.true_color is not None or entry.color is not None
                ):
                    vp_table.set_property(layer_h, vp_h, "true_color", entry.true_color)
                if (
                    mask & LayerStateMasks.LINE_TYPE
                ) and entry.linetype_handle is not None:
                    vp_table.set_property(
                        layer_h, vp_h, "linetype", int(entry.linetype_handle)
                    )
                if (
                    mask & LayerStateMasks.LINE_WEIGHT
                ) and entry.lineweight_index is not None:
                    idx = int(entry.lineweight_index)
                    if idx == -3:
                        idx = 31
                    lw, _ = _lineweight._lookup(
                        idx, getattr(document, "lineweight_table", None)
                    )
                    vp_table.set_property(layer_h, vp_h, "lineweight", lw)
                    vp_table.set_property(layer_h, vp_h, "lineweight_index", idx)
                if (
                    mask & LayerStateMasks.TRANSPARENCY
                ) and entry.transparency is not None:
                    vp_table.set_property(
                        layer_h, vp_h, "transparency", entry.transparency
                    )
                if (mask & LayerStateMasks.PLOT_STYLE) and entry.plot_style is not None:
                    if plotstyle_by_name is None:
                        plotstyle_by_name = _plotstyle_name_to_handle_map(
                            getattr(document, "decode_path", None)
                            or getattr(document, "path", None)
                        )
                    h = plotstyle_by_name.get(entry.plot_style)
                    if h is None:
                        h = next(
                            (
                                v
                                for k, v in plotstyle_by_name.items()
                                if k.lower() == entry.plot_style.lower()
                            ),
                            None,
                        )
                    if h is not None and h != 0:
                        vp_table.set_property(layer_h, vp_h, "plot_style", int(h))

            # --- Global Layer updates (always for non-VP flags; only when no VP for VP channels) ---
            updates: dict[str, Any] = {}
            if (mask & LayerStateMasks.ON) and entry.on is not None:
                updates["on"] = entry.on
            if (
                vp_table is None
                and (mask & LayerStateMasks.FROZEN)
                and entry.frozen is not None
            ):
                updates["frozen"] = entry.frozen
            if (mask & LayerStateMasks.LOCKED) and entry.locked is not None:
                updates["locked"] = entry.locked
            if (mask & LayerStateMasks.PLOT) and entry.plot is not None:
                updates["plot"] = entry.plot
            if (
                vp_table is None
                and (mask & LayerStateMasks.COLOR)
                and entry.color is not None
            ):
                updates["color"] = abs(int(entry.color))
            if (
                vp_table is None
                and (mask & LayerStateMasks.COLOR)
                and (entry.true_color is not None or entry.color is not None)
            ):
                tc = entry.true_color
                updates["true_color"] = tc
                updates["rgb"] = (
                    None
                    if tc is None
                    else ((tc >> 16) & 0xFF, (tc >> 8) & 0xFF, tc & 0xFF)
                )
            if (
                mask & LayerStateMasks.NEW_VIEWPORT
            ) and entry.frozen_in_new_viewports is not None:
                updates["frozen_in_new_viewports"] = entry.frozen_in_new_viewports
            if (
                vp_table is None
                and (mask & LayerStateMasks.LINE_TYPE)
                and entry.linetype_handle is not None
            ):
                updates["ltype_handle"] = entry.linetype_handle
                updates["linetype"] = entry.linetype
            if (
                vp_table is None
                and (mask & LayerStateMasks.TRANSPARENCY)
                and entry.transparency is not None
            ):
                updates["transparency"] = entry.transparency
            if (
                vp_table is None
                and (mask & LayerStateMasks.LINE_WEIGHT)
                and entry.lineweight_index is not None
            ):
                idx = int(entry.lineweight_index)
                if idx == -3:
                    idx = 31
                updates["lineweight_index"] = idx
                try:
                    lw, oob = _lineweight._lookup(
                        idx, getattr(document, "lineweight_table", None)
                    )
                    updates["lineweight"] = lw
                    updates["lineweight_index_out_of_range"] = oob
                except Exception:
                    pass
            if (
                vp_table is None
                and (mask & LayerStateMasks.PLOT_STYLE)
                and entry.plot_style is not None
            ):
                if plotstyle_by_name is None:
                    plotstyle_by_name = _plotstyle_name_to_handle_map(
                        getattr(document, "decode_path", None)
                        or getattr(document, "path", None)
                    )
                h = plotstyle_by_name.get(entry.plot_style)
                if h is None:
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

        return self.diff(document, properties=mask, viewport=viewport)

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
            "scope": self.scope,
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

    def viewport_header_stats(self) -> dict[str, int]:
        """Counts for groups 290/302 and derived ``scope`` (V1 dump aid)."""
        scope_counts = {"global": 0, "viewport": 0, "unknown": 0}
        vp290_true = vp290_false = vp290_none = 0
        code_nonempty = 0
        for s in self.states:
            scope_counts[s.scope] = scope_counts.get(s.scope, 0) + 1
            if s.current_viewport is True:
                vp290_true += 1
            elif s.current_viewport is False:
                vp290_false += 1
            else:
                vp290_none += 1
            if s.viewport_code and s.viewport_code not in ("", "0"):
                code_nonempty += 1
        return {
            "states": len(self.states),
            "group_290_true": vp290_true,
            "group_290_false": vp290_false,
            "group_290_absent": vp290_none,
            "group_302_nonempty": code_nonempty,
            "scope_global": scope_counts["global"],
            "scope_viewport": scope_counts["viewport"],
            "scope_unknown": scope_counts["unknown"],
            "mask_current_viewport": sum(
                1 for s in self.states if s.mask & LayerStateMasks.CURRENT_VIEWPORT
            ),
        }

    def __len__(self) -> int:
        return len(self.states)

    def __iter__(self) -> Iterator[LayerState]:
        return iter(self.states)

    def __getitem__(self, index: int) -> LayerState:
        return self.states[index]


def _decode_flags(
    flags: Optional[int],
) -> tuple[
    Optional[bool], Optional[bool], Optional[bool], Optional[bool], Optional[bool]
]:
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
    and is **not** used here for state group 62 in current samples.

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
            if (
                current is not None
                and not any(c == code for c, _ in current)
                and all(c in (GROUP_LAYER_HANDLE, GROUP_LAYER_NAME) for c, _ in current)
            ):
                current.append((code, val))
                continue
            current = [(code, val)]
            blocks.append(current)
            continue
        if current is None:
            header.append((code, val))
        else:
            current.append((code, val))
    return header, blocks


def _lineweight_index_from_dxf(value: int) -> Optional[int]:
    """Convert group 370 to a DWG enum without wrapping unknown weights."""
    return next(
        (index for index, weight in STANDARD_TABLE.items() if weight == value), None
    )


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
        elif (
            code == GROUP_LAYER_NAME and name_from_dxf8 is None and isinstance(val, str)
        ):
            name_from_dxf8 = val
        elif code == GROUP_FLAGS and flags is None and isinstance(val, int):
            flags = val
        elif code == GROUP_COLOR and color is None and isinstance(val, int):
            color = val
        elif code == GROUP_TRUE_COLOR and true_color is None and isinstance(val, int):
            true_color = val
        elif (
            code == GROUP_LINEWEIGHT
            and lineweight_index is None
            and isinstance(val, int)
        ):
            lineweight_index = _lineweight_index_from_dxf(val)
        elif code == 92 and isinstance(val, int):
            decoded = _properties_from_raw_color(val)
            if "true_color" in decoded:
                true_color = decoded["true_color"]
        elif code == GROUP_LINETYPE and linetype_handle is None:
            linetype_handle = _parse_handle_value(val)
        elif code == GROUP_PLOTSTYLE and plot_style is None and isinstance(val, str):
            plot_style = val
        elif (
            code == GROUP_TRANSPARENCY and transparency is None and isinstance(val, int)
        ):
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
        for handle, name, *_ in raw.decode_linetypes(path):
            out[int(handle)] = str(name)
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
    object-map fixes this does not occur on well-formed multi-page maps).
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
        entries = tuple(
            _entry_from_block(b, layer_names, linetype_names) for b in blocks
        )
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


def read_layer_vp_overrides(path: str) -> list[dict[str, Any]]:
    """Decode LAYER xdict viewport property overrides (V2b).

    Each row is a dict::

        {
          "layer_handle": int,
          "layer_name": str,
          "property": "color"|"linetype"|"lineweight"|"transparency"|"plot_style",
          "viewport_handle": int,
          "value": int,  # encoded color, ltype handle, lw index, alpha, or plotstyle handle
        }

    Source: ``ADSK_XREC_LAYER_*_OVR`` XRECORDs under each LAYER extension
    dictionary. Independent of layer
    states; combine with ``read_viewport_frozen_layers`` for full live VP
    visibility + display state.
    """
    fn = getattr(raw, "decode_layer_vp_overrides", None)
    if fn is None:
        return []
    rows: list[dict[str, Any]] = []
    try:
        for layer_h, layer_name, prop, vp_h, value in fn(str(path)):
            rows.append(
                {
                    "layer_handle": int(layer_h),
                    "layer_name": str(layer_name) if layer_name is not None else "",
                    "property": str(prop),
                    "viewport_handle": int(vp_h),
                    "value": int(value),
                }
            )
    except Exception:
        return []
    return rows


def read_viewport_frozen_layers(path: str) -> dict[int, tuple[int, ...]]:
    """Map viewport handle → frozen layer handles (live VPLAYER data).

    Uses ``raw.decode_viewport_details`` field ``frozen_layer_handles``
    (LibreDWG ``frozen_layers`` / DXF soft-pointers). Independent of layer
    states; pass a viewport handle to ``LayerState.diff`` or ``apply``.

    Returns
    -------
    dict
        ``{viewport_handle: (layer_handle, ...)}`` — only viewports with a
        non-empty freeze list are included when the list is empty we still
        omit them to keep the map compact; call ``decode_viewport_details``
        directly if empty lists matter.
    """
    out: dict[int, tuple[int, ...]] = {}
    try:
        for row in raw.decode_viewport_details(str(path)):
            if not row:
                continue
            vp_h = int(row[0])
            frozen = row[5] if len(row) > 5 else None
            if not frozen:
                continue
            handles = tuple(int(h) for h in frozen if h)
            if handles:
                out[vp_h] = handles
    except Exception:
        pass
    return out
