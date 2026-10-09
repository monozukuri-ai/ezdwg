"""
High-level layer-filter API for ezdwg.

Two storage families under the LAYER table extension dictionary:

1. **Property filters** — ``ACAD_LAYERFILTERS`` XRECORDs (legacy group-1
   patterns). Flat list; see ``build_layer_filter_table``.

2. **Nested AcLy filters** — ``ACLYDICTIONARY`` XRECORDs marked
   ``AcLyLayerFilter`` with group 300 (name) / 301 (expression). Children
   hang off each filter's extension dictionary → nested ``ACLYDICTIONARY``.
   See ``build_layer_filter_tree``.

XRECORD group schema (property filters, AC1032)
---------------------------------------------------------
  GROUP_NAME (1)          filter display name (first string)
  GROUP_NAME_PATTERN (1)  layer-name wildcard (second string, e.g. ``AR*``)
  GROUP_COLOR_PATTERN (1) color filter (``*`` = any)
  GROUP_LTYPE_PATTERN (1) linetype filter
  GROUP_FLAGS (70)        filter flags (RS)
  GROUP_LW_PATTERN (1)    lineweight filter
  GROUP_PLOT_PATTERN (1)  plot-style filter

AcLy nested schema
------------------
  GROUP_MARKER (1)        ``AcLyLayerFilter``
  GROUP_VERSION (90)      integer (typically 1)
  GROUP_NAME (300)        display name
  GROUP_EXPRESSION (301)  filter expression string

Filter counts depend on the drawing.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Iterator, Mapping, Optional, Sequence

from . import raw

GROUP_STRING = 1
GROUP_FLAGS = 70
GROUP_EXPRESSION = 301
GROUP_NAME_ALT = 300


@dataclass(frozen=True)
class LayerFilter:
    """One named layer filter (property or AcLy)."""

    name: str
    handle: Optional[int]
    expression: Optional[str]
    flags: Optional[int] = None
    name_pattern: Optional[str] = None
    color_pattern: Optional[str] = None
    linetype_pattern: Optional[str] = None
    lineweight_pattern: Optional[str] = None
    plotstyle_pattern: Optional[str] = None
    kind: str = "property"  # "property" | "acly"
    id_key: Optional[str] = None
    depth: int = 0
    groups: tuple[tuple[int, Any], ...] = ()
    children: tuple["LayerFilter", ...] = ()

    def matches_name(self, layer_name: str) -> bool:
        """Return True if *layer_name* matches this filter's name pattern.

        Only the name pattern is evaluated for property filters. For AcLy
        filters with an expression, a simple ``NAME == "pat"`` form is
        recognised; otherwise a missing/``*`` pattern matches everything.
        """
        pat = self.name_pattern
        if pat is None and self.expression:
            pat = _name_pattern_from_expression(self.expression)
        if pat is None or pat == "*":
            return True
        return _wildcard_match(pat, layer_name)

    def walk(self) -> Iterator["LayerFilter"]:
        """Pre-order walk of this node and descendants."""
        yield self
        for c in self.children:
            yield from c.walk()

    def find(self, name: str) -> Optional["LayerFilter"]:
        for node in self.walk():
            if node.name == name:
                return node
        return None


@dataclass(frozen=True)
class LayerFilterTable:
    """Ordered collection of property layer filters for one drawing."""

    filters: tuple[LayerFilter, ...]

    def __len__(self) -> int:
        return len(self.filters)

    def __iter__(self) -> Iterator[LayerFilter]:
        return iter(self.filters)

    def __contains__(self, name: str) -> bool:
        return any(f.name == name for f in self.filters)

    def get(self, name: str) -> Optional[LayerFilter]:
        for f in self.filters:
            if f.name == name:
                return f
        return None

    def names(self) -> tuple[str, ...]:
        return tuple(f.name for f in self.filters)

    def as_mapping(self) -> Mapping[str, LayerFilter]:
        return {f.name: f for f in self.filters}


@dataclass(frozen=True)
class LayerFilterTree:
    """Nested AcLy filter tree (roots under ACLYDICTIONARY)."""

    roots: tuple[LayerFilter, ...]

    def __len__(self) -> int:
        return sum(1 for _ in self.walk())

    def __iter__(self) -> Iterator[LayerFilter]:
        return iter(self.roots)

    def walk(self) -> Iterator[LayerFilter]:
        for r in self.roots:
            yield from r.walk()

    def get(self, name: str) -> Optional[LayerFilter]:
        for node in self.walk():
            if node.name == name:
                return node
        return None

    def names(self) -> tuple[str, ...]:
        return tuple(n.name for n in self.walk())

    @property
    def root_count(self) -> int:
        return len(self.roots)


def _wildcard_match(pattern: str, text: str) -> bool:
    import re

    parts: list[str] = []
    i = 0
    while i < len(pattern):
        c = pattern[i]
        if c == "*":
            parts.append(".*")
        elif c == "?":
            parts.append(".")
        else:
            parts.append(re.escape(c))
        i += 1
    rx = re.compile("^" + "".join(parts) + "$", re.IGNORECASE)
    return rx.match(text) is not None


def _name_pattern_from_expression(expr: str) -> Optional[str]:
    """Extract a simple NAME pattern from ``( NAME == "pat" )`` style expr."""
    import re

    m = re.search(r'NAME\s*==\s*"([^"]+)"', expr, re.IGNORECASE)
    if m:
        return m.group(1)
    m = re.search(r"NAME\s*==\s*'([^']+)'", expr, re.IGNORECASE)
    if m:
        return m.group(1)
    return None


def _strings_from_groups(groups: Sequence[tuple[int, Any]]) -> list[str]:
    out: list[str] = []
    for code, val in groups:
        if code == GROUP_STRING and isinstance(val, str):
            out.append(val)
    return out


def _filter_from_property_row(row: tuple) -> LayerFilter:
    name = row[0]
    handle = int(row[1]) if row[1] else None
    expression = row[2]
    flags = row[3]
    groups = tuple((int(c), v) for c, v in (row[4] or ()))
    strings = _strings_from_groups(groups)
    name_pat = strings[1] if len(strings) > 1 else None
    color_pat = strings[2] if len(strings) > 2 else None
    ltype_pat = strings[3] if len(strings) > 3 else None
    lw_pat = strings[4] if len(strings) > 4 else None
    plot_pat = strings[5] if len(strings) > 5 else None
    return LayerFilter(
        name=name,
        handle=handle,
        expression=expression,
        flags=int(flags) if flags is not None else None,
        name_pattern=name_pat,
        color_pattern=color_pat,
        linetype_pattern=ltype_pat,
        lineweight_pattern=lw_pat,
        plotstyle_pattern=plot_pat,
        kind="property",
        groups=groups,
    )


def build_layer_filter_table(path: str) -> LayerFilterTable:
    """Decode all ACAD_LAYERFILTERS property filters for *path*."""
    decode = getattr(raw, "decode_layer_filter_xrecords", None)
    if decode is None:
        return LayerFilterTable(filters=())
    rows = decode(path) or ()
    filters = tuple(_filter_from_property_row(r) for r in rows)
    return LayerFilterTable(filters=filters)


def decode_layer_filter_names(path: str) -> list[str]:
    """Convenience wrapper around the native property-filter name list."""
    fn = getattr(raw, "decode_layer_filter_names", None)
    if fn is None:
        return []
    return list(fn(path) or ())


def _build_tree_from_flat(nodes: Sequence[tuple]) -> tuple[LayerFilter, ...]:
    """Rebuild parent→children from pre-order flat list with depth/child_count.

    Node shape: (name, handle, expression, id_key, depth, child_count, groups)
    """
    if not nodes:
        return ()

    # Convert to mutable builders then freeze.
    class _B:
        __slots__ = (
            "name",
            "handle",
            "expression",
            "id_key",
            "depth",
            "child_count",
            "groups",
            "children",
        )

        def __init__(self, row: tuple) -> None:
            self.name = row[0]
            self.handle = int(row[1]) if row[1] else None
            self.expression = row[2]
            self.id_key = row[3] or None
            self.depth = int(row[4])
            self.child_count = int(row[5])
            self.groups = tuple((int(c), v) for c, v in (row[6] or ()))
            self.children: list[_B] = []

        def freeze(self) -> LayerFilter:
            name_pat = (
                _name_pattern_from_expression(self.expression or "")
                if self.expression
                else None
            )
            return LayerFilter(
                name=self.name,
                handle=self.handle,
                expression=self.expression,
                kind="acly",
                id_key=self.id_key,
                depth=self.depth,
                name_pattern=name_pat,
                groups=self.groups,
                children=tuple(c.freeze() for c in self.children),
            )

    builders = [_B(r) for r in nodes]
    stack: list[_B] = []
    roots: list[_B] = []
    for b in builders:
        while stack and stack[-1].depth >= b.depth:
            stack.pop()
        if not stack:
            roots.append(b)
        else:
            stack[-1].children.append(b)
        stack.append(b)
    return tuple(r.freeze() for r in roots)


def build_layer_filter_tree(path: str) -> LayerFilterTree:
    """Decode nested AcLy filters under ACLYDICTIONARY."""
    decode = getattr(raw, "decode_layer_filter_tree", None)
    if decode is None:
        return LayerFilterTree(roots=())
    flat = decode(path) or ()
    roots = _build_tree_from_flat(flat)
    return LayerFilterTree(roots=roots)
