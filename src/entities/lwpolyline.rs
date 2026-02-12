use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r14,
    parse_common_entity_header_r2007, parse_common_entity_header_r2010,
    parse_common_entity_header_r2013, parse_common_entity_layer_handle, CommonEntityColor,
    CommonEntityHeader,
};

#[derive(Debug, Clone)]
pub struct LwPolylineEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub flags: u16,
    pub vertices: Vec<(f64, f64)>,
    pub const_width: Option<f64>,
    pub bulges: Vec<f64>,
    pub widths: Vec<(f64, f64)>,
}

const MAX_LWPOLYLINE_ITEMS: usize = 1_000_000;
const MAX_R14_LWPOLYLINE_SCAN_BITS: u64 = 4096;

pub fn decode_lwpolyline(reader: &mut BitReader<'_>) -> Result<LwPolylineEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_lwpolyline_with_header(reader, header, false, false, false)
}

pub fn decode_lwpolyline_r14(
    reader: &mut BitReader<'_>,
    object_handle: u64,
    type_code: u16,
) -> Result<LwPolylineEntity> {
    let start = reader.get_pos();
    let attempts_default = [
        (true, true),   // R14 common header + R14 vertex format (2RD x N)
        (true, false),  // R14 common header + R2000+ vertex format
        (false, true),  // R2000 common header + R14 vertex format
        (false, false), // R2000 common header + R2000+ vertex format
    ];
    let debug_enabled = std::env::var("EZDWG_DEBUG_LWPOLYLINE")
        .ok()
        .is_some_and(|value| value != "0");
    let mut last_err: Option<crate::core::error::DwgError> = None;

    // R14 dynamic-class LWPOLYLINE often uses compact header/body layout.
    if type_code >= 0x01F4 {
        for r13_r14_vertex_mode in [true, false] {
            reader.set_pos(start.0, start.1);
            match decode_lwpolyline_r14_compact_attempt(reader, object_handle, r13_r14_vertex_mode)
            {
                Ok(entity) => {
                    if is_plausible_lwpolyline_entity(&entity) {
                        if debug_enabled {
                            eprintln!(
                                "[lwpolyline-r14] recovered by compact header: verts={} flags=0x{:X}",
                                entity.vertices.len(),
                                entity.flags
                            );
                        }
                        return Ok(entity);
                    }
                }
                Err(err) => {
                    if debug_enabled {
                        eprintln!(
                            "[lwpolyline-r14] compact attempt vertex_mode={} failed: {}",
                            if r13_r14_vertex_mode { "r14" } else { "r2000" },
                            err
                        );
                    }
                    last_err = Some(err);
                }
            }
        }
    }

    for (use_r14_header, r13_r14_vertex_mode) in attempts_default {
        reader.set_pos(start.0, start.1);
        match decode_lwpolyline_r14_attempt(
            reader,
            object_handle,
            use_r14_header,
            r13_r14_vertex_mode,
        ) {
            Ok(entity) => {
                if is_plausible_lwpolyline_entity(&entity) {
                    return Ok(entity);
                }
                if debug_enabled {
                    eprintln!(
                        "[lwpolyline-r14] attempt header={} vertex_mode={} produced implausible entity (verts={}, flags=0x{:X})",
                        if use_r14_header { "r14" } else { "r2000" },
                        if r13_r14_vertex_mode { "r14" } else { "r2000" },
                        entity.vertices.len(),
                        entity.flags
                    );
                }
            }
            Err(err) => {
                if debug_enabled {
                    eprintln!(
                        "[lwpolyline-r14] attempt header={} vertex_mode={} failed: {}",
                        if use_r14_header { "r14" } else { "r2000" },
                        if r13_r14_vertex_mode { "r14" } else { "r2000" },
                        err
                    );
                }
                last_err = Some(err);
            }
        }
    }

    if type_code < 0x01F4 {
        for r13_r14_vertex_mode in [true, false] {
            reader.set_pos(start.0, start.1);
            match decode_lwpolyline_r14_compact_attempt(reader, object_handle, r13_r14_vertex_mode)
            {
                Ok(entity) => {
                    if is_plausible_lwpolyline_entity(&entity) {
                        if debug_enabled {
                            eprintln!(
                                "[lwpolyline-r14] recovered by compact header: verts={} flags=0x{:X}",
                                entity.vertices.len(),
                                entity.flags
                            );
                        }
                        return Ok(entity);
                    }
                }
                Err(err) => {
                    if debug_enabled {
                        eprintln!(
                            "[lwpolyline-r14] compact attempt vertex_mode={} failed: {}",
                            if r13_r14_vertex_mode { "r14" } else { "r2000" },
                            err
                        );
                    }
                    last_err = Some(err);
                }
            }
        }
    }

    reader.set_pos(start.0, start.1);
    if let Ok(entity) = decode_lwpolyline_r14_scan_by_obj_size(reader, object_handle) {
        if debug_enabled {
            eprintln!(
                "[lwpolyline-r14] recovered by body scan: verts={} flags=0x{:X}",
                entity.vertices.len(),
                entity.flags
            );
        }
        return Ok(entity);
    }

    if let Some(err) = last_err {
        return Err(err);
    }

    Err(DwgError::new(
        ErrorKind::Decode,
        "failed to decode R14 LWPOLYLINE entity",
    ))
}

