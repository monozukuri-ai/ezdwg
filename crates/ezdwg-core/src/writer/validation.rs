//! Fail before serializing or opening the destination, including through the raw API.
use super::ir::*;
use crate::bit::BitWriter;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use std::collections::{HashMap, HashSet, VecDeque};

fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(DwgError::new(ErrorKind::Format, message))
    }
}
fn finite(values: &[f64]) -> Result<()> {
    require(
        values.iter().all(|v| v.is_finite()),
        "writer values must be finite",
    )
}
fn point(p: (f64, f64, f64)) -> Result<()> {
    finite(&[p.0, p.1, p.2])
}
fn direction(p: (f64, f64, f64)) -> Result<()> {
    point(p)?;
    require(p != (0., 0., 0.), "direction/extrusion must be nonzero")
}

fn validate_spline(s: &SplineEntity) -> Result<()> {
    require(
        (1..=10).contains(&s.degree),
        "SPLINE degree must be in 1..10",
    )?;
    let tolerance = |v: f64| -> Result<()> {
        require(
            v.is_finite() && v >= 0.,
            "SPLINE tolerance must be finite and nonnegative",
        )
    };
    match &s.data {
        SplineData::Control {
            rational,
            closed,
            periodic,
            knot_tolerance,
            ctrl_tolerance,
            knots,
            control_points,
            weights,
        } => {
            let n = control_points.len();
            let p = s.degree as usize;
            require(
                n > p && knots.len() <= 1_000_000 && knots.len() == n + p + 1,
                "SPLINE knot/control count does not match degree",
            )?;
            tolerance(*knot_tolerance)?;
            tolerance(*ctrl_tolerance)?;
            finite(knots)?;
            finite(weights)?;
            for &v in control_points {
                point(v)?;
            }
            require(
                knots.windows(2).all(|v| v[0] <= v[1])
                    && knots[p] < knots[n]
                    && (knots[n] - knots[p]).is_finite(),
                "SPLINE knots must be nondecreasing with a nonempty domain",
            )?;
            require(
                knots.windows(p + 2).all(|v| v[0] != v[p + 1]),
                "SPLINE knot multiplicity exceeds degree + 1",
            )?;
            require(if *rational { weights.len() == n && weights.iter().all(|w| *w > 0.) } else { weights.is_empty() }, "SPLINE weights must be positive, one per rational control point, and absent otherwise")?;
            require(!*periodic || *closed, "periodic SPLINE requires closed")?;
            if *closed {
                // External CAD can retain the periodic flag after clamping a
                // closed curve. Both representations have the same geometry.
                let clamped = control_points[0] == control_points[n - 1]
                    && knots[0] == knots[p]
                    && knots[n] == knots[n + p];
                let period = knots[n] - knots[p];
                let wrapped = control_points[..p] == control_points[n - p..]
                    && (!*rational || weights[..p] == weights[n - p..])
                    && period.is_finite()
                    && (0..=2 * p)
                        .all(|i| ((knots[i + n - p] - knots[i]) / period - 1.).abs() <= 1e-9);
                require(
                    clamped || wrapped,
                    "closed SPLINE requires clamped matching endpoints or periodic wrapped data",
                )?;
            }
        }
        SplineData::Fit {
            fit_tolerance,
            start_tangent,
            end_tangent,
            fit_points,
        } => {
            require(s.degree == 3, "fit SPLINE requires degree 3")?;
            require(
                (2..=1_000_000).contains(&fit_points.len()),
                "fit SPLINE requires at least two points",
            )?;
            tolerance(*fit_tolerance)?;
            point(*start_tangent)?;
            point(*end_tangent)?;
            for &v in fit_points {
                point(v)?;
            }
            require(
                fit_points.windows(2).all(|v| v[0] != v[1])
                    && fit_points.first() != fit_points.last(),
                "fit SPLINE requires distinct adjacent points and open endpoints",
            )?;
        }
    }
    Ok(())
}
pub fn lineweight_index(value: i16) -> Result<u8> {
    const WEIGHTS: [i16; 24] = [
        0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158,
        200, 211,
    ];
    match value {
        -1 => Ok(29),
        -2 => Ok(30),
        -3 => Ok(31),
        _ => WEIGHTS
            .iter()
            .position(|v| *v == value)
            .map(|v| v as u8)
            .ok_or_else(|| {
                DwgError::new(ErrorKind::Format, format!("invalid lineweight: {value}"))
            }),
    }
}
fn name(value: &str, codepage: u16) -> Result<()> {
    require(
        !value.is_empty()
            && value.chars().count() <= 255
            && !value
                .chars()
                .any(|c| c.is_control() || "<>/\\\":;?*|,=`".contains(c)),
        "invalid table name",
    )?;
    BitWriter::with_codepage(codepage).write_tv(value)
}

