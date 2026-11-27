//! HATCH (0x4E): boundary paths, pattern definition and seed points (ODA 20.4.75).
use crate::bit::Endian;
use crate::core::result::Result;
use crate::writer::ir::{HatchEdge, HatchEntity, HatchPathData};

use super::common::{encode_entity_payload, CommonEntityEncodeInput};

pub fn encode_hatch_entity_payload(
    common: CommonEntityEncodeInput,
    hatch: &HatchEntity,
) -> Result<Vec<u8>> {
    encode_entity_payload(0x4E, common, |w| {
        w.write_bd(common.formatting.elevation)?;
        let n = common.formatting.extrusion;
        w.write_3bd(n.0, n.1, n.2)?;
        w.write_tv(&hatch.name)?;
        w.write_b(u8::from(hatch.solid_fill))?;
        w.write_b(0)?; // associative: the writer stores no boundary objects
        w.write_bl(hatch.paths.len() as u32)?;
        let mut derived = false;
        for path in &hatch.paths {
            w.write_bl(path.flags)?;
            derived |= path.flags & 0x04 != 0;
            match &path.data {
                HatchPathData::Edges(edges) => {
                    w.write_bl(edges.len() as u32)?;
                    for edge in edges {
                        match edge {
                            HatchEdge::Line { start, end } => {
                                w.write_rc(1)?;
                                point2rd(w, *start)?;
                                point2rd(w, *end)?;
                            }
                            HatchEdge::Arc {
                                center,
                                radius,
                                start_angle,
                                end_angle,
                                ccw,
                            } => {
                                w.write_rc(2)?;
                                point2rd(w, *center)?;
                                w.write_bd(*radius)?;
                                w.write_bd(*start_angle)?;
                                w.write_bd(*end_angle)?;
                                w.write_b(u8::from(*ccw))?;
                            }
                            HatchEdge::Ellipse {
                                center,
                                major_endpoint,
                                ratio,
                                start_angle,
                                end_angle,
                                ccw,
                            } => {
                                w.write_rc(3)?;
                                point2rd(w, *center)?;
                                point2rd(w, *major_endpoint)?;
                                w.write_bd(*ratio)?;
                                w.write_bd(*start_angle)?;
                                w.write_bd(*end_angle)?;
                                w.write_b(u8::from(*ccw))?;
                            }
                            HatchEdge::Spline {
                                degree,
                                rational,
                                periodic,
                                knots,
                                control_points,
                                weights,
                            } => {
                                w.write_rc(4)?;
                                w.write_bl(*degree)?;
                                w.write_b(u8::from(*rational))?;
                                w.write_b(u8::from(*periodic))?;
                                w.write_bl(knots.len() as u32)?;
                                w.write_bl(control_points.len() as u32)?;
                                for knot in knots {
                                    w.write_bd(*knot)?;
                                }
                                for (i, point) in control_points.iter().enumerate() {
                                    point2rd(w, *point)?;
                                    if *rational {
                                        w.write_bd(weights.get(i).copied().unwrap_or(1.))?;
                                    }
                                }
                            }
                        }
                    }
                }
                HatchPathData::Polyline {
                    closed,
                    vertices,
                    bulges,
                } => {
                    w.write_b(u8::from(bulges.is_some()))?;
                    w.write_b(u8::from(*closed))?;
                    w.write_bl(vertices.len() as u32)?;
                    for (i, vertex) in vertices.iter().enumerate() {
                        point2rd(w, *vertex)?;
                        if let Some(bulges) = bulges {
                            w.write_bd(bulges.get(i).copied().unwrap_or(0.))?;
                        }
                    }
                }
            }
            w.write_bl(0)?; // boundary object handles
        }
        w.write_bs(hatch.style)?;
        w.write_bs(hatch.pattern_type)?;
        if !hatch.solid_fill {
            if let Some(pattern) = &hatch.pattern {
                w.write_bd(pattern.angle)?;
                w.write_bd(pattern.scale)?;
                w.write_b(u8::from(pattern.double))?;
                w.write_bs(pattern.lines.len() as u16)?;
                for line in &pattern.lines {
                    w.write_bd(line.angle)?;
                    w.write_bd(line.base.0)?;
                    w.write_bd(line.base.1)?;
                    w.write_bd(line.offset.0)?;
                    w.write_bd(line.offset.1)?;
                    w.write_bs(line.dashes.len() as u16)?;
                    for dash in &line.dashes {
                        w.write_bd(*dash)?;
                    }
                }
            } else {
                // Validation requires a definition; an empty one keeps the layout.
                w.write_bd(0.)?;
                w.write_bd(1.)?;
                w.write_b(0)?;
                w.write_bs(0)?;
            }
        }
        if derived {
            w.write_bd(hatch.pixel_size.unwrap_or(1.))?;
        }
        w.write_bl(hatch.seed_points.len() as u32)?;
        for seed in &hatch.seed_points {
            point2rd(w, *seed)?;
        }
        Ok(())
    })
}