pub fn decode_lwpolyline_r2007(reader: &mut BitReader<'_>) -> Result<LwPolylineEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_lwpolyline_with_header(reader, header, true, true, false)
}

pub fn decode_lwpolyline_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<LwPolylineEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_lwpolyline_with_header(reader, header, true, true, false)
}

pub fn decode_lwpolyline_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<LwPolylineEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_lwpolyline_with_header(reader, header, true, true, false)
}

fn decode_lwpolyline_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
    r13_r14_vertex_mode: bool,
) -> Result<LwPolylineEntity> {
    let body = decode_lwpolyline_body(reader, r13_r14_vertex_mode)?;
    let layer_handle = decode_lwpolyline_layer_handle(
        reader,
        &header,
        allow_handle_decode_failure,
        r2007_layer_only,
    )?;

    Ok(LwPolylineEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        flags: body.flags,
        vertices: body.vertices,
        const_width: body.const_width,
        bulges: body.bulges,
        widths: body.widths,
    })
}

fn decode_lwpolyline_r14_attempt(
    reader: &mut BitReader<'_>,
    object_handle: u64,
    use_r14_header: bool,
    r13_r14_vertex_mode: bool,
) -> Result<LwPolylineEntity> {
    let mut header = if use_r14_header {
        parse_common_entity_header_r14(reader)?
    } else {
        parse_common_entity_header(reader)?
    };
    if header.handle == 0 {
        header.handle = object_handle;
    }
    decode_lwpolyline_with_header(reader, header, true, false, r13_r14_vertex_mode)
}

fn decode_lwpolyline_r14_compact_attempt(
    reader: &mut BitReader<'_>,
    object_handle: u64,
    r13_r14_vertex_mode: bool,
) -> Result<LwPolylineEntity> {
    let header = parse_r14_lwpolyline_compact_header(reader, object_handle)?;
    decode_lwpolyline_with_header(reader, header, true, false, r13_r14_vertex_mode)
}

#[derive(Debug, Clone)]
struct LwPolylineBody {
    flags: u16,
    vertices: Vec<(f64, f64)>,
    const_width: Option<f64>,
    bulges: Vec<f64>,
    widths: Vec<(f64, f64)>,
}