pub fn validate_document(doc: &WriterDocument) -> Result<()> {
    let m = &doc.metadata;
    require(
        m.insunits <= 20
            && (1..=5).contains(&m.lunits)
            && m.luprec <= 8
            && m.aunits <= 4
            && m.auprec <= 8,
        "invalid AC1015 units/precision",
    )?;
    finite(&[m.ltscale])?;
    require(m.ltscale > 0., "LTSCALE must be positive")?;
    point(m.insertion_base)?;
    for p in [m.ext_min, m.ext_max].into_iter().flatten() {
        point(p)?;
    }
    let mut names = HashSet::new();
    for l in &doc.layers {
        name(&l.name, m.codepage)?;
        require(names.insert(l.name.to_uppercase()), "duplicate layer name")?;
        require(
            (1..=255).contains(&l.color_index),
            "layer ACI must be in 1..255",
        )?;
        require(
            !matches!(
                l.linetype.to_uppercase().as_str(),
                "BYLAYER" | "BYBLOCK" | ""
            ),
            "layer linetype must not inherit",
        )?;
        lineweight_index(l.lineweight)?;
    }
    names.clear();
    for l in &doc.linetypes {
        name(&l.name, m.codepage)?;
        require(
            !matches!(
                l.name.to_uppercase().as_str(),
                "BYLAYER" | "BYBLOCK" | "CONTINUOUS"
            ) && names.insert(l.name.to_uppercase()),
            "duplicate/reserved linetype name",
        )?;
        BitWriter::with_codepage(m.codepage).write_tv(&l.description)?;
        require(l.dashes.len() <= 12, "linetype supports at most 12 dashes")?;
        finite(&l.dashes)?;
        finite(&[l.dashes.iter().map(|v| v.abs()).sum()])?;
    }
    names.clear();
    for st in &doc.text_styles {
        name(&st.name, m.codepage)?;
        require(names.insert(st.name.to_uppercase()), "duplicate text style")?;
        finite(&[st.height, st.width, st.oblique, st.last_height])?;
        require(
            st.height >= 0. && st.width > 0. && st.last_height >= 0. && st.generation & !6 == 0,
            "invalid text style",
        )?;
        BitWriter::with_codepage(m.codepage).write_tv(&st.font)?;
        BitWriter::with_codepage(m.codepage).write_tv(&st.bigfont)?;
    }
    let mut blocks = HashMap::new();
    for (i, block) in doc.blocks.iter().enumerate() {
        name(
            block.name.strip_prefix('*').unwrap_or(&block.name),
            m.codepage,
        )?;
        require(
            !block.name.to_uppercase().starts_with("*MODEL_SPACE")
                && !block.name.to_uppercase().starts_with("*PAPER_SPACE"),
            "reserved block name",
        )?;
        require(
            blocks.insert(block.name.to_uppercase(), i).is_none(),
            "duplicate block name",
        )?;
        point(block.base_point)?;
        BitWriter::with_codepage(m.codepage).write_tv(&block.description)?;
    }
    let mut incoming = vec![0usize; doc.blocks.len()];
    let mut edges = vec![Vec::new(); doc.blocks.len()];
    for (layout_index, entities) in doc.layouts().enumerate() {
        for e in entities {
            require(
                !matches!(e, WriterEntity::Attrib(_)),
                "ATTRIB must belong to INSERT",
            )?;
            validate_entity(e, m.codepage)?;
            if let WriterEntity::Insert(insert) = e {
                let Some(&target) = blocks.get(&insert.block_name.to_uppercase()) else {
                    return Err(DwgError::new(
                        ErrorKind::Format,
                        format!("undefined block: {}", insert.block_name),
                    ));
                };
                if layout_index > 0 {
                    edges[layout_index - 1].push(target);
                    incoming[target] += 1;
                }
                let mut tags = HashSet::new();
                for attr in &insert.attributes {
                    validate_attribute(attr, m.codepage)?;
                    require(
                        tags.insert(attr.tag.to_uppercase()),
                        "duplicate INSERT attribute tag",
                    )?;
                    require(attr.prompt.is_empty(), "ATTRIB cannot store ATTDEF prompt")?;
                }
            }
        }
    }
    // Iterative topological walk bounds stack use even for very deep block DAGs.
    let mut queue: VecDeque<_> = incoming
        .iter()
        .enumerate()
        .filter_map(|(i, &n)| (n == 0).then_some(i))
        .collect();
    let mut visited = 0;
    while let Some(i) = queue.pop_front() {
        visited += 1;
        for &j in &edges[i] {
            incoming[j] -= 1;
            if incoming[j] == 0 {
                queue.push_back(j);
            }
        }
    }
    require(visited == doc.blocks.len(), "cyclic block references")?;
    Ok(())
}

