from typing import Sequence

from .convert import ConvertResult, WriteResult, to_dwg, to_dxf
from .document import Document, Layout, read
from .entity import Entity
from . import raw
from .render import plot

__all__ = [
    "read",
    "Document",
    "Layout",
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
