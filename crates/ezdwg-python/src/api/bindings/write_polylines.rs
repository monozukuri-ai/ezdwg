/// Legacy POLYLINE_2D (`is_3d` false) or POLYLINE_3D rows.
fn build_write_polylines(
    out: &mut Vec<writer::WriterEntity>,
    rows: Option<Vec<Bound<'_, PyDict>>>,
    is_3d: bool,
) -> PyResult<()> {
    use writer::ir::{PolylineEntity, PolylineVertex};
    {
        for d in rows.unwrap_or_default() {
            check_write_keys(
                &d,
                &[
                    "handle",
                    "points",
                    "flags",
                    "width_start",
                    "width_end",
                    "widths",
                    "bulges",
                ],
            )?;
            let handle = write_value(&d, "handle", 0u64)?;
            let points: Vec<Point3> = if is_3d {
                write_value(&d, "points", Vec::new())?
            } else {
                let xy: Vec<(f64, f64)> = write_value(&d, "points", Vec::new())?;
                xy.into_iter().map(|(x, y)| (x, y, 0.)).collect()
            };
            let widths: Vec<(f64, f64)> = write_value(&d, "widths", Vec::new())?;
            let bulges: Vec<f64> = write_value(&d, "bulges", Vec::new())?;
            if widths.len() > points.len() || bulges.len() > points.len() {
                return Err(PyValueError::new_err("polyline arrays exceed vertex count"));
            }
            let poly = PolylineEntity {
                common: writer::CommonEntityProps {
                    handle: (handle != 0).then_some(handle),
                    ..Default::default()
                },
                flags: write_value(&d, "flags", if is_3d { 8 } else { 0 })?,
                width_start: write_value(&d, "width_start", 0.)?,
                width_end: write_value(&d, "width_end", 0.)?,
                vertices: points
                    .into_iter()
                    .enumerate()
                    .map(|(i, position)| {
                        let (start_width, end_width) = widths.get(i).copied().unwrap_or_default();
                        PolylineVertex {
                            position,
                            start_width,
                            end_width,
                            bulge: bulges.get(i).copied().unwrap_or_default(),
                        }
                    })
                    .collect(),
            };
            out.push(if is_3d {
                writer::WriterEntity::Polyline3d(poly)
            } else {
                writer::WriterEntity::Polyline2d(poly)
            });
        }
    }
    Ok(())
}