pub fn validate_entity(e: &WriterEntity, codepage: u16) -> Result<()> {
    let c = e.common();
    let f = c.formatting;
    require(
        c.color_index.unwrap_or(256) <= 256,
        "entity ACI must be in 0..256",
    )?;
    require(c.true_color.is_none(), "AC1015 cannot preserve TrueColor")?;
    lineweight_index(f.lineweight)?;
    finite(&[
        f.linetype_scale,
        f.thickness,
        f.elevation,
        f.oblique,
        f.width_factor,
        f.line_spacing_factor,
    ])?;
    direction(f.extrusion)?;
    require(f.linetype_scale > 0., "linetype scale must be positive")?;
    require(
        matches!(e, WriterEntity::LwPolyline(_) | WriterEntity::Polyline2d(_)) || f.elevation == 0.,
        "elevation is only supported for 2D polylines",
    )?;
    require(
        matches!(
            e,
            WriterEntity::Text(_)
                | WriterEntity::MText(_)
                | WriterEntity::Attdef(_)
                | WriterEntity::Attrib(_)
        ) || c.style_name.is_empty(),
        "text style requires TEXT or MTEXT",
    )?;
    require(
        matches!(
            e,
            WriterEntity::Text(_) | WriterEntity::Attdef(_) | WriterEntity::Attrib(_)
        ) || (f.alignment.is_none()
            && f.oblique == 0.
            && f.width_factor == 1.
            && f.generation == 0
            && f.halign == 0
            && f.valign == 0),
        "text alignment attributes require TEXT",
    )?;
    require(
        matches!(e, WriterEntity::MText(_))
            || (f.line_spacing_style == 1 && f.line_spacing_factor == 1.),
        "line spacing attributes require MTEXT",
    )?;
    require(
        !matches!(
            e,
            WriterEntity::Ray(_)
                | WriterEntity::XLine(_)
                | WriterEntity::MText(_)
                | WriterEntity::Insert(_)
                | WriterEntity::Ellipse(_)
                | WriterEntity::Polyline3d(_)
                | WriterEntity::Face3d(_)
                | WriterEntity::Spline(_)
        ) || f.thickness == 0.,
        "thickness is not supported for this entity type",
    )?;
    require(
        !matches!(
            e,
            WriterEntity::Ray(_)
                | WriterEntity::XLine(_)
                | WriterEntity::Polyline3d(_)
                | WriterEntity::Face3d(_)
                | WriterEntity::Spline(_)
        ) || f.extrusion == (0., 0., 1.),
        "extrusion is not supported for this entity type",
    )?;
    match e {
        WriterEntity::Line(v) => {
            point(v.start)?;
            point(v.end)?;
        }
        WriterEntity::Point(v) => {
            point(v.location)?;
            finite(&[v.x_axis_angle])?;
        }
        WriterEntity::Ray(v) => {
            point(v.start)?;
            direction(v.unit_vector)?;
        }
        WriterEntity::XLine(v) => {
            point(v.start)?;
            direction(v.unit_vector)?;
        }
        WriterEntity::Circle(v) => {
            point(v.center)?;
            finite(&[v.radius])?;
            require(v.radius > 0., "radius must be positive")?;
        }
        WriterEntity::Arc(v) => {
            point(v.center)?;
            finite(&[v.radius, v.angle_start_rad, v.angle_end_rad])?;
            require(v.radius > 0., "radius must be positive")?;
        }
        WriterEntity::Ellipse(v) => {
            point(v.center)?;
            direction(v.major_axis)?;
            finite(&[v.axis_ratio, v.start_param, v.end_param])?;
            require(
                (1e-6..=1.).contains(&v.axis_ratio),
                "ellipse axis ratio must be in 1e-6..1",
            )?;
            let a = v.major_axis;
            let n = f.extrusion;
            let alen = a.0.hypot(a.1).hypot(a.2);
            let nlen = n.0.hypot(n.1).hypot(n.2);
            require(
                alen.is_finite() && nlen.is_finite() && alen > 1e-6 && nlen > 1e-6,
                "ellipse axis/normal length must be finite and greater than 1e-6",
            )?;
            let dot = (a.0 / alen) * (n.0 / nlen)
                + (a.1 / alen) * (n.1 / nlen)
                + (a.2 / alen) * (n.2 / nlen);
            require(
                dot.abs() <= 1e-6,
                "ellipse major axis must be perpendicular to extrusion",
            )?;
            let span = v.end_param - v.start_param;
            let full = (span.abs() - std::f64::consts::TAU).abs() <= 1e-12;
            require(
                span.is_finite()
                    && span.abs() <= std::f64::consts::TAU + 1e-12
                    && (full || span.rem_euclid(std::f64::consts::TAU) > 1e-6),
                "ellipse parameter span must be nonzero and at most one revolution",
            )?;
        }
        WriterEntity::Spline(v) => validate_spline(v)?,
        WriterEntity::Solid(v) | WriterEntity::Trace(v) => {
            for p in v.points {
                point(p)?;
                require(
                    p.2 == v.points[0].2,
                    "SOLID/TRACE corners must have the same OCS Z",
                )?;
            }
        }
        WriterEntity::Face3d(v) => {
            for p in v.points {
                point(p)?;
            }
            require(
                v.invisible_edge_flags & !15 == 0,
                "3DFACE invisible edge flags must be in 0..15",
            )?;
        }
        WriterEntity::Polyline2d(v) | WriterEntity::Polyline3d(v) => {
            let is_3d = matches!(e, WriterEntity::Polyline3d(_));
            require(
                v.vertices.len() >= 2,
                "POLYLINE requires at least two vertices",
            )?;
            require(
                if is_3d {
                    v.flags & !9 == 0
                } else {
                    v.flags & !129 == 0
                },
                "unsupported POLYLINE flags (fit/mesh)",
            )?;
            finite(&[v.width_start, v.width_end])?;
            require(
                v.width_start >= 0. && v.width_end >= 0.,
                "polyline width must be nonnegative",
            )?;
            require(
                !is_3d || (v.width_start == 0. && v.width_end == 0.),
                "3D polyline cannot store widths",
            )?;
            for vertex in &v.vertices {
                point(vertex.position)?;
                finite(&[vertex.start_width, vertex.end_width, vertex.bulge])?;
                require(
                    vertex.start_width >= 0. && vertex.end_width >= 0.,
                    "polyline width must be nonnegative",
                )?;
                require(
                    is_3d || vertex.position.2 == 0.,
                    "2D vertex Z must be zero; use elevation",
                )?;
                require(
                    !is_3d
                        || (vertex.start_width == 0.
                            && vertex.end_width == 0.
                            && vertex.bulge == 0.),
                    "3D polyline cannot store widths/bulges",
                )?;
            }
        }
        WriterEntity::LwPolyline(v) => {
            require(!v.vertices.is_empty(), "LWPOLYLINE requires vertices")?;
            require(
                v.bulges.len() <= v.vertices.len() && v.widths.len() <= v.vertices.len(),
                "polyline arrays exceed vertex count",
            )?;
            require(v.flags & !0x2BF == 0, "unsupported polyline flags")?;
            for p in &v.vertices {
                finite(&[p.0, p.1])?;
            }
            finite(&v.bulges)?;
            for p in &v.widths {
                finite(&[p.0, p.1])?;
                require(p.0 >= 0. && p.1 >= 0., "width must be nonnegative")?;
            }
            if let Some(w) = v.const_width {
                finite(&[w])?;
                require(w >= 0., "width must be nonnegative")?;
            }
        }
        WriterEntity::Text(v) => {
            point(v.insert)?;
            finite(&[v.height, v.rotation_rad])?;
            require(
                v.height > 0. && f.width_factor > 0.,
                "text height/width must be positive",
            )?;
            require(
                f.halign <= 5 && f.valign <= 3 && f.generation & !6 == 0,
                "invalid text alignment/generation",
            )?;
            require(
                f.oblique.abs() < std::f64::consts::FRAC_PI_2,
                "text oblique angle must be between -90 and 90 degrees",
            )?;
            if let Some(p) = f.alignment {
                point(p)?;
                require(
                    p.2 == v.insert.2,
                    "text alignment elevation differs from insertion",
                )?;
            }
            require(
                (f.halign == 0 && f.valign == 0) || f.alignment.is_some(),
                "aligned text requires alignment point",
            )?;
            require(
                f.halign < 3 || f.valign == 0,
                "aligned/middle/fit text requires baseline vertical alignment",
            )?;
            BitWriter::with_codepage(codepage).write_tv(&v.text)?;
        }
        WriterEntity::Insert(v) => {
            point(v.insert)?;
            point(v.scale)?;
            finite(&[v.rotation_rad])?;
            require(
                v.scale.0 != 0. && v.scale.1 != 0. && v.scale.2 != 0.,
                "INSERT scale must be nonzero",
            )?;
        }
        WriterEntity::Attdef(v) | WriterEntity::Attrib(v) => validate_attribute(v, codepage)?,
        WriterEntity::MText(v) => {
            point(v.insert)?;
            direction(v.text_direction)?;
            finite(&[v.rect_width, v.char_height])?;
            require(
                v.rect_width >= 0. && v.char_height > 0.,
                "invalid MTEXT width/height",
            )?;
            require(
                (1..=9).contains(&v.attachment_point) && matches!(v.drawing_direction, 1 | 3 | 5),
                "invalid MTEXT attachment/direction",
            )?;
            require(
                matches!(f.line_spacing_style, 1 | 2)
                    && (0.25..=4.).contains(&f.line_spacing_factor),
                "invalid MTEXT line spacing",
            )?;
            BitWriter::with_codepage(codepage).write_tv(&v.text)?;
        }
    }
    Ok(())
}

