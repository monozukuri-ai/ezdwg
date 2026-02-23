pub mod classes;
pub mod entities;
pub mod object_map;
pub mod object_record;
pub mod sections;

use self::classes::encode_minimal_classes_section;
use self::entities::{
    encode_arc_entity_payload, encode_circle_entity_payload, encode_line_entity_payload,
    encode_lwpolyline_entity_payload, encode_mtext_entity_payload, encode_point_entity_payload,
    encode_ray_entity_payload, encode_text_entity_payload, encode_xline_entity_payload,
    ArcEncodeInput, CircleEncodeInput, LineEncodeInput, LwPolylineEncodeInput, MTextEncodeInput,
    PointEncodeInput, RayEncodeInput, TextEncodeInput, XLineEncodeInput,
};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::objects::{Handle, ObjectRef};
use crate::writer::config::WriterConfig;
use crate::writer::ir::{WriterDocument, WriterEntity};
use crate::writer::HandleAllocator;

pub use object_map::encode_object_map_section;
pub use object_record::{encode_ms_value, encode_object_record};

const SECTION_DIRECTORY_SENTINEL: [u8; 16] = [
    0x95, 0xA0, 0x4E, 0x28, 0x99, 0x82, 0x1A, 0xE5, 0x5E, 0x41, 0xE0, 0x5F, 0x9D, 0x3A, 0x4D, 0x00,
];

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

    let classes_section = encode_minimal_classes_section()?;
    let mut allocator = HandleAllocator::new(0x10);
    let mut record_rows: Vec<(ObjectRef, Vec<u8>)> = Vec::new();

    for entity in &doc.modelspace {
        match entity {
            WriterEntity::Line(line) => {
                let handle = resolve_handle(&mut allocator, line.common.handle, config)?;
                let payload = encode_line_entity_payload(LineEncodeInput {
                    handle,
                    owner_handle: 1,
                    layer_handle: 2,
                    color_index: line.common.color_index.unwrap_or(7) as u8,
                    start: line.start,
                    end: line.end,
                })?;
                let record = encode_object_record(&payload)?;
                record_rows.push((
                    ObjectRef {
                        handle: Handle(handle),
                        offset: 0,
                    },
                    record,
                ));
            }
            WriterEntity::Point(point) => {
                let handle = resolve_handle(&mut allocator, point.common.handle, config)?;
                let payload = encode_point_entity_payload(PointEncodeInput {
                    handle,
                    owner_handle: 1,
                    layer_handle: 2,
                    color_index: point.common.color_index.unwrap_or(7) as u8,
                    location: point.location,
                    x_axis_angle: point.x_axis_angle,
                })?;
                let record = encode_object_record(&payload)?;
                record_rows.push((
                    ObjectRef {
                        handle: Handle(handle),
                        offset: 0,
                    },
                    record,
                ));
            }
            WriterEntity::Ray(ray) => {
                let handle = resolve_handle(&mut allocator, ray.common.handle, config)?;
                let payload = encode_ray_entity_payload(RayEncodeInput {
                    handle,
                    owner_handle: 1,
                    layer_handle: 2,
                    color_index: ray.common.color_index.unwrap_or(7) as u8,
                    start: ray.start,
                    unit_vector: ray.unit_vector,
                })?;
                let record = encode_object_record(&payload)?;
                record_rows.push((
                    ObjectRef {
                        handle: Handle(handle),
                        offset: 0,
                    },
                    record,
                ));
            }
            WriterEntity::XLine(xline) => {
                let handle = resolve_handle(&mut allocator, xline.common.handle, config)?;
                let payload = encode_xline_entity_payload(XLineEncodeInput {
                    handle,
                    owner_handle: 1,
                    layer_handle: 2,
                    color_index: xline.common.color_index.unwrap_or(7) as u8,
                    start: xline.start,
                    unit_vector: xline.unit_vector,
                })?;
                let record = encode_object_record(&payload)?;
                record_rows.push((
                    ObjectRef {
                        handle: Handle(handle),
                        offset: 0,
                    },
                    record,
                ));
            }
            WriterEntity::Arc(arc) => {
                let handle = resolve_handle(&mut allocator, arc.common.handle, config)?;
                let payload = encode_arc_entity_payload(ArcEncodeInput {
                    handle,
                    owner_handle: 1,
                    layer_handle: 2,
                    color_index: arc.common.color_index.unwrap_or(7) as u8,
                    center: arc.center,
                    radius: arc.radius,
                    angle_start: arc.angle_start_rad,
                    angle_end: arc.angle_end_rad,
                })?;
                let record = encode_object_record(&payload)?;
                record_rows.push((
                    ObjectRef {
                        handle: Handle(handle),
                        offset: 0,
                    },
                    record,
                ));
            }
            WriterEntity::Circle(circle) => {
                let handle = resolve_handle(&mut allocator, circle.common.handle, config)?;
                let payload = encode_circle_entity_payload(CircleEncodeInput {
                    handle,
                    owner_handle: 1,
                    layer_handle: 2,
                    color_index: circle.common.color_index.unwrap_or(7) as u8,
                    center: circle.center,
                    radius: circle.radius,
                })?;
                let record = encode_object_record(&payload)?;
                record_rows.push((
                    ObjectRef {
                        handle: Handle(handle),
                        offset: 0,
                    },
                    record,
                ));
            }
            WriterEntity::LwPolyline(poly) => {
                let handle = resolve_handle(&mut allocator, poly.common.handle, config)?;
                let payload = encode_lwpolyline_entity_payload(LwPolylineEncodeInput {
                    handle,
                    owner_handle: 1,
                    layer_handle: 2,
                    color_index: poly.common.color_index.unwrap_or(7) as u8,
                    flags: poly.flags,
                    vertices: poly.vertices.clone(),
                    const_width: poly.const_width,
                    bulges: poly.bulges.clone(),
                    widths: poly.widths.clone(),
                })?;
                let record = encode_object_record(&payload)?;
                record_rows.push((
                    ObjectRef {
                        handle: Handle(handle),
                        offset: 0,
                    },
                    record,
                ));
            }
            WriterEntity::Text(text) => {
                let handle = resolve_handle(&mut allocator, text.common.handle, config)?;
                let payload = encode_text_entity_payload(&TextEncodeInput {
                    handle,
                    owner_handle: 1,
                    layer_handle: 2,
                    color_index: text.common.color_index.unwrap_or(7) as u8,
                    text: text.text.clone(),
                    insertion: text.insert,
                    height: text.height,
                    rotation: text.rotation_rad,
                })?;
                let record = encode_object_record(&payload)?;
                record_rows.push((
                    ObjectRef {
                        handle: Handle(handle),
                        offset: 0,
                    },
                    record,
                ));
            }
            WriterEntity::MText(mtext) => {
                let handle = resolve_handle(&mut allocator, mtext.common.handle, config)?;
                let payload = encode_mtext_entity_payload(&MTextEncodeInput {
                    handle,
                    owner_handle: 1,
                    layer_handle: 2,
                    color_index: mtext.common.color_index.unwrap_or(7) as u8,
                    text: mtext.text.clone(),
                    insertion: mtext.insert,
                    text_direction: mtext.text_direction,
                    rect_width: mtext.rect_width,
                    text_height: mtext.char_height,
                    attachment: mtext.attachment_point,
                    drawing_dir: mtext.drawing_direction,
                })?;
                let record = encode_object_record(&payload)?;
                record_rows.push((
                    ObjectRef {
                        handle: Handle(handle),
                        offset: 0,
                    },
                    record,
                ));
            }
        }
    }

    record_rows.sort_by_key(|(obj_ref, _)| obj_ref.handle.0);

    let record_count = 2usize;
    let directory_size = 0x15usize + 4 + record_count * 9 + 2 + SECTION_DIRECTORY_SENTINEL.len();
    let mut cursor = align_up(directory_size, 4);

    let classes_offset = cursor;
    cursor = cursor.saturating_add(classes_section.len());
    cursor = align_up(cursor, 4);

    for (obj_ref, record) in record_rows.iter_mut() {
        obj_ref.offset = cursor as u32;
        cursor = cursor.saturating_add(record.len());
    }
    cursor = align_up(cursor, 4);

    let object_refs: Vec<ObjectRef> = record_rows.iter().map(|(obj_ref, _)| *obj_ref).collect();
    let object_map_section = encode_object_map_section(&object_refs)?;
    let object_map_offset = cursor;
    cursor = cursor.saturating_add(object_map_section.len());

    let mut bytes = vec![0u8; cursor];
    bytes[0..6].copy_from_slice(b"AC1015");
    write_u32_le(&mut bytes, 0x15, record_count as u32);
    let mut entry_off = 0x15usize + 4;

    write_section_record(
        &mut bytes,
        entry_off,
        1,
        classes_offset as u32,
        classes_section.len() as u32,
    );
    entry_off += 9;
    write_section_record(
        &mut bytes,
        entry_off,
        2,
        object_map_offset as u32,
        object_map_section.len() as u32,
    );
    entry_off += 9;

    write_u16_le(&mut bytes, entry_off, 0);
    entry_off += 2;
    bytes[entry_off..entry_off + SECTION_DIRECTORY_SENTINEL.len()]
        .copy_from_slice(&SECTION_DIRECTORY_SENTINEL);

    copy_section(&mut bytes, classes_offset, &classes_section)?;
    for (obj_ref, record) in &record_rows {
        copy_section(&mut bytes, obj_ref.offset as usize, record)?;
    }
    copy_section(&mut bytes, object_map_offset, &object_map_section)?;

    Ok(bytes)
}

