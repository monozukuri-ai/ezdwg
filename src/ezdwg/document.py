from __future__ import annotations

import fnmatch
import hashlib
import math
import re
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Iterator

from . import raw
from .entity import Entity

SUPPORTED_VERSIONS = {"AC1015", "AC1018", "AC1024"}
SUPPORTED_PARSE_VERSIONS = {"AC1015", "AC1018"}
SUPPORTED_ENTITY_TYPES = (
    "LINE",
    "LWPOLYLINE",
    "ARC",
    "CIRCLE",
    "ELLIPSE",
    "POINT",
    "TEXT",
    "MTEXT",
    "DIMENSION",
)

TYPE_ALIASES = {
    "DIM_LINEAR": "DIMENSION",
}


def read(path: str) -> "Document":
    version = raw.detect_version(path)
    if version not in SUPPORTED_VERSIONS:
        raise ValueError(f"unsupported DWG version: {version}")
    decode_path = path
    decode_version = version
    if version == "AC1024":
        decode_path = _convert_ac1024_to_ac1018(path)
        decode_version = raw.detect_version(decode_path)
    if decode_version not in SUPPORTED_PARSE_VERSIONS:
        raise ValueError(f"unsupported DWG parse backend version: {decode_version}")
    return Document(
        path=path,
        version=version,
        decode_path=decode_path,
        decode_version=decode_version,
    )


@dataclass(frozen=True)
class Document:
    path: str
    version: str
    decode_path: str
    decode_version: str

    def modelspace(self) -> "Layout":
        return Layout(self, "MODELSPACE")

    def plot(self, *args, **kwargs):
        from .render import plot

        return plot(self, *args, **kwargs)

    @property
    def raw(self):
        return raw


