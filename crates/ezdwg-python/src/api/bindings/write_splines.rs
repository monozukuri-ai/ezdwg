fn append_write_splines(
    doc: &mut writer::WriterDocument,
    rows: Option<Vec<Bound<'_, PyDict>>>,
) -> PyResult<()> {
    use writer::ir::{SplineData, SplineEntity};
    for d in rows.unwrap_or_default() {
        check_write_keys(
            &d,
            &[
                "handle",
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
            ],
        )?;
        let scenario = write_value(&d, "scenario", 1u32)?;
        let rational = write_value(&d, "rational", false)?;
        let closed = write_value(&d, "closed", false)?;
        let periodic = write_value(&d, "periodic", false)?;
        let fit_tolerance: Option<f64> = write_value(&d, "fit_tolerance", None)?;
        let knot_tolerance: Option<f64> = write_value(&d, "knot_tolerance", None)?;
        let ctrl_tolerance: Option<f64> = write_value(&d, "ctrl_tolerance", None)?;
        let start_tangent: Option<Point3> = write_value(&d, "start_tangent", None)?;
        let end_tangent: Option<Point3> = write_value(&d, "end_tangent", None)?;
        let knots: Vec<f64> = write_value(&d, "knots", Vec::new())?;
        let control_points: Vec<Point3> = write_value(&d, "control_points", Vec::new())?;
        let weights: Vec<f64> = write_value(&d, "weights", Vec::new())?;
        let fit_points: Vec<Point3> = write_value(&d, "fit_points", Vec::new())?;
        let data = match scenario {
            1 => {
                if fit_tolerance.is_some()
                    || start_tangent.is_some()
                    || end_tangent.is_some()
                    || !fit_points.is_empty()
                {
                    return Err(PyValueError::new_err(
                        "control SPLINE cannot also store fit data",
                    ));
                }
                SplineData::Control {
                    rational,
                    closed,
                    periodic,
                    knot_tolerance: knot_tolerance.unwrap_or(1e-7),
                    ctrl_tolerance: ctrl_tolerance.unwrap_or(1e-7),
                    knots,
                    control_points,
                    weights,
                }
            }
            2 => {
                if rational
                    || closed
                    || periodic
                    || knot_tolerance.is_some()
                    || ctrl_tolerance.is_some()
                    || !knots.is_empty()
                    || !control_points.is_empty()
                    || !weights.is_empty()
                {
                    return Err(PyValueError::new_err(
                        "fit SPLINE cannot also store control data or closure flags",
                    ));
                }
                SplineData::Fit {
                    fit_tolerance: fit_tolerance.unwrap_or(1e-10),
                    start_tangent: start_tangent.unwrap_or_default(),
                    end_tangent: end_tangent.unwrap_or_default(),
                    fit_points,
                }
            }
            _ => return Err(PyValueError::new_err("SPLINE scenario must be 1 or 2")),
        };
        let handle = write_value(&d, "handle", 0u64)?;
        doc.modelspace
            .push(writer::WriterEntity::Spline(SplineEntity {
                common: writer::CommonEntityProps {
                    handle: (handle != 0).then_some(handle),
                    ..Default::default()
                },
                degree: write_value(&d, "degree", 3u32)?,
                data,
            }));
    }
    Ok(())
}

/// Supplement the stable legacy tuple with tangents and R2013+ interpretation metadata.
#[pyfunction]
pub fn decode_write_splines(py: Python<'_>, path: &str) -> PyResult<Vec<PyObject>> {
    let entities =
        collect_entity_rows(path, None, 0x24, "SPLINE", decode_spline_for_version, |e| e)?;
    let mut rows = Vec::new();
    for e in entities {
        let d = PyDict::new_bound(py);
        d.set_item("handle", e.handle)?;
        d.set_item("scenario", e.scenario)?;
        d.set_item("degree", e.degree)?;
        d.set_item("rational", e.rational)?;
        d.set_item("closed", e.closed)?;
        d.set_item("periodic", e.periodic)?;
        d.set_item("fit_tolerance", e.fit_tolerance)?;
        d.set_item("knot_tolerance", e.knot_tolerance)?;
        d.set_item("ctrl_tolerance", e.ctrl_tolerance)?;
        d.set_item("start_tangent", e.start_tangent)?;
        d.set_item("end_tangent", e.end_tangent)?;
        d.set_item("knots", e.knots)?;
        d.set_item("control_points", e.control_points)?;
        d.set_item("weights", e.weights)?;
        d.set_item("fit_points", e.fit_points)?;
        d.set_item("spline_flags1", e.spline_flags1)?;
        d.set_item("knot_parameter", e.knot_parameter)?;
        rows.push(d.into_any().unbind());
    }
    Ok(rows)
}