fn resolve_handle(
    allocator: &mut HandleAllocator,
    requested: Option<u64>,
    config: &WriterConfig,
) -> Result<u64> {
    if config.preserve_input_handles {
        if let Some(handle) = requested {
            allocator.reserve(handle)?;
            return Ok(handle);
        }
    }
    allocator.allocate()
}

fn align_up(value: usize, align: usize) -> usize {
    if align == 0 {
        return value;
    }
    let rem = value % align;
    if rem == 0 {
        value
    } else {
        value + (align - rem)
    }
}

fn copy_section(dst: &mut [u8], offset: usize, src: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(src.len())
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "section offset overflow"))?;
    if end > dst.len() {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!(
                "section write out of bounds: offset={offset} len={}",
                src.len()
            ),
        ));
    }
    dst[offset..end].copy_from_slice(src);
    Ok(())
}

fn write_section_record(
    bytes: &mut [u8],
    offset: usize,
    record_no: u8,
    section_offset: u32,
    section_size: u32,
) {
    bytes[offset] = record_no;
    write_u32_le(bytes, offset + 1, section_offset);
    write_u32_le(bytes, offset + 5, section_size);
}

fn write_u16_le(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset] = (value & 0x00FF) as u8;
    bytes[offset + 1] = (value >> 8) as u8;
}

fn write_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset] = (value & 0x0000_00FF) as u8;
    bytes[offset + 1] = ((value >> 8) & 0x0000_00FF) as u8;
    bytes[offset + 2] = ((value >> 16) & 0x0000_00FF) as u8;
    bytes[offset + 3] = ((value >> 24) & 0x0000_00FF) as u8;
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
        assert_eq!(index.len(), 1);
        let obj_ref = index.objects[0];
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
        assert_eq!(line.layer_handle, 2);
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
        assert_eq!(index.len(), 8);

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
                    assert!(poly.flags & 1 != 0);
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
                other => panic!("unexpected type_code: {other:#X}"),
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
