from typing import Sequence

from .convert import ConvertResult, WriteResult, to_dwg, to_dxf
from .document import Document, Layout, clear_decode_caches, read
from .entity import Entity
from .layers import Layer, LayerTable, LayerPropertyNotImplemented
from .layer_states import (
    LayerState,
    LayerStateDiff,
    LayerStateDiffEntry,
    LayerStateEntry,
    LayerStateMasks,
    LayerStateTable,
    build_layer_state_table,
)
from .layer_filters import (
    LayerFilter,
    LayerFilterTable,
    LayerFilterTree,
    build_layer_filter_table,
    build_layer_filter_tree,
)
from . import lineweight
from .lineweight import lineweight_to_mm
from . import raw
from .graph import DocumentGraph, HeaderHandles, ObjectEdge, read_graph
from .render import plot

__all__ = [
    "read",
    "clear_decode_caches",
    "read_graph",
    "Document",
    "DocumentGraph",
    "HeaderHandles",
    "ObjectEdge",
    "Layout",
    "Entity",
    "Layer",
    "LayerTable",
    "LayerPropertyNotImplemented",
    "LayerState",
    "LayerStateDiff",
    "LayerStateDiffEntry",
    "LayerStateEntry",
    "LayerStateMasks",
    "LayerStateTable",
    "build_layer_state_table",
    "LayerFilter",
    "LayerFilterTable",
    "LayerFilterTree",
    "build_layer_filter_table",
    "build_layer_filter_tree",
    "lineweight",
    "lineweight_to_mm",
    "plot",
    "to_dxf",
    "to_dwg",
    "ConvertResult",
    "WriteResult",
    "raw",
]


def main(argv: Sequence[str] | None = None) -> int:
    from ezdwg.cli import main as cli_main

    return cli_main(argv)
