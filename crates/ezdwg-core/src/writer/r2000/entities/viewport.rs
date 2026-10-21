//! VIEWPORT entity (type 34, ODA 20.4.38): the R2000 body and its handle data.
use crate::bit::{BitWriter, Endian};
use crate::core::result::Result;
use crate::writer::ir::ViewportEntity;

use super::common::{encode_entity_payload_with_handles, CommonEntityEncodeInput};

pub const VIEWPORT_TYPE: u16 = 0x22;

/// Encode a paper-space VIEWPORT. `frozen_layer_handles` are the LAYER records
/// of `viewport.frozen_layers` (hard pointers in R2000). The clip boundary, the
/// R2000-only viewport entity header and the named/base UCS handles are null,
/// as AutoCAD writes them for a rectangular viewport with the world UCS.
pub fn encode_viewport_entity_payload(
    common: CommonEntityEncodeInput,
    viewport: &ViewportEntity,
    frozen_layer_handles: &[u64],
) -> Result<Vec<u8>> {
    let mut handles: Vec<(u8, u64)> = frozen_layer_handles.iter().map(|h| (5, *h)).collect();
    // AutoCAD stores every one of these null references with code 5.
    handles.push((5, 0)); // clip boundary
    handles.push((5, 0)); // VIEWPORT ENT HEADER (R2000 only; null in inactive layouts)
    handles.push((5, 0)); // named UCS
    handles.push((5, 0)); // base UCS
    encode_entity_payload_with_handles(
        VIEWPORT_TYPE,
        common,
        |w| write_viewport_body(w, viewport, frozen_layer_handles.len() as u32),
        &handles,
    )
}

