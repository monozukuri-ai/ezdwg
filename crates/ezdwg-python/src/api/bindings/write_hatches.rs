/// HATCH rows are dictionaries: boundary paths, pattern definition and seeds.
fn append_write_hatches(
    doc: &mut writer::WriterDocument,
    rows: Option<Vec<Bound<'_, PyDict>>>,
) -> PyResult<()> {
    use writer::ir::{
        HatchEdge, HatchEntity, HatchPathData, HatchPathDef, HatchPatternDef, HatchPatternLineDef,
    };
    for d in rows.unwrap_or_default() {
        check_write_keys(
            &d,
            &[
                "handle",
                "name",
                "solid_fill",
                "paths",
                "style",
                "pattern_type",
                "pattern",
                "pixel_size",
                "seed_points",
            ],
        )?;
        let handle = write_value(&d, "handle", 0u64)?;
        let mut paths = Vec::new();
        for path in write_value::<Vec<Bound<'_, PyDict>>>(&d, "paths", Vec::new())? {
            check_write_keys(&path, &["flags", "closed", "vertices", "bulges", "edges"])?;
            let edges: Option<Vec<Bound<'_, PyDict>>> = write_value(&path, "edges", None)?;
            let data = match edges {
                Some(edges) => {
                    let mut parsed = Vec::with_capacity(edges.len());
                    for edge in edges {
                        check_write_keys(
                            &edge,
                            &[
                                "type",
                                "start",
                                "end",
                                "center",
                                "radius",
                                "major_endpoint",
                                "ratio",
                                "start_angle",
                                "end_angle",
                                "ccw",
                                "degree",
                                "rational",
                                "periodic",
                                "knots",
                                "control_points",
                                "weights",
                            ],
                        )?;
                        let kind: u8 = write_value(&edge, "type", 1u8)?;
                        parsed.push(match kind {
                            1 => HatchEdge::Line {
                                start: write_value(&edge, "start", (0., 0.))?,
                                end: write_value(&edge, "end", (0., 0.))?,
                            },
                            2 => HatchEdge::Arc {
                                center: write_value(&edge, "center", (0., 0.))?,
                                radius: write_value(&edge, "radius", 0.)?,
                                start_angle: write_value(&edge, "start_angle", 0.)?,
                                end_angle: write_value(&edge, "end_angle", 0.)?,
                                ccw: write_value(&edge, "ccw", true)?,
                            },
                            3 => HatchEdge::Ellipse {
                                center: write_value(&edge, "center", (0., 0.))?,
                                major_endpoint: write_value(&edge, "major_endpoint", (0., 0.))?,
                                ratio: write_value(&edge, "ratio", 1.)?,
                                start_angle: write_value(&edge, "start_angle", 0.)?,
                                end_angle: write_value(&edge, "end_angle", 0.)?,
                                ccw: write_value(&edge, "ccw", true)?,
                            },
                            4 => HatchEdge::Spline {
                                degree: write_value(&edge, "degree", 3u32)?,
                                rational: write_value(&edge, "rational", false)?,
                                periodic: write_value(&edge, "periodic", false)?,
                                knots: write_value(&edge, "knots", Vec::new())?,
                                control_points: write_value(&edge, "control_points", Vec::new())?,
                                weights: write_value(&edge, "weights", Vec::new())?,
                            },
                            _ => {
                                return Err(PyValueError::new_err(
                                    "hatch edge type must be 1 (line), 2 (arc), 3 (ellipse) or 4 (spline)",
                                ))
                            }
                        });
                    }
                    HatchPathData::Edges(parsed)
                }
                None => HatchPathData::Polyline {
                    closed: write_value(&path, "closed", true)?,
                    vertices: write_value(&path, "vertices", Vec::new())?,
                    bulges: write_value(&path, "bulges", None)?,
                },
            };
            let default_flags = if matches!(data, HatchPathData::Polyline { .. }) { 3 } else { 1 };
            paths.push(HatchPathDef {
                flags: write_value(&path, "flags", default_flags)?,
                data,
            });
        }
        let pattern: Option<Bound<'_, PyDict>> = write_value(&d, "pattern", None)?;
        let pattern = match pattern {
            Some(p) => {
                check_write_keys(&p, &["angle", "scale", "double", "lines"])?;
                let mut lines = Vec::new();
                for line in write_value::<Vec<Bound<'_, PyDict>>>(&p, "lines", Vec::new())? {
                    check_write_keys(&line, &["angle", "base", "offset", "dashes"])?;
                    lines.push(HatchPatternLineDef {
                        angle: write_value(&line, "angle", 0.)?,
                        base: write_value(&line, "base", (0., 0.))?,
                        offset: write_value(&line, "offset", (0., 0.))?,
                        dashes: write_value(&line, "dashes", Vec::new())?,
                    });
                }
                Some(HatchPatternDef {
                    angle: write_value(&p, "angle", 0.)?,
                    scale: write_value(&p, "scale", 1.)?,
                    double: write_value(&p, "double", false)?,
                    lines,
                })
            }
            None => None,
        };
        doc.modelspace.push(writer::WriterEntity::Hatch(HatchEntity {
            common: writer::CommonEntityProps {
                handle: (handle != 0).then_some(handle),
                ..Default::default()
            },
            name: write_value(&d, "name", "SOLID".to_owned())?,
            solid_fill: write_value(&d, "solid_fill", true)?,
            paths,
            style: write_value(&d, "style", 0u16)?,
            pattern_type: write_value(&d, "pattern_type", 1u16)?,
            pattern,
            pixel_size: write_value(&d, "pixel_size", None)?,
            seed_points: write_value(&d, "seed_points", Vec::new())?,
        }));
    }
    Ok(())
}
