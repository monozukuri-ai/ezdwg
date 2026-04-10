//! Shared helpers for dimension entity decoders.
//!
//! The R2010+ dimension variants and plausibility scoring heuristics are
//! identical across `dim_linear`, `dim_diameter`, and `dim_radius`, so they
//! live here to avoid drift between copies.

use crate::entities::dim_linear::DimLinearEntity;

#[derive(Clone, Copy)]
pub(crate) struct R2010PlusVariant {
    pub(crate) has_dimension_version: bool,
    pub(crate) has_user_text: bool,
    pub(crate) extrusion_is_be: bool,
}

pub(crate) const R2010_PLUS_VARIANTS: [R2010PlusVariant; 8] = [
    R2010PlusVariant {
        has_dimension_version: true,
        has_user_text: true,
        extrusion_is_be: false,
    },
    R2010PlusVariant {
        has_dimension_version: true,
        has_user_text: false,
        extrusion_is_be: false,
    },
    R2010PlusVariant {
        has_dimension_version: false,
        has_user_text: true,
        extrusion_is_be: false,
    },
    R2010PlusVariant {
        has_dimension_version: false,
        has_user_text: false,
        extrusion_is_be: false,
    },
    R2010PlusVariant {
        has_dimension_version: true,
        has_user_text: true,
        extrusion_is_be: true,
    },
    R2010PlusVariant {
        has_dimension_version: true,
        has_user_text: false,
        extrusion_is_be: true,
    },
    R2010PlusVariant {
        has_dimension_version: false,
        has_user_text: true,
        extrusion_is_be: true,
    },
    R2010PlusVariant {
        has_dimension_version: false,
        has_user_text: false,
        extrusion_is_be: true,
    },
];

pub(crate) fn plausibility_score(entity: &DimLinearEntity) -> u64 {
    let mut score = 0u64;
    let common = &entity.common;

    for pt in [
        entity.point10,
        entity.point13,
        entity.point14,
        common.text_midpoint,
    ] {
        score = score.saturating_add(point_score(pt));
    }
    if let Some(insert_point) = common.insert_point {
        score = score.saturating_add(point_score(insert_point));
    }
    score = score.saturating_add(point_score(common.extrusion));
    score = score.saturating_add(point_score(common.insert_scale));
    score = score.saturating_add(extrusion_score(common.extrusion));
    score = score.saturating_add(scale_score(common.insert_scale));

    // For DIM_DIAMETER and DIM_RADIUS ext_line_rotation/dim_rotation are fixed
    // at 0.0, and angle_score(0.0) == 0, so including them here is a no-op for
    // those entity types.
    for angle in [
        common.text_rotation,
        common.horizontal_direction,
        entity.ext_line_rotation,
        entity.dim_rotation,
        common.insert_rotation,
    ] {
        score = score.saturating_add(angle_score(angle));
    }

    if let Some(measurement) = common.actual_measurement {
        score = score.saturating_add(value_score(measurement));
    }
    if let Some(line_spacing) = common.line_spacing_factor {
        score = score.saturating_add(value_score(line_spacing));
    }
    if let Some(attachment_point) = common.attachment_point {
        if attachment_point > 9 {
            score = score.saturating_add(10_000);
        }
    }
    if let Some(line_spacing_style) = common.line_spacing_style {
        if line_spacing_style > 2 {
            score = score.saturating_add(10_000);
        }
    }
    if common.dim_flags > 0x3F {
        score = score.saturating_add(1_000);
    }

    score
}

pub(crate) fn extrusion_score(extrusion: (f64, f64, f64)) -> u64 {
    if !extrusion.0.is_finite() || !extrusion.1.is_finite() || !extrusion.2.is_finite() {
        return 1_000_000;
    }
    let norm_sq = extrusion.0 * extrusion.0 + extrusion.1 * extrusion.1 + extrusion.2 * extrusion.2;
    if norm_sq <= 1e-12 {
        return 50_000;
    }
    let norm = norm_sq.sqrt();
    let mut score = 0u64;
    let norm_err = (norm - 1.0).abs();
    if norm_err > 0.25 {
        score = score.saturating_add(25_000);
    } else if norm_err > 0.05 {
        score = score.saturating_add(2_500);
    }
    if extrusion.2.abs() < 0.5 {
        score = score.saturating_add(250);
    }
    score
}

pub(crate) fn scale_score(scale: (f64, f64, f64)) -> u64 {
    let mut score = 0u64;
    for value in [scale.0, scale.1, scale.2] {
        if !value.is_finite() {
            return 1_000_000;
        }
        if value.abs() < 1e-12 {
            score = score.saturating_add(2_500);
        } else if value.abs() > 1_000.0 {
            score = score.saturating_add(250);
        }
    }
    score
}

pub(crate) fn point_score(point: (f64, f64, f64)) -> u64 {
    value_score(point.0)
        .saturating_add(value_score(point.1))
        .saturating_add(value_score(point.2))
}

pub(crate) fn angle_score(value: f64) -> u64 {
    if !value.is_finite() {
        return 1_000_000;
    }
    let abs = value.abs();
    if abs <= 1_000.0 {
        0
    } else if abs <= 1_000_000.0 {
        25
    } else if abs <= 1_000_000_000_000.0 {
        250
    } else {
        1_000_000
    }
}

pub(crate) fn value_score(value: f64) -> u64 {
    if !value.is_finite() {
        return 1_000_000;
    }
    let abs = value.abs();
    if abs <= 1_000_000.0 {
        0
    } else if abs <= 1_000_000_000.0 {
        10
    } else if abs <= 1_000_000_000_000.0 {
        100
    } else if abs <= 1.0e18 {
        1_000
    } else if abs <= 1.0e24 {
        10_000
    } else {
        1_000_000
    }
}
