/// Supplementary geometry fields without changing existing raw row shapes.
#[pyfunction]
pub fn decode_write_geometry(
    path: &str,
) -> PyResult<Vec<(u64, Option<f64>, Option<Point3>, Option<f64>)>> {
    let mut rows = Vec::new();
    macro_rules! geometry {
        ($code:expr,$name:expr,$decode:ident) => {
            rows.extend(collect_entity_rows(
                path,
                None,
                $code,
                $name,
                $decode,
                |e| (e.handle, e.thickness, e.extrusion, None),
            )?);
        };
    }
    geometry!(0x13, "LINE", decode_line_for_version);
    geometry!(0x11, "ARC", decode_arc_for_version);
    geometry!(0x12, "CIRCLE", decode_circle_for_version);
    geometry!(0x1B, "POINT", decode_point_for_version);
    rows.extend(collect_entity_rows(
        path,
        None,
        0x0f,
        "POLYLINE_2D",
        decode_polyline_2d_for_version,
        |e| {
            (
                e.handle,
                Some(e.thickness),
                Some(e.extrusion),
                Some(e.elevation),
            )
        },
    )?);
    rows.extend(collect_entity_rows(
        path,
        None,
        0x4D,
        "LWPOLYLINE",
        decode_lwpolyline_for_version,
        |e| {
            (
                e.handle,
                Some(e.thickness),
                Some(e.extrusion),
                Some(e.elevation),
            )
        },
    )?);
    rows.extend(collect_entity_rows(
        path,
        None,
        7,
        "INSERT",
        decode_insert_for_version,
        |e| (e.handle, None, Some(e.extrusion), None),
    )?);
    Ok(rows)
}

#[pyfunction]
pub fn decode_mtext_write_properties(path: &str) -> PyResult<Vec<(u64, Option<u64>, u16, f64)>> {
    collect_entity_rows(path, None, 0x2C, "MTEXT", decode_mtext_for_version, |e| {
        (
            e.handle,
            e.style_handle,
            e.line_spacing_style,
            e.line_spacing_factor,
        )
    })
}

/// Retain STYLE properties and identify complex LTYPE definitions before export.
#[pyfunction]
pub fn decode_write_tables(py: Python<'_>, path: &str) -> PyResult<Vec<PyObject>> {
    let shared = decoded_file(path)?;
    let decoder = shared.decoder()?;
    let best_effort = shared.best_effort;
    let dynamic = &shared.dynamic_types;
    let index = &shared.index;
    let mut result = Vec::new();
    for obj in &index.objects {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        let style = matches_type_name(header.type_code, 0x35, "STYLE", dynamic);
        let ltype = matches_type_name(header.type_code, 0x39, "LTYPE", dynamic);
        if !style && !ltype {
            continue;
        }
        let decoded = (|| -> PyResult<PyObject> {
            let mut r = record.bit_reader();
            skip_object_type_prefix(&mut r, decoder.version()).map_err(to_py_err)?;
            let pre = read_table_record_preamble(&mut r, decoder.version(), &header)
                .map_err(to_py_err)?;
            let streamed = table_record_strings_in_stream(decoder.version());
            let strings = if streamed {
                read_table_record_stream_strings(&r, pre.data_end_bit, if style { 3 } else { 2 })
            } else {
                Vec::new()
            };
            let name = if streamed {
                strings[0].clone()
            } else {
                r.read_tv().map_err(to_py_err)?
            };
            if !streamed {
                r.read_b().map_err(to_py_err)?;
            }
            r.read_bs().map_err(to_py_err)?;
            if !streamed {
                r.read_b().map_err(to_py_err)?;
            }
            let d = PyDict::new_bound(py);
            d.set_item("handle", obj.handle.0)?;
            d.set_item("name", name)?;
            d.set_item("kind", if style { "STYLE" } else { "LTYPE" })?;
            d.set_item("extended_data", pre.has_eed)?;
            if style {
                d.set_item("shape", r.read_b().map_err(to_py_err)? != 0)?;
                d.set_item("vertical", r.read_b().map_err(to_py_err)? != 0)?;
                for key in ["height", "width", "oblique"] {
                    d.set_item(key, r.read_bd().map_err(to_py_err)?)?;
                }
                d.set_item("generation", r.read_rc().map_err(to_py_err)?)?;
                d.set_item("last_height", r.read_bd().map_err(to_py_err)?)?;
                d.set_item(
                    "font",
                    if streamed {
                        strings[1].clone()
                    } else {
                        r.read_tv().map_err(to_py_err)?
                    },
                )?;
                d.set_item(
                    "bigfont",
                    if streamed {
                        strings[2].clone()
                    } else {
                        r.read_tv().map_err(to_py_err)?
                    },
                )?;
            } else {
                let _description = if streamed {
                    strings[1].clone()
                } else {
                    r.read_tv().map_err(to_py_err)?
                };
                r.read_bd().map_err(to_py_err)?;
                r.read_rc().map_err(to_py_err)?;
                let count = r.read_rc().map_err(to_py_err)?;
                let mut complex = false;
                for _ in 0..count {
                    r.read_bd().map_err(to_py_err)?;
                    let shape = r.read_bs().map_err(to_py_err)?;
                    let x = r.read_rd(Endian::Little).map_err(to_py_err)?;
                    let y = r.read_rd(Endian::Little).map_err(to_py_err)?;
                    let scale = r.read_bd().map_err(to_py_err)?;
                    let rotation = r.read_bd().map_err(to_py_err)?;
                    let flags = r.read_bs().map_err(to_py_err)?;
                    complex |= shape != 0
                        || flags != 0
                        || x != 0.
                        || y != 0.
                        || scale != 1.
                        || rotation != 0.;
                }
                d.set_item("complex", complex)?;
            }
            Ok(d.into_any().unbind())
        })();
        // Missing records remain detectable by the high-level preflight.
        if let Ok(value) = decoded {
            result.push(value);
        }
    }
    Ok(result)
}

