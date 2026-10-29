//! AC1032 header variables in ODA DWG specification section 9 order.
//! These are deterministic defaults for a new, model-space-only drawing.
use crate::bit::{BitWriter, Endian};
use crate::core::result::Result;
use crate::writer::ir::{LayoutSettings, WriterMetadata};
use crate::writer::r2000::{checksum::crc16, database::DatabaseHandles};

pub fn encode_header(
    h: &DatabaseHandles,
    handseed: u64,
    metadata: &WriterMetadata,
    layout: &LayoutSettings,
) -> Result<Vec<u8>> {
    let mut w = BitWriter::with_unicode_handles();
    w.write_rl(Endian::Little, 0)?; // data and string bit size
    w.write_3b(0)?; // REQUIREDVERSIONS
    for v in [412148564080., 1., 1., 1.] {
        w.write_bd(v)?;
    }
    for _ in 0..4 {
        w.write_tv("")?;
    }
    w.write_bl(24)?;
    w.write_bl(0)?;
    // DIMASO, DIMSHO, PLINEGEN, ORTHOMODE, REGENMODE, FILLMODE,
    // QTEXTMODE, PSLTSCALE, LIMCHECK, USRTIMER, SKPOLY, ANGDIR,
    // SPLFRAME, MIRRTEXT, WORLDVIEW, TILEMODE, PLIMCHECK, VISRETAIN,
    // DISPSILH, PELLIPSE.
    for v in [
        1, 1, 0, 0, 1, 1, 0, 1, 0, 0, 1, 0, 0, 0, 1, 1, 1, 0, 1, 0, 0,
    ] {
        w.write_b(v)?;
    }
    for (key, v) in [
        ("PROXYGRAPHICS", 1),
        ("TREEDEPTH", 3020),
        ("LUNITS", metadata.lunits),
        ("LUPREC", metadata.luprec),
        ("AUNITS", metadata.aunits),
        ("AUPREC", metadata.auprec),
        ("ATTMODE", 1),
        ("PDMODE", 0),
        ("USERI1", 0),
        ("USERI2", 0),
        ("USERI3", 0),
        ("USERI4", 0),
        ("USERI5", 0),
        ("SPLINESEGS", 8),
        ("SURFU", 6),
        ("SURFV", 6),
        ("SURFTYPE", 6),
        ("SURFTAB1", 6),
        ("SURFTAB2", 6),
        ("SPLINETYPE", 6),
        ("SHADEDGE", 3),
        ("SHADEDIF", 70),
        ("UNITMODE", 0),
        ("MAXACTVP", 64),
        ("ISOLINES", 4),
        ("CMLJUST", 0),
        ("TEXTQLTY", 50),
    ] {
        w.write_bs(v)?;
        if key == "PDMODE" {
            for _ in 0..3 {
                w.write_bl(0)?;
            }
        }
    }
    for (_, v) in [
        ("LTSCALE", metadata.ltscale),
        ("TEXTSIZE", 2.5),
        ("TRACEWID", 1.),
        ("SKETCHINC", 1.),
        ("FILLETRAD", 0.),
        ("THICKNESS", 0.),
        ("ANGBASE", 0.),
        ("PDSIZE", 0.),
        ("PLINEWID", 0.),
        ("USERR1", 0.),
        ("USERR2", 0.),
        ("USERR3", 0.),
        ("USERR4", 0.),
        ("USERR5", 0.),
        ("CHAMFERA", 0.),
        ("CHAMFERB", 0.),
        ("CHAMFERC", 0.),
        ("CHAMFERD", 0.),
        ("FACETRES", 0.5),
        ("CMLSCALE", 1.),
        ("CELTSCALE", 1.),
    ] {
        w.write_bd(v)?;
    }
    w.write_tv("")?; // MENUNAME remains in the Unicode string stream
    for v in [2451545, 0, 2451545, 0, 0, 0, 0, 0, 0, 0, 0] {
        w.write_bl(v)?;
    }
    w.write_cmc(256, None)?; // CECOLOR: BYLAYER
    w.write_inline_h(0, handseed)?;
    // CLAYER, TEXTSTYLE, CELTYPE, DIMSTYLE, CMLSTYLE
    for handle in [h.layer, h.style, h.bylayer, 0, h.dimstyle, 0] {
        w.write_h(5, handle)?;
    }
    w.write_bd(0.)?; // PSVPSCALE
    paper_space(&mut w, layout, h.paper_viewport != 0)?;
    space(&mut w, metadata)?;
    w.write_tv("")?;
    w.write_tv("")?; // DIMPOST, DIMAPOST
    dimension_variables(&mut w, h.style)?;
    for handle in &h.controls[..9] {
        w.write_h(3, *handle)?;
    }
    w.write_h(5, h.dictionaries[1])?;
    w.write_h(5, h.dictionaries[2])?;
    w.write_h(3, h.dictionaries[0])?;
    w.write_bs(1)?;
    w.write_bs(70)?;
    w.write_tv("")?;
    w.write_tv("")?; // hyperlink base, stylesheet
    for handle in &h.dictionaries[3..] {
        w.write_h(5, *handle)?;
    }
    for _ in 0..4 {
        w.write_h(5, 0)?;
    }
    w.write_bl(0x2A1D)?; // BYLAYER weight, extended names, color-dependent plotting
    w.write_bs(metadata.insunits)?;
    w.write_bs(0)?; // CEPSNTYPE BYLAYER
    w.write_tv("")?;
    w.write_tv("")?; // fingerprints
    for v in [127, 0, 1, 2, 2, 0] {
        w.write_rc(v)?;
    }
    w.write_bs(257)?;
    w.write_bs(257)?;
    w.write_rc(0)?;
    w.write_rc(0)?;
    w.write_tv("")?;
    for handle in [h.paper, h.model, h.bylayer, h.byblock, h.continuous] {
        w.write_h(5, handle)?;
    }
    w.write_b(0)?;
    w.write_bl(0)?;
    w.write_bl(0)?;
    for v in [0., 2., 6., 2., 50., 0.] {
        w.write_bd(v)?;
    }
    w.write_rc(0)?;
    w.write_rc(0)?;
    for v in [
        5.,
        80.,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::FRAC_PI_2,
        0.,
        0.,
    ] {
        w.write_bd(v)?;
    }
    w.write_bs(7)?;
    w.write_rc(1)?;
    for v in [0., 0., 0.] {
        w.write_bd(v)?;
    }
    w.write_bl(0)?;
    for v in [1, 1, 2, 0] {
        w.write_rc(v)?;
    }
    w.write_b(0)?;
    w.write_cmc(7, Some(0xff0000))?;
    for _ in 0..3 {
        w.write_h(5, 0)?;
    }
    w.write_rc(0)?;
    w.write_bd(0.)?;
    for _ in 0..4 {
        w.write_bs(u16::MAX)?;
    }
    w.write_bl(0)?;
    w.write_bl(0)?;
    w.write_b(0)?;
    w.finish_strings()?;
    let bits = w.len_bits() as u32;
    w.set_bit_pos(0);
    w.write_rl(Endian::Little, bits)?;
    w.set_bit_pos(bits);
    w.finish_handles()?;
    let data = w.into_bytes();
    let mut out = vec![
        0xCF, 0x7B, 0x1F, 0x23, 0xFD, 0xDE, 0x38, 0xA9, 0x5F, 0x7C, 0x68, 0xB8, 0x4E, 0x6D, 0x33,
        0x5F,
    ];
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&data);
    let crc = crc16(0xC0C1, &out[16..]);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&[
        0x30, 0x84, 0xE0, 0xDC, 0x02, 0x21, 0xC7, 0x56, 0xA0, 0x83, 0x97, 0x47, 0xB1, 0x92, 0xCC,
        0xA0,
    ]);
    Ok(out)
}

