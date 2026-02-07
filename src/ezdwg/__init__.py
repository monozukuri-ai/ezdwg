from typing import Sequence

from ezdwg.document import Document, Layout, read
from ezdwg.entity import Entity
from ezdwg import raw
from ezdwg.render import plot

__all__ = [
    "read",
    "Document",
    "Layout",
    "Entity",
    "plot",
    "raw",
]


def main(argv: Sequence[str] | None = None) -> int:
    from ezdwg.cli import main as cli_main

    return cli_main(argv)
