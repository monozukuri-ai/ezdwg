pub(crate) mod checksum;
pub mod classes;
pub(crate) mod database;
pub mod entities;
pub(crate) mod header;
pub mod object_map;
pub mod object_record;
pub mod sections;

use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::writer::config::WriterConfig;
use crate::writer::encoder::encode_document;
use crate::writer::ir::WriterDocument;

pub use object_map::encode_object_map_section;
pub use object_record::{encode_ms_value, encode_object_record};

/// Encode a complete AC1015 document. The entities go through the incremental
/// [`DocumentEncoder`](crate::writer::DocumentEncoder) one at a time.
pub fn write_document(doc: &WriterDocument, config: &WriterConfig) -> Result<Vec<u8>> {
    if !matches!(doc.version, crate::dwg::version::DwgVersion::R2000) {
        return Err(DwgError::new(
            ErrorKind::Unsupported,
            format!(
                "writer r2000 only supports AC1015, got {}",
                doc.version.as_str()
            ),
        ));
    }
    encode_document(doc, config)?.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::write_document;
    use crate::core::config::ParseConfig;
    use crate::dwg::decoder::Decoder;
    use crate::dwg::version::{detect_version, DwgVersion};
    use crate::entities::{
        decode_arc, decode_circle, decode_line, decode_lwpolyline, decode_mtext, decode_point,
        decode_ray, decode_text, decode_xline,
    };
    use crate::objects::object_header_r2000;
    use crate::writer::config::WriterConfig;
    use crate::writer::ir::{
        ArcEntity, CircleEntity, CommonEntityProps, LineEntity, LwPolylineEntity, MTextEntity,
        PointEntity, RayEntity, TextEntity, WriterDocument, WriterEntity, XLineEntity,
    };

    #[test]
    fn writes_minimal_r2000_line_document() {
        let doc = WriterDocument {
            version: DwgVersion::R2000,
            modelspace: vec![WriterEntity::Line(LineEntity {
                common: CommonEntityProps {
                    handle: Some(0x30),
                    layer_name: "0".to_string(),
                    color_index: Some(7),
                    true_color: None,
                    ..CommonEntityProps::default()
                },
                start: (1.0, 2.0, 0.0),
                end: (4.5, 7.0, 0.0),
            })],
            ..WriterDocument::default()
        };

        let bytes = write_document(&doc, &WriterConfig::default()).expect("write_document");
        assert_eq!(
            detect_version(&bytes).expect("detect version"),
            DwgVersion::R2000
        );

        let decoder = Decoder::new(&bytes, ParseConfig::default()).expect("decoder");
        let index = decoder.build_object_index().expect("object index");
        let obj_ref = *index
            .get(crate::objects::Handle(0x30))
            .expect("written LINE");
        assert_eq!(obj_ref.handle.0, 0x30);

        let record = decoder
            .parse_object_record(obj_ref.offset)
            .expect("parse object record");
        let header = object_header_r2000::parse_from_record(&record).expect("object header");
        assert_eq!(header.type_code, 0x13);

        let mut reader = record.bit_reader();
        let type_code = reader.read_bs().expect("read type prefix");
        assert_eq!(type_code, 0x13);
        let line = decode_line(&mut reader).expect("decode line");

        assert_eq!(line.handle, 0x30);
        assert_eq!(line.start, (1.0, 2.0, 0.0));
        assert_eq!(line.end, (4.5, 7.0, 0.0));
        assert_eq!(line.color_index, Some(7));
        let layer_ref = index
            .get(crate::objects::Handle(line.layer_handle))
            .expect("referenced layer exists");
        let layer_record = decoder.parse_object_record(layer_ref.offset).unwrap();
        assert_eq!(
            object_header_r2000::parse_from_record(&layer_record)
                .unwrap()
                .type_code,
            0x33
        );
    }

    #[test]
    fn writes_mixed_r2000_entities() {
        let doc = WriterDocument {
            version: DwgVersion::R2000,
            modelspace: vec![
                WriterEntity::Arc(ArcEntity {
                    common: CommonEntityProps {
                        handle: Some(0x40),
                        layer_name: "0".to_string(),
                        color_index: Some(7),
                        true_color: None,
                        ..CommonEntityProps::default()
                    },
                    center: (2.0, 3.0, 0.0),
                    radius: 5.0,
                    angle_start_rad: 0.25,
                    angle_end_rad: 1.50,
                }),
                WriterEntity::Circle(CircleEntity {
                    common: CommonEntityProps {
                        handle: Some(0x41),
                        layer_name: "0".to_string(),
                        color_index: Some(7),
                        true_color: None,
                        ..CommonEntityProps::default()
                    },
                    center: (4.0, 5.0, 0.0),
                    radius: 2.5,
                }),
                WriterEntity::LwPolyline(LwPolylineEntity {
                    common: CommonEntityProps {
                        handle: Some(0x42),
                        layer_name: "0".to_string(),
                        color_index: Some(7),
                        true_color: None,
                        ..CommonEntityProps::default()
                    },
                    flags: 1,
                    vertices: vec![(0.0, 0.0), (2.0, 0.0), (2.0, 1.0)],
                    const_width: None,
                    bulges: vec![],
                    widths: vec![],
                }),
                WriterEntity::Text(TextEntity {
                    common: CommonEntityProps {
                        handle: Some(0x43),
                        layer_name: "0".to_string(),
                        color_index: Some(7),
                        true_color: None,
                        ..CommonEntityProps::default()
                    },
                    text: "HELLO".to_string(),
                    insert: (1.5, 2.5, 0.0),
                    height: 2.0,
                    rotation_rad: 0.2,
                }),
                WriterEntity::MText(MTextEntity {
                    common: CommonEntityProps {
                        handle: Some(0x44),
                        layer_name: "0".to_string(),
                        color_index: Some(7),
                        true_color: None,
                        ..CommonEntityProps::default()
                    },
                    text: "MULTI".to_string(),
                    insert: (3.0, 4.0, 0.0),
                    text_direction: (1.0, 0.0, 0.0),
                    rect_width: 12.0,
                    char_height: 1.5,
                    attachment_point: 1,
                    drawing_direction: 1,
                }),
                WriterEntity::Point(PointEntity {
                    common: CommonEntityProps {
                        handle: Some(0x45),
                        layer_name: "0".to_string(),
                        color_index: Some(7),
                        true_color: None,
                        ..CommonEntityProps::default()
                    },
                    location: (7.0, 8.0, 0.0),
                    x_axis_angle: 0.3,
                }),
                WriterEntity::Ray(RayEntity {
                    common: CommonEntityProps {
                        handle: Some(0x46),
                        layer_name: "0".to_string(),
                        color_index: Some(7),
                        true_color: None,
                        ..CommonEntityProps::default()
                    },
                    start: (9.0, 1.0, 0.0),
                    unit_vector: (1.0, 0.0, 0.0),
                }),
                WriterEntity::XLine(XLineEntity {
                    common: CommonEntityProps {
                        handle: Some(0x47),
                        layer_name: "0".to_string(),
                        color_index: Some(7),
                        true_color: None,
                        ..CommonEntityProps::default()
                    },
                    start: (10.0, 2.0, 0.0),
                    unit_vector: (0.0, 1.0, 0.0),
                }),
            ],
            ..WriterDocument::default()
        };

        let bytes = write_document(&doc, &WriterConfig::default()).expect("write_document");
        let decoder = Decoder::new(&bytes, ParseConfig::default()).expect("decoder");
        let index = decoder.build_object_index().expect("object index");
        assert!(index.len() > 8); // includes the database graph

        let mut seen_arc = false;
        let mut seen_circle = false;
        let mut seen_lwpolyline = false;
        let mut seen_text = false;
        let mut seen_mtext = false;
        let mut seen_point = false;
        let mut seen_ray = false;
        let mut seen_xline = false;
        for obj_ref in index.objects {
            let record = decoder
                .parse_object_record(obj_ref.offset)
                .expect("parse object record");
            let header = object_header_r2000::parse_from_record(&record).expect("header");
            let mut reader = record.bit_reader();
            let prefix = reader.read_bs().expect("type prefix");
            assert_eq!(prefix, header.type_code);
            match header.type_code {
                0x11 => {
                    let arc = decode_arc(&mut reader).expect("decode arc");
                    assert_eq!(arc.handle, 0x40);
                    assert_eq!(arc.center, (2.0, 3.0, 0.0));
                    assert!((arc.radius - 5.0).abs() < 1.0e-9);
                    seen_arc = true;
                }
                0x12 => {
                    let circle = decode_circle(&mut reader).expect("decode circle");
                    assert_eq!(circle.handle, 0x41);
                    assert_eq!(circle.center, (4.0, 5.0, 0.0));
                    assert!((circle.radius - 2.5).abs() < 1.0e-9);
                    seen_circle = true;
                }
                0x4D => {
                    let poly = decode_lwpolyline(&mut reader).expect("decode lwpolyline");
                    assert_eq!(poly.handle, 0x42);
                    assert_eq!(poly.vertices, vec![(0.0, 0.0), (2.0, 0.0), (2.0, 1.0)]);
                    assert!(poly.flags & 0x200 != 0);
                    seen_lwpolyline = true;
                }
                0x01 => {
                    let text = decode_text(&mut reader).expect("decode text");
                    assert_eq!(text.handle, 0x43);
                    assert_eq!(text.text, "HELLO");
                    assert_eq!(text.insertion, (1.5, 2.5, 0.0));
                    assert!((text.height - 2.0).abs() < 1.0e-9);
                    assert!((text.rotation - 0.2).abs() < 1.0e-9);
                    seen_text = true;
                }
                0x2C => {
                    let mtext = decode_mtext(&mut reader).expect("decode mtext");
                    assert_eq!(mtext.handle, 0x44);
                    assert_eq!(mtext.text, "MULTI");
                    assert_eq!(mtext.insertion, (3.0, 4.0, 0.0));
                    assert!((mtext.text_height - 1.5).abs() < 1.0e-9);
                    assert_eq!(mtext.attachment, 1);
                    seen_mtext = true;
                }
                0x1B => {
                    let point = decode_point(&mut reader).expect("decode point");
                    assert_eq!(point.handle, 0x45);
                    assert_eq!(point.location, (7.0, 8.0, 0.0));
                    assert!((point.x_axis_angle - 0.3).abs() < 1.0e-9);
                    seen_point = true;
                }
                0x28 => {
                    let ray = decode_ray(&mut reader).expect("decode ray");
                    assert_eq!(ray.handle, 0x46);
                    assert_eq!(ray.start, (9.0, 1.0, 0.0));
                    assert_eq!(ray.unit_vector, (1.0, 0.0, 0.0));
                    seen_ray = true;
                }
                0x29 => {
                    let xline = decode_xline(&mut reader).expect("decode xline");
                    assert_eq!(xline.handle, 0x47);
                    assert_eq!(xline.start, (10.0, 2.0, 0.0));
                    assert_eq!(xline.unit_vector, (0.0, 1.0, 0.0));
                    seen_xline = true;
                }
                _ => {} // symbol tables, dictionaries, layouts, BLOCK/ENDBLK
            }
        }

        assert!(seen_arc);
        assert!(seen_circle);
        assert!(seen_lwpolyline);
        assert!(seen_text);
        assert!(seen_mtext);
        assert!(seen_point);
        assert!(seen_ray);
        assert!(seen_xline);
    }
}
