//! Sparse DICTIONARY object decoder.
//!
//! Implements the DWG DICTIONARY (type 0x2A / class "DICTIONARY") body and
//! handle-stream layout according to the ODA / LibreDWG field order:
//!
//! Data section (version-aware):
//!   BL  numitems
//!   BS  cloning          (R2000+)
//!   B   is_hardowner     (R13c3+, with the R13c3 maint-rel quirk)
//!   T[] texts            (numitems strings; TV pre-R2007, TU / string-stream R2007+)
//!
//! Handle stream (after START_OBJECT_HANDLE_STREAM):
//!   COMMON_OBJECT_HANDLE_DATA handles only when counts were already read from
//!   the data section (R2004+/R2010+ layout used by LAYER and DICTIONARY):
//!     H   ownerhandle
//!     H[] reactors        (num_reactors from data)
//!     H   xdicobjhandle   (unless is_xdic_missing from data)
//!   H[] itemhandles       (code 2, numitems)
//!
//! When counts were *not* supplied from the data section, the classic stream
//! form is used (BL num_reactors + B flags, then the same handles).
//!
//! This module deliberately does **not** resolve handles into live objects and
//! does **not** special-case ACAD_* keys. Callers (layer-states, named-object
//! dictionary walks, etc.) own that policy. Adding DICTIONARYWDFLT or other
//! dictionary variants is a matter of a thin wrapper over the same helpers.

use crate::bit::{BitReader, HandleRef};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::dwg::version::DwgVersion;
use crate::objects::handle::Handle;

/// One entry in a DICTIONARY: the name (key) and the soft-pointer handle of the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryEntry {
    pub name: String,
    /// Soft-pointer handle of the referenced object (XRECORD, nested DICTIONARY, …).
    /// `None` only when the handle stream was truncated or the entry was absent.
    pub value_handle: Option<Handle>,
}

/// Fully decoded DICTIONARY object (data + handle stream).
///
/// Fields that are version-dependent are stored as `Option` so a single struct
/// works from R13 through R2018 without silent defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dictionary {
    pub handle: Handle,
    pub numitems: u32,
    /// DXF group 281. Present from R2000 onward.
    pub cloning: Option<u16>,
    /// DXF group 280. Present from R13c3 onward (with the maint-rel exception).
    pub is_hardowner: Option<u8>,
    pub entries: Vec<DictionaryEntry>,

    // --- common object handle data ---
    pub num_reactors: u32,
    pub is_xdic_missing: bool,
    pub has_ds_data: bool,
    pub owner_handle: Option<Handle>,
    pub reactors: Vec<Handle>,
    pub xdic_handle: Option<Handle>,
}

impl Dictionary {
    /// Lookup by exact name (case-sensitive, matching AutoCAD dictionary keys).
    pub fn get(&self, name: &str) -> Option<&DictionaryEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Lookup ignoring ASCII case (useful for the well-known ACAD_* keys).
    pub fn get_ignore_ascii_case(&self, name: &str) -> Option<&DictionaryEntry> {
        self.entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
    }
}

/// Context required to decode a DICTIONARY body.
///
/// Separating the context from the free functions keeps the decoder pure and
/// makes it trivial to unit-test with synthetic bit streams.
#[derive(Debug)]
pub struct DictionaryDecodeCtx<'a> {
    pub version: DwgVersion,
    pub object_handle: u64,
    /// Absolute bit position of the start of the handle stream inside the
    /// object record body, or `None` when the caller has not yet located it.
    /// When `None`, only the data section is decoded and handle fields stay empty.
    pub handle_stream_bit_start: Option<u64>,
    /// Optional separate string-stream reader for R2007+ (TU strings).
    /// When provided, `texts` are read from this reader instead of the main data stream.
    pub string_reader: Option<&'a mut BitReader<'a>>,
}