fn decode_lwpolyline_body(
    reader: &mut BitReader<'_>,
    r13_r14_vertex_mode: bool,
) -> Result<LwPolylineBody> {
    let flags = reader.read_bs()?;
    let const_width = if (flags & 0x04) != 0 {
        Some(reader.read_bd()?)
    } else {
        None
    };
    if (flags & 0x08) != 0 {
        let _elevation = reader.read_bd()?;
    }
    if (flags & 0x02) != 0 {
        let _thickness = reader.read_bd()?;
    }
    if (flags & 0x01) != 0 {
        let _normal = reader.read_3bd()?;
    }

    let num_verts = reader.read_bl()? as usize;
    validate_lwpolyline_count("vertex count", num_verts)?;
    let num_bulges = if (flags & 0x10) != 0 {
        reader.read_bl()? as usize
    } else {
        0
    };
    validate_lwpolyline_count("bulge count", num_bulges)?;
    let num_vertex_ids = if (flags & 0x0400) != 0 {
        reader.read_bl()? as usize
    } else {
        0
    };
    validate_lwpolyline_count("vertex-id count", num_vertex_ids)?;
    let num_widths = if (flags & 0x20) != 0 {
        reader.read_bl()? as usize
    } else {
        0
    };
    validate_lwpolyline_count("width count", num_widths)?;

    let mut vertices = Vec::with_capacity(num_verts);
    if num_verts > 0 {
        if r13_r14_vertex_mode {
            for _ in 0..num_verts {
                let x = reader.read_rd(Endian::Little)?;
                let y = reader.read_rd(Endian::Little)?;
                vertices.push((x, y));
            }
        } else {
            let x0 = reader.read_rd(Endian::Little)?;
            let y0 = reader.read_rd(Endian::Little)?;
            vertices.push((x0, y0));
            for _ in 1..num_verts {
                let x = reader.read_dd(vertices.last().unwrap().0)?;
                let y = reader.read_dd(vertices.last().unwrap().1)?;
                vertices.push((x, y));
            }
        }
    }

    let mut bulges = Vec::new();
    if num_bulges > 0 {
        let mut normalized = vec![0.0; num_verts];
        for idx in 0..num_bulges {
            let bulge = reader.read_bd()?;
            if idx < normalized.len() {
                normalized[idx] = bulge;
            }
        }
        bulges = normalized;
    }

    for _ in 0..num_vertex_ids {
        let _vertex_id = reader.read_bl()?;
    }

    let mut widths = Vec::new();
    if num_widths > 0 || const_width.is_some() {
        let mut normalized = vec![(0.0, 0.0); num_verts];
        if let Some(width) = const_width {
            normalized.fill((width, width));
        }
        for idx in 0..num_widths {
            let start_width = reader.read_bd()?;
            let end_width = reader.read_bd()?;
            if idx < normalized.len() {
                normalized[idx] = (start_width, end_width);
            }
        }
        widths = normalized;
    }

    Ok(LwPolylineBody {
        flags,
        vertices,
        const_width,
        bulges,
        widths,
    })
}

fn decode_lwpolyline_layer_handle(
    reader: &mut BitReader<'_>,
    header: &CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
) -> Result<u64> {
    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let layer_handle = match if r2007_layer_only {
        parse_common_entity_layer_handle(reader, header)
    } else {
        parse_common_entity_handles(reader, header).map(|common_handles| common_handles.layer)
    } {
        Ok(layer_handle) => layer_handle,
        Err(err)
            if allow_handle_decode_failure
                && matches!(
                    err.kind,
                    ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                ) =>
        {
            0
        }
        Err(err) => return Err(err),
    };
    Ok(layer_handle)
}

fn parse_r14_lwpolyline_compact_header(
    reader: &mut BitReader<'_>,
    object_handle: u64,
) -> Result<CommonEntityHeader> {
    let mut handle = reader.read_h()?.value;
    if handle == 0 {
        handle = object_handle;
    }
    skip_eed(reader)?;

    let graphic_present_flag = reader.read_b()?;
    if graphic_present_flag == 1 {
        let graphic_size = reader.read_rl(Endian::Little)? as usize;
        let _ = reader.read_rcs(graphic_size)?;
    }

    let obj_size = reader.read_rl(Endian::Little)?;
    let entity_mode = reader.read_bb()?;
    let num_of_reactors = reader.read_bl()?;
    let xdic_missing_flag = reader.read_b()?;
    let is_bylayer_ltype = reader.read_b()? != 0;
    let no_links = reader.read_b()?;
    if no_links == 0 {
        return Err(DwgError::new(
            ErrorKind::Format,
            "R14 compact LWPOLYLINE header is not applicable",
        ));
    }

    let _color_unknown = reader.read_b()?;
    let _ltype_scale = reader.read_bd()?;
    let _invisibility = reader.read_bs()?;

    Ok(CommonEntityHeader {
        obj_size,
        handle,
        color: CommonEntityColor::default(),
        entity_mode,
        num_of_reactors,
        xdic_missing_flag,
        has_ds_binary_data: false,
        ltype_flags: if is_bylayer_ltype { 0 } else { 3 },
        plotstyle_flags: 0,
        material_flags: 0,
        has_full_visual_style: false,
        has_face_visual_style: false,
        has_edge_visual_style: false,
        has_legacy_entity_links: false,
    })
}

