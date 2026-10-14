mod checksum;
pub mod classes;
mod database;
pub mod entities;
mod header;
pub mod object_map;
pub mod object_record;
pub mod sections;

use self::classes::{encode_classes_section, IMAGEDEF_REACTOR_CLASS, IMAGE_CLASS};
use self::entities::{
    encode_arc_entity_payload, encode_circle_entity_payload, encode_ellipse_entity_payload,
    encode_line_entity_payload, encode_lwpolyline_entity_payload, encode_mtext_entity_payload,
    encode_point_entity_payload, encode_ray_entity_payload, encode_text_entity_payload,
    encode_xline_entity_payload, ArcEncodeInput, CircleEncodeInput, EllipseEncodeInput,
    LineEncodeInput, LwPolylineEncodeInput, MTextEncodeInput, PointEncodeInput, RayEncodeInput,
    TextEncodeInput, XLineEncodeInput,
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

    crate::writer::validation::validate_document(doc)?;
    let with_images = !doc.image_defs.is_empty();
    let classes_section = encode_classes_section(with_images)?;
    let mut allocator = HandleAllocator::new(1);
    if config.preserve_input_handles {
        for entity in doc.layouts().flatten() {
            if let Some(h) = entity.common().handle {
                allocator.reserve(h)?;
            }
            if let WriterEntity::Insert(insert) = entity {
                for attr in &insert.attributes {
                    if let Some(h) = attr.text.common.handle {
                        allocator.reserve(h)?;
                    }
                }
            }
        }
    }
    let mut allocate = |h: Option<u64>| -> Result<u64> {
        if config.preserve_input_handles {
            if let Some(h) = h {
                return Ok(h);
            }
        }
        allocator.allocate()
    };
    let layout_handles: Vec<Vec<u64>> = doc
        .layouts()
        .map(|entities| {
            entities
                .iter()
                .map(|e| allocate(e.common().handle))
                .collect::<Result<_>>()
        })
        .collect::<Result<_>>()?;
    let mut attribute_handles = std::collections::HashMap::new();
    let mut polyline_handles = std::collections::HashMap::new();
    let mut image_reactors = std::collections::HashMap::new();
    for (entities, handles) in doc.layouts().zip(&layout_handles) {
        for (e, &h) in entities.iter().zip(handles) {
            if let WriterEntity::Polyline2d(poly) | WriterEntity::Polyline3d(poly) = e {
                let vertices = poly
                    .vertices
                    .iter()
                    .map(|_| allocate(None))
                    .collect::<Result<Vec<_>>>()?;
                polyline_handles.insert(h, (vertices, allocate(None)?));
            }
            if let WriterEntity::Insert(insert) = e {
                let attrs = insert
                    .attributes
                    .iter()
                    .map(|a| allocate(a.text.common.handle))
                    .collect::<Result<Vec<_>>>()?;
                let seqend = if attrs.is_empty() { 0 } else { allocate(None)? };
                attribute_handles.insert(h, (attrs, seqend));
            }
            if let WriterEntity::Image(_) = e {
                image_reactors.insert(h, allocate(None)?);
            }
        }
    }
    let database = database::DatabaseHandles::allocate(&mut allocator, doc)?;
    let mut record_rows: Vec<(ObjectRef, Vec<u8>)> = Vec::new();

    for (layout_index, (entities, handles)) in doc.layouts().zip(&layout_handles).enumerate() {
        let owner = if layout_index == 0 {
            database.model
        } else {
            database.blocks[&doc.blocks[layout_index - 1].name.to_uppercase()].0
        };
        for (i, entity) in entities.iter().enumerate() {
            let handle = handles[i];
            let previous_handle = i.checked_sub(1).map(|j| handles[j]).unwrap_or(0);
            let next_handle = handles.get(i + 1).copied().unwrap_or(0);
            let props = entity.common();
            let common = entities::common::CommonEntityEncodeInput {
                handle,
                owner_handle: owner,
                layer_handle: database.layer_handle(&props.layer_name)?,
                previous_handle,
                next_handle,
                style_handle: database.style_handle(&props.style_name)?,
                color_index: props.color_index.unwrap_or(256),
                linetype_flags: match props.linetype.to_uppercase().as_str() {
                    "" | "BYLAYER" => 0,
                    "BYBLOCK" => 1,
                    "CONTINUOUS" => 2,
                    _ => 3,
                },
                linetype_handle: database.linetype_handle(&props.linetype)?,
                codepage: doc.metadata.codepage,
                formatting: props.formatting,
            };
            match entity {
                WriterEntity::Line(line) => {
                    let payload = encode_line_entity_payload(LineEncodeInput {
                        common,
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
                    let payload = encode_point_entity_payload(PointEncodeInput {
                        common,
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
                    let payload = encode_ray_entity_payload(RayEncodeInput {
                        common,
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
                    let payload = encode_xline_entity_payload(XLineEncodeInput {
                        common,
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
                    let payload = encode_arc_entity_payload(ArcEncodeInput {
                        common,
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
                    let payload = encode_circle_entity_payload(CircleEncodeInput {
                        common,
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
                WriterEntity::Ellipse(ellipse) => {
                    let payload = encode_ellipse_entity_payload(EllipseEncodeInput {
                        common,
                        center: ellipse.center,
                        major_axis: ellipse.major_axis,
                        axis_ratio: ellipse.axis_ratio,
                        start_param: ellipse.start_param,
                        end_param: ellipse.end_param,
                    })?;
                    record_rows.push((
                        ObjectRef {
                            handle: Handle(handle),
                            offset: 0,
                        },
                        encode_object_record(&payload)?,
                    ));
                }
                WriterEntity::Solid(face) | WriterEntity::Trace(face) => {
                    let payload = entities::face::encode_planar_face_entity_payload(
                        common,
                        face.points,
                        matches!(entity, WriterEntity::Trace(_)),
                    )?;
                    record_rows.push((
                        ObjectRef {
                            handle: Handle(handle),
                            offset: 0,
                        },
                        encode_object_record(&payload)?,
                    ));
                }
                WriterEntity::Spline(spline) => {
                    let payload = entities::spline::encode_spline_entity_payload(common, spline)?;
                    record_rows.push((
                        ObjectRef {
                            handle: Handle(handle),
                            offset: 0,
                        },
                        encode_object_record(&payload)?,
                    ));
                }
                WriterEntity::Face3d(face) => {
                    let payload = entities::face::encode_3dface_entity_payload(
                        common,
                        face.points,
                        face.invisible_edge_flags,
                    )?;
                    record_rows.push((
                        ObjectRef {
                            handle: Handle(handle),
                            offset: 0,
                        },
                        encode_object_record(&payload)?,
                    ));
                }
                WriterEntity::Polyline2d(poly) | WriterEntity::Polyline3d(poly) => {
                    let is_3d = matches!(entity, WriterEntity::Polyline3d(_));
                    let (vertices, seqend) = &polyline_handles[&handle];
                    let payload = entities::polyline::encode_polyline(
                        common, poly, is_3d, vertices, *seqend,
                    )?;
                    record_rows.push((
                        ObjectRef {
                            handle: Handle(handle),
                            offset: 0,
                        },
                        encode_object_record(&payload)?,
                    ));
                    for (j, vertex) in poly.vertices.iter().enumerate() {
                        let child = entities::common::CommonEntityEncodeInput {
                            handle: vertices[j],
                            owner_handle: handle,
                            previous_handle: j.checked_sub(1).map(|k| vertices[k]).unwrap_or(0),
                            next_handle: vertices.get(j + 1).copied().unwrap_or(0),
                            color_index: 256,
                            linetype_flags: 0,
                            linetype_handle: 0,
                            formatting: Default::default(),
                            ..common
                        };
                        let payload = entities::polyline::encode_vertex(child, vertex, is_3d)?;
                        record_rows.push((
                            ObjectRef {
                                handle: Handle(vertices[j]),
                                offset: 0,
                            },
                            encode_object_record(&payload)?,
                        ));
                    }
                    let child = entities::common::CommonEntityEncodeInput {
                        handle: *seqend,
                        owner_handle: handle,
                        previous_handle: 0,
                        next_handle: 0,
                        color_index: 256,
                        linetype_flags: 0,
                        linetype_handle: 0,
                        formatting: Default::default(),
                        ..common
                    };
                    let payload = entities::common::encode_entity_payload(6, child, |_| Ok(()))?;
                    record_rows.push((
                        ObjectRef {
                            handle: Handle(*seqend),
                            offset: 0,
                        },
                        encode_object_record(&payload)?,
                    ));
                }
                WriterEntity::LwPolyline(poly) => {
                    let payload = encode_lwpolyline_entity_payload(LwPolylineEncodeInput {
                        common,
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
                    let payload = encode_text_entity_payload(&TextEncodeInput {
                        common,
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
                    let payload = encode_mtext_entity_payload(&MTextEncodeInput {
                        common,
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
                WriterEntity::Dimension(dim) => {
                    let block = database
                        .blocks
                        .get(&dim.block_name.to_uppercase())
                        .map(|b| b.0)
                        .ok_or_else(|| {
                            DwgError::new(
                                ErrorKind::Format,
                                format!("undefined dimension block: {}", dim.block_name),
                            )
                        })?;
                    let payload = entities::dimension::encode_dimension_entity_payload(
                        common,
                        dim,
                        database.dimstyle,
                        block,
                    )?;
                    record_rows.push((
                        ObjectRef {
                            handle: Handle(handle),
                            offset: 0,
                        },
                        encode_object_record(&payload)?,
                    ));
                }
                WriterEntity::Image(image) => {
                    let reactor = image_reactors[&handle];
                    let def = database.image_def_handle(&image.image_def)?;
                    let payload = entities::image::encode_image_entity_payload(
                        IMAGE_CLASS,
                        common,
                        image,
                        def,
                        reactor,
                    )?;
                    record_rows.push((
                        ObjectRef {
                            handle: Handle(handle),
                            offset: 0,
                        },
                        encode_object_record(&payload)?,
                    ));
                    let reactor_payload = entities::image::encode_image_def_reactor_object(
                        doc.metadata.codepage,
                        IMAGEDEF_REACTOR_CLASS,
                        reactor,
                        handle,
                    )?;
                    record_rows.push((
                        ObjectRef {
                            handle: Handle(reactor),
                            offset: 0,
                        },
                        encode_object_record(&reactor_payload)?,
                    ));
                }
                WriterEntity::Hatch(hatch) => {
                    let payload = entities::hatch::encode_hatch_entity_payload(common, hatch)?;
                    record_rows.push((
                        ObjectRef {
                            handle: Handle(handle),
                            offset: 0,
                        },
                        encode_object_record(&payload)?,
                    ));
                }
                WriterEntity::Attdef(attr) => {
                    let payload = entities::block::encode_attribute(common, attr, true)?;
                    record_rows.push((
                        ObjectRef {
                            handle: Handle(handle),
                            offset: 0,
                        },
                        encode_object_record(&payload)?,
                    ));
                }
                WriterEntity::Attrib(_) => {
                    unreachable!("standalone attributes rejected by validation")
                }
                WriterEntity::Insert(insert) => {
                    let (attrs, seqend) = &attribute_handles[&handle];
                    let block = database.blocks[&insert.block_name.to_uppercase()].0;
                    let payload =
                        entities::block::encode_insert(common, insert, block, attrs, *seqend)?;
                    record_rows.push((
                        ObjectRef {
                            handle: Handle(handle),
                            offset: 0,
                        },
                        encode_object_record(&payload)?,
                    ));
                    for (i, attr) in insert.attributes.iter().enumerate() {
                        let c = &attr.text.common;
                        let child = entities::common::CommonEntityEncodeInput {
                            handle: attrs[i],
                            owner_handle: handle,
                            previous_handle: i.checked_sub(1).map(|j| attrs[j]).unwrap_or(0),
                            next_handle: attrs.get(i + 1).copied().unwrap_or(0),
                            layer_handle: database.layer_handle(&c.layer_name)?,
                            color_index: c.color_index.unwrap_or(256),
                            style_handle: database.style_handle(&c.style_name)?,
                            linetype_flags: match c.linetype.to_uppercase().as_str() {
                                "" | "BYLAYER" => 0,
                                "BYBLOCK" => 1,
                                "CONTINUOUS" => 2,
                                _ => 3,
                            },
                            linetype_handle: database.linetype_handle(&c.linetype)?,
                            formatting: c.formatting,
                            ..common
                        };
                        let payload = entities::block::encode_attribute(child, attr, false)?;
                        record_rows.push((
                            ObjectRef {
                                handle: Handle(attrs[i]),
                                offset: 0,
                            },
                            encode_object_record(&payload)?,
                        ));
                    }
                    if *seqend != 0 {
                        let child = entities::common::CommonEntityEncodeInput {
                            handle: *seqend,
                            owner_handle: handle,
                            previous_handle: 0,
                            next_handle: 0,
                            layer_handle: database.layer,
                            formatting: Default::default(),
                            linetype_flags: 0,
                            color_index: 256,
                            ..common
                        };
                        let payload =
                            entities::common::encode_entity_payload(6, child, |_| Ok(()))?;
                        record_rows.push((
                            ObjectRef {
                                handle: Handle(*seqend),
                                offset: 0,
                            },
                            encode_object_record(&payload)?,
                        ));
                    }
                }
            }
        }
    }

    for (handle, payload) in database.objects(&layout_handles, doc, &image_reactors)? {
        record_rows.push((
            ObjectRef {
                handle: Handle(handle),
                offset: 0,
            },
            encode_object_record(&payload)?,
        ));
    }
    let handseed = record_rows
        .iter()
        .map(|(r, _)| r.handle.0)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| DwgError::new(ErrorKind::Unsupported, "handle space exhausted"))?;
    let header_section = header::encode_header(&database, handseed, &doc.metadata)?;
    record_rows.sort_by_key(|(obj_ref, _)| obj_ref.handle.0);

    let record_count = 3usize;
    let directory_size = 0x15usize + 4 + record_count * 9 + 2 + SECTION_DIRECTORY_SENTINEL.len();
    let mut cursor = align_up(directory_size, 4);

    let header_offset = cursor;
    cursor += header_section.len();
    let classes_offset = cursor;
    cursor = cursor.saturating_add(classes_section.len() + 0x200); // R13C3+ padding
    cursor = align_up(cursor, 4);

    for (obj_ref, record) in record_rows.iter_mut() {
        obj_ref.offset = u32::try_from(cursor)
            .map_err(|_| DwgError::new(ErrorKind::Unsupported, "AC1015 file exceeds 4 GiB"))?;
        cursor = cursor.saturating_add(record.len());
    }
    cursor = align_up(cursor, 4);

    let object_refs: Vec<ObjectRef> = record_rows.iter().map(|(obj_ref, _)| *obj_ref).collect();
    let object_map_section = encode_object_map_section(&object_refs)?;
    let object_map_offset = cursor;
    cursor = cursor.saturating_add(object_map_section.len());

    u32::try_from(cursor)
        .map_err(|_| DwgError::new(ErrorKind::Unsupported, "AC1015 file exceeds 4 GiB"))?;

    let mut bytes = vec![0u8; cursor];
    bytes[0..6].copy_from_slice(b"AC1015");
    bytes[0x0C] = 1;
    bytes[0x11] = 0x17;
    bytes[0x12] = 0x0F;
    write_u16_le(&mut bytes, 0x13, doc.metadata.codepage);
    write_u32_le(&mut bytes, 0x15, record_count as u32);
    let mut entry_off = 0x15usize + 4;
    write_section_record(
        &mut bytes,
        entry_off,
        0,
        header_offset as u32,
        header_section.len() as u32,
    );
    entry_off += 9;

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

    let crc = checksum::crc16(0, &bytes[..entry_off]) ^ 0xA598;
    write_u16_le(&mut bytes, entry_off, crc);
    entry_off += 2;
    bytes[entry_off..entry_off + SECTION_DIRECTORY_SENTINEL.len()]
        .copy_from_slice(&SECTION_DIRECTORY_SENTINEL);

    copy_section(&mut bytes, header_offset, &header_section)?;
    copy_section(&mut bytes, classes_offset, &classes_section)?;
    for (obj_ref, record) in &record_rows {
        copy_section(&mut bytes, obj_ref.offset as usize, record)?;
    }
    copy_section(&mut bytes, object_map_offset, &object_map_section)?;

    Ok(bytes)
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