/// PINSBASE, PEXTMIN/PEXTMAX, PLIMMIN/PLIMMAX and the paper-space UCS: the
/// limits follow the paper of `layout`; the extents are the paper rectangle
/// once the sheet holds entities.
fn paper_space(w: &mut BitWriter, layout: &LayoutSettings, has_entities: bool) -> Result<()> {
    let ((x0, y0), (x1, y1)) = layout.limits();
    let (ext_min, ext_max) = if has_entities {
        ((x0, y0, 0.), (x1, y1, 0.))
    } else {
        ((0., 0., 0.), (0., 0., 0.))
    };
    for p in [(0., 0., 0.), ext_min, ext_max] {
        w.write_3bd(p.0, p.1, p.2)?;
    }
    for v in [x0, y0, x1, y1] {
        w.write_rd(Endian::Little, v)?;
    }
    ucs_tail(w)
}

fn space(w: &mut BitWriter, m: &WriterMetadata) -> Result<()> {
    for p in [
        m.insertion_base,
        m.ext_min.unwrap_or((0., 0., 0.)),
        m.ext_max.unwrap_or((0., 0., 0.)),
    ] {
        w.write_3bd(p.0, p.1, p.2)?;
    }
    for v in [0., 0., 420., 297.] {
        w.write_rd(Endian::Little, v)?;
    }
    ucs_tail(w)
}

