from __future__ import annotations

import ezdwg
import ezdwg.document as document_module


def test_clear_decode_caches_releases_cached_document_data() -> None:
    ezdwg.clear_decode_caches()
    document_module._present_supported_types(None)

    assert document_module._present_supported_types.cache_info().currsize == 1

    ezdwg.clear_decode_caches()

    assert document_module._present_supported_types.cache_info().currsize == 0
