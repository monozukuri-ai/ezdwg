//! Sparse XRECORD object decoder.
//!
//! Implements the DWG XRECORD body and handle-stream layout:
//!
//! Data section:
//!   BL          xdata_size
//!   XDATA[…]    raw group-code / value pairs (xdata_size bytes)
//!   BS          cloning          (R2000+)
//!
//! Handle stream:
//!   COMMON_OBJECT_HANDLE_DATA   (owner, reactors, xdic)
//!   H[]         objid_handles   (code 4, variable length — read until stream end)
//!
//! The XDATA payload is deliberately kept as an opaque byte slice plus a
//! structured parse helper (`parse_xdata_groups`). Callers that need the
//! AutoCAD layer-state layout (or any other XRECORD schema) build on top of
//! the group-code view without the core having to know about them.
//!
//! This keeps the decoder stable when new XRECORD consumers appear and avoids
//! the "quick parse only the fields we need today" trap.

use crate::bit::{BitReader, HandleRef};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::dwg::version::DwgVersion;
use crate::objects::dictionary::{
    decode_common_object_handles, resolve_handle_ref, version_at_least, CommonObjectHandles,
};
use crate::objects::handle::Handle;

/// A single DXF-style group code + value extracted from XRECORD xdata.
///
/// Values are stored in a tagged union so the decoder never discards
/// information. Higher layers convert to domain types (mask integers,
/// layer names, colours, …) as needed.
#[derive(Debug, Clone, PartialEq)]
pub enum XDataValue {
    /// Group codes 0–9, 100–104, 300–309, 410–419, 430–439, 470–479, 999, …
    String(String),
    /// Group codes 10–39 (3D points are three consecutive codes).
    Real(f64),
    /// Group codes 40–59.
    Real2(f64),
    /// Group codes 60–79, 170–179, 270–289, 370–379, 400–409, 1060–1070.
    Int16(i16),
    /// Group codes 90–99, 420–429, 440–449, 1071.
    Int32(i32),
    /// Group codes 160–169.
    Int64(i64),
    /// Group codes 290–299.
    Bool(bool),
    /// Group codes 310–319, 320–329, 330–369, 390–399 (handles / binary).
    Binary(Vec<u8>),
    /// Group codes 330–369, 390–399 when interpreted as a handle reference.
    Handle(HandleRef),
    /// Unrecognised or truncated payload kept as raw bytes for diagnostics.
    Raw(Vec<u8>),
}

/// One group-code entry inside an XRECORD.
#[derive(Debug, Clone, PartialEq)]
pub struct XDataGroup {
    pub code: i16,
    pub value: XDataValue,
}

/// Fully decoded XRECORD.
#[derive(Debug, Clone, PartialEq)]
pub struct XRecord {
    pub handle: Handle,
    /// Size in bytes of the xdata payload as stored in the DWG.
    pub xdata_size: u32,
    /// Raw xdata bytes (exactly `xdata_size` long when decode succeeded).
    pub xdata: Vec<u8>,
    /// Structured view of `xdata`. Empty when the payload could not be parsed
    /// as classic DXF groups (corrupt or proprietary binary).
    pub groups: Vec<XDataGroup>,
    /// DXF group 280. Present from R2000 onward.
    pub cloning: Option<u16>,

    // --- common object handle data ---
    pub num_reactors: u32,
    pub is_xdic_missing: bool,
    pub has_ds_data: bool,
    pub owner_handle: Option<Handle>,
    pub reactors: Vec<Handle>,
    pub xdic_handle: Option<Handle>,

    /// Soft-pointer handles owned by this XRECORD (layer handles, etc.).
    pub objid_handles: Vec<Handle>,
}

impl XRecord {
    /// Convenience: first string value for a given group code, if any.
    pub fn first_string(&self, code: i16) -> Option<&str> {
        self.groups.iter().find_map(|g| {
            if g.code == code {
                if let XDataValue::String(s) = &g.value {
                    return Some(s.as_str());
                }
            }
            None
        })
    }

    /// Convenience: first Int32 value for a given group code, if any.
    pub fn first_i32(&self, code: i16) -> Option<i32> {
        self.groups.iter().find_map(|g| {
            if g.code == code {
                if let XDataValue::Int32(v) = &g.value {
                    return Some(*v);
                }
            }
            None
        })
    }
}

/// Context for XRECORD decoding (mirrors `DictionaryDecodeCtx`).
#[derive(Debug, Clone)]
pub struct XRecordDecodeCtx {
    pub version: DwgVersion,
    pub object_handle: u64,
}

