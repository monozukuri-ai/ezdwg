//! DIMENSION (20-26): common dimension data, type-specific points and the
//! DIMSTYLE/anonymous block handles (ODA 20.4.22-20.4.29).
use crate::bit::{BitWriter, Endian};
use crate::core::result::Result;
use crate::writer::ir::{DimensionEntity, DimensionKind};

use super::common::{encode_entity_payload_with_handles, CommonEntityEncodeInput};

pub fn encode_dimension_entity_payload(
    common: CommonEntityEncodeInput,
    dim: &DimensionEntity,
    dimstyle_handle: u64,
    block_handle: u64,
) -> Result<Vec<u8>> {
    encode_entity_payload_with_handles(
        dim.kind.type_code(),
        common,
        |w| {
            let n = common.formatting.extrusion;
            w.write_3bd(n.0, n.1, n.2)?;
            point2rd(w, dim.text_midpoint)?;
            w.write_bd(dim.elevation)?;
            w.write_rc(dim.flags)?;
            w.write_tv(&dim.user_text)?;
            w.write_bd(dim.text_rotation)?;
            w.write_bd(dim.horizontal_direction)?;
            w.write_bd(dim.insert_scale.0)?;
            w.write_bd(dim.insert_scale.1)?;
            w.write_bd(dim.insert_scale.2)?;
            w.write_bd(dim.insert_rotation)?;
            w.write_bs(dim.attachment_point)?;
            w.write_bs(dim.line_spacing_style)?;
            w.write_bd(dim.line_spacing_factor)?;
            w.write_bd(dim.actual_measurement)?;
            point2rd(w, dim.insert_point)?;
            match &dim.kind {
                DimensionKind::Linear {
                    point13,
                    point14,
                    point10,
                    ext_line_rotation,
                    dim_rotation,
                } => {
                    point3bd(w, *point13)?;
                    point3bd(w, *point14)?;
                    point3bd(w, *point10)?;
                    w.write_bd(*ext_line_rotation)?;
                    w.write_bd(*dim_rotation)?;
                }
                DimensionKind::Aligned {
                    point13,
                    point14,
                    point10,
                    ext_line_rotation,
                } => {
                    point3bd(w, *point13)?;
                    point3bd(w, *point14)?;
                    point3bd(w, *point10)?;
                    w.write_bd(*ext_line_rotation)?;
                }
                DimensionKind::Angular3Pt {
                    point10,
                    point13,
                    point14,
                    point15,
                } => {
                    point3bd(w, *point10)?;
                    point3bd(w, *point13)?;
                    point3bd(w, *point14)?;
                    point3bd(w, *point15)?;
                }
                DimensionKind::Angular2Ln {
                    point16,
                    point13,
                    point14,
                    point15,
                    point10,
                } => {
                    point2rd(w, *point16)?;
                    point3bd(w, *point13)?;
                    point3bd(w, *point14)?;
                    point3bd(w, *point15)?;
                    point3bd(w, *point10)?;
                }
                DimensionKind::Radius {
                    point10,
                    point15,
                    leader_length,
                } => {
                    point3bd(w, *point10)?;
                    point3bd(w, *point15)?;
                    w.write_bd(*leader_length)?;
                }
                DimensionKind::Diameter {
                    point15,
                    point10,
                    leader_length,
                } => {
                    point3bd(w, *point15)?;
                    point3bd(w, *point10)?;
                    w.write_bd(*leader_length)?;
                }
                DimensionKind::Ordinate {
                    point10,
                    point13,
                    point14,
                    flags2,
                } => {
                    point3bd(w, *point10)?;
                    point3bd(w, *point13)?;
                    point3bd(w, *point14)?;
                    w.write_rc(*flags2)?;
                }
            }
            Ok(())
        },
        &[(5, dimstyle_handle), (5, block_handle)],
    )
}

fn point2rd(w: &mut BitWriter, point: (f64, f64)) -> Result<()> {
    w.write_rd(Endian::Little, point.0)?;
    w.write_rd(Endian::Little, point.1)
}

