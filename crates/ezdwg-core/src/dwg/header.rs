//! AcDb:Header (HEADER VARIABLES) sequential decoder.
//!
//! The header section is one bit-coded stream with no random access, so the
//! variables are decoded in specification order and decoding stops right after
//! `INSUNITS` (the last variable this module needs). Layout differences per
//! version are handled as follows:
//!
//! - R2007+ (AC1021+): `TV` strings live in the separate string stream and
//!   `H` handles live in the separate handle stream, so neither consumes bits
//!   from the data stream here. The only exception is `HANDSEED`, which the
//!   specification keeps in the normal data stream.
//! - The prologue between the section size and the first data item varies with
//!   version and maintenance release (optional unknown `RL`, optional bit-size
//!   `RL`). Instead of trusting the maintenance byte, the decoder anchors on
//!   the first four data items (`BD` 412148564080.0 followed by three `BD`
//!   1.0), which form an unambiguous signature.
//! - `REQUIREDVERSIONS` (R2013+) uses a fixed 3-bit length prefix in real
//!   files, unlike the variable 1-3 bit form the specification suggests.
//! - R14 files have no `INSUNITS` header variable; decoding returns an empty
//!   result.

use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::dwg::version::DwgVersion;

/// 16-byte sentinel that introduces the header variables data.
const SENTINEL: [u8; 16] = [
    0xCF, 0x7B, 0x1F, 0x23, 0xFD, 0xDE, 0x38, 0xA9, 0x5F, 0x7C, 0x68, 0xB8, 0x4E, 0x6D, 0x33, 0x5F,
];

/// First unknown BD in the data stream; used as the start-of-data signature.
const FIRST_BD_SIGNATURE: f64 = 412_148_564_080.0;

/// Decoded subset of the DWG header variables.
///
/// Every field is `None` when the variable is absent for the file version
/// (R14) or decoding did not reach it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeaderVariables {
    /// $INSUNITS drawing units (0 = unitless, 1 = inches, 4 = millimeters, ...).
    pub insunits: Option<u16>,
    /// $LUNITS linear unit format.
    pub lunits: Option<u16>,
    /// $LUPREC linear unit precision.
    pub luprec: Option<u16>,
    /// $AUNITS angular unit format.
    pub aunits: Option<u16>,
    /// $AUPREC angular unit precision.
    pub auprec: Option<u16>,
    /// $LTSCALE global linetype scale.
    pub ltscale: Option<f64>,
    /// $TEXTSIZE default text height.
    pub textsize: Option<f64>,
    /// $EXTMIN model-space extents minimum.
    pub extmin: Option<(f64, f64, f64)>,
    /// $EXTMAX model-space extents maximum.
    pub extmax: Option<(f64, f64, f64)>,
    /// $LIMMIN model-space limits minimum.
    pub limmin: Option<(f64, f64)>,
    /// $LIMMAX model-space limits maximum.
    pub limmax: Option<(f64, f64)>,
}

fn rank(version: &DwgVersion) -> Option<u8> {
    match version {
        DwgVersion::R13 | DwgVersion::R14 => Some(0),
        DwgVersion::R2000 => Some(1),
        DwgVersion::R2004 => Some(2),
        DwgVersion::R2007 => Some(3),
        DwgVersion::R2010 => Some(4),
        DwgVersion::R2013 => Some(5),
        DwgVersion::R2018 => Some(6),
        DwgVersion::Unknown(_) => None,
    }
}

struct HeaderCursor<'a> {
    reader: BitReader<'a>,
    r2004_plus: bool,
    r2007_plus: bool,
}

impl<'a> HeaderCursor<'a> {
    fn skip_b(&mut self, count: usize) -> Result<()> {
        for _ in 0..count {
            self.reader.read_b()?;
        }
        Ok(())
    }

    fn skip_bs(&mut self, count: usize) -> Result<()> {
        for _ in 0..count {
            self.reader.read_bs()?;
        }
        Ok(())
    }

    fn skip_bl(&mut self, count: usize) -> Result<()> {
        for _ in 0..count {
            self.reader.read_bl()?;
        }
        Ok(())
    }

    fn skip_bd(&mut self, count: usize) -> Result<()> {
        for _ in 0..count {
            self.reader.read_bd()?;
        }
        Ok(())
    }

    fn skip_3bd(&mut self, count: usize) -> Result<()> {
        for _ in 0..count {
            self.reader.read_3bd()?;
        }
        Ok(())
    }

    /// `TV`: inline text through R2004; string-stream (no data bits) for R2007+.
    fn skip_tv(&mut self) -> Result<()> {
        if !self.r2007_plus {
            self.reader.read_tv()?;
        }
        Ok(())
    }