@dataclass(frozen=True)
class Layout:
    doc: Document
    name: str

    def iter_entities(self, types: str | Iterable[str] | None = None) -> Iterator[Entity]:
        return self.query(types)

    def query(self, types: str | Iterable[str] | None = None) -> Iterator[Entity]:
        type_set = _normalize_types(types)
        for dxftype in type_set:
            yield from self._iter_type(dxftype)

    def plot(self, *args, **kwargs):
        from .render import plot

        return plot(self, *args, **kwargs)

    def _iter_type(self, dxftype: str) -> Iterator[Entity]:
        decode_path = self.doc.decode_path
        if dxftype == "LINE":
            for handle, sx, sy, sz, ex, ey, ez in raw.decode_line_entities(decode_path):
                yield Entity(
                    dxftype="LINE",
                    handle=handle,
                    dxf={
                        "start": (sx, sy, sz),
                        "end": (ex, ey, ez),
                    },
                )
            return

        if dxftype == "ARC":
            for handle, cx, cy, cz, radius, start_angle, end_angle in raw.decode_arc_entities(
                decode_path
            ):
                start_deg = math.degrees(start_angle)
                end_deg = math.degrees(end_angle)
                yield Entity(
                    dxftype="ARC",
                    handle=handle,
                    dxf={
                        "center": (cx, cy, cz),
                        "radius": radius,
                        "start_angle": start_deg,
                        "end_angle": end_deg,
                    },
                )
            return

        if dxftype == "LWPOLYLINE":
            for handle, flags, points in raw.decode_lwpolyline_entities(decode_path):
                points3d = [(x, y, 0.0) for x, y in points]
                yield Entity(
                    dxftype="LWPOLYLINE",
                    handle=handle,
                    dxf={
                        "points": points3d,
                        "flags": flags,
                        "closed": bool(flags & 1),
                    },
                )
            return

        if dxftype == "POINT":
            for handle, x, y, z, angle in raw.decode_point_entities(decode_path):
                yield Entity(
                    dxftype="POINT",
                    handle=handle,
                    dxf={
                        "location": (x, y, z),
                        "x_axis_angle": angle,
                    },
                )
            return

        if dxftype == "CIRCLE":
            for handle, cx, cy, cz, radius in raw.decode_circle_entities(decode_path):
                yield Entity(
                    dxftype="CIRCLE",
                    handle=handle,
                    dxf={
                        "center": (cx, cy, cz),
                        "radius": radius,
                    },
                )
            return

        if dxftype == "ELLIPSE":
            for (
                handle,
                center,
                major_axis,
                extrusion,
                axis_ratio,
                start_angle,
                end_angle,
            ) in raw.decode_ellipse_entities(decode_path):
                yield Entity(
                    dxftype="ELLIPSE",
                    handle=handle,
                    dxf={
                        "center": center,
                        "major_axis": major_axis,
                        "extrusion": extrusion,
                        "axis_ratio": axis_ratio,
                        "start_angle": start_angle,
                        "end_angle": end_angle,
                    },
                )
            return

        if dxftype == "TEXT":
            for (
                handle,
                text,
                insertion,
                alignment,
                extrusion,
                metrics,
                align_flags,
                style_handle,
            ) in raw.decode_text_entities(decode_path):
                thickness, oblique_angle, height, rotation, width_factor = metrics
                generation, horizontal_alignment, vertical_alignment = align_flags
                yield Entity(
                    dxftype="TEXT",
                    handle=handle,
                    dxf={
                        "text": text,
                        "insert": insertion,
                        "align_point": alignment,
                        "extrusion": extrusion,
                        "thickness": thickness,
                        "oblique": math.degrees(oblique_angle),
                        "height": height,
                        "rotation": math.degrees(rotation),
                        "width": width_factor,
                        "text_generation_flag": generation,
                        "halign": horizontal_alignment,
                        "valign": vertical_alignment,
                        "style_handle": style_handle,
                    },
                )
            return

        if dxftype == "MTEXT":
            for (
                handle,
                text,
                insertion,
                extrusion,
                x_axis_dir,
                rect_width,
                text_height,
                attachment,
                drawing_dir,
            ) in raw.decode_mtext_entities(decode_path):
                rotation = math.degrees(math.atan2(x_axis_dir[1], x_axis_dir[0]))
                plain_text = _decode_mtext_plain_text(text)
                yield Entity(
                    dxftype="MTEXT",
                    handle=handle,
                    dxf={
                        "text": plain_text,
                        "raw_text": text,
                        "insert": insertion,
                        "extrusion": extrusion,
                        "text_direction": x_axis_dir,
                        "rotation": rotation,
                        "rect_width": rect_width,
                        "char_height": text_height,
                        "attachment_point": attachment,
                        "drawing_direction": drawing_dir,
                    },
                )
            return

        if dxftype == "DIMENSION":
            for (
                handle,
                user_text,
                point10,
                point13,
                point14,
                text_midpoint,
                insert_point,
                transforms,
                angles,
                common_data,
                handle_data,
            ) in raw.decode_dim_linear_entities(decode_path):
                extrusion, insert_scale = transforms
                text_rotation, horizontal_direction, ext_line_rotation, dim_rotation = angles
                (
                    dim_flags,
                    actual_measurement,
                    attachment_point,
                    line_spacing_style,
                    line_spacing_factor,
                    insert_rotation,
                ) = common_data
                dimstyle_handle, anonymous_block_handle = handle_data
                yield Entity(
                    dxftype="DIMENSION",
                    handle=handle,
                    dxf={
                        "dimtype": "LINEAR",
                        "defpoint": point10,
                        "defpoint2": point13,
                        "defpoint3": point14,
                        "text_midpoint": text_midpoint,
                        "insert": insert_point,
                        "extrusion": extrusion,
                        "insert_scale": insert_scale,
                        "text": user_text,
                        "text_rotation": math.degrees(text_rotation),
                        "horizontal_direction": math.degrees(horizontal_direction),
                        "oblique_angle": math.degrees(ext_line_rotation),
                        "angle": math.degrees(dim_rotation),
                        "dim_flags": dim_flags,
                        "actual_measurement": actual_measurement,
                        "attachment_point": attachment_point,
                        "line_spacing_style": line_spacing_style,
                        "line_spacing_factor": line_spacing_factor,
                        "insert_rotation": math.degrees(insert_rotation),
                        "dimstyle_handle": dimstyle_handle,
                        "anonymous_block_handle": anonymous_block_handle,
                    },
                )
            return

        raise ValueError(
            f"unsupported entity type: {dxftype}. "
            "Supported types: LINE, LWPOLYLINE, ARC, CIRCLE, ELLIPSE, POINT, TEXT, MTEXT, DIMENSION"
        )


