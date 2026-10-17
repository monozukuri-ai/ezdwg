#[pyfunction]
pub fn decode_insert_write_properties(
    path: &str,
) -> PyResult<Vec<(u64, Point3, Option<u64>, Option<Vec<u64>>)>> {
    collect_entity_rows(path, None, 7, "INSERT", decode_insert_for_version, |e| {
        (
            e.handle,
            e.extrusion,
            e.block_header_handle,
            e.attribute_handles,
        )
    })
}

/// Exact block metadata used by bounded write/import; undecoded rows stay absent.
#[pyfunction]
pub fn decode_write_blocks(py: Python<'_>, path: &str) -> PyResult<Vec<PyObject>> {
    let shared = decoded_file(path)?;
    let decoder = shared.decoder()?;
    let best = shared.best_effort;
    let dynamic = &shared.dynamic_types;
    let index = &shared.index;
    let mut rows = Vec::new();
    for obj in &index.objects {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best)? else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", dynamic) {
            continue;
        }
        let decoded = (|| -> PyResult<PyObject> {
            let mut r = record.bit_reader();
            skip_object_type_prefix(&mut r, decoder.version()).map_err(to_py_err)?;
            let pre = read_table_record_preamble(&mut r, decoder.version(), &header)
                .map_err(to_py_err)?;
            let streamed = table_record_strings_in_stream(decoder.version());
            let strings = if streamed {
                read_table_record_stream_strings(&r, pre.data_end_bit, 3)
            } else {
                Vec::new()
            };
            if streamed && strings.len() != 3 {
                return Err(PyValueError::new_err("undecoded block strings"));
            }
            let name = if streamed {
                strings[0].clone()
            } else {
                r.read_tv().map_err(to_py_err)?
            };
            if !streamed {
                r.read_b().map_err(to_py_err)?;
            }
            let xref_index = r.read_bs().map_err(to_py_err)?;
            let xdep = if !streamed {
                r.read_b().map_err(to_py_err)? != 0
            } else {
                false
            };
            let _anonymous = r.read_b().map_err(to_py_err)?;
            let _has_atts = r.read_b().map_err(to_py_err)?;
            let xref = r.read_b().map_err(to_py_err)? != 0;
            let overlay = r.read_b().map_err(to_py_err)? != 0;
            let r14 = matches!(decoder.version(), DwgVersion::R13 | DwgVersion::R14);
            let unloaded = if !r14 {
                r.read_b().map_err(to_py_err)? != 0
            } else {
                false
            };
            if !r14 && !matches!(decoder.version(), DwgVersion::R2000) {
                r.read_bl().map_err(to_py_err)?;
            }
            let base = r.read_3bd().map_err(to_py_err)?;
            let xref_path = if streamed {
                strings[1].clone()
            } else {
                r.read_tv().map_err(to_py_err)?
            };
            let mut description = String::new();
            let mut preview = 0;
            if !r14 {
                while r.read_rc().map_err(to_py_err)? != 0 {}
                description = if streamed {
                    strings[2].clone()
                } else {
                    r.read_tv().map_err(to_py_err)?
                };
                preview = r.read_bl().map_err(to_py_err)?;
                for _ in 0..preview {
                    r.read_rc().map_err(to_py_err)?;
                }
            }
            let mut losses = Vec::new();
            if pre.has_eed {
                losses.push("xdata");
            }
            if pre.num_reactors != 0 {
                losses.push("reactors");
            }
            if xref || overlay || unloaded || xdep || xref_index != 0 || !xref_path.is_empty() {
                losses.push("xref");
            }
            if preview != 0 {
                losses.push("preview");
            }
            if streamed {
                if r.read_bs().map_err(to_py_err)? != 0 {
                    losses.push("block_units");
                }
                if r.read_b().map_err(to_py_err)? == 0 {
                    losses.push("not_explodable");
                }
                if r.read_rc().map_err(to_py_err)? != 0 {
                    losses.push("scale_constraints");
                }
            }
            if let Some(end) = pre.data_end_bit {
                r.set_bit_pos(end);
                r.read_h().map_err(to_py_err)?;
                for _ in 0..pre.num_reactors {
                    r.read_h().map_err(to_py_err)?;
                }
                if !pre.xdic_missing && r.read_h().map_err(to_py_err)?.value != 0 {
                    losses.push("extension_dict");
                }
            }
            let d = PyDict::new_bound(py);
            d.set_item("handle", obj.handle.0)?;
            d.set_item("name", name)?;
            d.set_item("base_point", base)?;
            d.set_item("description", description)?;
            d.set_item("losses", losses)?;
            Ok(d.into_any().unbind())
        })();
        if let Ok(row) = decoded {
            rows.push(row);
        }
    }
    Ok(rows)
}

#[pyfunction]
pub fn decode_attribute_links(path: &str) -> PyResult<Vec<(u64, u64)>> {
    let shared = decoded_file(path)?;
    let decoder = shared.decoder()?;
    let best = shared.best_effort;
    let index = &shared.index;
    let mut rows = Vec::new();
    for obj in &index.objects {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best)? else {
            continue;
        };
        if header.type_code != 2 {
            continue;
        }
        if let Some((_common, handles)) =
            decode_common_entity_header_and_handles(&record, decoder.version(), &header)
        {
            rows.push((
                obj.handle.0,
                handles.next.unwrap_or(obj.handle.0.saturating_add(1)),
            ));
        }
    }
    Ok(rows)
}
