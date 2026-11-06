from __future__ import annotations

import ezdwg
import ezdwg.document as document_module


def test_clear_decode_caches_releases_cached_document_data() -> None:
    ezdwg.clear_decode_caches()
    document_module._present_supported_types(None)

    assert document_module._present_supported_types.cache_info().currsize == 1

    ezdwg.clear_decode_caches()

    assert document_module._present_supported_types.cache_info().currsize == 0


def test_cached_tables_share_one_object_per_value_and_keep_signed_zeros() -> None:
    # The per-entity tables hold one object per distinct value (a drawing of
    # 185,000 entities had tens of them), but 0.0 and -0.0 stay apart.
    shared = document_module._shared_values(signed_zeros=True)
    default = shared((0.0, (0.0, 0.0, 1.0), 0.0))
    assert shared((0.0, (0.0, 0.0, 1.0), 0.0)) is default
    mirrored = shared((0.0, (-0.0, 0.0, 1.0), 0.0))
    assert mirrored is not default and str(mirrored[1][0]) == "-0.0"
    plain = document_module._shared_values()
    assert plain((3, 7)) is plain((3, 7))
    assert plain([1, 2]) == [1, 2]  # an unhashable value is kept as it is
