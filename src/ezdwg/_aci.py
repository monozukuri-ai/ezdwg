"""AutoCAD Color Index (ACI) palette and true-colour rounding for AC1015.

R2000 stores entity and layer colours as ACI only. True colours (``0xRRGGBB``)
are rounded to the nearest of the 255 palette entries before writing; the
writer reports that approximation in ``WriteResult.diagnostics``.
"""

from __future__ import annotations

from functools import lru_cache

_LEVELS = (255, 189, 129, 104, 79)
_GRAYS = (51, 91, 132, 173, 214, 255)
_BASE = {
    1: (255, 0, 0),
    2: (255, 255, 0),
    3: (0, 255, 0),
    4: (0, 255, 255),
    5: (0, 0, 255),
    6: (255, 0, 255),
    7: (255, 255, 255),
    8: (128, 128, 128),
    9: (192, 192, 192),
}


@lru_cache(maxsize=1)
def aci_palette() -> dict[int, tuple[int, int, int]]:
    """RGB of ACI 1..255 (7 is the foreground colour, listed as white)."""
    table = dict(_BASE)
    for index in range(10, 250):
        hue = ((index - 10) // 10) * 15.0
        sector, fraction = divmod(hue / 60.0, 1.0)
        sector = int(sector) % 6
        up, down = int(255 * fraction), int(255 * (1.0 - fraction))
        full = (
            (255, up, 0),
            (down, 255, 0),
            (0, 255, up),
            (0, down, 255),
            (up, 0, 255),
            (255, 0, down),
        )[sector]
        level = _LEVELS[(index % 10) // 2]
        rgb = tuple(int(c * level / 255) for c in full)
        if index % 2:
            rgb = tuple(int(c + (level - c) * 2 / 3) for c in rgb)
        table[index] = rgb  # type: ignore[assignment]
    for index, gray in zip(range(250, 256), _GRAYS):
        table[index] = (gray, gray, gray)
    return table


def true_color_to_rgb(value: int) -> tuple[int, int, int]:
    """Split a ``0xRRGGBB`` integer (DXF group 420 layout) into channels."""
    if isinstance(value, bool) or not isinstance(value, int) or not 0 <= value <= 0xFFFFFF:
        raise ValueError("true_color must be an integer 0x000000..0xFFFFFF")
    return (value >> 16) & 0xFF, (value >> 8) & 0xFF, value & 0xFF


def nearest_aci(value: int | tuple[int, int, int]) -> int:
    """Nearest ACI (1..255) by RGB distance.

    Pure black and pure white both map to 7 (the foreground colour of AutoCAD
    displays); the search otherwise skips 7 so that dark greys round to the
    grey entries and not to the background-dependent foreground colour.
    """
    rgb = true_color_to_rgb(value) if isinstance(value, int) else tuple(value)
    if len(rgb) != 3 or any(
        isinstance(c, bool) or not isinstance(c, int) or not 0 <= c <= 255 for c in rgb
    ):
        raise ValueError("RGB components must be integers 0..255")
    if rgb in ((0, 0, 0), (255, 255, 255)):
        return 7
    palette = aci_palette()
    return min(
        (index for index in palette if index != 7),
        key=lambda index: sum((a - b) ** 2 for a, b in zip(rgb, palette[index])),
    )


def describe_approximation(value: int, index: int) -> str:
    r, g, b = true_color_to_rgb(value)
    pr, pg, pb = aci_palette()[index]
    return f"true color #{r:02X}{g:02X}{b:02X} is written as ACI {index} (#{pr:02X}{pg:02X}{pb:02X})"