fn skip_eed(reader: &mut BitReader<'_>) -> Result<()> {
    let mut ext_size = reader.read_bs()?;
    while ext_size > 0 {
        let _app_handle = reader.read_h()?;
        for _ in 0..ext_size {
            let _ = reader.read_rc()?;
        }
        ext_size = reader.read_bs()?;
    }
    Ok(())
}

fn decode_lwpolyline_r14_scan_by_obj_size(
    reader: &mut BitReader<'_>,
    object_handle: u64,
) -> Result<LwPolylineEntity> {
    let start = reader.get_pos();
    let base_bit = (start.0 as u64) * 8 + start.1 as u64;
    let mut header = parse_common_entity_header_r14(reader)?;
    if header.handle == 0 {
        header.handle = object_handle;
    }
    let target_end = header.obj_size as u64;
    if target_end <= base_bit {
        return Err(DwgError::new(
            ErrorKind::Format,
            "invalid R14 LWPOLYLINE object stream boundary",
        ));
    }

    let max_scan = (target_end - base_bit).min(MAX_R14_LWPOLYLINE_SCAN_BITS);
    let mut best: Option<(u64, bool, LwPolylineBody)> = None;
    for delta in 0..=max_scan {
        for r13_r14_vertex_mode in [true, false] {
            let mut probe = reader.clone();
            let Some(start_bit) = base_bit.checked_add(delta) else {
                continue;
            };
            if start_bit >= target_end {
                continue;
            }
            let Ok(start_bit_u32) = u32::try_from(start_bit) else {
                continue;
            };
            probe.set_bit_pos(start_bit_u32);
            let Ok(body) = decode_lwpolyline_body(&mut probe, r13_r14_vertex_mode) else {
                continue;
            };
            if probe.tell_bits() != target_end {
                continue;
            }
            if !is_plausible_lwpolyline_body(&body) {
                continue;
            }
            let score = delta + if r13_r14_vertex_mode { 0 } else { 16 };
            match &best {
                Some((best_score, _, _)) if *best_score <= score => {}
                _ => best = Some((score, r13_r14_vertex_mode, body)),
            }
        }
    }

    let (_, _mode, body) = best.ok_or_else(|| {
        DwgError::new(
            ErrorKind::Decode,
            "failed to locate plausible R14 LWPOLYLINE body",
        )
    })?;
    let layer_handle = decode_lwpolyline_layer_handle(reader, &header, true, false)?;

    Ok(LwPolylineEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        flags: body.flags,
        vertices: body.vertices,
        const_width: body.const_width,
        bulges: body.bulges,
        widths: body.widths,
    })
}

fn is_plausible_lwpolyline_body(body: &LwPolylineBody) -> bool {
    is_plausible_lwpolyline_vertices(&body.vertices)
}

fn is_plausible_lwpolyline_vertices(vertices: &[(f64, f64)]) -> bool {
    if vertices.len() < 2 {
        return false;
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for &(x, y) in vertices {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        if x.abs() > 1.0e9 || y.abs() > 1.0e9 {
            return false;
        }
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    let span = (max_x - min_x).abs() + (max_y - min_y).abs();
    span > 1.0e-9
}

fn is_plausible_lwpolyline_entity(entity: &LwPolylineEntity) -> bool {
    is_plausible_lwpolyline_vertices(&entity.vertices)
}

fn validate_lwpolyline_count(label: &str, count: usize) -> Result<()> {
    if count > MAX_LWPOLYLINE_ITEMS {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!(
                "invalid LWPOLYLINE {}: {} (max {})",
                label, count, MAX_LWPOLYLINE_ITEMS
            ),
        ));
    }
    Ok(())
}
