use super::common::{encode_entity_payload, CommonEntityEncodeInput};
use crate::core::result::Result;
use crate::writer::ir::{SplineData, SplineEntity};

pub fn encode_spline_entity_payload(
    common: CommonEntityEncodeInput,
    spline: &SplineEntity,
) -> Result<Vec<u8>> {
    encode_entity_payload(0x24, common, |w| {
        w.write_bl(if matches!(spline.data, SplineData::Control { .. }) {
            1
        } else {
            2
        })?;
        w.write_bl(spline.degree)?;
        match &spline.data {
            SplineData::Control {
                rational,
                closed,
                periodic,
                knot_tolerance,
                ctrl_tolerance,
                knots,
                control_points,
                weights,
            } => {
                w.write_b(u8::from(*rational))?;
                w.write_b(u8::from(*closed))?;
                w.write_b(u8::from(*periodic))?;
                w.write_bd(*knot_tolerance)?;
                w.write_bd(*ctrl_tolerance)?;
                w.write_bl(knots.len() as u32)?;
                w.write_bl(control_points.len() as u32)?;
                w.write_b(u8::from(*rational))?;
                for knot in knots {
                    w.write_bd(*knot)?;
                }
                for (i, p) in control_points.iter().enumerate() {
                    w.write_3bd(p.0, p.1, p.2)?;
                    if *rational {
                        w.write_bd(weights[i])?;
                    }
                }
            }
            SplineData::Fit {
                fit_tolerance,
                start_tangent,
                end_tangent,
                fit_points,
            } => {
                w.write_bd(*fit_tolerance)?;
                for p in [start_tangent, end_tangent] {
                    w.write_3bd(p.0, p.1, p.2)?;
                }
                w.write_bl(fit_points.len() as u32)?;
                for p in fit_points {
                    w.write_3bd(p.0, p.1, p.2)?;
                }
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bit::BitReader;
    use crate::writer::ir::{CommonEntityProps, EntityFormatting, WriterEntity};

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
            modern: false,
            true_color: None,
            formatting: EntityFormatting::default(),
            previous_handle: 50,
            next_handle: 200,
            style_handle: 0,
        }
    }

    #[test]
    fn rational_control_spline_roundtrip_preserves_weights_knots_and_tolerances() {
        let points = vec![(0., 0., 1.), (2., 3., 4.), (5., 6., 2.)];
        let weights = vec![1., std::f64::consts::FRAC_1_SQRT_2, 1.];
        let knots = vec![2., 2., 2., 7., 7., 7.];
        let s = SplineEntity {
            common: CommonEntityProps::default(),
            degree: 2,
            data: SplineData::Control {
                rational: true,
                closed: false,
                periodic: false,
                knot_tolerance: 1e-9,
                ctrl_tolerance: 1e-8,
                knots: knots.clone(),
                control_points: points.clone(),
                weights: weights.clone(),
            },
        };
        crate::writer::validation::validate_entity(&WriterEntity::Spline(s.clone()), 30).unwrap();
        let bytes = encode_spline_entity_payload(common(), &s).unwrap();
        let mut r = BitReader::new(&bytes);
        r.set_pre_r2004_layout(true);
        assert_eq!(r.read_bs().unwrap(), 0x24);
        let out = crate::entities::spline::decode_spline(&mut r).unwrap();
        assert_eq!(out.knots, knots);
        assert_eq!(out.control_points, points);
        assert_eq!(out.weights, weights);
        assert!(out.rational);
        assert_eq!(
            (out.handle, out.layer_handle, out.color_index),
            (100, 20, Some(3))
        );
        assert_eq!(
            (out.knot_tolerance, out.ctrl_tolerance),
            (Some(1e-9), Some(1e-8))
        );
    }

    #[test]
    fn fit_spline_roundtrip_preserves_tangents_and_fit_tolerance() {
        let points = vec![(0., 0., 1.), (2., 3., 4.), (5., 6., 2.)];
        let s = SplineEntity {
            common: CommonEntityProps::default(),
            degree: 3,
            data: SplineData::Fit {
                fit_tolerance: 1e-10,
                start_tangent: (1., 0., 0.),
                end_tangent: (0., 1., 0.),
                fit_points: points.clone(),
            },
        };
        crate::writer::validation::validate_entity(&WriterEntity::Spline(s.clone()), 30).unwrap();
        let bytes = encode_spline_entity_payload(common(), &s).unwrap();
        let mut r = BitReader::new(&bytes);
        r.set_pre_r2004_layout(true);
        assert_eq!(r.read_bs().unwrap(), 0x24);
        let out = crate::entities::spline::decode_spline(&mut r).unwrap();
        assert_eq!(out.fit_points, points);
        assert_eq!(out.scenario, 2);
        assert_eq!(out.start_tangent, Some((1., 0., 0.)));
        assert_eq!(out.end_tangent, Some((0., 1., 0.)));
        assert_eq!(out.fit_tolerance, Some(1e-10));
    }

    #[test]
    fn invalid_spline_geometry_is_rejected_by_core() {
        let s = SplineEntity {
            common: CommonEntityProps::default(),
            degree: 2,
            data: SplineData::Control {
                rational: true,
                closed: false,
                periodic: false,
                knot_tolerance: 1e-7,
                ctrl_tolerance: 1e-7,
                knots: vec![0., 0., 0., 1., 1., 1.],
                control_points: vec![(0., 0., 0.), (1., 2., 0.), (2., 0., 0.)],
                weights: vec![1., -1., 1.],
            },
        };
        assert!(crate::writer::validation::validate_entity(&WriterEntity::Spline(s), 30).is_err());
        let s = SplineEntity {
            common: CommonEntityProps::default(),
            degree: 3,
            data: SplineData::Fit {
                fit_tolerance: 1e-7,
                start_tangent: (f64::NAN, 0., 0.),
                end_tangent: (0., 0., 0.),
                fit_points: vec![(0., 0., 0.), (1., 2., 3.)],
            },
        };
        assert!(crate::writer::validation::validate_entity(&WriterEntity::Spline(s), 30).is_err());
    }
}
