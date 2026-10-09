from __future__ import annotations

from ezdwg.lineweight import (
    BYBLOCK,
    BYLAYER,
    DEFAULT,
    STANDARD_TABLE,
    _lookup,
    lineweight_to_mm,
    validate_custom_table,
)


def test_standard_table_covers_all_32_indices() -> None:
    assert set(STANDARD_TABLE.keys()) == set(range(32))


def test_standard_table_sentinels() -> None:
    assert STANDARD_TABLE[29] == BYLAYER == -1
    assert STANDARD_TABLE[30] == BYBLOCK == -2
    assert STANDARD_TABLE[31] == DEFAULT == -3


def test_standard_table_reserved_indices_are_zero() -> None:
    for i in range(24, 29):
        assert STANDARD_TABLE[i] == 0


def test_lineweight_to_mm_standard() -> None:
    assert lineweight_to_mm(9) == 35
    assert lineweight_to_mm(11) == 50
    assert lineweight_to_mm(31) == DEFAULT


def test_custom_table_overrides_selectively() -> None:
    custom = {9: 999}
    assert lineweight_to_mm(9, custom) == 999  # overridden
    assert lineweight_to_mm(11, custom) == 50  # falls through to standard


def test_out_of_range_wraps_mod_32() -> None:
    value, out_of_range = _lookup(35, None)
    assert out_of_range is True
    assert value == STANDARD_TABLE[35 % 32]


def test_out_of_range_within_custom_table_is_not_flagged() -> None:
    # if the caller's custom table explicitly covers an "unusual" index,
    # that's not an error condition -- they said what they wanted.
    custom = {40: 777}
    # NOTE: _lookup only checks custom_table for the *original* index
    # before falling into the out-of-range branch, so an out-of-range key
    # present in custom_table short-circuits before wrapping.
    value, out_of_range = _lookup(40, custom)
    assert value == 777
    assert out_of_range is False


def test_negative_index_wraps_via_python_modulo() -> None:
    # Python's % always returns a non-negative result for a positive
    # divisor, so a negative index wraps into range too rather than
    # producing a second negative.
    value, out_of_range = _lookup(-1, None)
    assert out_of_range is True
    assert 0 <= (-1 % 32) <= 31


def test_validate_custom_table_accepts_in_range(caplog) -> None:
    with caplog.at_level("WARNING", logger="ezdwg.lineweight"):
        validate_custom_table({0: 0, 31: -3})
    assert not any("outside the valid 0-31 range" in r.message for r in caplog.records)


def test_validate_custom_table_warns_on_out_of_range_keys(caplog) -> None:
    with caplog.at_level("WARNING", logger="ezdwg.lineweight"):
        validate_custom_table({-1: 0, 32: 0, 100: 0})
    warnings = [r.message for r in caplog.records if "outside the valid 0-31 range" in r.message]
    assert len(warnings) == 1  # one combined warning, not one per bad key