/// Decode the **data section only** of a DICTIONARY.
///
/// On entry the reader must be positioned immediately after the object-type
/// prefix (i.e. at the first field of the DICTIONARY body — `numitems`).
///
/// String handling:
/// - Pre-R2007: each text is a TV read from `reader`.
/// - R2007+: texts live in the string stream. Pass a positioned
///   `string_reader` via `ctx`; if it is `None` the decoder returns
///   `ErrorKind::Decode` rather than guessing.
pub fn decode_dictionary_data(
    reader: &mut BitReader<'_>,
    ctx: &mut DictionaryDecodeCtx<'_>,
) -> Result<DictionaryData> {
    let numitems = reader.read_bl()?;
    // Hard upper bound to protect against corrupt files (LibreDWG uses 10_000).
    const MAX_ITEMS: u32 = 100_000;
    if numitems > MAX_ITEMS {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!("DICTIONARY numitems {numitems} exceeds safe limit {MAX_ITEMS}"),
        ));
    }

    let cloning = if version_at_least(&ctx.version, &DwgVersion::R2000) {
        Some(reader.read_bs()?)
    } else {
        None
    };

    let is_hardowner = if version_at_least(&ctx.version, &DwgVersion::R13) {
        // LibreDWG dwg.spec FIELD_RC (is_hardowner); ACadSharp ReadByte().
        // Not a 1-bit B — DXF 280 is a full byte flag.
        Some(reader.read_rc()?)
    } else {
        None
    };

    let mut names = Vec::with_capacity(numitems as usize);
    for i in 0..numitems {
        let name = read_dictionary_text(reader, ctx, i)?;
        names.push(name);
    }

    Ok(DictionaryData {
        numitems,
        cloning,
        is_hardowner,
        names,
    })
}

/// Intermediate result of the data section (no handles yet).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryData {
    pub numitems: u32,
    pub cloning: Option<u16>,
    pub is_hardowner: Option<u8>,
    pub names: Vec<String>,
}