/// Nondefault properties that the bounded AC1015 writer cannot preserve.
#[pyfunction]
pub fn decode_write_property_losses(path: &str) -> PyResult<Vec<(u64, Vec<String>)>> {
    let shared = decoded_file(path)?;
    let decoder = shared.decoder()?;
    let best = shared.best_effort;
    let dynamic = &shared.dynamic_types;
    let classes = &shared.dynamic_type_classes;
    let index = &shared.index;
    let mut rows = Vec::new();
    for obj in &index.objects {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best)? else {
            continue;
        };
        let name = resolved_type_name(header.type_code, dynamic);
        if resolved_type_class(header.type_code, &name, classes) != "E" {
            continue;
        }
        let mut losses = Vec::new();
        if let Some((common, handles)) =
            decode_common_entity_header_and_handles(&record, decoder.version(), &header)
        {
            for (attribute, present) in [
                ("transparency", common.color.transparency.is_some()),
                ("color_book", common.has_color_handle),
                ("plotstyle", common.plotstyle_flags != 0),
                ("material", common.material_flags != 0),
                (
                    "visual_style",
                    common.has_full_visual_style
                        || common.has_face_visual_style
                        || common.has_edge_visual_style,
                ),
                ("extension_dict", handles.xdic_obj.is_some_and(|h| h != 0)),
                ("reactors", !handles.reactors.is_empty()),
                ("binary_data", common.has_ds_binary_data),
            ] {
                if present {
                    losses.push(attribute.to_owned());
                }
            }
        } else {
            losses.push("common_properties_undecoded".into());
        }
        let mut r = record.bit_reader();
        let eed = (|| -> crate::core::result::Result<bool> {
            skip_object_type_prefix(&mut r, decoder.version())?;
            if matches!(
                decoder.version(),
                DwgVersion::R2000 | DwgVersion::R2004 | DwgVersion::R2007
            ) {
                r.read_rl(Endian::Little)?;
            }
            r.read_h()?;
            Ok(r.read_bs()? != 0)
        })();
        if eed.unwrap_or(true) {
            losses.push("xdata".into());
        }
        rows.push((obj.handle.0, losses));
    }
    Ok(rows)
}
