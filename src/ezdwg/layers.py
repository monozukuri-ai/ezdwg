"""
High-level layer API for ezdwg.

Pure Python, built on top of ezdwg.raw.decode_layer_names /
decode_layer_colors / decode_layer_flags / decode_layer_handles /
decode_layer_eed / decode_layer_color_details.

Layer.transparency, Layer.standard and Layer.description are filled from EED:
  - AcCmTransparency  (binary code 71 / DXF 1071) → transparency percent
  - AcAecLayerStandard (1000-strings) → standard (1st) + description (2nd)

See ezdwg_layers_api_proposal.md / ezdwg_layers_implementation_plan.md for
the design rationale.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Iterator, Mapping, Optional

from . import raw
from . import lineweight as _lineweight


class LayerPropertyNotImplemented(NotImplementedError):
    """Raised when accessing a layer property ezdwg can't decode yet.

    A dedicated subclass rather than a bare NotImplementedError so callers
    can catch it specifically:

        try:
            lw = layer.linetype
        except ezdwg.LayerPropertyNotImplemented:
            lw = None
    """


@dataclass(frozen=True)
class Layer:
    """A single layer's metadata. Immutable snapshot, built once per
    Document access -- not a live view.

    No default values on any field, deliberately -- every Layer should be
    built from real decoded data, never partially constructed.

    Optional fields use None for "known absent / not set in the DWG":
      - true_color / rgb: layer has no true color
      - transparency: no AcCmTransparency XDATA (treat as 0% opaque if needed)
      - standard: no AcAecLayerStandard category / standard-id string
      - description: no AcAecLayerStandard description string
      - linetype / handles: unresolved or not present

    transparency is AutoCAD UI percent as float in [0.0, 90.0]
    (0 = opaque). Derived from EED appid "AcCmTransparency" group 71/1071.

    standard and description come from EED appid "AcAecLayerStandard":
      - standard: 1st 1000-string (category / standard id, e.g. "ÖNorm_A6241-1")
      - description: 2nd 1000-string (Layer Manager description)
    When only one string is present it is treated as standard if it looks like
    a standard tag, otherwise as description (see _aec_standard_from_eed).
    """

    handle: int
    name: str

    color: int                          # ACI index, always positive (sign doesn't encode visibility)
    true_color: Optional[int]           # 0xRRGGBB, or None if this layer has no true color set
    rgb: Optional[tuple[int, int, int]]  # derived from true_color; convenience, same nullability

    on: bool
    frozen: bool
    locked: bool
    frozen_in_new_viewports: bool
    plot: bool

    lineweight_index: int               # raw DWG enum index (0-31ish); ready now
    lineweight_index_out_of_range: bool  # True if lineweight_index needed mod-32
                                           # wraparound to resolve -- see lineweight.py.
                                           # False for every real file decoded so far;
                                           # exists for defensive/malformed-data cases.
    lineweight: int                       # 1/100 mm, or a BYLAYER/BYBLOCK/DEFAULT
                                           # sentinel (see the lineweight module) --
                                           # converted from lineweight_index at
                                           # construction time using whichever table
                                           # (standard, or custom via Document) applied.

    linetype: Optional[str]
    ltype_handle: Optional[int]
    material_handle: Optional[int]
    plotstyle_handle: Optional[int]
    visualstyle_handle: Optional[int]
    owner_handle: Optional[int]
    xdic_handle: Optional[int]
    color_name: Optional[str]
    book_name: Optional[str]
    eed: tuple
    transparency: Optional[float]
    standard: Optional[str]
    description: Optional[str]

    def to_dict(self) -> dict:
        """Flat dict of decoded fields -- safe for csv.DictWriter or
        pandas.DataFrame(...)."""
        return {
            "handle": self.handle,
            "name": self.name,
            "color": self.color,
            "true_color": self.true_color,
            "rgb": self.rgb,
            "on": self.on,
            "frozen": self.frozen,
            "locked": self.locked,
            "frozen_in_new_viewports": self.frozen_in_new_viewports,
            "plot": self.plot,
            "lineweight_index": self.lineweight_index,
            "lineweight_index_out_of_range": self.lineweight_index_out_of_range,
            "lineweight": self.lineweight,
            "linetype": self.linetype,
            "ltype_handle": self.ltype_handle,
            "material_handle": self.material_handle,
            "plotstyle_handle": self.plotstyle_handle,
            "visualstyle_handle": self.visualstyle_handle,
            "owner_handle": self.owner_handle,
            "xdic_handle": self.xdic_handle,
            "color_name": self.color_name,
            "book_name": self.book_name,
            "eed": self.eed,
            "transparency": self.transparency,
            "standard": self.standard,
            "description": self.description,
        }


class LayerTable:
    """Collection of Layers for one Document.

    Deliberately NOT a @dataclass -- this has real collection behavior
    (dict-like lookup, iteration, membership), not just fields to hold.
    """

    def __init__(
        self,
        layers: list[Layer],
        incomplete_handles: Optional[list[int]] = None,
        lineweight_flagged_handles: Optional[list[int]] = None,
    ):
        self._by_name: dict[str, Layer] = {layer.name: layer for layer in layers}
        self._by_handle: dict[int, Layer] = {layer.handle: layer for layer in layers}
        self._ordered: list[Layer] = layers  # file order, i.e. LAYER_CONTROL entry order
        self.incomplete_handles: list[int] = incomplete_handles or []
        self.lineweight_flagged_handles: list[int] = lineweight_flagged_handles or []

    def __len__(self) -> int:
        return len(self._ordered)

    def __iter__(self) -> Iterator[Layer]:
        return iter(self._ordered)

    def __contains__(self, name: str) -> bool:
        return name in self._by_name

    def __getitem__(self, name: str) -> Layer:
        return self._by_name[name]  # KeyError on miss -- standard dict semantics

    def get(self, name: str, default: Optional[Layer] = None) -> Optional[Layer]:
        return self._by_name.get(name, default)

    def __call__(self) -> dict[str, dict]:
        """Legacy dict view of the table (backward-compatible ``doc.layers()``).

        Keys match the older Document.layers() method: ``handle``,
        ``color_index``, ``true_color``, ``linetype``, ``frozen``, ``off``,
        ``locked``, ``plot``, ``lineweight``. Prefer iterating the table or
        ``to_records()`` for new code.
        """
        return {
            layer.name: {
                "handle": layer.handle,
                "color_index": layer.color,
                "true_color": layer.true_color,
                "linetype": layer.linetype,
                "frozen": layer.frozen,
                "off": not layer.on,
                "locked": layer.locked,
                "plot": layer.plot,
                "lineweight": layer.lineweight,
            }
            for layer in self._ordered
        }

    def by_handle(self, handle: int) -> Layer:
        return self._by_handle[handle]  # KeyError on miss

    def replace_layer(self, layer: Layer) -> None:
        """Replace an existing layer in-place by name (and handle).

        Used by layer-state apply to update the in-memory table only.
        Raises KeyError if ``layer.name`` is not in the table.
        """
        if layer.name not in self._by_name:
            raise KeyError(layer.name)
        old = self._by_name[layer.name]
        self._by_name[layer.name] = layer
        if old.handle in self._by_handle:
            del self._by_handle[old.handle]
        self._by_handle[layer.handle] = layer
        for i, existing in enumerate(self._ordered):
            if existing.name == layer.name:
                self._ordered[i] = layer
                break

    def to_records(self) -> list[dict]:
        """One dict per layer (via Layer.to_dict()), in file order --
        ready for csv.DictWriter or pandas.DataFrame(...)."""
        return [layer.to_dict() for layer in self._ordered]


def _rgb_from_true_color(true_color: Optional[int]) -> Optional[tuple[int, int, int]]:
    if true_color is None:
        return None
    return ((true_color >> 16) & 0xFF, (true_color >> 8) & 0xFF, true_color & 0xFF)


def _transparency_from_raw(value: int) -> float:
    """Decode AcCmTransparency XDATA (DXF 1071 / binary EED code 71).

    Encoding (Autodesk / ezdxf): high bits carry the type; the low byte is
    alpha 0–255. AutoCAD UI percent ≈ (255 - alpha) * 100 / 255, clamped
    to the 0–90 range the Layer Manager exposes.

    A raw value of 0 (or missing XDATA) means fully opaque (0%).
    """
    if value == 0:
        return 0.0
    alpha = value & 0xFF
    percent = (255 - alpha) * 100.0 / 255.0
    if percent < 0.0:
        percent = 0.0
    if percent > 90.0:
        percent = 90.0
    return round(percent, 2)


def _aec_standard_from_eed(eed_norm: tuple) -> tuple[Optional[str], Optional[str]]:
    """Pull (standard, description) from AcAecLayerStandard EED.

    DXF layout is two 1000-strings: [standard-or-empty, description].
      - standard: naming standard / category id (e.g. "ÖNorm_A6241-1")
      - description: Layer Manager description text

    When APPID names fail to resolve, accept any EED block with that
    two-string shape. A lone single string is assigned to *standard* when
    it looks like a standard tag (contains "Norm" / "norm" / "Standard"),
    otherwise to *description*.
    """
    for _size, _app_h, app_name, items in eed_norm:
        strs = [v for c, v in items if c == 0 and isinstance(v, str)]
        if not strs:
            continue
        is_std = app_name == "AcAecLayerStandard"
        if app_name is not None and not is_std:
            continue
        if len(strs) >= 2:
            standard = strs[0] if strs[0] else None
            description = strs[1] if strs[1] else None
            return standard, description
        if len(strs) == 1 and strs[0]:
            sole = strs[0]
            # Named appid + one string: prefer description (older files)
            if is_std:
                return None, sole
            # Unresolved appid: classify by content
            lower = sole.lower()
            if "norm" in lower or "standard" in lower:
                return sole, None
            return None, sole
    return None, None


def _transparency_from_eed(eed_norm: tuple) -> Optional[float]:
    """Pull layer transparency percent from AcCmTransparency EED (code 71).

    Returns None when no AcCmTransparency block is present (caller may
    treat that as 0% opaque). Returns 0.0 when the block is present with
    value 0.
    """
    found = False
    value = 0
    for _size, _app_h, app_name, items in eed_norm:
        longs = [v for c, v in items if c == 71 and isinstance(v, int)]
        if not longs:
            continue
        if app_name == "AcCmTransparency" or app_name is None:
            # Prefer named appid; otherwise accept lone 71-blocks (common when
            # APPID handle resolution failed).
            if app_name is None and len(items) != 1:
                continue
            found = True
            value = longs[0]
            if app_name == "AcCmTransparency":
                break
    if not found:
        return None
    return _transparency_from_raw(value)


def build_layer_table(
    path: str, lineweight_table: Optional[Mapping[int, int]] = None
) -> LayerTable:
    """Zip decode_layer_names / decode_layer_colors / decode_layer_flags
    together by handle into a LayerTable. decode_layer_names' handle order
    is used as canonical (see the implementation plan for why); any handle
    missing from colors or flags is dropped from the table rather than
    filled with a placeholder, and recorded in incomplete_handles instead.

    lineweight_table, if given, is a custom index->mm mapping checked
    before the standard table for every layer in this document (see
    ezdwg.lineweight for the lookup semantics). Validated once here rather
    than per-layer.
    """
    if lineweight_table:
        _lineweight.validate_custom_table(lineweight_table)

    names = raw.decode_layer_names(path)
    colors_by_handle = {row[0]: row for row in raw.decode_layer_colors(path)}
    flags_by_handle = {row[0]: row for row in raw.decode_layer_flags(path)}
    handles_by_handle = {row[0]: row for row in raw.decode_layer_handles(path)}
    eed_by_handle = {row[0]: row[1] for row in raw.decode_layer_eed(path)}
    details_by_handle = {}
    try:
        details_by_handle = {row[0]: row for row in raw.decode_layer_color_details(path)}
    except Exception:
        pass

    # decode_layer_states (LibreDWG-aligned flag path) is more reliable for the
    # off/plot bits on some versions (notably R14) than decode_layer_flags.
    # Shape: (handle, frozen, off, frozen_in_new_vp, locked, plot, lineweight_mm).
    states_by_handle: dict[int, tuple] = {}
    try:
        decode_states = getattr(raw, "decode_layer_states", None)
        if callable(decode_states):
            states_by_handle = {int(row[0]): row for row in decode_states(path)}
    except Exception:
        states_by_handle = {}

    # Resolve linetype names from the full LTYPE table when the handle decoder
    # left the name empty (handle present, name missing).
    ltype_name_by_handle: dict[int, str] = {}
    try:
        for row in raw.decode_linetypes(path):
            ltype_name_by_handle[int(row[0])] = row[1]
    except Exception:
        pass

    layers: list[Layer] = []
    incomplete: list[int] = []
    lineweight_flagged: list[int] = []

    for handle, name in names:
        color_row = colors_by_handle.get(handle)
        flags_row = flags_by_handle.get(handle)
        state_row = states_by_handle.get(handle)
        if color_row is None and flags_row is None and state_row is None:
            incomplete.append(handle)
            continue
        if color_row is None:
            incomplete.append(handle)
            continue
        if flags_row is None and state_row is None:
            incomplete.append(handle)
            continue

        _, color_index, true_color = color_row

        lw_mm_from_states = None
        if state_row is not None:
            # Prefer states path for visibility flags.
            _, frozen, off, frozen_in_new, locked, plotflag, lw_mm_from_states = state_row
            frozen = bool(frozen)
            off = bool(off)
            frozen_in_new = bool(frozen_in_new)
            locked = bool(locked)
            plotflag = bool(plotflag)
            if flags_row is not None:
                lineweight_index = flags_row[6]
            else:
                # Fall back to sentinel DEFAULT when only states provided mm.
                lineweight_index = 31
        else:
            _, frozen, off, frozen_in_new, locked, plotflag, lineweight_index = flags_row

        lw_mm, lw_out_of_range = _lineweight._lookup(lineweight_index, lineweight_table)
        # decode_layer_states returns the DXF lineweight (1/100 mm or -3 DEFAULT).
        # Prefer it when the flags path left the index at 0 (common on R14, which
        # has no real lineweight field) and states reports a sentinel/value.
        if lw_mm_from_states is not None and lineweight_index == 0:
            lw_mm = int(lw_mm_from_states)
            lw_out_of_range = False
        if lw_out_of_range:
            lineweight_flagged.append(handle)

        hrow = handles_by_handle.get(handle)
        owner_h = xdic_h = plotstyle_h = material_h = ltype_h = visualstyle_h = None
        linetype_name = None
        if hrow is not None:
            _, owner_h, xdic_h, plotstyle_h, material_h, ltype_h, visualstyle_h, linetype_name = hrow
        if linetype_name is None and ltype_h is not None:
            linetype_name = ltype_name_by_handle.get(ltype_h)

        eed_raw = eed_by_handle.get(handle, [])
        eed_norm = tuple(
            (size, app_h, app_name, tuple((code, val) for code, val in items))
            for size, app_h, app_name, items in eed_raw
        )

        color_name = book_name = None
        drow = details_by_handle.get(handle)
        if drow is not None and len(drow) >= 5:
            color_name = drow[3]
            book_name = drow[4]

        standard, description = _aec_standard_from_eed(eed_norm)
        transparency = _transparency_from_eed(eed_norm)

        layers.append(
            Layer(
                handle=handle,
                name=name,
                color=color_index,
                true_color=true_color,
                rgb=_rgb_from_true_color(true_color),
                on=not off,
                frozen=frozen,
                locked=locked,
                frozen_in_new_viewports=frozen_in_new,
                plot=plotflag,
                lineweight_index=lineweight_index,
                lineweight_index_out_of_range=lw_out_of_range,
                lineweight=lw_mm,
                linetype=linetype_name,
                ltype_handle=ltype_h,
                material_handle=material_h,
                plotstyle_handle=plotstyle_h,
                visualstyle_handle=visualstyle_h,
                owner_handle=owner_h,
                xdic_handle=xdic_h,
                color_name=color_name,
                book_name=book_name,
                eed=eed_norm,
                transparency=transparency,
                standard=standard,
                description=description,
            )
        )

    return LayerTable(
        layers,
        incomplete_handles=incomplete,
        lineweight_flagged_handles=lineweight_flagged,
    )