/// Decode the data section of an XRECORD.
///
/// On entry the reader **must** be positioned at `xdata_size` (first field of
/// the XRECORD body proper). Callers are responsible for skipping the common
/// non-entity data-section preamble (object handle, EED, reactor count /
/// xdic flag / has_ds on R2004+) before calling this.
///
/// `data_section_end_bit` is the absolute bit position where the handle stream
/// begins. When provided, the decoder asserts that after `xdata` + optional
/// `cloning` the reader sits exactly one bit before the handle stream (the
/// pad bit observed on AC1032 layer-state XRECORDs). A wrong gap is a hard
/// error — there is no size-recovery heuristic.
pub fn decode_xrecord_data(
    reader: &mut BitReader<'_>,
    ctx: &XRecordDecodeCtx,
) -> Result<XRecordData> {
    decode_xrecord_data_with_end(reader, ctx, None)
}

/// Same as [`decode_xrecord_data`] with an optional handle-stream start bit
/// for the 1-bit gap assertion after cloning.
pub fn decode_xrecord_data_with_end(
    reader: &mut BitReader<'_>,
    ctx: &XRecordDecodeCtx,
    data_section_end_bit: Option<u64>,
) -> Result<XRecordData> {
    let xdata_size = reader.read_bl()?;
    // Protect against corrupt sizes. 16 MiB is already far beyond any
    // legitimate single XRECORD.
    const MAX_XDATA: u32 = 16 * 1024 * 1024;
    if xdata_size > MAX_XDATA {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!("XRECORD xdata_size {xdata_size} exceeds safe limit {MAX_XDATA}"),
        ));
    }

    let xdata = reader.read_rcs(xdata_size as usize)?;

    let cloning = if version_at_least(&ctx.version, &DwgVersion::R2000) {
        Some(reader.read_bs()?)
    } else {
        None
    };

    if let Some(end) = data_section_end_bit {
        let now = reader.tell_bits();
        // AC1032 layer-state XRECORDs leave exactly 1 pad bit before the
        // handle stream. Zero is also accepted (byte-aligned objects).
        // Any other gap means the preamble/size/cloning geometry is wrong.
        let gap = end.saturating_sub(now);
        if gap > 1 {
            return Err(DwgError::new(
                ErrorKind::Format,
                format!(
                    "XRECORD data section ends at bit {now}, handle stream at {end} \
                     (gap {gap} bits; expected 0 or 1). Caller likely skipped the \
                     wrong preamble or mis-read xdata_size."
                ),
            ));
        }
    }

    let r2007 = version_at_least(&ctx.version, &DwgVersion::R2007);
    let groups = parse_xdata_groups_versioned(&xdata, r2007)?;

    Ok(XRecordData {
        xdata_size,
        xdata,
        groups,
        cloning,
    })
}

/// Intermediate data-section result.
#[derive(Debug, Clone, PartialEq)]
pub struct XRecordData {
    pub xdata_size: u32,
    pub xdata: Vec<u8>,
    pub groups: Vec<XDataGroup>,
    pub cloning: Option<u16>,
}

