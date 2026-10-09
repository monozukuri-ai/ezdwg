"""
Lineweight enum -> mm conversion for ezdwg.

The standard table is ported directly from LibreDWG's dxf_cvt_lweight
(src/dwg.c) rather than re-derived -- see ezdwg_lineweight_proposal.md for
the cross-check against values already observed in this project's own test
fixtures (index 9 -> 35 == Layer_lw_035's 0.35mm, index 11 -> 50 ==
Layer_lw_050's 0.50mm, index 31 -> -3 == the "Default" sentinel that
explains why nearly every layer tested this session showed
lineweight_index=31).
"""

from __future__ import annotations

import logging
from typing import Mapping, Optional

_logger = logging.getLogger(__name__)

# Sentinel values, matching the DXF/ezdxf convention for these three special
# lineweight states (not a real thickness).
BYLAYER = -1
BYBLOCK = -2
DEFAULT = -3

# Index -> hundredths of a millimeter. 24-28 are reserved/unused in the
# format as of this writing (map to 0); 29-31 are the sentinels above.
STANDARD_TABLE: dict[int, int] = {
    0: 0, 1: 5, 2: 9, 3: 13, 4: 15, 5: 18, 6: 20, 7: 25,
    8: 30, 9: 35, 10: 40, 11: 50, 12: 53, 13: 60, 14: 70, 15: 80,
    16: 90, 17: 100, 18: 106, 19: 120, 20: 140, 21: 158, 22: 200, 23: 211,
    24: 0, 25: 0, 26: 0, 27: 0, 28: 0,
    29: BYLAYER, 30: BYBLOCK, 31: DEFAULT,
}


def validate_custom_table(custom_table: Mapping[int, int]) -> None:
    """Warn (not raise) about keys that can't correspond to real file data --
    the raw index is a 5-bit field, so any key outside 0-31 in a
    caller-supplied table can only be a mistake, not something ezdwg will
    ever actually look up from a decoded file."""
    bad_keys = [k for k in custom_table if not (0 <= k <= 31)]
    if bad_keys:
        _logger.warning(
            "custom lineweight table has keys outside the valid 0-31 range: %s "
            "-- these can never be matched against a decoded lineweight_index",
            sorted(bad_keys),
        )


def _lookup(index: int, custom_table: Optional[Mapping[int, int]]) -> tuple[int, bool]:
    """Returns (mm_value, was_out_of_range). Internal -- callers that also
    need the out-of-range signal (build_layer_table, to flag a Layer) use
    this directly; lineweight_to_mm wraps it for the common case where only
    the value is wanted."""
    if custom_table and index in custom_table:
        return custom_table[index], False
    if 0 <= index <= 31:
        return STANDARD_TABLE[index], False

    wrapped = index % 32
    _logger.error(
        "lineweight index %d is out of the valid 0-31 range -- wrapping to "
        "%d (mod 32) and flagging the affected layer",
        index, wrapped,
    )
    value = custom_table[wrapped] if custom_table and wrapped in custom_table else STANDARD_TABLE[wrapped]
    return value, True


def lineweight_to_mm(index: int, custom_table: Optional[Mapping[int, int]] = None) -> int:
    """Convert a raw DWG lineweight enum index to hundredths of a
    millimeter (matching ezdxf's `layer.dxf.lineweight` convention), or one
    of the BYLAYER/BYBLOCK/DEFAULT sentinels above.

    custom_table, if given, is checked first; any index it doesn't cover
    falls back to the standard table -- a partial override table works the
    same as a complete one, no separate "merge" flag needed.

    An index outside 0-31 (which the format's 5-bit field should never
    actually produce from real decoded data, but this function doesn't
    assume that holds) is wrapped via `% 32`, matching LibreDWG's own
    defensive handling, and logged as an error rather than raising --
    consistent with ezdwg's existing best-effort philosophy elsewhere.
    Callers that need to know whether wrapping happened (to flag a Layer
    object, for instance) should call `_lookup` directly instead.
    """
    value, _ = _lookup(index, custom_table)
    return value
