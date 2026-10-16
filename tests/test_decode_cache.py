from __future__ import annotations

import shutil
from pathlib import Path

import ezdwg
from ezdwg import raw

ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "test_dwg"


def test_repeated_full_file_passes_are_served_from_the_file_decode_state(tmp_path) -> None:
    first = SAMPLES / "insert_2004.dwg"
    second = SAMPLES / "acadsharp" / "sample_AC1032.dwg"
    path = tmp_path / "doc.dwg"
    shutil.copy(first, path)

    headers = raw.list_object_headers_with_type(str(path))
    assert headers == raw.list_object_headers_with_type(str(first))
    assert raw.list_object_headers_with_type(str(path)) == headers
    assert raw.list_object_headers_with_type(str(path), limit=3) == headers[:3]

    names = raw.decode_block_header_names(str(path))
    assert names == raw.decode_block_header_names(str(first))
    assert raw.decode_block_header_names(str(path)) == names
    assert raw.decode_block_header_names(str(path), limit=1) == names[:1]

    linetypes = raw.decode_entity_linetypes(str(path))
    assert linetypes == raw.decode_entity_linetypes(str(first))
    assert raw.decode_entity_linetypes(str(path)) == linetypes
    assert raw.decode_entity_linetypes(str(path), limit=2) == linetypes[:2]

    # The file is replaced in place (other size and mtime): no stale rows.
    shutil.copy(second, path)
    assert raw.list_object_headers_with_type(str(path)) == raw.list_object_headers_with_type(str(second))
    assert raw.decode_block_header_names(str(path)) == raw.decode_block_header_names(str(second))
    assert raw.decode_entity_linetypes(str(path)) == raw.decode_entity_linetypes(str(second))

    ezdwg.clear_decode_caches()
    assert raw.list_object_headers_with_type(str(path)) == raw.list_object_headers_with_type(str(second))
