/// One SPATIAL_FILTER: an INSERT's clip boundary as XCLIP stores it.
type SpatialFilterRow = (
    u64,                    // handle
    Option<u64>,            // owner: the ACAD_FILTER dictionary of the INSERT's extension dictionary
    Vec<(f64, f64)>,        // boundary points, in the boundary's plane; two points are a rectangle's corners
    (f64, f64, f64),        // extrusion of the boundary's plane
    (f64, f64, f64),        // origin of the boundary's plane
    bool,                   // the boundary is displayed
    Option<f64>,            // front clipping distance, when front clipping is on
    Option<f64>,            // back clipping distance, when back clipping is on
    Vec<f64>,               // inverse of the block transform, 12 reals as stored
    Vec<f64>,               // transform of the clip boundary, 12 reals as stored
);

type SpatialFilterData = (
    Vec<(f64, f64)>,
    (f64, f64, f64),
    (f64, f64, f64),
    bool,
    Option<f64>,
    Option<f64>,
    Vec<f64>,
    Vec<f64>,
);

/// A SPATIAL_FILTER's data section after its common object data: BS point count, the points as 2RD, extrusion
/// and origin as 3BD, BS display flag, BS front clip flag and BD distance when set, the same for back, then the
/// inverse block transform and the boundary transform as 12 BD each.
fn read_spatial_filter_data(r: &mut BitReader<'_>) -> crate::core::result::Result<SpatialFilterData> {
    let count = r.read_bs()?;
    let mut points = Vec::with_capacity(count as usize);
    for _ in 0..count {
        points.push((r.read_rd(Endian::Little)?, r.read_rd(Endian::Little)?));
    }
    let extrusion = r.read_3bd()?;
    let origin = r.read_3bd()?;
    let displayed = r.read_bs()? != 0;
    let front = if r.read_bs()? != 0 { Some(r.read_bd()?) } else { None };
    let back = if r.read_bs()? != 0 { Some(r.read_bd()?) } else { None };
    let mut inverse_block = Vec::with_capacity(12);
    for _ in 0..12 {
        inverse_block.push(r.read_bd()?);
    }
    let mut boundary = Vec::with_capacity(12);
    for _ in 0..12 {
        boundary.push(r.read_bd()?);
    }
    Ok((points, extrusion, origin, displayed, front, back, inverse_block, boundary))
}

/// Every SPATIAL_FILTER of the drawing, read as stored; a record that cannot be read is left out unless
/// best-effort decoding is off.
#[pyfunction]
pub fn decode_spatial_filters(path: &str) -> PyResult<Vec<SpatialFilterRow>> {
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
        if !matches_type_name(header.type_code, 0, "SPATIAL_FILTER", dynamic) {
            continue;
        }
        let decoded = (|| -> PyResult<SpatialFilterRow> {
            let mut r = record.bit_reader();
            skip_object_type_prefix(&mut r, decoder.version()).map_err(to_py_err)?;
            let pre = read_table_record_preamble(&mut r, decoder.version(), &header)
                .map_err(to_py_err)?;
            let (points, extrusion, origin, displayed, front, back, inverse_block, boundary) =
                read_spatial_filter_data(&mut r).map_err(to_py_err)?;
            let mut owner = None;
            if let Some(end) = pre.data_end_bit {
                r.set_bit_pos(end);
                let reference = r.read_h().map_err(to_py_err)?;
                owner = objects::resolve_handle_ref(&reference, obj.handle.0).map_err(to_py_err)?;
            }
            Ok((
                obj.handle.0,
                owner,
                points,
                extrusion,
                origin,
                displayed,
                front,
                back,
                inverse_block,
                boundary,
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

/// Each DICTIONARY's handle with its owner's handle, and its entries as (name, handle): the chain from an
/// object an extension dictionary holds back to the entity it extends.
#[pyfunction]
pub fn decode_dictionary_owners(
    path: &str,
) -> PyResult<Vec<(u64, Option<u64>, Vec<(String, Option<u64>)>)>> {
    let shared = decoded_file(path)?;
    let decoder = shared.decoder()?;
    let best = shared.best_effort;
    let dynamic = &shared.dynamic_types;
    let index = &shared.index;
    let mut rows = Vec::new();
    for obj in &index.objects {
        let Some((_record, header)) = parse_record_and_header(&decoder, obj.offset, best)? else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x2A, "DICTIONARY", dynamic) {
            continue;
        }
        if let Some(dictionary) = decode_one_dictionary(&decoder, obj, best) {
            rows.push((
                obj.handle.0,
                dictionary.owner_handle.map(|handle| handle.0),
                dictionary
                    .entries
                    .into_iter()
                    .map(|entry| (entry.name, entry.value_handle.map(|handle| handle.0)))
                    .collect(),
            ));
        }
    }
    Ok(rows)
}

#[cfg(test)]
mod spatial_filter_tests {
    use super::*;
    use crate::bit::BitWriter;

    #[test]
    fn a_spatial_filter_reads_its_boundary_planes_and_matrices_to_their_end() {
        let identity = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let shifted = [1.0, 0.0, 0.0, 5.0, 0.0, 1.0, 0.0, -2.0, 0.0, 0.0, 1.0, 0.0];
        let mut w = BitWriter::new();
        w.write_bs(2).unwrap();
        for value in [125.0, 60.0, 295.0, 185.0] {
            w.write_rd(Endian::Little, value).unwrap();
        }
        w.write_3bd(0.0, 0.0, 1.0).unwrap();
        w.write_3bd(10.0, 20.0, 0.0).unwrap();
        w.write_bs(1).unwrap(); // displayed
        w.write_bs(0).unwrap(); // no front clipping
        w.write_bs(1).unwrap(); // back clipping
        w.write_bd(7.5).unwrap();
        for value in identity.iter().chain(shifted.iter()) {
            w.write_bd(*value).unwrap();
        }
        w.write_bl(0xBEEF).unwrap(); // sentinel
        let data = w.into_bytes();
        let mut reader = BitReader::new(&data);
        let (points, extrusion, origin, displayed, front, back, inverse, boundary) =
            read_spatial_filter_data(&mut reader).unwrap();
        assert_eq!(points, vec![(125.0, 60.0), (295.0, 185.0)]);
        assert_eq!((extrusion, origin), ((0.0, 0.0, 1.0), (10.0, 20.0, 0.0)));
        assert!(displayed);
        assert_eq!((front, back), (None, Some(7.5)));
        assert_eq!(inverse, identity.to_vec());
        assert_eq!(boundary, shifted.to_vec());
        assert_eq!(reader.read_bl().unwrap(), 0xBEEF);
    }
}