/// Decode the handle stream of an XRECORD.
///
/// After the common object handles, every remaining handle in the stream is
/// collected as an `objid_handle` (LibreDWG reads until the handle-stream
/// bit budget is exhausted).
///
/// When `data_section_common` is `Some((num_reactors, is_xdic_missing, has_ds_data))`,
/// those counts/flags were already read from the data section (R2004+ layout)
/// and the handle stream starts at ownerhandle — do not re-read BL/B/B.
pub fn decode_xrecord_handles(
    handle_reader: &mut BitReader<'_>,
    version: &DwgVersion,
    object_handle: u64,
    handle_stream_end_bits: u64,
    data_section_common: Option<(u32, bool, bool)>,
) -> Result<XRecordHandles> {
    let common = match data_section_common {
        Some((num_reactors, is_xdic_missing, has_ds_data)) => {
            crate::objects::dictionary::decode_common_object_handle_refs(
                handle_reader,
                version,
                object_handle,
                num_reactors,
                is_xdic_missing,
                has_ds_data,
            )?
        }
        None => decode_common_object_handles(handle_reader, version, object_handle)?,
    };

    let mut objid_handles = Vec::new();
    // Read handles until we hit the end of the handle stream or a null ref.
    // Bound the loop so a corrupt stream cannot allocate forever.
    const MAX_OBJID: usize = 100_000;
    while (handle_reader.tell_bits() as u64) < handle_stream_end_bits
        && objid_handles.len() < MAX_OBJID
    {
        let href = match handle_reader.read_h() {
            Ok(h) => h,
            Err(_) => break,
        };
        match resolve_handle_ref(&href, object_handle)? {
            Some(h) if h != 0 => objid_handles.push(Handle(h)),
            _ => break,
        }
    }

    Ok(XRecordHandles {
        common,
        objid_handles,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XRecordHandles {
    pub common: CommonObjectHandles,
    pub objid_handles: Vec<Handle>,
}

/// Assemble the final `XRecord`.
pub fn assemble_xrecord(
    object_handle: u64,
    data: XRecordData,
    handles: XRecordHandles,
) -> XRecord {
    XRecord {
        handle: Handle(object_handle),
        xdata_size: data.xdata_size,
        xdata: data.xdata,
        groups: data.groups,
        cloning: data.cloning,
        num_reactors: handles.common.num_reactors,
        is_xdic_missing: handles.common.is_xdic_missing,
        has_ds_data: handles.common.has_ds_data,
        owner_handle: handles.common.owner_handle,
        reactors: handles.common.reactors,
        xdic_handle: handles.common.xdic_handle,
        objid_handles: handles.objid_handles,
    }
}

/// One-shot convenience decoder.
///
/// `data_section_common` — see [`decode_xrecord_handles`].
pub fn decode_xrecord(
    reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    ctx: &XRecordDecodeCtx,
    handle_stream_end_bits: u64,
    data_section_common: Option<(u32, bool, bool)>,
) -> Result<XRecord> {
    let data = decode_xrecord_data(reader, ctx)?;
    let handles = decode_xrecord_handles(
        handle_reader,
        &ctx.version,
        ctx.object_handle,
        handle_stream_end_bits,
        data_section_common,
    )?;
    Ok(assemble_xrecord(ctx.object_handle, data, handles))
}

// ---------------------------------------------------------------------------
// XDATA group-code parser
// ---------------------------------------------------------------------------

/// Why [`parse_xdata_groups_versioned`] stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XDataParseStop {
    /// Consumed the entire buffer with only well-formed groups.
    CleanEnd,
    /// Group code outside LibreDWG's 0..2000 range.
    InvalidCode { code: i16, byte_offset: usize },
    /// Value payload truncated mid-field.
    TruncatedValue { code: i16, byte_offset: usize },
    /// Unrecognised group code (no fixed width; refused to guess).
    UnknownCode { code: i16, byte_offset: usize },
    /// Hit the safety group-count ceiling.
    MaxGroups,
}

/// Detailed result of an XDATA parse (groups + stop reason + bytes consumed).
#[derive(Debug, Clone, PartialEq)]
pub struct XDataParseResult {
    pub groups: Vec<XDataGroup>,
    pub stop: XDataParseStop,
    pub bytes_consumed: usize,
}

/// Parse a classic DXF-style xdata blob into group-code / value pairs.
///
/// Layout follows LibreDWG `dwg_decode_xdata` (src/decode.c):
///
///   RS   group code
///   …    value whose width depends on the code range / DWG version
///
/// String encoding (LibreDWG / ODA):
/// - **R2007+:** RS length (code units) + UCS-2LE (`length` × RS)
/// - **pre-R2007:** RS length + RC codepage + `length` raw bytes
///
/// When `r2007_plus` is true, the R2007+ string form is used. Callers that
/// do not know the version should prefer `true` for modern DWGs (AC1021+).
///
/// Returns `Ok` only on a **clean end** (entire buffer consumed, no invalid
/// or unknown codes). Any other stop reason is `Err` with the reason in the
/// message; use [`parse_xdata_groups_detailed`] when partial groups are useful.
pub fn parse_xdata_groups(xdata: &[u8]) -> Result<Vec<XDataGroup>> {
    parse_xdata_groups_versioned(xdata, true)
}

/// Version-aware XDATA parser. See [`parse_xdata_groups`].
pub fn parse_xdata_groups_versioned(
    xdata: &[u8],
    r2007_plus: bool,
) -> Result<Vec<XDataGroup>> {
    let detailed = parse_xdata_groups_detailed(xdata, r2007_plus);
    match detailed.stop {
        XDataParseStop::CleanEnd => Ok(detailed.groups),
        XDataParseStop::InvalidCode { code, byte_offset } => Err(DwgError::new(
            ErrorKind::Format,
            format!("XDATA invalid group code {code} at byte {byte_offset}"),
        )),
        XDataParseStop::TruncatedValue { code, byte_offset } => Err(DwgError::new(
            ErrorKind::Format,
            format!("XDATA truncated value for group {code} at byte {byte_offset}"),
        )),
        XDataParseStop::UnknownCode { code, byte_offset } => Err(DwgError::new(
            ErrorKind::Format,
            format!("XDATA unknown group code {code} at byte {byte_offset}"),
        )),
        XDataParseStop::MaxGroups => Err(DwgError::new(
            ErrorKind::Format,
            "XDATA exceeded maximum group count".to_string(),
        )),
    }
}

/// Parse XDATA and always return groups + stop reason (never errors on
/// content; only empty input yields CleanEnd with zero groups).
pub fn parse_xdata_groups_detailed(
    xdata: &[u8],
    r2007_plus: bool,
) -> XDataParseResult {
    let mut reader = BitReader::new(xdata);
    reader.set_pos(0, 0);

    let mut groups = Vec::new();
    const MAX_GROUPS: usize = 1_000_000;

    if xdata.is_empty() {
        return XDataParseResult {
            groups,
            stop: XDataParseStop::CleanEnd,
            bytes_consumed: 0,
        };
    }

    loop {
        let (byte_pos, bit_pos) = reader.get_pos();
        if byte_pos >= xdata.len() {
            return XDataParseResult {
                groups,
                stop: XDataParseStop::CleanEnd,
                bytes_consumed: byte_pos,
            };
        }
        // Residual bits in the last byte with no full RS left → clean if no bit offset issues.
        if byte_pos + 2 > xdata.len() {
            // Incomplete RS at end of buffer.
            if bit_pos == 0 && byte_pos == xdata.len() {
                return XDataParseResult {
                    groups,
                    stop: XDataParseStop::CleanEnd,
                    bytes_consumed: byte_pos,
                };
            }
            if groups.is_empty() && byte_pos == 0 {
                // nothing consumed
            }
            return XDataParseResult {
                groups,
                stop: if bit_pos == 0 && byte_pos == xdata.len() {
                    XDataParseStop::CleanEnd
                } else {
                    XDataParseStop::TruncatedValue {
                        code: -1,
                        byte_offset: byte_pos,
                    }
                },
                bytes_consumed: byte_pos,
            };
        }

        if groups.len() >= MAX_GROUPS {
            return XDataParseResult {
                groups,
                stop: XDataParseStop::MaxGroups,
                bytes_consumed: byte_pos,
            };
        }

        let code = match reader.read_rs(crate::bit::Endian::Little) {
            Ok(c) => c as i16,
            Err(_) => {
                return XDataParseResult {
                    groups,
                    stop: XDataParseStop::TruncatedValue {
                        code: -1,
                        byte_offset: byte_pos,
                    },
                    bytes_consumed: byte_pos,
                };
            }
        };
        // LibreDWG rejects type < 0 or >= 2000 as invalid xdata.
        if code < 0 || code >= 2000 {
            return XDataParseResult {
                groups,
                stop: XDataParseStop::InvalidCode {
                    code,
                    byte_offset: byte_pos,
                },
                bytes_consumed: byte_pos,
            };
        }

        let value = match read_xdata_value(&mut reader, code, r2007_plus) {
            Ok(v) => v,
            Err(e) => {
                // Distinguish unknown-code from truncation via message prefix.
                let msg = e.to_string();
                let stop = if msg.contains("unknown XDATA group") {
                    XDataParseStop::UnknownCode {
                        code,
                        byte_offset: byte_pos,
                    }
                } else {
                    XDataParseStop::TruncatedValue {
                        code,
                        byte_offset: byte_pos,
                    }
                };
                return XDataParseResult {
                    groups,
                    stop,
                    bytes_consumed: byte_pos,
                };
            }
        };
        groups.push(XDataGroup { code, value });
    }
}

fn read_xdata_value(
    reader: &mut BitReader<'_>,
    code: i16,
    r2007_plus: bool,
) -> Result<XDataValue> {
    // Ranges follow LibreDWG `dwg_resbuf_value_type` / `dwg_decode_xdata`.
    let abs = code.unsigned_abs();
    match abs {
        // Strings
        0..=9
        | 100..=104
        | 300..=309
        | 410..=419
        | 430..=439
        | 470..=479
        | 999 => {
            if r2007_plus {
                // LibreDWG R2007+: RS length, then length × UCS-2 code units.
                let length = reader.read_rs(crate::bit::Endian::Little)? as usize;
                if length > 32_767 {
                    return Err(DwgError::new(
                        ErrorKind::Format,
                        format!("XDATA string length {length} exceeds UCS-2 max"),
                    ));
                }
                let mut chars = Vec::with_capacity(length);
                for _ in 0..length {
                    let cu = reader.read_rs(crate::bit::Endian::Little)?;
                    if cu == 0 {
                        // tolerate embedded NUL; stop early
                        break;
                    }
                    if let Some(c) = char::from_u32(u32::from(cu)) {
                        chars.push(c);
                    }
                }
                Ok(XDataValue::String(chars.into_iter().collect()))
            } else {
                // pre-R2007: RS length + RC codepage + TF bytes
                let length = reader.read_rs(crate::bit::Endian::Little)? as usize;
                let _codepage = reader.read_rc()?;
                let bytes = reader.read_rcs(length)?;
                let s = String::from_utf8_lossy(&bytes)
                    .trim_end_matches('\0')
                    .to_string();
                Ok(XDataValue::String(s))
            }
        }
        // 3D points / reals
        10..=59 => {
            let v = reader.read_rd(crate::bit::Endian::Little)?;
            if abs <= 39 {
                Ok(XDataValue::Real(v))
            } else {
                Ok(XDataValue::Real2(v))
            }
        }
        // Int16
        60..=79 | 170..=179 | 270..=289 | 370..=379 | 400..=409 | 1060..=1070 => {
            let v = reader.read_rs(crate::bit::Endian::Little)? as i16;
            Ok(XDataValue::Int16(v))
        }
        // Int32
        90..=99 | 420..=429 | 440..=449 | 1071 => {
            let v = reader.read_rl(crate::bit::Endian::Little)? as i32;
            Ok(XDataValue::Int32(v))
        }
        // Int64
        160..=169 => {
            let lo = reader.read_rl(crate::bit::Endian::Little)? as u64;
            let hi = reader.read_rl(crate::bit::Endian::Little)? as u64;
            let v = ((hi << 32) | lo) as i64;
            Ok(XDataValue::Int64(v))
        }
        // Bool / Int8
        290..=299 => {
            let v = reader.read_rc()? != 0;
            Ok(XDataValue::Bool(v))
        }
        // Binary
        310..=319 => {
            let len = reader.read_rc()? as usize;
            let bytes = reader.read_rcs(len)?;
            Ok(XDataValue::Binary(bytes))
        }
        // Handles / object ids — LibreDWG stores absolute RLL (8 bytes)
        320..=369 | 390..=399 => {
            // LibreDWG: absolute 8-byte handle (RLL LE), not a relative H ref.
            let lo = reader.read_rl(crate::bit::Endian::Little)? as u64;
            let hi = reader.read_rl(crate::bit::Endian::Little)? as u64;
            let absref = (hi << 32) | lo;
            Ok(XDataValue::Handle(crate::bit::HandleRef {
                code: 0,
                counter: 8,
                value: absref,
            }))
        }
        _ => Err(DwgError::new(
            ErrorKind::Format,
            format!("unknown XDATA group code {code}"),
        )),
    }
}

// ---------------------------------------------------------------------------
// Unit tests (LibreDWG dwg_decode_xdata / XRECORD reference)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bit::BitWriter;

    /// LibreDWG R2007+: RS group + RS length + length×UCS-2LE code units.
    #[test]
    fn parse_r2007_ucs2_string_group() {
        let mut w = BitWriter::new();
        w.write_rs(crate::bit::Endian::Little, 301).unwrap(); // description
        w.write_rs(crate::bit::Endian::Little, 4).unwrap(); // 4 code units
        for cu in [b'T' as u16, b'E' as u16, b'S' as u16, b'T' as u16] {
            w.write_rs(crate::bit::Endian::Little, cu).unwrap();
        }
        let bytes = w.into_bytes();
        let groups = parse_xdata_groups_versioned(&bytes, true).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].code, 301);
        assert_eq!(groups[0].value, XDataValue::String("TEST".into()));
    }

    /// German ß as single UCS-2 code unit U+00DF (LibreDWG TU).
    #[test]
    fn parse_r2007_ucs2_german_sharp_s() {
        let mut w = BitWriter::new();
        w.write_rs(crate::bit::Endian::Little, 8).unwrap(); // layer name
        // "Maß" = M (0x004D) a (0x0061) ß (0x00DF)
        w.write_rs(crate::bit::Endian::Little, 3).unwrap();
        for cu in [0x004Du16, 0x0061u16, 0x00DFu16] {
            w.write_rs(crate::bit::Endian::Little, cu).unwrap();
        }
        let bytes = w.into_bytes();
        let groups = parse_xdata_groups_versioned(&bytes, true).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].value, XDataValue::String("Maß".into()));
    }

    /// LibreDWG pre-R2007: RS length + RC codepage + TF bytes.
    #[test]
    fn parse_pre_r2007_string_with_codepage() {
        let mut w = BitWriter::new();
        w.write_rs(crate::bit::Endian::Little, 1).unwrap();
        w.write_rs(crate::bit::Endian::Little, 4).unwrap(); // length
        w.write_rc(30).unwrap(); // codepage ANSI_1252
        for b in b"TEST" {
            w.write_rc(*b).unwrap();
        }
        let bytes = w.into_bytes();
        let groups = parse_xdata_groups_versioned(&bytes, false).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].value, XDataValue::String("TEST".into()));
    }

    #[test]
    fn parse_int32_group_90() {
        let mut w = BitWriter::new();
        w.write_rs(crate::bit::Endian::Little, 90).unwrap();
        w.write_rl(crate::bit::Endian::Little, 0x0000_07FF).unwrap();
        let bytes = w.into_bytes();
        let groups = parse_xdata_groups_versioned(&bytes, true).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].code, 90);
        assert_eq!(groups[0].value, XDataValue::Int32(0x7FF));
    }

    #[test]
    fn parse_mask_91_and_description_301() {
        let mut w = BitWriter::new();
        // 91 mask
        w.write_rs(crate::bit::Endian::Little, 91).unwrap();
        w.write_rl(crate::bit::Endian::Little, 2047).unwrap();
        // 301 description "Layer"
        w.write_rs(crate::bit::Endian::Little, 301).unwrap();
        w.write_rs(crate::bit::Endian::Little, 5).unwrap();
        for cu in [b'L', b'a', b'y', b'e', b'r'].map(|b| b as u16) {
            w.write_rs(crate::bit::Endian::Little, cu).unwrap();
        }
        let bytes = w.into_bytes();
        let groups = parse_xdata_groups_versioned(&bytes, true).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].value, XDataValue::Int32(2047));
        assert_eq!(groups[1].value, XDataValue::String("Layer".into()));
    }

    /// Invalid group codes are hard errors (no silent stop).
    #[test]
    fn parse_errors_on_invalid_group_code() {
        let mut w = BitWriter::new();
        w.write_rs(crate::bit::Endian::Little, 90).unwrap();
        w.write_rl(crate::bit::Endian::Little, 1).unwrap();
        w.write_rs(crate::bit::Endian::Little, 2500).unwrap(); // invalid
        let bytes = w.into_bytes();
        let err = parse_xdata_groups_versioned(&bytes, true).unwrap_err();
        assert!(err.to_string().contains("invalid group code"));
        let detailed = parse_xdata_groups_detailed(&bytes, true);
        assert_eq!(detailed.groups.len(), 1);
        assert!(matches!(
            detailed.stop,
            XDataParseStop::InvalidCode { code: 2500, .. }
        ));
    }

    #[test]
    fn parse_empty_xdata() {
        assert!(parse_xdata_groups(&[]).unwrap().is_empty());
    }

    /// Declared-size path with empty xdata + cloning (no recovery heuristic).
    #[test]
    fn xrecord_data_empty_payload_with_cloning() {
        let mut w = BitWriter::new();
        w.write_bb(0b10).unwrap(); // BL = 0
        w.write_bb(0b01).unwrap(); // BS low bits
        w.write_rc(1).unwrap(); // BS value 1
        let bytes = w.into_bytes();
        let mut reader = BitReader::new(&bytes);
        let ctx = XRecordDecodeCtx {
            version: DwgVersion::R2018,
            object_handle: 0x30,
        };
        let data = decode_xrecord_data(&mut reader, &ctx).unwrap();
        assert_eq!(data.xdata_size, 0);
        assert!(data.groups.is_empty());
        assert_eq!(data.cloning, Some(1));
    }

    #[test]
    fn parse_errors_on_unknown_group_code() {
        let mut w = BitWriter::new();
        w.write_rs(crate::bit::Endian::Little, 9999).unwrap(); // will fail as invalid (>=2000)
        let bytes = w.into_bytes();
        assert!(parse_xdata_groups_versioned(&bytes, true).is_err());
    }
}
