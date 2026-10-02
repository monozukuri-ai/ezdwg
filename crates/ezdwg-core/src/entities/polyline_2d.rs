use crate::bit::BitReader;
use crate::core::error::ErrorKind;
use crate::core::result::Result;
use crate::entities::common::{
    checked_handle_count, parse_common_entity_handles, parse_common_entity_header,
    parse_common_entity_header_r14, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, read_handle_reference, CommonEntityHeader,
};

#[derive(Debug, Clone, Copy)]
pub struct PolylineFlagsInfo {
    pub closed: bool,
    pub curve_fit: bool,
    pub spline_fit: bool,
    pub is_3d_polyline: bool,
    pub is_3d_mesh: bool,
    pub is_closed_mesh: bool,
    pub is_polyface_mesh: bool,
    pub continuous_linetype: bool,
}

impl PolylineFlagsInfo {
    pub fn from_flags(flags: u16) -> Self {
        Self {
            closed: flags & 0x01 != 0,
            curve_fit: flags & 0x02 != 0,
            spline_fit: flags & 0x04 != 0,
            is_3d_polyline: flags & 0x08 != 0,
            is_3d_mesh: flags & 0x10 != 0,
            is_closed_mesh: flags & 0x20 != 0,
            is_polyface_mesh: flags & 0x40 != 0,
            continuous_linetype: flags & 0x80 != 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PolylineCurveType {
    None,
    QuadraticBSpline,
    CubicBSpline,
    Bezier,
    Unknown(u16),
}

impl PolylineCurveType {
    pub fn from_code(code: u16) -> Self {
        match code {
            0 => Self::None,
            5 => Self::QuadraticBSpline,
            6 => Self::CubicBSpline,
            8 => Self::Bezier,
            other => Self::Unknown(other),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::QuadraticBSpline => "QuadraticBSpline",
            Self::CubicBSpline => "CubicBSpline",
            Self::Bezier => "Bezier",
            Self::Unknown(_) => "Unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Polyline2dEntity {
    pub handle: u64,
    pub flags: u16,
    pub curve_type: u16,
    pub flags_info: PolylineFlagsInfo,
    pub curve_type_info: PolylineCurveType,
    pub width_start: f64,
    pub width_end: f64,
    pub thickness: f64,
    pub elevation: f64,
    pub owned_handles: Vec<u64>,
}

/// R2004 layout (also the fallback for newer versions): owned VERTEX handles are
/// listed explicitly after an "Owned Object Count" BL.
pub fn decode_polyline_2d(reader: &mut BitReader<'_>) -> Result<Polyline2dEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_polyline_2d_with_header(reader, header, false, true)
}

/// R2000 layout: there is no "Owned Object Count" (ODA spec 20.4.16, R2004+ only);
/// the handle stream carries first/last VERTEX soft pointers instead, and the
/// vertices are resolved by scanning the objects that follow the polyline.
pub fn decode_polyline_2d_r2000(reader: &mut BitReader<'_>) -> Result<Polyline2dEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_polyline_2d_with_header(reader, header, false, false)
}

/// R13/R14 (ODA specification 20.4.16): thickness and extrusion are stored in
/// full (BD and 3BD), and there is no owned object count. The handle stream
/// names the first and the last vertex; the vertices are resolved through
/// their owner handle or by scanning the objects that follow.
pub fn decode_polyline_2d_r14(
    reader: &mut BitReader<'_>,
    object_handle: u64,
) -> Result<Polyline2dEntity> {
    let mut header = parse_common_entity_header_r14(reader)?;
    if header.handle == 0 {
        header.handle = object_handle;
    }
    let flags = reader.read_bs()?;
    let curve_type = reader.read_bs()?;
    let width_start = reader.read_bd()?;
    let width_end = reader.read_bd()?;
    let thickness = reader.read_bd()?;
    let elevation = reader.read_bd()?;
    let _extrusion = reader.read_3bd()?;

    Ok(Polyline2dEntity {
        handle: header.handle,
        flags,
        curve_type,
        flags_info: PolylineFlagsInfo::from_flags(flags),
        curve_type_info: PolylineCurveType::from_code(curve_type),
        width_start,
        width_end,
        thickness,
        elevation,
        owned_handles: Vec::new(),
    })
}

/// The R2004 layout read behind R13/R14 common entity data. No R13/R14 file
/// stores a polyline this way; the bindings use it to probe objects of unknown
/// classes, where an owned object count that resolves is the sign of a
/// polyline.
pub fn decode_polyline_2d_r14_speculative(
    reader: &mut BitReader<'_>,
    object_handle: u64,
) -> Result<Polyline2dEntity> {
    let mut header = parse_common_entity_header_r14(reader)?;
    if header.handle == 0 {
        header.handle = object_handle;
    }
    decode_polyline_2d_with_header(reader, header, false, true)
}

pub fn decode_polyline_2d_r2007(reader: &mut BitReader<'_>) -> Result<Polyline2dEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_polyline_2d_with_header(reader, header, true, true)
}

pub fn decode_polyline_2d_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<Polyline2dEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_polyline_2d_with_header(reader, header, true, true)
}

pub fn decode_polyline_2d_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<Polyline2dEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_polyline_2d_with_header(reader, header, true, true)
}

fn decode_polyline_2d_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    has_owned_count: bool,
) -> Result<Polyline2dEntity> {
    let flags = reader.read_bs()?;
    let curve_type = reader.read_bs()?;
    let flags_info = PolylineFlagsInfo::from_flags(flags);
    let curve_type_info = PolylineCurveType::from_code(curve_type);
    let width_start = reader.read_bd()?;
    let width_end = reader.read_bd()?;
    let thickness = reader.read_bt()?;
    let elevation = reader.read_bd()?;
    let _extrusion = reader.read_be()?;
    let owned_obj_count = if has_owned_count {
        reader.read_bl()? as usize
    } else {
        0
    };
    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let handles_pos = reader.get_pos();
    let owned_handles = match (|| -> Result<Vec<u64>> {
        let _common_handles = parse_common_entity_handles(reader, &header)?;
        let owned_obj_count =
            checked_handle_count(reader, owned_obj_count, "POLYLINE_2D owned object")?;
        let mut owned_handles = Vec::with_capacity(owned_obj_count);
        for _ in 0..owned_obj_count {
            owned_handles.push(read_handle_reference(reader, header.handle)?);
        }
        Ok(owned_handles)
    })() {
        Ok(owned_handles) => owned_handles,
        Err(err)
            if allow_handle_decode_failure
                && matches!(
                    err.kind,
                    ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                ) =>
        {
            reader.set_pos(handles_pos.0, handles_pos.1);
            let _ = parse_common_entity_layer_handle(reader, &header);
            Vec::new()
        }
        Err(err) => return Err(err),
    };

    Ok(Polyline2dEntity {
        handle: header.handle,
        flags,
        curve_type,
        flags_info,
        curve_type_info,
        width_start,
        width_end,
        thickness,
        elevation,
        owned_handles,
    })
}

#[cfg(test)]
mod tests {
    use super::{decode_polyline_2d, decode_polyline_2d_r2000};
    use crate::bit::{BitReader, BitWriter, Endian};
    use crate::core::error::ErrorKind;

    /// Minimal R2000 POLYLINE_2D object stream: common entity header, entity
    /// body, then the handle stream (layer only) at `obj_size` bits.
    fn build_polyline_2d_stream(owned_count: Option<u32>) -> Vec<u8> {
        let build = |obj_size: u32| -> (Vec<u8>, u32) {
            let mut w = BitWriter::new();
            w.write_rl(Endian::Little, obj_size).unwrap();
            w.write_h(4, 0x2D).unwrap(); // handle
            w.write_bs(0).unwrap(); // no EED
            w.write_b(0).unwrap(); // no proxy graphics
            w.write_bb(2).unwrap(); // entity mode: model space (no owner handle)
            w.write_bl(0).unwrap(); // reactors
            w.write_b(1).unwrap(); // xdic missing
            w.write_b(1).unwrap(); // no links
            w.write_b(0).unwrap(); // color
            w.write_bd(1.0).unwrap(); // ltype scale
            w.write_bb(0).unwrap(); // ltype flags
            w.write_bb(0).unwrap(); // plotstyle flags
            w.write_bs(0).unwrap(); // invisibility
            w.write_rc(0).unwrap(); // lineweight
            w.write_bs(1).unwrap(); // flags: closed
            w.write_bs(0).unwrap(); // curve type
            w.write_bd(0.5).unwrap(); // start width
            w.write_bd(0.25).unwrap(); // end width
            w.write_bt(0.0).unwrap(); // thickness
            w.write_bd(3.0).unwrap(); // elevation
            w.write_be(0.0, 0.0, 1.0).unwrap(); // extrusion
            if let Some(count) = owned_count {
                w.write_bl(count).unwrap();
            }
            let handles_at = w.tell_bits() as u32;
            w.write_h(4, 0x10).unwrap(); // layer
            (w.into_bytes(), handles_at)
        };
        let (_, handles_at) = build(0);
        build(handles_at).0
    }

    #[test]
    fn r2000_layout_has_no_owned_object_count() {
        let bytes = build_polyline_2d_stream(None);
        let mut reader = BitReader::new(&bytes);
        let entity = decode_polyline_2d_r2000(&mut reader).expect("decode R2000 polyline");
        assert_eq!(entity.handle, 0x2D);
        assert_eq!(entity.flags, 1);
        assert!(entity.flags_info.closed);
        assert_eq!(entity.width_start, 0.5);
        assert_eq!(entity.width_end, 0.25);
        assert_eq!(entity.elevation, 3.0);
        // Vertices of R13-R2000 polylines are resolved by scanning the objects
        // that follow, so no owned handles are reported here.
        assert!(entity.owned_handles.is_empty());
    }

    #[test]
    fn garbage_owned_object_count_is_a_format_error_not_an_abort() {
        // A corrupted (or misread) count near u32::MAX used to drive
        // `Vec::with_capacity`, reserving ~32 GB and aborting the process.
        let bytes = build_polyline_2d_stream(Some(u32::MAX - 7));
        let mut reader = BitReader::new(&bytes);
        let err = decode_polyline_2d(&mut reader).expect_err("garbage count must fail");
        assert_eq!(err.kind, ErrorKind::Format);
        assert!(err
            .to_string()
            .contains("exceeds the remaining handle stream"));
    }

    #[test]
    fn r2004_layout_reads_owned_handles() {
        let bytes = build_polyline_2d_stream(Some(0));
        let mut reader = BitReader::new(&bytes);
        let entity = decode_polyline_2d(&mut reader).expect("decode R2004 polyline");
        assert_eq!(entity.handle, 0x2D);
        assert!(entity.owned_handles.is_empty());
    }
}