def _convert_ac1024_to_ac1018(path: str) -> str:
    source = Path(path).resolve()
    if not source.exists():
        raise FileNotFoundError(path)

    converter = shutil.which("ODAFileConverter")
    xvfb_run = shutil.which("xvfb-run")
    if converter is None or xvfb_run is None:
        raise ValueError(
            "AC1024 reading requires ODAFileConverter and xvfb-run for compatibility conversion."
        )

    stat = source.stat()
    digest = hashlib.sha1(
        f"{source}:{stat.st_mtime_ns}:{stat.st_size}".encode("utf-8")
    ).hexdigest()
    cache_dir = Path(tempfile.gettempdir()) / "ezdwg_ac1024_cache"
    cache_dir.mkdir(parents=True, exist_ok=True)
    converted_path = cache_dir / f"{digest}.dwg"
    if converted_path.exists():
        return str(converted_path)

    with tempfile.TemporaryDirectory(dir=cache_dir) as workdir:
        in_dir = Path(workdir) / "in"
        out_dir = Path(workdir) / "out"
        in_dir.mkdir(parents=True, exist_ok=True)
        out_dir.mkdir(parents=True, exist_ok=True)

        shutil.copy2(source, in_dir / "source.DWG")
        cmd = [
            xvfb_run,
            "-a",
            converter,
            str(in_dir),
            str(out_dir),
            "ACAD2004",
            "DWG",
            "0",
            "1",
            "*.DWG",
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
        if proc.returncode != 0:
            message = (proc.stderr or proc.stdout or "").strip()
            raise ValueError(f"AC1024 conversion failed: {message}")

        candidates = sorted(out_dir.glob("*.dwg")) + sorted(out_dir.glob("*.DWG"))
        if not candidates:
            raise ValueError("AC1024 conversion produced no output DWG")
        shutil.copy2(candidates[0], converted_path)

    return str(converted_path)


def _decode_mtext_plain_text(value: str) -> str:
    if not value:
        return ""

    out: list[str] = []
    i = 0
    n = len(value)
    while i < n:
        ch = value[i]

        if ch in "{}":
            i += 1
            continue
        if ch != "\\":
            out.append(ch)
            i += 1
            continue
        if i + 1 >= n:
            out.append("\\")
            break

        code = value[i + 1]
        if code in "\\{}":
            out.append(code)
            i += 2
            continue
        if code in {"P", "X"}:
            out.append("\n")
            i += 2
            continue
        if code == "~":
            out.append(" ")
            i += 2
            continue
        if code in {"L", "l", "O", "o", "K", "k"}:
            i += 2
            continue
        if code in {"U", "u"} and i + 6 < n and value[i + 2] == "+":
            hex_digits = value[i + 3 : i + 7]
            if all(c in "0123456789abcdefABCDEF" for c in hex_digits):
                out.append(chr(int(hex_digits, 16)))
                i += 7
                continue
        if code == "S":
            i += 2
            stacked: list[str] = []
            while i < n and value[i] != ";":
                token = value[i]
                if token in {"#", "^"}:
                    token = "/"
                stacked.append(token)
                i += 1
            if i < n and value[i] == ";":
                i += 1
            out.append("".join(stacked))
            continue
        if code in {"A", "C", "c", "F", "f", "H", "h", "Q", "q", "T", "t", "W", "w", "p"}:
            i += 2
            while i < n and value[i] != ";":
                i += 1
            if i < n and value[i] == ";":
                i += 1
            continue

        out.append(code)
        i += 2

    return "".join(out)


def _normalize_types(types: str | Iterable[str] | None) -> list[str]:
    if types is None:
        return list(SUPPORTED_ENTITY_TYPES)
    if isinstance(types, str):
        tokens = re.split(r"[,\s]+", types.strip())
    else:
        tokens = list(types)

    normalized = [token.strip().upper() for token in tokens if token and token.strip()]
    normalized = [TYPE_ALIASES.get(token, token) for token in normalized]
    if not normalized:
        return list(SUPPORTED_ENTITY_TYPES)

    if any(token in {"*", "ALL"} for token in normalized):
        return list(SUPPORTED_ENTITY_TYPES)

    selected: list[str] = []
    seen = set()

    for token in normalized:
        if any(ch in token for ch in "*?[]"):
            matches = [
                name for name in SUPPORTED_ENTITY_TYPES if fnmatch.fnmatchcase(name, token)
            ]
            if not matches:
                continue
            for name in matches:
                if name not in seen:
                    seen.add(name)
                    selected.append(name)
            continue

        if token in SUPPORTED_ENTITY_TYPES:
            if token not in seen:
                seen.add(token)
                selected.append(token)

    return selected
