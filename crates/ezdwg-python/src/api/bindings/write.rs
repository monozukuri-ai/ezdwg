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
    xlines=None
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
            },
            start,
            unit_vector,
        }));
    }

    let doc = writer::WriterDocument {
        version: DwgVersion::R2000,
        modelspace,
        ..writer::WriterDocument::default()
    };
    let bytes =
        writer::r2000::write_document(&doc, &writer::WriterConfig::default()).map_err(to_py_err)?;

    let out_path = Path::new(output_path);
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|err| PyIOError::new_err(err.to_string()))?;
        }
    }
    std::fs::write(out_path, bytes).map_err(|err| PyIOError::new_err(err.to_string()))?;
    Ok(())
}
