from typing import Sequence

from .convert import ConvertResult, WriteResult, to_dwg, to_dxf
from .document import Document, Layout, clear_decode_caches, read
from .layers import Layer, LayerTable, LayerPropertyNotImplemented, build_layer_table
from . import lineweight
from .lineweight import lineweight_to_mm
from .entity import Entity
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
    "Layer",
    "LayerTable",
    "LayerPropertyNotImplemented",
    "build_layer_table",
    "lineweight",
    "lineweight_to_mm",
    "Entity",
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