fn point2rd(w: &mut crate::bit::BitWriter, point: (f64, f64)) -> Result<()> {
    w.write_rd(Endian::Little, point.0)?;
    w.write_rd(Endian::Little, point.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bit::BitReader;
    use crate::entities::hatch::decode_hatch;
    use crate::writer::ir::{
        CommonEntityProps, EntityFormatting, HatchPathDef, HatchPatternDef, HatchPatternLineDef,
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
            modern: false,
            true_color: None,
            formatting: EntityFormatting {
                elevation: 2.5,
                extrusion: (0., 0., 1.),
                ..EntityFormatting::default()
            },
            previous_handle: 50,
            next_handle: 200,
            style_handle: 0,
        }
    }

    #[test]
    fn solid_polyline_hatch_roundtrips_through_the_reader() {
        let hatch = HatchEntity {
            common: CommonEntityProps::default(),
            name: "SOLID".into(),
            solid_fill: true,
            paths: vec![HatchPathDef {
                flags: 3,
                data: HatchPathData::Polyline {
                    closed: true,
                    vertices: vec![(0., 0.), (10., 0.), (10., 5.), (0., 5.)],
                    bulges: None,
                },
            }],
            style: 1,
            pattern_type: 1,
            pattern: None,
            pixel_size: None,
            seed_points: vec![],
        };
        let payload = encode_hatch_entity_payload(common(), &hatch).unwrap();
        let mut reader = BitReader::new(&payload);
        reader.set_pre_r2004_layout(true);
        assert_eq!(reader.read_bs().unwrap(), 0x4E);
        let decoded = decode_hatch(&mut reader).unwrap();
        assert_eq!(decoded.handle, 100);
        assert_eq!(decoded.layer_handle, 20);
        assert_eq!(decoded.color_index, Some(3));
        assert_eq!(decoded.name, "SOLID");
        assert!(decoded.solid_fill && !decoded.associative);
        assert_eq!(decoded.elevation, 2.5);
        assert_eq!(decoded.paths.len(), 1);
        assert!(decoded.paths[0].closed);
        assert_eq!(
            decoded.paths[0].points,
            vec![(0., 0.), (10., 0.), (10., 5.), (0., 5.), (0., 0.)]
        );
        assert!(decoded.pattern.is_none());
    }

    #[test]
    fn pattern_hatch_with_edges_keeps_its_definition() {
        let hatch = HatchEntity {
            common: CommonEntityProps::default(),
            name: "ANSI31".into(),
            solid_fill: false,
            paths: vec![HatchPathDef {
                flags: 1,
                data: HatchPathData::Edges(vec![
                    HatchEdge::Line {
                        start: (0., 0.),
                        end: (4., 0.),
                    },
                    HatchEdge::Arc {
                        center: (4., 2.),
                        radius: 2.,
                        start_angle: -std::f64::consts::FRAC_PI_2,
                        end_angle: std::f64::consts::FRAC_PI_2,
                        ccw: true,
                    },
                    HatchEdge::Line {
                        start: (4., 4.),
                        end: (0., 4.),
                    },
                    HatchEdge::Line {
                        start: (0., 4.),
                        end: (0., 0.),
                    },
                ]),
            }],
            style: 0,
            pattern_type: 1,
            pattern: Some(HatchPatternDef {
                angle: 0.5,
                scale: 2.,
                double: true,
                lines: vec![HatchPatternLineDef {
                    angle: 0.75,
                    base: (0., 0.),
                    offset: (-1.5, 1.5),
                    dashes: vec![1., -0.5],
                }],
            }),
            pixel_size: None,
            seed_points: vec![(1., 1.)],
        };
        let payload = encode_hatch_entity_payload(common(), &hatch).unwrap();
        let mut reader = BitReader::new(&payload);
        reader.set_pre_r2004_layout(true);
        assert_eq!(reader.read_bs().unwrap(), 0x4E);
        let decoded = decode_hatch(&mut reader).unwrap();
        assert_eq!(decoded.name, "ANSI31");
        assert!(!decoded.solid_fill);
        let path = &decoded.paths[0];
        assert!(path.closed);
        assert_eq!(path.points.first(), Some(&(0., 0.)));
        assert_eq!(path.points.last(), Some(&(0., 0.)));
        assert!(
            path.points.len() > 10,
            "the arc edge is tessellated by the reader"
        );
        let pattern = decoded.pattern.expect("pattern definition");
        assert_eq!(
            (pattern.angle, pattern.scale, pattern.double),
            (0.5, 2., true)
        );
        assert_eq!(pattern.lines.len(), 1);
        assert_eq!(pattern.lines[0].angle, 0.75);
        assert_eq!(pattern.lines[0].offset, (-1.5, 1.5));
        assert_eq!(pattern.lines[0].dashes, vec![1., -0.5]);
    }

    #[test]
    fn bulged_polyline_and_derived_flag_write_pixel_size() {
        let hatch = HatchEntity {
            common: CommonEntityProps::default(),
            name: "SOLID".into(),
            solid_fill: true,
            paths: vec![HatchPathDef {
                flags: 7,
                data: HatchPathData::Polyline {
                    closed: true,
                    vertices: vec![(0., 0.), (10., 0.), (10., 10.)],
                    bulges: Some(vec![0., 1., 0.]),
                },
            }],
            style: 0,
            pattern_type: 1,
            pattern: None,
            pixel_size: Some(0.25),
            seed_points: vec![],
        };
        let payload = encode_hatch_entity_payload(common(), &hatch).unwrap();
        let mut reader = BitReader::new(&payload);
        reader.set_pre_r2004_layout(true);
        reader.read_bs().unwrap();
        let decoded = decode_hatch(&mut reader).unwrap();
        assert!(decoded.paths[0].closed);
        assert!(
            decoded.paths[0].points.len() > 4,
            "the bulge is tessellated"
        );
    }

    fn solid_hatch_payload(paths: Vec<HatchPathData>) -> Vec<u8> {
        let hatch = HatchEntity {
            common: CommonEntityProps::default(),
            name: "SOLID".into(),
            solid_fill: true,
            paths: paths
                .into_iter()
                .map(|data| HatchPathDef {
                    flags: if matches!(data, HatchPathData::Edges(_)) {
                        1
                    } else {
                        3
                    },
                    data,
                })
                .collect(),
            style: 0,
            pattern_type: 1,
            pattern: None,
            pixel_size: None,
            seed_points: vec![],
        };
        encode_hatch_entity_payload(common(), &hatch).unwrap()
    }

    fn decode_paths(paths: Vec<HatchPathData>) -> crate::entities::hatch::HatchEntity {
        let payload = solid_hatch_payload(paths);
        let mut reader = BitReader::new(&payload);
        reader.set_pre_r2004_layout(true);
        reader.read_bs().unwrap();
        decode_hatch(&mut reader).unwrap()
    }

    fn decode_edge_paths(paths: Vec<Vec<HatchEdge>>) -> crate::entities::hatch::HatchEntity {
        decode_paths(paths.into_iter().map(HatchPathData::Edges).collect())
    }

    fn long_line_path() -> Vec<HatchEdge> {
        // 140,000 line edges charge 280,000 points, over the 262,144 budget.
        (0..140_000)
            .map(|i| HatchEdge::Line {
                start: (f64::from(i), 0.),
                end: (f64::from(i + 1), 0.),
            })
            .collect()
    }

    fn square(x: f64) -> Vec<HatchEdge> {
        let corners = [(x, 0.), (x + 1., 0.), (x + 1., 1.), (x, 1.)];
        (0..4)
            .map(|i| HatchEdge::Line {
                start: corners[i],
                end: corners[(i + 1) % 4],
            })
            .collect()
    }

    #[test]
    fn clockwise_arc_edge_is_flattened_from_its_mirrored_angles() {
        // Bottom (4, 0) clockwise through (2, 2) to top (4, 4): stored as the
        // negated real angles -90 and -270 degrees, as DXF group codes 50/51 store them.
        let decoded = decode_edge_paths(vec![vec![
            HatchEdge::Line {
                start: (0., 0.),
                end: (4., 0.),
            },
            HatchEdge::Arc {
                center: (4., 2.),
                radius: 2.,
                start_angle: std::f64::consts::FRAC_PI_2,
                end_angle: 3. * std::f64::consts::FRAC_PI_2,
                ccw: false,
            },
            HatchEdge::Line {
                start: (4., 4.),
                end: (0., 0.),
            },
        ]]);
        let points = &decoded.paths[0].points;
        assert!(points.iter().all(|p| p.0 <= 4. + 1e-9), "{points:?}");
        assert!(points
            .iter()
            .any(|p| (p.0 - 2.).abs() < 1e-9 && (p.1 - 2.).abs() < 1e-9));
        let (start, end) = (points[1], points[points.len() - 2]);
        assert!(
            (start.0 - 4.).abs() < 1e-9 && start.1.abs() < 1e-9,
            "{start:?}"
        );
        assert!(
            (end.0 - 4.).abs() < 1e-9 && (end.1 - 4.).abs() < 1e-9,
            "{end:?}"
        );
    }

    #[test]
    fn clockwise_ellipse_edge_is_flattened_from_its_mirrored_parameters() {
        // (4, 1) clockwise through (2, 2) to (4, 3): real parameters -90 and -270 degrees.
        let decoded = decode_edge_paths(vec![vec![
            HatchEdge::Line {
                start: (0., 1.),
                end: (4., 1.),
            },
            HatchEdge::Ellipse {
                center: (4., 2.),
                major_endpoint: (2., 0.),
                ratio: 0.5,
                start_angle: std::f64::consts::FRAC_PI_2,
                end_angle: 3. * std::f64::consts::FRAC_PI_2,
                ccw: false,
            },
            HatchEdge::Line {
                start: (4., 3.),
                end: (0., 1.),
            },
        ]]);
        let points = &decoded.paths[0].points;
        assert!(points.iter().all(|p| p.0 <= 4. + 1e-9), "{points:?}");
        assert!(points
            .iter()
            .any(|p| (p.0 - 2.).abs() < 1e-9 && (p.1 - 2.).abs() < 1e-9));
    }

    #[test]
    fn bulged_paths_over_the_point_budget_store_no_more_points() {
        // Bulge 5 flattens each segment into about 56 points: 2,048 vertices give
        // about 115,000 points per path, so the third path breaks the budget.
        let vertices: Vec<(f64, f64)> = (0..2048)
            .map(|i| (f64::from(i), f64::from(i % 2)))
            .collect();
        let path = || HatchPathData::Polyline {
            closed: true,
            vertices: vertices.clone(),
            bulges: Some(vec![5.; vertices.len()]),
        };
        let decoded = decode_paths(vec![path(), path(), path(), path()]);
        let paths = &decoded.paths;
        assert_eq!(paths.len(), 4);
        assert!(!paths[0].points.is_empty());
        assert!(paths[2].points.is_empty() && paths[3].points.is_empty());
        let stored: usize = paths.iter().map(|p| p.points.capacity()).sum();
        assert!(stored <= 262_144 + paths.len(), "{stored}");
        for path in paths {
            match &path.boundary {
                crate::entities::HatchBoundary::Polyline {
                    vertices, bulges, ..
                } => {
                    assert_eq!((vertices.len(), bulges.len()), (2048, 2048));
                }
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn r2004_hatch_whose_first_path_breaks_the_budget_is_kept() {
        // R2004+ decoding scores candidate layouts; a hatch whose every path lost
        // its points to the budget must not be taken for a misaligned read.
        let payload = solid_hatch_payload(vec![HatchPathData::Edges(long_line_path())]);
        let mut reader = BitReader::new(&payload);
        reader.set_pre_r2004_layout(true);
        reader.read_bs().unwrap();
        let decoded = crate::entities::hatch::decode_hatch_r2004(&mut reader).expect("decoded");
        assert_eq!(decoded.paths.len(), 1);
        assert!(decoded.paths[0].points.is_empty());
        match &decoded.paths[0].boundary {
            crate::entities::HatchBoundary::Edges(edges) => assert_eq!(edges.len(), 140_000),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn hatch_over_the_point_budget_keeps_every_boundary() {
        let decoded = decode_edge_paths(vec![square(0.), long_line_path(), square(5.)]);
        let paths = &decoded.paths;
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0].points.len(), 5);
        // From the path that broke the budget on, the points are empty.
        assert!(paths[1].points.is_empty() && paths[2].points.is_empty());
        let edge_counts: Vec<usize> = paths
            .iter()
            .map(|path| match &path.boundary {
                crate::entities::HatchBoundary::Edges(edges) => edges.len(),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(edge_counts, vec![4, 140_000, 4]);
    }
}