/// ELEVATION, UCSORG/UCSXDIR/UCSYDIR, UCS name, ORTHOREF, ORTHOVIEW, UCS base
/// and the six orthographic UCS origins of one space.
fn ucs_tail(w: &mut BitWriter) -> Result<()> {
    w.write_bd(0.)?;
    for p in [(0., 0., 0.), (1., 0., 0.), (0., 1., 0.)] {
        w.write_3bd(p.0, p.1, p.2)?;
    }
    w.write_h(5, 0)?;
    w.write_h(5, 0)?;
    w.write_bs(0)?;
    w.write_h(5, 0)?;
    for _ in 0..6 {
        w.write_3bd(0., 0., 0.)?;
    }
    Ok(())
}

fn dimension_variables(w: &mut BitWriter, style: u64) -> Result<()> {
    // DIMSCALE through DIMTM
    for v in [1., 2.5, 0.625, 3.75, 1.25, 0., 0., 0., 0.] {
        w.write_bd(v)?;
    }
    w.write_bd(1.)?;
    w.write_bd(std::f64::consts::FRAC_PI_4)?;
    w.write_bs(0)?;
    w.write_cmc(0, None)?;
    for v in [0, 0, 0, 0, 0, 0] {
        w.write_b(v)?;
    }
    for v in [1, 8, 0] {
        w.write_bs(v)?;
    } // DIMTAD, DIMZIN, DIMAZIN
    w.write_bs(0)?; // DIMARCSYM
    for v in [2.5, 2.5, 0., 0.03937007874015748, 1., 0., 1., 0.625] {
        w.write_bd(v)?;
    }
    w.write_bd(0.)?;
    w.write_b(0)?;
    w.write_bs(2)?;
    for v in [1, 0, 0, 0] {
        w.write_b(v)?;
    }
    for _ in 0..3 {
        w.write_cmc(0, None)?;
    } // dimension colors BYBLOCK
      // DIMADEC..DIMJUST with DIMDSEP '.' (46), matching the Standard DIMSTYLE.
    for v in [0, 2, 2, 2, 2, 0, 0, 2, 46, 0, 0] {
        w.write_bs(v)?;
    }
    w.write_b(0)?;
    w.write_b(0)?;
    for v in [1, 0, 0, 0] {
        w.write_bs(v)?;
    }
    w.write_b(0)?;
    w.write_bs(3)?;
    w.write_b(0)?;
    w.write_b(0)?;
    w.write_bd(100.)?;
    w.write_tv("")?;
    w.write_bd(100.)?;
    w.write_tv("")?;
    for handle in [style, 0, 0, 0, 0, 0, 0, 0] {
        w.write_h(5, handle)?;
    }
    w.write_bs((-2i16) as u16)?;
    w.write_bs((-2i16) as u16)
}
