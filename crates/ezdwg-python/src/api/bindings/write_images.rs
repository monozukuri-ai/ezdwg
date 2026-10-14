/// IMAGE rows and IMAGEDEF table rows are dictionaries.
fn append_write_images(
    doc: &mut writer::WriterDocument,
    images: Option<Vec<Bound<'_, PyDict>>>,
    image_defs: Option<Vec<Bound<'_, PyDict>>>,
) -> PyResult<()> {
    use writer::ir::{ImageDef, ImageEntity};
    for d in image_defs.unwrap_or_default() {
        check_write_keys(
            &d,
            &[
                "name",
                "file_path",
                "size",
                "is_loaded",
                "resolution_units",
                "pixel_size",
            ],
        )?;
        doc.image_defs.push(ImageDef {
            name: write_value(&d, "name", String::new())?,
            file_path: write_value(&d, "file_path", String::new())?,
            size: write_value(&d, "size", (0., 0.))?,
            is_loaded: write_value(&d, "is_loaded", true)?,
            resolution_units: write_value(&d, "resolution_units", 0u8)?,
            pixel_size: write_value(&d, "pixel_size", (1., 1.))?,
        });
    }
    for d in images.unwrap_or_default() {
        check_write_keys(
            &d,
            &[
                "handle",
                "image_def",
                "insert",
                "u_vector",
                "v_vector",
                "size",
                "display_props",
                "clipping",
                "brightness",
                "contrast",
                "fade",
                "clip_type",
                "clip_vertices",
            ],
        )?;
        let handle = write_value(&d, "handle", 0u64)?;
        let size: (f64, f64) = write_value(&d, "size", (0., 0.))?;
        let clip_type: u16 = write_value(&d, "clip_type", 1u16)?;
        let mut clip_vertices: Vec<(f64, f64)> = write_value(&d, "clip_vertices", Vec::new())?;
        if clip_type == 1 && clip_vertices.is_empty() {
            clip_vertices = vec![(-0.5, -0.5), (size.0 - 0.5, size.1 - 0.5)];
        }
        doc.modelspace.push(writer::WriterEntity::Image(ImageEntity {
            common: writer::CommonEntityProps {
                handle: (handle != 0).then_some(handle),
                ..Default::default()
            },
            image_def: write_value(&d, "image_def", String::new())?,
            insert: write_value(&d, "insert", (0., 0., 0.))?,
            u_vector: write_value(&d, "u_vector", (1., 0., 0.))?,
            v_vector: write_value(&d, "v_vector", (0., 1., 0.))?,
            size,
            display_props: write_value(&d, "display_props", 3u16)?,
            clipping: write_value(&d, "clipping", false)?,
            brightness: write_value(&d, "brightness", 50u8)?,
            contrast: write_value(&d, "contrast", 50u8)?,
            fade: write_value(&d, "fade", 0u8)?,
            clip_type,
            clip_vertices,
        }));
    }
    Ok(())
}


/// IMAGEDEF objects: `(handle, file_path, size, is_loaded, resolution_units, pixel_size)`.
#[pyfunction]
pub fn decode_image_defs(
    path: &str,
) -> PyResult<Vec<(u64, String, (f64, f64), bool, u8, (f64, f64))>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best = is_best_effort_compat_version(&decoder);
    let dynamic = load_dynamic_types(&decoder, best)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut rows = Vec::new();
    for obj in &index.objects {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best)? else {
            continue;
        };
        if !matches_type_name(header.type_code, 0, "IMAGEDEF", &dynamic) {
            continue;
        }
        let decoded = (|| -> PyResult<_> {
            let mut r = record.bit_reader();
            skip_object_type_prefix(&mut r, decoder.version()).map_err(to_py_err)?;
            let pre = read_table_record_preamble(&mut r, decoder.version(), &header)
                .map_err(to_py_err)?;
            let streamed = table_record_strings_in_stream(decoder.version());
            let _class_version = r.read_bl().map_err(to_py_err)?;
            let size = (
                r.read_rd(Endian::Little).map_err(to_py_err)?,
                r.read_rd(Endian::Little).map_err(to_py_err)?,
            );
            let file_path = if streamed {
                read_table_record_stream_strings(&r, pre.data_end_bit, 1)
                    .into_iter()
                    .next()
                    .unwrap_or_default()
            } else {
                r.read_tv().map_err(to_py_err)?
            };
            let is_loaded = r.read_b().map_err(to_py_err)? != 0;
            let resolution_units = r.read_rc().map_err(to_py_err)?;
            let pixel_size = (
                r.read_rd(Endian::Little).map_err(to_py_err)?,
                r.read_rd(Endian::Little).map_err(to_py_err)?,
            );
            Ok((
                obj.handle.0,
                file_path,
                size,
                is_loaded,
                resolution_units,
                pixel_size,
            ))
        })();
        match decoded {
            Ok(row) => rows.push(row),
            Err(_) if best => continue,
            Err(err) => return Err(err),
        }
    }
    Ok(rows)
}
