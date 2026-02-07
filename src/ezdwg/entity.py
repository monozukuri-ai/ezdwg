from __future__ import annotations

from dataclasses import dataclass
from typing import Any

Point3D = tuple[float, float, float]


@dataclass(frozen=True)
class Entity:
    dxftype: str
    handle: int
    dxf: dict[str, Any]

    def to_points(self) -> list[Point3D]:
        if self.dxftype == "LINE":
            return [self.dxf["start"], self.dxf["end"]]
        if self.dxftype == "LWPOLYLINE":
            return list(self.dxf.get("points", []))
        if self.dxftype == "POINT":
            return [self.dxf["location"]]
        if self.dxftype in {"TEXT", "MTEXT"}:
            return [self.dxf["insert"]]
        if self.dxftype == "DIMENSION":
            points = []
            if "defpoint2" in self.dxf:
                points.append(self.dxf["defpoint2"])
            if "defpoint3" in self.dxf:
                points.append(self.dxf["defpoint3"])
            if points:
                return points
            return [self.dxf["text_midpoint"]]
        raise NotImplementedError(f"to_points is not supported for {self.dxftype}")
