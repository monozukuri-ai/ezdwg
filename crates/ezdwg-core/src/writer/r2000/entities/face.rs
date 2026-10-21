//! Four stored corners: planar OCS SOLID/TRACE and compressed WCS 3DFACE.
use crate::bit::Endian;
use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

pub fn encode_planar_face_entity_payload(
    common: CommonEntityEncodeInput,
    points: [(f64, f64, f64); 4],
    is_trace: bool,
) -> Result<Vec<u8>> {
    encode_entity_payload(if is_trace { 0x20 } else { 0x1f }, common, |writer| {
        writer.write_bt(common.formatting.thickness)?;
        writer.write_bd(points[0].2)?;
        for (x, y, _) in points {
            writer.write_rd(Endian::Little, x)?;
            writer.write_rd(Endian::Little, y)?;
        }
        let n = common.formatting.extrusion;
        writer.write_be(n.0, n.1, n.2)
    })
}

pub fn encode_3dface_entity_payload(
    common: CommonEntityEncodeInput,
    points: [(f64, f64, f64); 4],
    invisible_edge_flags: u16,
) -> Result<Vec<u8>> {
    encode_entity_payload(0x1c, common, |writer| {
        writer.write_b(u8::from(invisible_edge_flags == 0))?;
        writer.write_b(u8::from(points[0].2 == 0.))?;
        writer.write_rd(Endian::Little, points[0].0)?;
        writer.write_rd(Endian::Little, points[0].1)?;
        if points[0].2 != 0. {
            writer.write_rd(Endian::Little, points[0].2)?;
        }
        for pair in points.windows(2) {
            writer.write_dd(pair[0].0, pair[1].0)?;
            writer.write_dd(pair[0].1, pair[1].1)?;
            writer.write_dd(pair[0].2, pair[1].2)?;
        }
        if invisible_edge_flags != 0 {
            writer.write_bs(invisible_edge_flags)?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bit::BitReader;
    use crate::writer::ir::{
        CommonEntityProps, EntityFormatting, Face3dEntity, PlanarFaceEntity, WriterEntity,
    };

    fn common() -> CommonEntityEncodeInput {
        CommonEntityEncodeInput {
            handle: 100,
            owner_handle: 10,
            entity_mode: 0,
            layer_handle: 20,
            color_index: 3,
            linetype_flags: 0,
            linetype_handle: 0,
            codepage: 30,
            formatting: EntityFormatting::default(),
            previous_handle: 50,
            next_handle: 200,
            style_handle: 0,
        }
    }

    #[test]
    fn planar_faces_preserve_stored_order_elevation_thickness_and_normal() {
        let mut common = common();
        common.formatting.thickness = -2.;
        common.formatting.extrusion = (0., 0.6, 0.8);
        let points = [(1., 2., 5.), (7., 3., 5.), (2., 8., 5.), (6., 9., 5.)];
        for is_trace in [false, true] {
            let payload = encode_planar_face_entity_payload(common, points, is_trace).unwrap();
            let mut reader = BitReader::new(&payload);
            reader.set_pre_r2004_layout(true);
            assert_eq!(
                reader.read_bs().unwrap(),
                if is_trace { 0x20 } else { 0x1f }
            );
            let (actual, handle, layer, color, thickness, extrusion) = if is_trace {
                let v = crate::entities::trace::decode_trace(&mut reader).unwrap();
                (
                    [v.p1, v.p2, v.p3, v.p4],
                    v.handle,
                    v.layer_handle,
                    v.color_index,
                    v.thickness,
                    v.extrusion,
                )
            } else {
                let v = crate::entities::solid::decode_solid(&mut reader).unwrap();
                (
                    [v.p1, v.p2, v.p3, v.p4],
                    v.handle,
                    v.layer_handle,
                    v.color_index,
                    v.thickness,
                    v.extrusion,
                )
            };
            assert_eq!(actual, points);
            assert_eq!((handle, layer, color), (100, 20, Some(3)));
            assert_eq!(thickness, -2.);
            assert_eq!(extrusion, common.formatting.extrusion);
        }
    }

    #[test]
    fn face_compression_handles_zero_z_repeated_corners_and_all_edge_masks() {
        for z in [0., 2.] {
            let points = [(1., 2., z), (1., 4., 5.), (6., 4., -3.), (6., 4., -3.)];
            for flags in 0..16 {
                let payload = encode_3dface_entity_payload(common(), points, flags).unwrap();
                let mut reader = BitReader::new(&payload);
                reader.set_pre_r2004_layout(true);
                assert_eq!(reader.read_bs().unwrap(), 0x1c);
                let v = crate::entities::face3d::decode_3dface(&mut reader).unwrap();
                assert_eq!([v.p1, v.p2, v.p3, v.p4], points);
                assert_eq!(v.invisible_edge_flags, flags);
                assert_eq!(
                    (v.handle, v.layer_handle, v.color_index),
                    (100, 20, Some(3))
                );
            }
        }
    }

    #[test]
    fn invalid_faces_fail_core_validation() {
        let points = [(0., 0., 0.), (4., 0., 0.), (0., 3., 0.), (4., 3., 0.)];
        let good = PlanarFaceEntity {
            common: CommonEntityProps::default(),
            points,
        };
        let check = |e| crate::writer::validation::validate_entity(&e, 30);
        assert!(check(WriterEntity::Solid(good.clone())).is_ok());
        let mut v = good.clone();
        v.points[3].2 = 1.;
        assert!(check(WriterEntity::Solid(v.clone())).is_err());
        assert!(check(WriterEntity::Trace(v)).is_err());
        let mut v = good;
        v.points[0].0 = f64::NAN;
        assert!(check(WriterEntity::Trace(v)).is_err());
        let good = Face3dEntity {
            common: CommonEntityProps::default(),
            points,
            invisible_edge_flags: 15,
        };
        assert!(check(WriterEntity::Face3d(good.clone())).is_ok());
        let mut v = good.clone();
        v.invisible_edge_flags = 16;
        assert!(check(WriterEntity::Face3d(v)).is_err());
        let mut v = good.clone();
        v.common.formatting.thickness = 1.;
        assert!(check(WriterEntity::Face3d(v)).is_err());
        let mut v = good;
        v.common.formatting.extrusion = (0., 1., 0.);
        assert!(check(WriterEntity::Face3d(v)).is_err());
    }
}