    /// `H`: inline handle through R2004; handle-stream (no data bits) for R2007+.
    fn skip_h(&mut self) -> Result<()> {
        if !self.r2007_plus {
            self.reader.read_h()?;
        }
        Ok(())
    }

    /// `H` that stays in the data stream for every version (HANDSEED).
    fn skip_h_inline(&mut self) -> Result<()> {
        self.reader.read_h()?;
        Ok(())
    }

    /// `CMC`: bit-short index up to R2000; index + RGB + flag byte from R2004.
    /// Color/book names are inline `TV` for R2004 and move to the string
    /// stream for R2007+.
    fn skip_cmc(&mut self) -> Result<()> {
        self.reader.read_bs()?;
        if self.r2004_plus {
            self.reader.read_bl()?;
            let flags = self.reader.read_rc()?;
            if !self.r2007_plus {
                if flags & 0x01 != 0 {
                    self.reader.read_tv()?;
                }
                if flags & 0x02 != 0 {
                    self.reader.read_tv()?;
                }
            }
        }
        Ok(())
    }
}

/// Decode the header-variables subset from raw `AcDb:Header` section bytes.
///
/// `section` must contain the beginning sentinel; bytes before it (as returned
/// by some section loaders) are tolerated.
pub fn decode_header_variables(
    section: &[u8],
    version: &DwgVersion,
    codepage: Option<u16>,
) -> Result<HeaderVariables> {
    let Some(version_rank) = rank(version) else {
        return Err(DwgError::new(
            ErrorKind::Unsupported,
            format!("unsupported DWG version: {}", version.as_str()),
        ));
    };
    if version_rank == 0 {
        // R14: INSUNITS does not exist in the header variables.
        return Ok(HeaderVariables::default());
    }
    let r2004_plus = version_rank >= 2;
    let r2007_plus = version_rank >= 3;
    let r2010_plus = version_rank >= 4;
    let r2013_plus = version_rank >= 5;
    let is_r2000 = version_rank == 1;

    let sentinel_pos = section
        .windows(SENTINEL.len())
        .position(|window| window == SENTINEL)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "header variables sentinel not found"))?;

    let mut size_reader = BitReader::new(section);
    let data_start = sentinel_pos + SENTINEL.len();
    size_reader.set_pos(data_start, 0);
    size_reader.read_rl(Endian::Little)?;
    let after_size_bits = size_reader.tell_bits();

    // Candidate bit offsets between the size RL and the first data item:
    //   +0  : no extra prologue (R2000 / R2004)
    //   +32 : bit-size RL (R2007+), or unknown RL alone
    //   +64 : unknown RL (maintenance > 3 / R2018+) followed by bit-size RL
    let candidates: &[u64] = if r2010_plus {
        &[32, 64, 0]
    } else if r2007_plus {
        &[32]
    } else {
        &[0]
    };

    for offset in candidates {
        let start_bits = after_size_bits + offset;
        let mut reader = BitReader::new_with_codepage(section, codepage);
        reader.set_pos((start_bits / 8) as usize, (start_bits % 8) as u8);
        if r2013_plus {
            // REQUIREDVERSIONS: fixed 3-bit length prefix, then that many
            // bytes (observed in real files; differs from the 1-3 bit form in
            // the specification text).
            let length = reader.read_bits_msb(3)?;
            for _ in 0..length {
                reader.read_rc()?;
            }
        }
        let Ok(unknown1) = reader.read_bd() else {
            continue;
        };
        if (unknown1 - FIRST_BD_SIGNATURE).abs() >= 1.0 {
            continue;
        }
        let signature_ok = (0..3).try_fold(true, |ok, _| {
            reader.read_bd().map(|value| ok && value == 1.0)
        });
        if !matches!(signature_ok, Ok(true)) {
            continue;
        }

        let mut cursor = HeaderCursor {
            reader,
            r2004_plus,
            r2007_plus,
        };
        return decode_after_signature(&mut cursor, is_r2000, r2007_plus, r2010_plus, r2013_plus);
    }

    Err(DwgError::new(
        ErrorKind::Format,
        "header variables data start not recognized",
    ))
}