fn point3bd(w: &mut BitWriter, point: (f64, f64, f64)) -> Result<()> {
    w.write_3bd(point.0, point.1, point.2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bit::BitReader;
    use crate::entities::dim_diameter::decode_dim_diameter;
    use crate::entities::dim_linear::{decode_dim_layout, DimSpecificLayout};
    use crate::entities::dim_radius::decode_dim_radius;
    use crate::writer::ir::{CommonEntityProps, EntityFormatting};

    fn common() -> CommonEntityEncodeInput {
        CommonEntityEncodeInput {
            handle: 100,
            owner_handle: 10,
            layer_handle: 20,
            color_index: 256,
            linetype_flags: 0,
            linetype_handle: 0,
            codepage: 30,
            formatting: EntityFormatting::default(),
            previous_handle: 50,
            next_handle: 200,
            style_handle: 0,
        }
    }

    fn dimension(kind: DimensionKind) -> DimensionEntity {
        DimensionEntity {
            common: CommonEntityProps::default(),
            kind,
            text_midpoint: (5., 7.5),
            elevation: 0.,
            flags: 3,
            user_text: "<>".into(),
            text_rotation: 0.1,
            horizontal_direction: 0.,
            insert_scale: (1., 1., 1.),
            insert_rotation: 0.,
            attachment_point: 5,
            line_spacing_style: 1,
            line_spacing_factor: 1.,
            actual_measurement: 10.,
            insert_point: (0., 0.),
            dimstyle: "Standard".into(),
            block_name: "*D1".into(),
        }
    }

    #[test]
    fn linear_and_aligned_dimensions_roundtrip_through_the_reader() {
        for (kind, layout, code) in [
            (
                DimensionKind::Linear {
                    point13: (0., 0., 0.),
                    point14: (10., 0., 0.),
                    point10: (10., 5., 0.),
                    ext_line_rotation: 0.,
                    dim_rotation: 0.25,
                },
                DimSpecificLayout::Linear,
                21,
            ),
            (
                DimensionKind::Aligned {
                    point13: (0., 0., 0.),
                    point14: (10., 0., 0.),
                    point10: (10., 5., 0.),
                    ext_line_rotation: 0.5,
                },
                DimSpecificLayout::Aligned,
                22,
            ),
        ] {
            let payload =
                encode_dimension_entity_payload(common(), &dimension(kind), 0x39A, 0x529).unwrap();
            let mut reader = BitReader::new(&payload);
            reader.set_pre_r2004_layout(true);
            assert_eq!(reader.read_bs().unwrap(), code);
            let decoded = decode_dim_layout(&mut reader, layout).unwrap();
            assert_eq!(decoded.common.handle, 100);
            assert_eq!(decoded.common.layer_handle, 20);
            assert_eq!(decoded.common.user_text, "<>");
            assert_eq!(decoded.common.text_midpoint, (5., 7.5, 0.));
            assert_eq!(decoded.common.attachment_point, Some(5));
            assert_eq!(decoded.common.actual_measurement, Some(10.));
            assert_eq!(decoded.common.dimstyle_handle, Some(0x39A));
            assert_eq!(decoded.common.anonymous_block_handle, Some(0x529));
            assert_eq!(decoded.point13, (0., 0., 0.));
            assert_eq!(decoded.point14, (10., 0., 0.));
            assert_eq!(decoded.point10, (10., 5., 0.));
            if layout == DimSpecificLayout::Linear {
                assert!((decoded.dim_rotation - 0.25).abs() < 1e-12);
            } else {
                assert!((decoded.ext_line_rotation - 0.5).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn radial_dimensions_keep_their_points_and_leader_length() {
        let radius = dimension(DimensionKind::Radius {
            point10: (0., 0., 0.),
            point15: (3., 4., 0.),
            leader_length: 2.,
        });
        let payload = encode_dimension_entity_payload(common(), &radius, 1, 2).unwrap();
        let mut reader = BitReader::new(&payload);
        reader.set_pre_r2004_layout(true);
        assert_eq!(reader.read_bs().unwrap(), 25);
        let decoded = decode_dim_radius(&mut reader).unwrap();
        assert_eq!(decoded.point10, (0., 0., 0.));
        assert_eq!(decoded.point15, Some((3., 4., 0.)));

        let diameter = dimension(DimensionKind::Diameter {
            point15: (-3., -4., 0.),
            point10: (3., 4., 0.),
            leader_length: 0.,
        });
        let payload = encode_dimension_entity_payload(common(), &diameter, 1, 2).unwrap();
        let mut reader = BitReader::new(&payload);
        reader.set_pre_r2004_layout(true);
        assert_eq!(reader.read_bs().unwrap(), 26);
        let decoded = decode_dim_diameter(&mut reader).unwrap();
        assert_eq!(decoded.point10, (3., 4., 0.));
        assert_eq!(decoded.point15, Some((-3., -4., 0.)));
    }

    #[test]
    fn angular_and_ordinate_dimensions_use_their_specific_layouts() {
        let cases = [
            (
                DimensionKind::Angular3Pt {
                    point10: (5., 5., 0.),
                    point13: (10., 0., 0.),
                    point14: (0., 10., 0.),
                    point15: (0., 0., 0.),
                },
                DimSpecificLayout::Ang3Pt,
                23,
            ),
            (
                DimensionKind::Angular2Ln {
                    point16: (6., 6.),
                    point13: (0., 0., 0.),
                    point14: (10., 0., 0.),
                    point15: (0., 0., 0.),
                    point10: (0., 10., 0.),
                },
                DimSpecificLayout::Ang2Ln,
                24,
            ),
            (
                DimensionKind::Ordinate {
                    point10: (0., 0., 0.),
                    point13: (10., 5., 0.),
                    point14: (10., 15., 0.),
                    flags2: 1,
                },
                DimSpecificLayout::Ordinate,
                20,
            ),
        ];
        for (kind, layout, code) in cases {
            let payload =
                encode_dimension_entity_payload(common(), &dimension(kind.clone()), 1, 2).unwrap();
            let mut reader = BitReader::new(&payload);
            reader.set_pre_r2004_layout(true);
            assert_eq!(reader.read_bs().unwrap(), code);
            let decoded = decode_dim_layout(&mut reader, layout).unwrap();
            match kind {
                DimensionKind::Angular3Pt { point15, .. } => {
                    assert_eq!(decoded.point15, Some(point15));
                    assert_eq!(decoded.point10, (5., 5., 0.));
                }
                DimensionKind::Angular2Ln { point16, .. } => {
                    assert_eq!(decoded.point16, Some(point16));
                    assert_eq!(decoded.point10, (0., 10., 0.));
                }
                DimensionKind::Ordinate { point14, .. } => {
                    assert_eq!(decoded.point14, point14);
                }
                _ => unreachable!(),
            }
        }
    }
}