fn write_viewport_body(w: &mut BitWriter, v: &ViewportEntity, frozen: u32) -> Result<()> {
    w.write_3bd(v.center.0, v.center.1, v.center.2)?;
    w.write_bd(v.width)?;
    w.write_bd(v.height)?;
    w.write_3bd(v.view_target.0, v.view_target.1, v.view_target.2)?;
    w.write_3bd(v.view_direction.0, v.view_direction.1, v.view_direction.2)?;
    w.write_bd(v.twist_angle)?;
    w.write_bd(v.view_height)?;
    w.write_bd(v.lens_length)?;
    w.write_bd(v.front_clip_z)?;
    w.write_bd(v.back_clip_z)?;
    w.write_bd(v.snap_angle)?;
    for (x, y) in [v.view_center, v.snap_base, v.snap_spacing, v.grid_spacing] {
        w.write_rd(Endian::Little, x)?;
        w.write_rd(Endian::Little, y)?;
    }
    w.write_bs(v.circle_zoom)?;
    w.write_bl(frozen)?;
    w.write_bl(v.status_flags)?;
    w.write_tv(&v.style_sheet)?;
    w.write_rc(v.render_mode)?;
    w.write_b(u8::from(v.ucs_at_origin))?;
    w.write_b(u8::from(v.ucs_per_viewport))?;
    w.write_3bd(v.ucs_origin.0, v.ucs_origin.1, v.ucs_origin.2)?;
    w.write_3bd(v.ucs_x_axis.0, v.ucs_x_axis.1, v.ucs_x_axis.2)?;
    w.write_3bd(v.ucs_y_axis.0, v.ucs_y_axis.1, v.ucs_y_axis.2)?;
    w.write_bd(v.ucs_elevation)?;
    w.write_bs(v.ucs_ortho_view_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bit::BitReader;
    use crate::core::config::ParseConfig;
    use crate::dwg::decoder::Decoder;
    use crate::entities::common::parse_common_entity_header;
    use crate::entities::viewport::decode_viewport;
    use crate::objects::Handle;
    use crate::writer::ir::{CommonEntityProps, EntityFormatting};
    use std::path::PathBuf;

    fn common(entity_mode: u8) -> CommonEntityEncodeInput {
        CommonEntityEncodeInput {
            handle: 0x26B,
            owner_handle: 0x58,
            entity_mode,
            layer_handle: 0x10,
            color_index: 256,
            linetype_flags: 0,
            linetype_handle: 0,
            codepage: 30,
            formatting: EntityFormatting::default(),
            previous_handle: 0x267,
            next_handle: 0,
            style_handle: 0,
        }
    }

    fn sample() -> ViewportEntity {
        ViewportEntity {
            common: CommonEntityProps::default(),
            center: (210., 148.5, 0.),
            width: 400.,
            height: 277.,
            view_center: (12_000., 7_500.),
            view_height: 27_700.,
            status_flags: 0xC8060 | 0x4000,
            frozen_layers: vec!["A".into(), "B".into()],
            ..ViewportEntity::default()
        }
    }

    /// Read the R2000 viewport body straight from the specification table
    /// (20.4.38) and return it with the frozen layer count.
    fn read_r2000_body(r: &mut BitReader<'_>) -> (ViewportEntity, u32) {
        let pair = |r: &mut BitReader<'_>| {
            (
                r.read_rd(Endian::Little).unwrap(),
                r.read_rd(Endian::Little).unwrap(),
            )
        };
        let center = r.read_3bd().unwrap();
        let width = r.read_bd().unwrap();
        let height = r.read_bd().unwrap();
        let view_target = r.read_3bd().unwrap();
        let view_direction = r.read_3bd().unwrap();
        let twist_angle = r.read_bd().unwrap();
        let view_height = r.read_bd().unwrap();
        let lens_length = r.read_bd().unwrap();
        let front_clip_z = r.read_bd().unwrap();
        let back_clip_z = r.read_bd().unwrap();
        let snap_angle = r.read_bd().unwrap();
        let view_center = pair(r);
        let snap_base = pair(r);
        let snap_spacing = pair(r);
        let grid_spacing = pair(r);
        let circle_zoom = r.read_bs().unwrap();
        let frozen = r.read_bl().unwrap();
        let status_flags = r.read_bl().unwrap();
        let style_sheet = r.read_tv().unwrap();
        let render_mode = r.read_rc().unwrap();
        let ucs_at_origin = r.read_b().unwrap() != 0;
        let ucs_per_viewport = r.read_b().unwrap() != 0;
        let ucs_origin = r.read_3bd().unwrap();
        let ucs_x_axis = r.read_3bd().unwrap();
        let ucs_y_axis = r.read_3bd().unwrap();
        let ucs_elevation = r.read_bd().unwrap();
        let ucs_ortho_view_type = r.read_bs().unwrap();
        (
            ViewportEntity {
                common: CommonEntityProps::default(),
                center,
                width,
                height,
                view_target,
                view_direction,
                twist_angle,
                view_height,
                lens_length,
                front_clip_z,
                back_clip_z,
                snap_angle,
                view_center,
                snap_base,
                snap_spacing,
                grid_spacing,
                circle_zoom,
                status_flags,
                style_sheet,
                render_mode,
                ucs_at_origin,
                ucs_per_viewport,
                ucs_origin,
                ucs_x_axis,
                ucs_y_axis,
                ucs_elevation,
                ucs_ortho_view_type,
                frozen_layers: Vec::new(),
            },
            frozen,
        )
    }

    #[test]
    fn viewport_roundtrips_through_the_reader() {
        let viewport = sample();
        let payload = encode_viewport_entity_payload(common(1), &viewport, &[0x31, 0x32]).unwrap();
        let mut reader = BitReader::new(&payload);
        reader.set_pre_r2004_layout(true);
        assert_eq!(reader.read_bs().unwrap(), VIEWPORT_TYPE);
        let decoded = decode_viewport(&mut reader).unwrap();
        assert_eq!(decoded.handle, 0x26B);
        assert_eq!(decoded.layer_handle, 0x10);
        assert_eq!(decoded.center, Some((210., 148.5, 0.)));
        assert_eq!((decoded.width, decoded.height), (400., 277.));
        let view = decoded.view.unwrap();
        assert_eq!(view.view_center, (12_000., 7_500.));
        assert_eq!(view.view_height, 27_700.);
        assert_eq!(view.direction, (0., 0., 1.));
        assert_eq!(view.lens_length, 50.);
        assert_eq!(view.status_flags, 0xCC060);
        assert_eq!(decoded.frozen_layer_handles, vec![0x31, 0x32]);
        assert_eq!(decoded.clip_boundary_handle, None);

        // Every body field, including the UCS tail, ends exactly at obj_size.
        let mut reader = BitReader::new(&payload);
        reader.set_pre_r2004_layout(true);
        reader.read_bs().unwrap();
        let header = parse_common_entity_header(&mut reader).unwrap();
        assert_eq!(header.entity_mode, 1);
        let (body, frozen) = read_r2000_body(&mut reader);
        assert_eq!(reader.tell_bits(), u64::from(header.obj_size));
        assert_eq!(frozen, 2);
        assert_eq!(body.ucs_x_axis, (1., 0., 0.));
        assert_eq!(body.ucs_y_axis, (0., 1., 0.));
        assert!(body.ucs_per_viewport);
        assert_eq!(body.circle_zoom, 1000);
    }

    #[test]
    fn viewport_in_a_block_stores_its_owner() {
        let payload = encode_viewport_entity_payload(common(0), &sample(), &[]).unwrap();
        let mut reader = BitReader::new(&payload);
        reader.set_pre_r2004_layout(true);
        reader.read_bs().unwrap();
        let decoded = decode_viewport(&mut reader).unwrap();
        assert_eq!(decoded.layer_handle, 0x10);
        assert!(decoded.frozen_layer_handles.is_empty());
    }

    /// Ground truth: the model viewport of the active layout of an AutoCAD
    /// AC1015 drawing decodes with the body layout the encoder writes (every
    /// field through the UCS tail ends at `obj_size`), stores entmode 1, and
    /// re-encoding the decoded values reproduces its body bit for bit.
    #[test]
    fn autocad_r2000_viewport_matches_the_encoder() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../test_dwg/acadsharp/sample_AC1015.dwg");
        let bytes = std::fs::read(path).expect("sample file");
        let decoder = Decoder::new(&bytes, ParseConfig::default()).expect("decoder");
        let index = decoder.build_object_index().expect("object index");
        // Active layout (`*Paper_Space`, entmode 1) and an inactive layout
        // (`*Paper_Space0`, entmode 0 with the block record as owner).
        for (handle, paper_viewport, entity_mode) in [
            (0x267u64, true, 1u8),
            (0x26Bu64, false, 1),
            (0x240, true, 0),
            (0x245, false, 0),
        ] {
            let obj = *index.get(Handle(handle)).expect("viewport record");
            let record = decoder.parse_object_record(obj.offset).expect("record");
            let mut reader = record.bit_reader();
            assert_eq!(reader.read_bs().unwrap(), VIEWPORT_TYPE);
            let header = parse_common_entity_header(&mut reader).unwrap();
            assert_eq!(header.entity_mode, entity_mode, "entmode of {handle:X}");
            assert_eq!(header.num_of_reactors, 0);
            assert!(header.has_legacy_entity_links);
            let body_start = reader.tell_bits();
            let (body, frozen) = read_r2000_body(&mut reader);
            assert_eq!(reader.tell_bits(), u64::from(header.obj_size));
            assert_eq!(frozen, 0);
            assert_eq!(body.view_direction, (0., 0., 1.));
            assert_eq!(body.lens_length, 50.);
            assert_eq!(body.ucs_origin, (0., 0., 0.));
            assert_eq!(body.ucs_x_axis, (1., 0., 0.));
            assert_eq!(body.ucs_y_axis, (0., 1., 0.));
            assert_eq!(body.ucs_ortho_view_type, 0);
            assert_eq!(body.render_mode, 0);
            if paper_viewport {
                assert_eq!(body.status_flags, crate::writer::ir::PAPER_VIEWPORT_STATUS);
                assert_eq!(body.view_center, (body.center.0, body.center.1));
                assert_eq!(body.view_height, body.height);
            } else {
                assert_eq!(body.status_flags, 0xC8260);
            }
            let original: Vec<u8> = {
                let mut r = record.bit_reader();
                r.set_bit_pos(body_start as u32);
                (body_start..u64::from(header.obj_size))
                    .map(|_| r.read_b().unwrap())
                    .collect()
            };

            // Handle stream: xdictionary, previous, next, layer, then the
            // viewport handles (no frozen layers, clip, VX header, two UCS).
            reader.set_bit_pos(header.obj_size);
            let mut refs = Vec::new();
            while reader.tell_bits() < reader.total_bits() {
                match reader.read_h() {
                    Ok(h) => refs.push((h.code, read_handle_reference_value(h, handle))),
                    Err(_) => break,
                }
            }
            eprintln!("viewport {handle:X} handle stream: {refs:?}");
            // [owner], xdictionary, previous, next, layer, then the viewport
            // references: clip boundary, VIEWPORT ENT HEADER, named and base
            // UCS, all stored with code 5 (trailing padding bits may follow).
            let owner = usize::from(entity_mode == 0);
            assert!(refs.len() >= owner + 8, "handle stream: {refs:?}");
            if entity_mode == 0 {
                assert_eq!(refs[0], (4, 0x58), "owner block record: {refs:?}");
            }
            assert_eq!(refs[owner].0, 3, "xdictionary: {refs:?}");
            assert_eq!(refs[owner + 3], (5, 0x10), "layer: {refs:?}");
            let tail = &refs[owner + 4..owner + 8];
            assert_eq!(tail[0], (5, 0), "clip boundary: {refs:?}");
            assert_eq!(tail[1].0, 5, "viewport entity header: {refs:?}");
            assert_eq!(tail[2], (5, 0), "named UCS: {refs:?}");
            assert_eq!(tail[3], (5, 0), "base UCS: {refs:?}");

            let encoded = encode_viewport_entity_payload(
                CommonEntityEncodeInput {
                    handle,
                    owner_handle: 0x58,
                    entity_mode,
                    layer_handle: 0x10,
                    color_index: 256,
                    linetype_flags: 0,
                    linetype_handle: 0,
                    codepage: 30,
                    formatting: EntityFormatting::default(),
                    previous_handle: 0,
                    next_handle: 0,
                    style_handle: 0,
                },
                &body,
                &[],
            )
            .unwrap();
            let mut r = BitReader::new(&encoded);
            r.set_pre_r2004_layout(true);
            r.read_bs().unwrap();
            let h2 = parse_common_entity_header(&mut r).unwrap();
            let start = r.tell_bits();
            let mine: Vec<u8> = (start..u64::from(h2.obj_size))
                .map(|_| r.read_b().unwrap())
                .collect();
            assert_eq!(mine, original, "viewport {handle:X} body differs from AutoCAD");
        }
    }

    /// Absolute handle of a reference, as `read_handle_reference` resolves it.
    fn read_handle_reference_value(h: crate::bit::HandleRef, base: u64) -> u64 {
        match h.code {
            0x06 => base + 1,
            0x08 => base.saturating_sub(1),
            0x0A => base.saturating_add(h.value),
            0x0C => base.saturating_sub(h.value),
            _ => h.value,
        }
    }
}