fn decode_after_signature(
    cursor: &mut HeaderCursor<'_>,
    is_r2000: bool,
    r2007_plus: bool,
    r2010_plus: bool,
    r2013_plus: bool,
) -> Result<HeaderVariables> {
    let r2004_plus = cursor.r2004_plus;
    let mut vars = HeaderVariables::default();

    for _ in 0..4 {
        cursor.skip_tv()?; // unknown text x4
    }
    cursor.skip_bl(2)?; // unknown long x2
    if is_r2000 {
        cursor.skip_h_inline()?; // pre-2004: current viewport entity header
    }
    cursor.skip_b(2)?; // DIMASO DIMSHO
    cursor.skip_b(7)?; // PLINEGEN..LIMCHECK
    if r2004_plus {
        cursor.skip_b(1)?; // undocumented
    }
    cursor.skip_b(4)?; // USRTIMER SKPOLY ANGDIR SPLFRAME
    cursor.skip_b(2)?; // MIRRTEXT WORLDVIEW
    cursor.skip_b(3)?; // TILEMODE PLIMCHECK VISRETAIN
    cursor.skip_b(2)?; // DISPSILH PELLIPSE
    cursor.skip_bs(1)?; // PROXYGRAPHICS
    cursor.skip_bs(1)?; // TREEDEPTH
    vars.lunits = Some(cursor.reader.read_bs()?);
    vars.luprec = Some(cursor.reader.read_bs()?);
    vars.aunits = Some(cursor.reader.read_bs()?);
    vars.auprec = Some(cursor.reader.read_bs()?);
    cursor.skip_bs(2)?; // ATTMODE PDMODE
    if r2004_plus {
        cursor.skip_bl(3)?; // unknown x3
    }
    cursor.skip_bs(5)?; // USERI1-5
    cursor.skip_bs(14)?; // SPLINESEGS..TEXTQLTY
    vars.ltscale = Some(cursor.reader.read_bd()?);
    vars.textsize = Some(cursor.reader.read_bd()?);
    cursor.skip_bd(7)?; // TRACEWID..PLINEWID
    cursor.skip_bd(5)?; // USERR1-5
    cursor.skip_bd(7)?; // CHAMFERA-D FACETRES CMLSCALE CELTSCALE
    if !r2007_plus {
        cursor.reader.read_tv()?; // MENUNAME (R13-R18)
    }
    cursor.skip_bl(4)?; // TDCREATE TDUPDATE (day/ms each)
    if r2004_plus {
        cursor.skip_bl(3)?; // unknown x3
    }
    cursor.skip_bl(4)?; // TDINDWG TDUSRTIMER (day/ms each)
    cursor.skip_cmc()?; // CECOLOR
    cursor.skip_h_inline()?; // HANDSEED (data stream for all versions)
    cursor.skip_h()?; // CLAYER
    cursor.skip_h()?; // TEXTSTYLE
    cursor.skip_h()?; // CELTYPE
    if r2007_plus {
        cursor.skip_h()?; // CMATERIAL
    }
    cursor.skip_h()?; // DIMSTYLE
    cursor.skip_h()?; // CMLSTYLE
    cursor.skip_bd(1)?; // PSVPSCALE (R2000+)

    // Paper-space block.
    cursor.skip_3bd(3)?; // INSBASE EXTMIN EXTMAX
    cursor.reader.read_rd(Endian::Little)?;
    cursor.reader.read_rd(Endian::Little)?; // LIMMIN
    cursor.reader.read_rd(Endian::Little)?;
    cursor.reader.read_rd(Endian::Little)?; // LIMMAX
    cursor.skip_bd(1)?; // ELEVATION
    cursor.skip_3bd(3)?; // UCSORG UCSXDIR UCSYDIR
    cursor.skip_h()?; // UCSNAME
    cursor.skip_h()?; // PUCSORTHOREF
    cursor.skip_bs(1)?; // PUCSORTHOVIEW
    cursor.skip_h()?; // PUCSBASE
    cursor.skip_3bd(6)?; // PUCSORGTOP..PUCSORGBACK

    // Model-space block.
    cursor.skip_3bd(1)?; // INSBASE
    vars.extmin = Some(cursor.reader.read_3bd()?);
    vars.extmax = Some(cursor.reader.read_3bd()?);
    let limmin = (
        cursor.reader.read_rd(Endian::Little)?,
        cursor.reader.read_rd(Endian::Little)?,
    );
    let limmax = (
        cursor.reader.read_rd(Endian::Little)?,
        cursor.reader.read_rd(Endian::Little)?,
    );
    vars.limmin = Some(limmin);
    vars.limmax = Some(limmax);
    cursor.skip_bd(1)?; // ELEVATION
    cursor.skip_3bd(3)?; // UCSORG UCSXDIR UCSYDIR
    cursor.skip_h()?; // UCSNAME
    cursor.skip_h()?; // UCSORTHOREF
    cursor.skip_bs(1)?; // UCSORTHOVIEW
    cursor.skip_h()?; // UCSBASE
    cursor.skip_3bd(6)?; // UCSORGTOP..UCSORGBACK

    cursor.skip_tv()?; // DIMPOST
    cursor.skip_tv()?; // DIMAPOST
    cursor.skip_bd(9)?; // DIMSCALE..DIMTM
    if r2007_plus {
        cursor.skip_bd(2)?; // DIMFXL DIMJOGANG
        cursor.skip_bs(1)?; // DIMTFILL
        cursor.skip_cmc()?; // DIMTFILLCLR
    }
    cursor.skip_b(6)?; // DIMTOL..DIMSE2
    cursor.skip_bs(3)?; // DIMTAD DIMZIN DIMAZIN
    if r2007_plus {
        cursor.skip_bs(1)?; // DIMARCSYM
    }
    cursor.skip_bd(8)?; // DIMTXT..DIMGAP
    cursor.skip_bd(1)?; // DIMALTRND
    cursor.skip_b(1)?; // DIMALT
    cursor.skip_bs(1)?; // DIMALTD
    cursor.skip_b(4)?; // DIMTOFL DIMSAH DIMTIX DIMSOXD
    cursor.skip_cmc()?; // DIMCLRD
    cursor.skip_cmc()?; // DIMCLRE
    cursor.skip_cmc()?; // DIMCLRT
    cursor.skip_bs(11)?; // DIMADEC..DIMJUST
    cursor.skip_b(2)?; // DIMSD1 DIMSD2
    cursor.skip_bs(4)?; // DIMTOLJ DIMTZIN DIMALTZ DIMALTTZ
    cursor.skip_b(1)?; // DIMUPT
    cursor.skip_bs(1)?; // DIMATFIT
    if r2007_plus {
        cursor.skip_b(1)?; // DIMFXLON
    }
    if r2010_plus {
        cursor.skip_b(1)?; // DIMTXTDIRECTION
        cursor.skip_bd(1)?; // DIMALTMZF
        cursor.skip_tv()?; // DIMALTMZS
        cursor.skip_bd(1)?; // DIMMZF
        cursor.skip_tv()?; // DIMMZS
    }
    for _ in 0..5 {
        cursor.skip_h()?; // DIMTXSTY DIMLDRBLK DIMBLK DIMBLK1 DIMBLK2
    }
    if r2007_plus {
        for _ in 0..3 {
            cursor.skip_h()?; // DIMLTYPE DIMLTEX1 DIMLTEX2
        }
    }
    cursor.skip_bs(2)?; // DIMLWD DIMLWE
    for _ in 0..9 {
        cursor.skip_h()?; // table control objects
    }
    if is_r2000 {
        cursor.skip_h()?; // VIEWPORT ENTITY HEADER CONTROL (R13-R15)
    }
    for _ in 0..3 {
        cursor.skip_h()?; // ACAD_GROUP ACAD_MLINESTYLE NAMED OBJECTS
    }
    cursor.skip_bs(2)?; // TSTACKALIGN TSTACKSIZE
    cursor.skip_tv()?; // HYPERLINKBASE
    cursor.skip_tv()?; // STYLESHEET
    for _ in 0..3 {
        cursor.skip_h()?; // LAYOUTS PLOTSETTINGS PLOTSTYLES
    }
    if r2004_plus {
        cursor.skip_h()?; // MATERIALS
        cursor.skip_h()?; // COLORS
    }
    if r2007_plus {
        cursor.skip_h()?; // VISUALSTYLE
    }
    if r2013_plus {
        cursor.skip_h()?; // unknown
    }
    cursor.skip_bl(1)?; // flags (CELWEIGHT/ENDCAPS/JOINSTYLE/...)
    vars.insunits = Some(cursor.reader.read_bs()?);
    Ok(vars)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::ParseConfig;
    use crate::dwg::decoder::Decoder;
    use crate::dwg::file_open;
    use std::path::PathBuf;

    fn sample(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative)
    }

    fn decode(relative: &str) -> HeaderVariables {
        let bytes = file_open::read_file(sample(relative)).expect("read sample");
        let decoder = Decoder::new(&bytes, ParseConfig::default()).expect("decoder");
        decoder.header_variables().expect("header variables")
    }

    #[test]
    fn r2000_metric_sample() {
        let vars = decode("examples/data/line_2000.dwg");
        assert_eq!(vars.insunits, Some(4));
        assert_eq!(vars.lunits, Some(2));
        assert_eq!(vars.aunits, Some(0));
        assert_eq!(vars.ltscale, Some(1.0));
    }

    #[test]
    fn r2004_sample() {
        let vars = decode("examples/data/insert_2004.dwg");
        assert_eq!(vars.insunits, Some(0));
        assert_eq!(vars.lunits, Some(2));
    }

    #[test]
    fn r2010_imperial_sample() {
        let vars = decode("examples/data/mechanical_example-imperial.dwg");
        assert_eq!(vars.insunits, Some(1));
        assert_eq!(vars.lunits, Some(4));
    }

    #[test]
    fn r2018_sample() {
        let vars = decode("test_dwg/acadsharp/BLOCKPOINTPARAMETER.dwg");
        assert_eq!(vars.insunits, Some(1));
        assert_eq!(vars.lunits, Some(2));
    }
}
