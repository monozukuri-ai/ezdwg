use crate::bit::BitReader;
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r14,
    read_handle_reference, CommonEntityHeader,
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

pub fn decode_polyline_2d(reader: &mut BitReader<'_>) -> Result<Polyline2dEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_polyline_2d_with_header(reader, header)
}

pub fn decode_polyline_2d_r14(
    reader: &mut BitReader<'_>,
    object_handle: u64,
) -> Result<Polyline2dEntity> {
    let mut header = parse_common_entity_header_r14(reader)?;
    if header.handle == 0 {
        header.handle = object_handle;
    }
    decode_polyline_2d_with_header(reader, header)
}

fn decode_polyline_2d_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
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
    let owned_obj_count = reader.read_bl()? as usize;
    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let _common_handles = parse_common_entity_handles(reader, &header)?;

    let mut owned_handles = Vec::with_capacity(owned_obj_count);
    for _ in 0..owned_obj_count {
        owned_handles.push(read_handle_reference(reader, header.handle)?);
    }

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
