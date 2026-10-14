/// DIMENSION rows are dictionaries: kind-specific definition points (3D WCS
/// tuples), angles in radians, and the DIMSTYLE/anonymous block names.
fn append_write_dimensions(
    doc: &mut writer::WriterDocument,
    rows: Option<Vec<Bound<'_, PyDict>>>,
) -> PyResult<()> {
    use writer::ir::{DimensionEntity, DimensionKind};
    for d in rows.unwrap_or_default() {
        check_write_keys(
            &d,
            &[
                "handle",
                "kind",
                "point10",
                "point13",
                "point14",
                "point15",
                "point16",
                "ext_line_rotation",
                "dim_rotation",
                "leader_length",
                "flags2",
                "text_midpoint",
                "elevation",
                "flags",
                "user_text",
                "text_rotation",
                "horizontal_direction",
                "insert_scale",
                "insert_rotation",
                "attachment_point",
                "line_spacing_style",
                "line_spacing_factor",
                "actual_measurement",
                "insert_point",
                "dimstyle",
                "block_name",
            ],
        )?;
        let origin = (0., 0., 0.);
        let point10: Point3 = write_value(&d, "point10", origin)?;
        let point13: Point3 = write_value(&d, "point13", origin)?;
        let point14: Point3 = write_value(&d, "point14", origin)?;
        let point15: Point3 = write_value(&d, "point15", origin)?;
        let point16: (f64, f64) = write_value(&d, "point16", (0., 0.))?;
        let ext_line_rotation = write_value(&d, "ext_line_rotation", 0.)?;
        let dim_rotation = write_value(&d, "dim_rotation", 0.)?;
        let leader_length = write_value(&d, "leader_length", 0.)?;
        let flags2 = write_value(&d, "flags2", 0u8)?;
        let kind_name: String = write_value(&d, "kind", "linear".to_owned())?;
        let kind = match kind_name.to_ascii_lowercase().as_str() {
            "linear" => DimensionKind::Linear {
                point13,
                point14,
                point10,
                ext_line_rotation,
                dim_rotation,
            },
            "aligned" => DimensionKind::Aligned {
                point13,
                point14,
                point10,
                ext_line_rotation,
            },
            "angular" | "angular3pt" => DimensionKind::Angular3Pt {
                point10,
                point13,
                point14,
                point15,
            },
            "angular2ln" | "angular2line" => DimensionKind::Angular2Ln {
                point16,
                point13,
                point14,
                point15,
                point10,
            },
            "radius" => DimensionKind::Radius {
                point10,
                point15,
                leader_length,
            },
            "diameter" => DimensionKind::Diameter {
                point15,
                point10,
                leader_length,
            },
            "ordinate" => DimensionKind::Ordinate {
                point10,
                point13,
                point14,
                flags2,
            },
            other => {
                return Err(PyValueError::new_err(format!(
                    "unsupported dimension kind: {other}"
                )))
            }
        };
        let handle = write_value(&d, "handle", 0u64)?;
        doc.modelspace
            .push(writer::WriterEntity::Dimension(DimensionEntity {
                common: writer::CommonEntityProps {
                    handle: (handle != 0).then_some(handle),
                    ..Default::default()
                },
                kind,
                text_midpoint: write_value(&d, "text_midpoint", (0., 0.))?,
                elevation: write_value(&d, "elevation", 0.)?,
                flags: write_value(&d, "flags", 3u8)?,
                user_text: write_value(&d, "user_text", String::new())?,
                text_rotation: write_value(&d, "text_rotation", 0.)?,
                horizontal_direction: write_value(&d, "horizontal_direction", 0.)?,
                insert_scale: write_value(&d, "insert_scale", (1., 1., 1.))?,
                insert_rotation: write_value(&d, "insert_rotation", 0.)?,
                attachment_point: write_value(&d, "attachment_point", 5u16)?,
                line_spacing_style: write_value(&d, "line_spacing_style", 1u16)?,
                line_spacing_factor: write_value(&d, "line_spacing_factor", 1.)?,
                actual_measurement: write_value(&d, "actual_measurement", 0.)?,
                insert_point: write_value(&d, "insert_point", (0., 0.))?,
                dimstyle: write_value(&d, "dimstyle", "Standard".to_owned())?,
                block_name: write_value(&d, "block_name", String::new())?,
            }));
    }
    Ok(())
}
