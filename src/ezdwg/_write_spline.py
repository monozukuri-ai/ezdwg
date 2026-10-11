"""Lossless AC1015 spline representations; no fit/control conversion or tessellation."""

import itertools

FIELDS = {
    "scenario",
    "degree",
    "rational",
    "closed",
    "periodic",
    "fit_tolerance",
    "knot_tolerance",
    "ctrl_tolerance",
    "start_tangent",
    "end_tangent",
    "knots",
    "control_points",
    "weights",
    "fit_points",
}


def validate_spline(d, number, point):
    degree = d.get("degree", 3)
    if isinstance(degree, bool) or not isinstance(degree, int) or not 1 <= degree <= 10:
        raise ValueError("SPLINE degree must be in 1..10")
    scenario = d.get("scenario", 1)
    if (
        isinstance(scenario, bool)
        or not isinstance(scenario, int)
        or scenario not in (1, 2)
    ):
        raise ValueError("SPLINE scenario must be 1 or 2")
    for key in ("rational", "closed", "periodic"):
        if key in d and not isinstance(d[key], bool):
            raise ValueError(f"SPLINE {key} must be a boolean")
    knots = [number(v) for v in d.get("knots", [])]
    controls = [point(v) for v in d.get("control_points", [])]
    weights = [number(v) for v in d.get("weights", [])]
    fit = [point(v) for v in d.get("fit_points", [])]

    def tolerance(key, default):
        value = d.get(key)
        if number(default if value is None else value) < 0:
            raise ValueError("SPLINE tolerance must be finite and nonnegative")

    if scenario == 1:
        if fit or any(
            d.get(k) is not None
            for k in ("fit_tolerance", "start_tangent", "end_tangent")
        ):
            raise ValueError("control SPLINE cannot also store fit data")
        tolerance("knot_tolerance", 1e-7)
        tolerance("ctrl_tolerance", 1e-7)
        n, p = len(controls), degree
        if n <= p or len(knots) != n + p + 1 or len(knots) > 1_000_000:
            raise ValueError("SPLINE knot/control count does not match degree")
        if any(a > b for a, b in itertools.pairwise(knots)) or knots[p] >= knots[n]:
            raise ValueError(
                "SPLINE knots must be nondecreasing with a nonempty domain"
            )
        period = number(knots[n] - knots[p])
        if any(a == b for a, b in zip(knots, knots[p + 1 :])):
            raise ValueError("SPLINE knot multiplicity exceeds degree + 1")
        if d.get("rational", False):
            if len(weights) != n or any(v <= 0 for v in weights):
                raise ValueError(
                    "rational SPLINE requires one positive weight per control point"
                )
        elif weights:
            raise ValueError("non-rational SPLINE cannot store weights")
        if d.get("periodic", False) and not d.get("closed", False):
            raise ValueError("periodic SPLINE requires closed")
        if d.get("closed", False):
            clamped = (
                controls[0] == controls[-1]
                and knots[0] == knots[p]
                and knots[n] == knots[n + p]
            )
            wrapped = (
                controls[:p] == controls[-p:]
                and (not weights or weights[:p] == weights[-p:])
                and all(
                    abs((knots[i + n - p] - knots[i]) / period - 1) <= 1e-9
                    for i in range(2 * p + 1)
                )
            )
            if not (clamped or wrapped):
                raise ValueError(
                    "closed SPLINE requires clamped matching endpoints or periodic wrapped data"
                )
    else:
        if (
            controls
            or knots
            or weights
            or any(d.get(k, False) for k in ("rational", "closed", "periodic"))
            or any(d.get(k) is not None for k in ("knot_tolerance", "ctrl_tolerance"))
        ):
            raise ValueError(
                "fit SPLINE cannot also store control data or closure flags"
            )
        if degree != 3 or not 2 <= len(fit) <= 1_000_000:
            raise ValueError("fit SPLINE requires degree 3 and at least two points")
        tolerance("fit_tolerance", 1e-10)
        for key in ("start_tangent", "end_tangent"):
            point(d.get(key) if d.get(key) is not None else (0, 0, 0))
        if any(a == b for a, b in itertools.pairwise(fit)) or fit[0] == fit[-1]:
            raise ValueError(
                "fit SPLINE requires distinct adjacent points and open endpoints"
            )
