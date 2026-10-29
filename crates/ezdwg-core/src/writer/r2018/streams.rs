use crate::bit::{BitReader, BitWriter};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::writer::r2000::{checksum::crc16, encode_ms_value};

/// Intermediate payload: UMC handle bits followed by data/strings/handles.
pub(crate) fn finish(mut data: BitWriter, handles: &BitWriter) -> Result<Vec<u8>> {
    data.finish_strings()?;
    let data_bits = data.len_bits();
    data.write_bits_from_bytes(&handles.to_bytes(), handles.len_bits())?;
    let padding = (8 - data.len_bits() % 8) % 8;
    for _ in 0..padding {
        data.write_b(1)?;
    }
    let mut out = BitWriter::new();
    // The size includes the final padding, allowing data end to be derived exactly.
    let handle_bits = u32::try_from(data.len_bits() - data_bits).map_err(|_| {
        DwgError::new(
            ErrorKind::Unsupported,
            "AC1032 handle stream exceeds 4 Gbit",
        )
    })?;
    out.write_umc(handle_bits)?;
    out.write_rcs(&data.into_bytes())?;
    Ok(out.into_bytes())
}

pub(crate) fn record(payload: &[u8]) -> Result<Vec<u8>> {
    let mut r = BitReader::new(payload);
    r.read_umc()?;
    let prefix = (r.tell_bits() / 8) as usize;
    let size = u32::try_from(payload.len() - prefix)
        .map_err(|_| DwgError::new(ErrorKind::Unsupported, "AC1032 object exceeds 4 GiB"))?;
    let mut out = encode_ms_value(size)?;
    out.extend_from_slice(payload);
    let crc = crc16(0xc0c1, &out);
    out.extend_from_slice(&crc.to_le_bytes());
    Ok(out)
}

pub(crate) fn object<F>(
    typ: u16,
    handle: u64,
    owner: u64,
    refs: &[(u8, u64)],
    body: F,
) -> Result<Vec<u8>>
where
    F: FnOnce(&mut BitWriter) -> Result<()>,
{
    let mut w = BitWriter::with_unicode();
    w.write_ot_r2010(typ)?;
    w.write_h(0, handle)?;
    w.write_bs(0)?;
    w.write_bl(0)?;
    w.write_b(1)?; // xdictionary missing
    w.write_b(0)?; // no DS binary data
    body(&mut w)?;
    let mut handles = BitWriter::new();
    handles.write_h(4, owner)?;
    for &(code, h) in refs {
        handles.write_h(code, h)?;
    }
    finish(w, &handles)
}
