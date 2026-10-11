use crate::core::result::Result;

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

#[derive(Debug, Clone, Copy)]
pub struct EllipseEncodeInput {
    pub common: CommonEntityEncodeInput,
    pub center: (f64, f64, f64),
    pub major_axis: (f64, f64, f64),
    pub axis_ratio: f64,
    pub start_param: f64,
    pub end_param: f64,
}

pub fn encode_ellipse_entity_payload(input: EllipseEncodeInput) -> Result<Vec<u8>> {
    encode_entity_payload(0x23, input.common, |writer| {
        writer.write_3bd(input.center.0, input.center.1, input.center.2)?;
        writer.write_3bd(input.major_axis.0, input.major_axis.1, input.major_axis.2)?;
        let normal = input.common.formatting.extrusion;
        writer.write_3bd(normal.0, normal.1, normal.2)?;
        writer.write_bd(input.axis_ratio)?;
        writer.write_bd(input.start_param)?;
        writer.write_bd(input.end_param)?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bit::BitReader;
    use crate::writer::ir::{CommonEntityProps, EllipseEntity, EntityFormatting, WriterEntity};

    #[test]
    fn tilted_ellipse_keeps_wcs_vectors_radian_parameters_and_common_handles() {
        let common = CommonEntityEncodeInput {
            handle: 100,
            owner_handle: 10,
            layer_handle: 20,
            color_index: 3,
            linetype_flags: 0,
            linetype_handle: 0,
            codepage: 30,
            formatting: EntityFormatting {
                extrusion: (0., 1., 1.),
                ..Default::default()
            },
            previous_handle: 50,
            next_handle: 200,
            style_handle: 0,
        };
        let input = EllipseEncodeInput {
            common,
            center: (3., 4., 5.),
            major_axis: (2., 3., -3.),
            axis_ratio: 0.25,
            start_param: 5.5,
            end_param: 0.5,
        };
        let payload = encode_ellipse_entity_payload(input).unwrap();
        let mut reader = BitReader::new(&payload);
        reader.set_pre_r2004_layout(true);
        assert_eq!(reader.read_bs().unwrap(), 0x23);
        let decoded = crate::entities::ellipse::decode_ellipse(&mut reader).unwrap();
        assert_eq!(decoded.handle, 100);
        assert_eq!(decoded.layer_handle, 20);
        assert_eq!(decoded.color_index, Some(3));
        assert_eq!(decoded.center, input.center);
        assert_eq!(decoded.major_axis, input.major_axis);
        assert_eq!(decoded.extrusion, common.formatting.extrusion);
        assert_eq!(decoded.axis_ratio, 0.25);
        assert_eq!((decoded.start_angle, decoded.end_angle), (5.5, 0.5));
    }

    #[test]
    fn raw_ellipse_validation_rejects_degenerate_geometry_and_unsupported_thickness() {
        let good = EllipseEntity {
            common: CommonEntityProps::default(),
            center: (0., 0., 0.),
            major_axis: (2., 0., 0.),
            axis_ratio: 0.5,
            start_param: 0.,
            end_param: std::f64::consts::TAU,
        };
        let check = |e| crate::writer::validation::validate_entity(&WriterEntity::Ellipse(e), 30);
        assert!(check(good.clone()).is_ok());
        let mut bad = Vec::new();
        let mut e = good.clone();
        e.axis_ratio = 1.1;
        bad.push(e);
        let mut e = good.clone();
        e.major_axis = (0., 0., 0.);
        bad.push(e);
        let mut e = good.clone();
        e.major_axis = (1., 0., 1.);
        bad.push(e);
        let mut e = good.clone();
        e.end_param = 0.;
        bad.push(e);
        let mut e = good.clone();
        e.end_param *= 2.;
        bad.push(e);
        let mut e = good.clone();
        e.center.0 = f64::NAN;
        bad.push(e);
        let mut e = good;
        e.common.formatting.thickness = 2.;
        bad.push(e);
        for e in bad {
            assert!(check(e).is_err());
        }
    }
}