fn validate_attribute(attr: &AttributeEntity, codepage: u16) -> Result<()> {
    validate_entity(&WriterEntity::Text(attr.text.clone()), codepage)?;
    require(
        !attr.tag.is_empty() && !attr.tag.chars().any(|c| c.is_whitespace() || c == '!'),
        "invalid attribute tag",
    )?;
    require(attr.flags & !15 == 0, "unsupported attribute flags")?;
    BitWriter::with_codepage(codepage).write_tv(&attr.tag)?;
    BitWriter::with_codepage(codepage).write_tv(&attr.prompt)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_geometry_and_reference_values() {
        let mut e = WriterEntity::Circle(CircleEntity {
            radius: -1.,
            ..Default::default()
        });
        assert!(validate_entity(&e, 30).is_err());
        if let WriterEntity::Circle(v) = &mut e {
            v.radius = 1.;
            v.common.color_index = Some(256);
        }
        assert!(validate_entity(&e, 30).is_ok());
        e.common_mut().formatting.extrusion = (0., 0., 0.);
        assert!(validate_entity(&e, 30).is_err());
        e.common_mut().formatting.extrusion = (0., 0., 1.);
        e.common_mut().color_index = Some(257);
        assert!(validate_entity(&e, 30).is_err());
        e.common_mut().color_index = Some(0);
        e.common_mut().formatting.lineweight = 123;
        assert!(validate_entity(&e, 30).is_err());
    }
    #[test]
    fn refuses_duplicate_case_insensitive_table_names() {
        let doc = WriterDocument {
            layers: vec![
                LayerDef {
                    name: "Test".into(),
                    ..Default::default()
                },
                LayerDef {
                    name: "test".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert!(validate_document(&doc).is_err());
    }
}
