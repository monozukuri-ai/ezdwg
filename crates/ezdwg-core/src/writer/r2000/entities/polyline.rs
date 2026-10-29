use super::common::{
    encode_entity_payload, encode_entity_payload_with_handles, CommonEntityEncodeInput,
};
use crate::core::result::Result;
use crate::writer::ir::{PolylineEntity, PolylineVertex};

pub fn encode_polyline(
    common: CommonEntityEncodeInput,
    poly: &PolylineEntity,
    is_3d: bool,
    vertices: &[u64],
    seqend: u64,
) -> Result<Vec<u8>> {
    let refs = [
        (4, vertices[0]),
        (4, vertices[vertices.len() - 1]),
        (3, seqend),
    ];
    encode_entity_payload_with_handles(
        if is_3d { 0x10 } else { 0x0f },
        common,
        |w| {
            if is_3d {
                w.write_rc(0)?; // no spline fitting
                w.write_rc((poly.flags & 1) as u8)?;
            } else {
                w.write_bs(poly.flags)?;
                w.write_bs(0)?; // no curve fitting
                w.write_bd(poly.width_start)?;
                w.write_bd(poly.width_end)?;
                w.write_bt(common.formatting.thickness)?;
                w.write_bd(common.formatting.elevation)?;
                let n = common.formatting.extrusion;
                w.write_be(n.0, n.1, n.2)?;
            }
            Ok(())
        },
        &refs,
    )
}

pub fn encode_vertex(
    common: CommonEntityEncodeInput,
    vertex: &PolylineVertex,
    is_3d: bool,
) -> Result<Vec<u8>> {
    encode_entity_payload(if is_3d { 0x0b } else { 0x0a }, common, |w| {
        w.write_rc(if is_3d { 32 } else { 0 })?;
        let p = vertex.position;
        w.write_3bd(p.0, p.1, p.2)?;
        if !is_3d {
            w.write_bd(vertex.start_width)?;
            w.write_bd(vertex.end_width)?;
            w.write_bd(vertex.bulge)?;
            w.write_bd(0.)?; // tangent direction
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bit::BitReader;
    use crate::entities::common::{parse_common_entity_handles, parse_common_entity_header};

    fn common() -> CommonEntityEncodeInput {
        CommonEntityEncodeInput {
            handle: 100,
            owner_handle: 10,
            entity_mode: 0,
            layer_handle: 20,
            color_index: 256,
            linetype_flags: 0,
            linetype_handle: 0,
            codepage: 30,
            modern: false,
            true_color: None,
            formatting: Default::default(),
            previous_handle: 90,
            next_handle: 110,
            style_handle: 0,
        }
    }

    #[test]
    fn legacy_polyline_header_retains_geometry_and_owned_sequence_references() {
        let mut c = common();
        c.formatting.extrusion = (0., 1., 0.);
        c.formatting.elevation = 4.;
        c.formatting.thickness = 2.;
        for is_3d in [false, true] {
            let p = PolylineEntity {
                flags: if is_3d { 9 } else { 129 },
                width_start: 0.5,
                width_end: 0.75,
                ..Default::default()
            };
            let bytes = encode_polyline(c, &p, is_3d, &[500, 300, 400], 600).unwrap();
            let mut r = BitReader::new(&bytes);
            r.set_pre_r2004_layout(true);
            assert_eq!(r.read_bs().unwrap(), if is_3d { 16 } else { 15 });
            let mut geometry = r.clone();
            if is_3d {
                let e =
                    crate::entities::polyline_3d::decode_polyline_3d_r2000(&mut geometry).unwrap();
                assert_eq!((e.flags_75_bits, e.flags_70_bits), (0, 1));
            } else {
                let e =
                    crate::entities::polyline_2d::decode_polyline_2d_r2000(&mut geometry).unwrap();
                assert_eq!(e.flags, 129);
                assert_eq!((e.width_start, e.width_end), (0.5, 0.75));
                assert_eq!((e.elevation, e.thickness), (4., 2.));
                assert_eq!(e.extrusion, (0., 1., 0.));
            }
            let h = parse_common_entity_header(&mut r).unwrap();
            r.set_bit_pos(h.obj_size);
            let handles = parse_common_entity_handles(&mut r, &h).unwrap();
            assert_eq!(handles.owner_ref, Some(10));
            for expected in [(4, 500), (4, 400), (3, 600)] {
                let actual = r.read_h().unwrap();
                assert_eq!((actual.code, actual.value), expected);
            }
        }
    }

    #[test]
    fn vertices_keep_2d_widths_bulge_and_3d_coordinates() {
        let v = PolylineVertex {
            position: (2., 3., 4.),
            start_width: 0.5,
            end_width: 0.75,
            bulge: -0.25,
        };
        for is_3d in [false, true] {
            let bytes = encode_vertex(common(), &v, is_3d).unwrap();
            let mut r = BitReader::new(&bytes);
            r.set_pre_r2004_layout(true);
            assert_eq!(r.read_bs().unwrap(), if is_3d { 11 } else { 10 });
            if is_3d {
                let e = crate::entities::vertex_3d::decode_vertex_3d(&mut r).unwrap();
                assert_eq!(e.flags, 32);
                assert_eq!(e.position, v.position);
            } else {
                let e = crate::entities::vertex_2d::decode_vertex_2d(&mut r).unwrap();
                assert_eq!(e.position, v.position);
                assert_eq!((e.start_width, e.end_width, e.bulge), (0.5, 0.75, -0.25));
                assert_eq!(e.owner_handle, Some(10));
            }
        }
    }
}
