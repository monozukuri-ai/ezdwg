use pyo3::types::PyDict;
use writer::ir::{EntityFormatting, LinetypeDef};

#[pyfunction]
pub fn write_ac1015_line_dwg(output_path: &str, lines: Vec<LineEntityRow>) -> PyResult<()> {
    write_ac1015_dwg(
        output_path,
        lines,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
}

#[pyfunction(signature = (
    output_path,
    lines,
    arcs,
    circles,
    lwpolylines,
    texts,
    mtexts,
    points=None,
    rays=None,
    xlines=None,
    *, entity_attributes=None, layers=None, linetypes=None, metadata=None, text_styles=None,
    blocks=None, inserts=None, attdefs=None, attribs=None, entity_owners=None, ellipses=None,
    polylines2d=None, polylines3d=None, solids=None, traces=None, faces3d=None, splines=None,
    hatches=None, dimensions=None, images=None, image_defs=None
))]
pub fn write_ac1015_dwg(
    output_path: &str,
    lines: Vec<LineEntityRow>,
    arcs: Vec<ArcEntityRow>,
    circles: Vec<CircleEntityRow>,
    lwpolylines: Vec<LwPolylineEntityRow>,
    texts: Vec<TextWriteRow>,
    mtexts: Vec<MTextWriteRow>,
    points: Option<Vec<PointWriteRow>>,
    rays: Option<Vec<RayEntityRow>>,
    xlines: Option<Vec<XLineEntityRow>>,
    entity_attributes: Option<HashMap<u64, Bound<'_, PyDict>>>,
    layers: Option<Vec<Bound<'_, PyDict>>>,
    linetypes: Option<Vec<Bound<'_, PyDict>>>,
    metadata: Option<Bound<'_, PyDict>>,
    text_styles: Option<Vec<Bound<'_, PyDict>>>,
    blocks: Option<Vec<Bound<'_, PyDict>>>,
    inserts: Option<Vec<Bound<'_, PyDict>>>,
    attdefs: Option<Vec<Bound<'_, PyDict>>>,
    attribs: Option<Vec<Bound<'_, PyDict>>>,
    entity_owners: Option<HashMap<u64, Bound<'_, PyAny>>>,
    ellipses: Option<Vec<EllipseEntityRow>>,
    polylines2d: Option<Vec<Bound<'_, PyDict>>>,
    polylines3d: Option<Vec<Bound<'_, PyDict>>>,
    solids: Option<Vec<SolidEntityRow>>,
    traces: Option<Vec<TraceEntityRow>>,
    faces3d: Option<Vec<Face3dEntityRow>>,
    splines: Option<Vec<Bound<'_, PyDict>>>,
    hatches: Option<Vec<Bound<'_, PyDict>>>,
    dimensions: Option<Vec<Bound<'_, PyDict>>>,
    images: Option<Vec<Bound<'_, PyDict>>>,
    image_defs: Option<Vec<Bound<'_, PyDict>>>,
) -> PyResult<()> {
    let points = points.unwrap_or_default();
    let rays = rays.unwrap_or_default();
    let xlines = xlines.unwrap_or_default();
    let mut modelspace = Vec::with_capacity(
        lines.len()
            + arcs.len()
            + circles.len()
            + lwpolylines.len()
            + texts.len()
            + mtexts.len()
            + points.len()
            + rays.len()
            + xlines.len(),
    );
    for (handle, sx, sy, sz, ex, ey, ez) in lines {
        modelspace.push(writer::WriterEntity::Line(writer::LineEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
                ..writer::CommonEntityProps::default()
            },
            start: (sx, sy, sz),
            end: (ex, ey, ez),
        }));
    }
    for (handle, cx, cy, cz, radius, angle_start, angle_end) in arcs {
        modelspace.push(writer::WriterEntity::Arc(writer::ArcEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
                ..writer::CommonEntityProps::default()
            },
            center: (cx, cy, cz),
            radius,
            angle_start_rad: angle_start,
            angle_end_rad: angle_end,
        }));
    }
    for (handle, cx, cy, cz, radius) in circles {
        modelspace.push(writer::WriterEntity::Circle(writer::CircleEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
                ..writer::CommonEntityProps::default()
            },
            center: (cx, cy, cz),
            radius,
        }));
    }
    for (handle, flags, points, bulges, widths, const_width) in lwpolylines {
        modelspace.push(writer::WriterEntity::LwPolyline(writer::LwPolylineEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
                ..writer::CommonEntityProps::default()
            },
            flags,
            vertices: points,
            const_width,
            bulges,
            widths,
        }));
    }
    for (handle, text, insertion, height, rotation) in texts {
        modelspace.push(writer::WriterEntity::Text(writer::TextEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
                ..writer::CommonEntityProps::default()
            },
            text,
            insert: insertion,
            height,
            rotation_rad: rotation,
        }));
    }
    for (
        handle,
        text,
        insertion,
        text_direction,
        rect_width,
        char_height,
        attachment_point,
        drawing_direction,
    ) in mtexts
    {
        modelspace.push(writer::WriterEntity::MText(writer::MTextEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
                ..writer::CommonEntityProps::default()
            },
            text,
            insert: insertion,
            text_direction,
            rect_width,
            char_height,
            attachment_point,
            drawing_direction,
        }));
    }
    for (handle, x, y, z, x_axis_angle) in points {
        modelspace.push(writer::WriterEntity::Point(writer::PointEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
                ..writer::CommonEntityProps::default()
            },
            location: (x, y, z),
            x_axis_angle,
        }));
    }
    for (handle, start, unit_vector) in rays {
        modelspace.push(writer::WriterEntity::Ray(writer::RayEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
                ..writer::CommonEntityProps::default()
            },
            start,
            unit_vector,
        }));
    }
    for (handle, start, unit_vector) in xlines {
        modelspace.push(writer::WriterEntity::XLine(writer::XLineEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
                ..writer::CommonEntityProps::default()
            },
            start,
            unit_vector,
        }));
    }

    for (handle, center, major_axis, extrusion, axis_ratio, start_param, end_param) in
        ellipses.unwrap_or_default()
    {
        modelspace.push(writer::WriterEntity::Ellipse(writer::EllipseEntity {
            common: writer::CommonEntityProps {
                handle: (handle != 0).then_some(handle),
                layer_name: "0".into(),
                color_index: Some(7),
                formatting: EntityFormatting {
                    extrusion,
                    ..Default::default()
                },
                ..Default::default()
            },
            center,
            major_axis,
            axis_ratio,
            start_param,
            end_param,
        }));
    }
    for (rows, is_trace) in [(solids, false), (traces, true)] {
        for (handle, p1, p2, p3, p4, thickness, extrusion) in rows.unwrap_or_default() {
            let face = writer::PlanarFaceEntity {
                common: writer::CommonEntityProps {
                    handle: (handle != 0).then_some(handle),
                    layer_name: "0".into(),
                    color_index: Some(7),
                    formatting: EntityFormatting {
                        thickness,
                        extrusion,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                points: [p1, p2, p3, p4],
            };
            modelspace.push(if is_trace {
                writer::WriterEntity::Trace(face)
            } else {
                writer::WriterEntity::Solid(face)
            });
        }
    }
    for (handle, p1, p2, p3, p4, invisible_edge_flags) in faces3d.unwrap_or_default() {
        modelspace.push(writer::WriterEntity::Face3d(writer::Face3dEntity {
            common: writer::CommonEntityProps {
                handle: (handle != 0).then_some(handle),
                layer_name: "0".into(),
                color_index: Some(7),
                ..Default::default()
            },
            points: [p1, p2, p3, p4],
            invisible_edge_flags,
        }));
    }
    let mut doc = writer::WriterDocument {
        version: DwgVersion::R2000,
        modelspace,
        ..writer::WriterDocument::default()
    };
    append_write_blocks(&mut doc, blocks, inserts, attdefs, attribs)?;
    append_write_polylines(&mut doc, polylines2d, polylines3d)?;
    append_write_splines(&mut doc, splines)?;
    append_write_hatches(&mut doc, hatches)?;
    append_write_dimensions(&mut doc, dimensions)?;
    append_write_images(&mut doc, images, image_defs)?;
    if let Some(mut attributes) = entity_attributes {
        for entity in &mut doc.modelspace {
            if let Some(h) = entity.common().handle {
                if let Some(values) = attributes.remove(&h) {
                    read_write_attributes(entity.common_mut(), &values)?;
                }
            }
        }
        if !attributes.is_empty() {
            return Err(PyValueError::new_err(
                "entity_attributes contains unknown/zero handles",
            ));
        }
    }
    assign_write_owners(&mut doc, entity_owners)?;
    if let Some(rows) = layers {
        doc.layers = rows
            .iter()
            .map(|d| {
                check_write_keys(
                    d,
                    &[
                        "name",
                        "color_index",
                        "linetype",
                        "lineweight",
                        "frozen",
                        "off",
                        "frozen_in_new_viewports",
                        "locked",
                        "plot",
                    ],
                )?;
                Ok(writer::LayerDef {
                    name: write_value(d, "name", "0".to_owned())?,
                    color_index: write_value(d, "color_index", 7)?,
                    linetype: write_value(d, "linetype", "Continuous".to_owned())?,
                    lineweight: write_value(d, "lineweight", -3)?,
                    frozen: write_value(d, "frozen", false)?,
                    off: write_value(d, "off", false)?,
                    frozen_in_new_viewports: write_value(d, "frozen_in_new_viewports", false)?,
                    locked: write_value(d, "locked", false)?,
                    plot: write_value(d, "plot", true)?,
                })
            })
            .collect::<PyResult<_>>()?;
    }
    if let Some(rows) = linetypes {
        doc.linetypes = rows
            .iter()
            .map(|d| {
                check_write_keys(d, &["name", "description", "dashes"])?;
                Ok(LinetypeDef {
                    name: write_value(d, "name", String::new())?,
                    description: write_value(d, "description", String::new())?,
                    dashes: write_value(d, "dashes", Vec::new())?,
                })
            })
            .collect::<PyResult<_>>()?;
    }
    if let Some(rows) = text_styles {
        doc.text_styles = rows
            .iter()
            .map(|d| {
                check_write_keys(
                    d,
                    &[
                        "name",
                        "vertical",
                        "height",
                        "width",
                        "oblique",
                        "generation",
                        "last_height",
                        "font",
                        "bigfont",
                    ],
                )?;
                let st = writer::ir::TextStyleDef::default();
                Ok(writer::ir::TextStyleDef {
                    name: write_value(d, "name", st.name)?,
                    vertical: write_value(d, "vertical", st.vertical)?,
                    height: write_value(d, "height", st.height)?,
                    width: write_value(d, "width", st.width)?,
                    oblique: write_value(d, "oblique", st.oblique)?,
                    generation: write_value(d, "generation", st.generation)?,
                    last_height: write_value(d, "last_height", st.last_height)?,
                    font: write_value(d, "font", st.font)?,
                    bigfont: write_value(d, "bigfont", st.bigfont)?,
                })
            })
            .collect::<PyResult<_>>()?;
    }
    if let Some(d) = metadata {
        check_write_keys(
            &d,
            &[
                "codepage", "insunits", "lunits", "luprec", "aunits", "auprec", "ltscale",
            ],
        )?;
        let m = &mut doc.metadata;
        m.codepage = write_value(&d, "codepage", m.codepage)?;
        m.insunits = write_value(&d, "insunits", m.insunits)?;
        m.lunits = write_value(&d, "lunits", m.lunits)?;
        m.luprec = write_value(&d, "luprec", m.luprec)?;
        m.aunits = write_value(&d, "aunits", m.aunits)?;
        m.auprec = write_value(&d, "auprec", m.auprec)?;
        m.ltscale = write_value(&d, "ltscale", m.ltscale)?;
    }
    let bytes =
        writer::r2000::write_document(&doc, &writer::WriterConfig::default()).map_err(to_py_err)?;

    let out_path = Path::new(output_path);
    crate::atomic_write::atomic_write(out_path, &bytes)
        .map_err(|err| PyIOError::new_err(err.to_string()))?;
    clear_decode_cache()?;
    Ok(())
}

fn write_value<'py, T: FromPyObject<'py>>(
    d: &Bound<'py, PyDict>,
    name: &str,
    default: T,
) -> PyResult<T> {
    match d.get_item(name)? {
        Some(v) => v.extract(),
        None => Ok(default),
    }
}
fn check_write_keys(d: &Bound<'_, PyDict>, allowed: &[&str]) -> PyResult<()> {
    for key in d.keys() {
        let name: String = key.extract()?;
        if !allowed.contains(&name.as_str()) {
            return Err(PyValueError::new_err(format!(
                "unsupported writer attribute: {name}"
            )));
        }
    }
    Ok(())
}
fn read_write_attributes(c: &mut writer::CommonEntityProps, d: &Bound<'_, PyDict>) -> PyResult<()> {
    check_write_keys(
        d,
        &[
            "style",
            "layer",
            "color_index",
            "true_color",
            "linetype",
            "linetype_scale",
            "lineweight",
            "invisible",
            "extrusion",
            "thickness",
            "elevation",
            "align_point",
            "oblique",
            "width",
            "text_generation_flag",
            "halign",
            "valign",
            "line_spacing_style",
            "line_spacing_factor",
        ],
    )?;
    c.style_name = write_value(d, "style", c.style_name.clone())?;
    c.layer_name = write_value(d, "layer", c.layer_name.clone())?;
    c.color_index = write_value(d, "color_index", c.color_index)?;
    c.true_color = write_value(d, "true_color", None)?;
    c.linetype = write_value(d, "linetype", c.linetype.clone())?;
    // Optional display attributes must not reset geometry supplied in raw rows.
    let f = c.formatting;
    c.formatting = EntityFormatting {
        linetype_scale: write_value(d, "linetype_scale", f.linetype_scale)?,
        lineweight: write_value(d, "lineweight", f.lineweight)?,
        invisible: write_value(d, "invisible", f.invisible)?,
        extrusion: write_value(d, "extrusion", f.extrusion)?,
        thickness: write_value(d, "thickness", f.thickness)?,
        elevation: write_value(d, "elevation", f.elevation)?,
        alignment: write_value(d, "align_point", f.alignment)?,
        oblique: write_value::<f64>(d, "oblique", 0.)?.to_radians(),
        width_factor: write_value(d, "width", f.width_factor)?,
        generation: write_value(d, "text_generation_flag", f.generation)?,
        halign: write_value(d, "halign", f.halign)?,
        valign: write_value(d, "valign", f.valign)?,
        line_spacing_style: write_value(d, "line_spacing_style", f.line_spacing_style)?,
        line_spacing_factor: write_value(d, "line_spacing_factor", f.line_spacing_factor)?,
    };
    Ok(())
}
