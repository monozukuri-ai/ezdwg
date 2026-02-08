from typing import Sequence

from .convert import ConvertResult, to_dxf
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
    "ConvertResult",
    "raw",
]


def main(argv: Sequence[str] | None = None) -> int:
    from ezdwg.cli import main as cli_main

    return cli_main(argv)