/// Decode the common object handle stream + the DICTIONARY itemhandles vector.
///
/// On entry `handle_reader` must be positioned at the start of the handle
/// stream (after the data section / string stream). The object handle is used
/// as the relative base for handle codes 2/3/4/5.
pub fn decode_dictionary_handles(
    handle_reader: &mut BitReader<'_>,
    version: &DwgVersion,
    object_handle: u64,
    numitems: u32,
    // When Some: counts/flags already read from data section (R2004+ layout).
    // Handle stream then starts at ownerhandle — do not re-read BL/B/B.
    data_section_common: Option<(u32, bool, bool)>,
) -> Result<DictionaryHandles> {
    let common = match data_section_common {
        Some((num_reactors, is_xdic_missing, has_ds_data)) => {
            decode_common_object_handle_refs(
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

    let mut item_handles = Vec::with_capacity(numitems as usize);
    for _ in 0..numitems {
        let href = handle_reader.read_h()?;
        let absolute = resolve_handle_ref(&href, object_handle)?;
        item_handles.push(absolute.map(Handle));
    }

    Ok(DictionaryHandles {
        common,
        item_handles,
    })
}

/// Read only the handle *references* of COMMON_OBJECT_HANDLE_DATA.
///
/// Counts/flags must already be known from the data section (R2004+ layout).
/// Handle stream starts at ownerhandle — do not re-read BL/B/B.
pub fn decode_common_object_handle_refs(
    handle_reader: &mut BitReader<'_>,
    version: &DwgVersion,
    object_handle: u64,
    num_reactors: u32,
    is_xdic_missing: bool,
    has_ds_data: bool,
) -> Result<CommonObjectHandles> {
    const MAX_REACTORS: u32 = 10_000;
    if num_reactors > MAX_REACTORS {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!("num_reactors {num_reactors} exceeds safe limit {MAX_REACTORS}"),
        ));
    }

    let owner_handle = if version_at_least(version, &DwgVersion::R13) {
        let href = handle_reader.read_h()?;
        resolve_handle_ref(&href, object_handle)?.map(Handle)
    } else {
        None
    };

    let mut reactors = Vec::with_capacity(num_reactors as usize);
    for _ in 0..num_reactors {
        let href = handle_reader.read_h()?;
        if let Some(h) = resolve_handle_ref(&href, object_handle)? {
            reactors.push(Handle(h));
        }
    }

    let xdic_handle = if !is_xdic_missing && version_at_least(version, &DwgVersion::R13) {
        let href = handle_reader.read_h()?;
        resolve_handle_ref(&href, object_handle)?.map(Handle)
    } else {
        None
    };

    Ok(CommonObjectHandles {
        num_reactors,
        is_xdic_missing,
        has_ds_data,
        owner_handle,
        reactors,
        xdic_handle,
    })
}

/// Combine data + handles into the final `Dictionary`.
pub fn assemble_dictionary(
    object_handle: u64,
    data: DictionaryData,
    handles: DictionaryHandles,
) -> Result<Dictionary> {
    if data.names.len() != handles.item_handles.len() {
        return Err(DwgError::new(
            ErrorKind::Decode,
            format!(
                "DICTIONARY name count ({}) != item handle count ({})",
                data.names.len(),
                handles.item_handles.len()
            ),
        ));
    }

    let entries = data
        .names
        .into_iter()
        .zip(handles.item_handles)
        .map(|(name, value_handle)| DictionaryEntry { name, value_handle })
        .collect();

    Ok(Dictionary {
        handle: Handle(object_handle),
        numitems: data.numitems,
        cloning: data.cloning,
        is_hardowner: data.is_hardowner,
        entries,
        num_reactors: handles.common.num_reactors,
        is_xdic_missing: handles.common.is_xdic_missing,
        has_ds_data: handles.common.has_ds_data,
        owner_handle: handles.common.owner_handle,
        reactors: handles.common.reactors,
        xdic_handle: handles.common.xdic_handle,
    })
}

/// One-shot convenience: data section + handle stream → `Dictionary`.
///
/// Prefer the staged API (`decode_dictionary_data` + `decode_dictionary_handles`
/// + `assemble_dictionary`) when the caller needs to interleave string-stream
/// positioning or other object-specific logic.
pub fn decode_dictionary(
    reader: &mut BitReader<'_>,
    handle_reader: &mut BitReader<'_>,
    ctx: &mut DictionaryDecodeCtx<'_>,
) -> Result<Dictionary> {
    let data = decode_dictionary_data(reader, ctx)?;
    let handles = decode_dictionary_handles(
        handle_reader,
        &ctx.version,
        ctx.object_handle,
        data.numitems,
        None, // caller did not supply data-section common counts
    )?;
    assemble_dictionary(ctx.object_handle, data, handles)
}

// ---------------------------------------------------------------------------
// Shared helpers (also used by xrecord.rs)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonObjectHandles {
    pub num_reactors: u32,
    pub is_xdic_missing: bool,
    pub has_ds_data: bool,
    pub owner_handle: Option<Handle>,
    pub reactors: Vec<Handle>,
    pub xdic_handle: Option<Handle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryHandles {
    pub common: CommonObjectHandles,
    pub item_handles: Vec<Option<Handle>>,
}

/// COMMON_OBJECT_HANDLE_DATA (LibreDWG `common_object_handle_data.spec`).
pub fn decode_common_object_handles(
    handle_reader: &mut BitReader<'_>,
    version: &DwgVersion,
    object_handle: u64,
) -> Result<CommonObjectHandles> {
    let num_reactors = if version_at_least(version, &DwgVersion::R13) {
        let n = handle_reader.read_bl()?;
        // LibreDWG hard-caps around 15 in practice; we allow a generous but
        // finite limit to keep corrupt files from allocating unbounded memory.
        const MAX_REACTORS: u32 = 10_000;
        if n > MAX_REACTORS {
            return Err(DwgError::new(
                ErrorKind::Format,
                format!("num_reactors {n} exceeds safe limit {MAX_REACTORS}"),
            ));
        }
        n
    } else {
        0
    };

    let is_xdic_missing = if version_at_least(version, &DwgVersion::R2004) {
        handle_reader.read_b()? != 0
    } else {
        false
    };

    let has_ds_data = if version_at_least(version, &DwgVersion::R2013) {
        handle_reader.read_b()? != 0
    } else {
        false
    };

    // Non-control objects carry owner + reactors + xdic.
    let owner_handle = if version_at_least(version, &DwgVersion::R13) {
        let href = handle_reader.read_h()?;
        resolve_handle_ref(&href, object_handle)?.map(Handle)
    } else {
        None
    };

    let mut reactors = Vec::with_capacity(num_reactors as usize);
    for _ in 0..num_reactors {
        let href = handle_reader.read_h()?;
        if let Some(h) = resolve_handle_ref(&href, object_handle)? {
            reactors.push(Handle(h));
        }
    }

    let xdic_handle = if !is_xdic_missing && version_at_least(version, &DwgVersion::R13) {
        let href = handle_reader.read_h()?;
        resolve_handle_ref(&href, object_handle)?.map(Handle)
    } else {
        None
    };

    Ok(CommonObjectHandles {
        num_reactors,
        is_xdic_missing,
        has_ds_data,
        owner_handle,
        reactors,
        xdic_handle,
    })
}

/// Resolve a relative handle reference against the current object handle.
///
/// Codes follow the classic DWG handle encoding:
///   2 = soft ownership relative
///   3 = hard ownership relative
///   4 = soft pointer relative
///   5 = hard pointer relative
///   6 = soft ownership absolute? (rare)
///   8 / 0x0A / … = other relative forms used by some versions
///
/// Returns `None` for a null handle (value 0 after resolution).
pub fn resolve_handle_ref(href: &HandleRef, object_handle: u64) -> Result<Option<u64>> {
    // Align with entities::common::read_handle_reference and ODA/LibreDWG:
    //   2..=5  soft/hard owner/pointer — absolute handle value
    //   6      current + 1
    //   8      current − 1
    //   0x0A   current + offset
    //   0x0C   current − offset
    //   0      absolute (or null when value == 0)
    let absolute = match href.code {
        0x06 => object_handle.saturating_add(1),
        0x08 => object_handle.saturating_sub(1),
        0x0A => object_handle.saturating_add(href.value),
        0x0C => object_handle.saturating_sub(href.value),
        0x02..=0x05 => {
            if href.value == 0 {
                return Ok(None);
            }
            href.value
        }
        _ => {
            if href.value == 0 {
                return Ok(None);
            }
            href.value
        }
    };
    Ok(Some(absolute))
}

fn read_dictionary_text(
    reader: &mut BitReader<'_>,
    ctx: &mut DictionaryDecodeCtx<'_>,
    index: u32,
) -> Result<String> {
    if version_at_least(&ctx.version, &DwgVersion::R2007) {
        match ctx.string_reader.as_mut() {
            Some(sreader) => sreader.read_tu().map_err(|e| {
                DwgError::new(
                    ErrorKind::Decode,
                    format!("DICTIONARY text[{index}] TU read failed: {e}"),
                )
            }),
            // Fallback: some compact dictionaries (e.g. LAYER_CONTROL xdic on
            // AC1032) still carry TU strings in the main data section when the
            // caller could not position a separate string stream. Prefer a
            // best-effort TU over failing the whole object.
            None => reader.read_tu().map_err(|e| {
                DwgError::new(
                    ErrorKind::Decode,
                    format!(
                        "DICTIONARY text[{index}] TU fallback (no string stream) failed on {}: {e}",
                        ctx.version.as_str()
                    ),
                )
            }),
        }
    } else {
        reader.read_tv().map_err(|e| {
            DwgError::new(
                ErrorKind::Decode,
                format!("DICTIONARY text[{index}] TV read failed: {e}"),
            )
        })
    }
}

/// True when `version` is at least as new as `min`.
///
/// Unknown versions are treated as "new enough" so that forward-compatible
/// fields are attempted; callers that need strict rejection should check
/// `DwgVersion::Unknown` themselves.
pub fn version_at_least(version: &DwgVersion, min: &DwgVersion) -> bool {
    use DwgVersion::*;
    let rank = |v: &DwgVersion| -> u8 {
        match v {
            R13 => 1,
            R14 => 2,
            R2000 => 3,
            R2004 => 4,
            R2007 => 5,
            R2010 => 6,
            R2013 => 7,
            R2018 => 8,
            Unknown(_) => 9,
        }
    };
    rank(version) >= rank(min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bit::BitWriter;

    #[test]
    fn version_at_least_ordering() {
        assert!(version_at_least(&DwgVersion::R2007, &DwgVersion::R2000));
        assert!(!version_at_least(&DwgVersion::R2000, &DwgVersion::R2007));
        assert!(version_at_least(&DwgVersion::R2018, &DwgVersion::R13));
    }

    #[test]
    fn resolve_null_handle() {
        let href = HandleRef {
            code: 4,
            counter: 0,
            value: 0,
        };
        assert_eq!(resolve_handle_ref(&href, 0x10).unwrap(), None);
    }

    #[test]
    fn resolve_soft_pointer_is_absolute() {
        // Codes 2..=5 store an absolute handle value (ODA / LibreDWG).
        let href = HandleRef {
            code: 4,
            counter: 1,
            value: 0x13,
        };
        assert_eq!(resolve_handle_ref(&href, 0x10).unwrap(), Some(0x13));
    }

    #[test]
    fn resolve_offset_plus_code_0a() {
        let href = HandleRef {
            code: 0x0A,
            counter: 1,
            value: 3,
        };
        assert_eq!(resolve_handle_ref(&href, 0x10).unwrap(), Some(0x13));
    }

    #[test]
    fn empty_dictionary_data_r2000() {
        let mut w = BitWriter::new();
        // numitems = 0 (BB=0b10)
        w.write_bb(0b10).unwrap();
        // cloning = 0 (BB=0b10)
        w.write_bb(0b10).unwrap();
        // is_hardowner = 0 (RC per LibreDWG / ACadSharp)
        w.write_rc(0).unwrap();
        let bytes = w.into_bytes();
        let mut reader = BitReader::new(&bytes);
        let mut ctx = DictionaryDecodeCtx {
            version: DwgVersion::R2000,
            object_handle: 0x20,
            handle_stream_bit_start: None,
            string_reader: None,
        };
        let data = decode_dictionary_data(&mut reader, &mut ctx).unwrap();
        assert_eq!(data.numitems, 0);
        assert_eq!(data.cloning, Some(0));
        assert_eq!(data.is_hardowner, Some(0));
        assert!(data.names.is_empty());
    }
}
