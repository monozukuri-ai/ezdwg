//! Minimal R2000 database graph. All handles are allocated after reserving input
//! handles; no drawing/template bytes or external converter are used by the writer.
use super::entities::common::{encode_entity_payload, CommonEntityEncodeInput};
use crate::bit::{BitWriter, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::writer::encoder::DocumentTables;
use crate::writer::ir::{ImageDef, LayerDef, LayoutSettings, TextStyleDef, WriterDocument};
use crate::writer::HandleAllocator;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug)]
pub struct DatabaseHandles {
    pub blocks: BTreeMap<String, (u64, u64, u64)>,
    pub styles: BTreeMap<String, u64>,
    pub layers: BTreeMap<String, u64>,
    pub linetypes: BTreeMap<String, u64>,
    pub controls: [u64; 10],
    pub dictionaries: [u64; 6],
    pub model: u64,
    pub paper: u64,
    pub layer: u64,
    pub style: u64,
    pub bylayer: u64,
    pub byblock: u64,
    pub continuous: u64,
    pub model_begin: u64,
    pub model_end: u64,
    pub paper_begin: u64,
    pub paper_end: u64,
    pub normal_plotstyle: u64,
    pub model_layout: u64,
    pub paper_layout: u64,
    /// The overall paper viewport of `Layout1` (0 when paper space is empty).
    pub paper_viewport: u64,
    /// The `Standard` DIMSTYLE record referenced by dimensions and the header.
    pub dimstyle: u64,
    /// `ACAD_IMAGE_DICT`, `ACAD_IMAGE_VARS` and the IMAGEDEF objects, when images exist.
    pub image_dict: u64,
    pub image_vars: u64,
    pub image_defs: BTreeMap<String, u64>,
}

impl DatabaseHandles {
    pub fn allocate(a: &mut HandleAllocator, tables: &DocumentTables) -> Result<Self> {
        let mut controls = [0; 10];
        let mut dictionaries = [0; 6];
        for h in controls.iter_mut().chain(dictionaries.iter_mut()) {
            *h = a.allocate()?;
        }
        let mut result = Self {
            blocks: BTreeMap::new(),
            styles: BTreeMap::new(),
            layers: BTreeMap::new(),
            linetypes: BTreeMap::new(),
            controls,
            dictionaries,
            model: a.allocate()?,
            paper: a.allocate()?,
            layer: a.allocate()?,
            style: a.allocate()?,
            bylayer: a.allocate()?,
            byblock: a.allocate()?,
            continuous: a.allocate()?,
            model_begin: a.allocate()?,
            model_end: a.allocate()?,
            paper_begin: a.allocate()?,
            paper_end: a.allocate()?,
            normal_plotstyle: a.allocate()?,
            model_layout: a.allocate()?,
            paper_layout: a.allocate()?,
            paper_viewport: 0,
            dimstyle: a.allocate()?,
            image_dict: 0,
            image_vars: 0,
            image_defs: BTreeMap::new(),
        };
        // The overall paper viewport is allocated by the encoder when the first
        // paper-space entity arrives.
        if !tables.image_defs.is_empty() {
            result.image_dict = a.allocate()?;
            result.image_vars = a.allocate()?;
            for def in &tables.image_defs {
                result
                    .image_defs
                    .insert(def.name.to_uppercase(), a.allocate()?);
            }
        }
        result.layers.insert("0".into(), result.layer);
        for layer in &tables.layers {
            if layer.name != "0" {
                result
                    .layers
                    .insert(layer.name.to_uppercase(), a.allocate()?);
            }
        }
        for (name, h) in [
            ("BYLAYER", result.bylayer),
            ("BYBLOCK", result.byblock),
            ("CONTINUOUS", result.continuous),
        ] {
            result.linetypes.insert(name.into(), h);
        }
        for lt in &tables.linetypes {
            result
                .linetypes
                .insert(lt.name.to_uppercase(), a.allocate()?);
        }
        result.styles.insert("STANDARD".into(), result.style);
        for st in &tables.text_styles {
            if !st.name.eq_ignore_ascii_case("Standard") {
                result.styles.insert(st.name.to_uppercase(), a.allocate()?);
            }
        }
        for block in &tables.blocks {
            result.blocks.insert(
                block.name.to_uppercase(),
                (a.allocate()?, a.allocate()?, a.allocate()?),
            );
        }
        Ok(result)
    }

    pub fn style_handle(&self, name: &str) -> Result<u64> {
        self.styles
            .get(
                if name.is_empty() { "STANDARD" } else { name }
                    .to_uppercase()
                    .as_str(),
            )
            .copied()
            .ok_or_else(|| {
                DwgError::new(ErrorKind::Format, format!("undefined text style: {name}"))
            })
    }
    pub fn layer_handle(&self, name: &str) -> Result<u64> {
        self.layers
            .get(
                if name.is_empty() { "0" } else { name }
                    .to_uppercase()
                    .as_str(),
            )
            .copied()
            .ok_or_else(|| DwgError::new(ErrorKind::Format, format!("undefined layer: {name}")))
    }
    pub fn linetype_handle(&self, name: &str) -> Result<u64> {
        self.linetypes
            .get(
                if name.is_empty() { "BYLAYER" } else { name }
                    .to_uppercase()
                    .as_str(),
            )
            .copied()
            .ok_or_else(|| DwgError::new(ErrorKind::Format, format!("undefined linetype: {name}")))
    }

    pub fn image_def_handle(&self, name: &str) -> Result<u64> {
        self.image_defs
            .get(name.to_uppercase().as_str())
            .copied()
            .ok_or_else(|| {
                DwgError::new(
                    ErrorKind::Format,
                    format!("undefined image definition: {name}"),
                )
            })
    }

    /// The table, dictionary and block records of a document whose entities
    /// have been encoded already (`summary` carries what they contributed).
    pub fn objects(&self, summary: &DocumentSummary<'_>) -> Result<Vec<(u64, Vec<u8>)>> {
        let tables = summary.tables;
        let modern = summary.modern;
        let codepage = tables.metadata.codepage;
        let mut objects = Vec::new();
        // Header variable table order: BLOCK, LAYER, STYLE, LTYPE, VIEW, UCS,
        // VPORT, APPID, DIMSTYLE, VX (viewport entity header).
        let types = [0x30, 0x32, 0x34, 0x38, 0x3C, 0x3E, 0x40, 0x42, 0x44, 0x46];
        for (i, (&h, typ)) in self.controls.iter().zip(types).enumerate() {
            let entries: Vec<u64> = match i {
                0 => self.blocks.values().map(|v| v.0).collect(),
                1 => self.layers.values().copied().collect(),
                2 => self.styles.values().copied().collect(),
                3 => self
                    .linetypes
                    .iter()
                    .filter(|(n, _)| !matches!(n.as_str(), "BYLAYER" | "BYBLOCK"))
                    .map(|(_, h)| *h)
                    .collect(),
                8 => vec![self.dimstyle],
                _ => vec![],
            };
            let mut refs: Vec<(u8, u64)> = entries.iter().map(|h| (2, *h)).collect();
            if i == 0 {
                refs.extend([(3, self.model), (3, self.paper)]);
            }
            if i == 3 {
                // The LTYPE control stores BYBLOCK before BYLAYER (ODA roundtrip).
                refs.extend([(3, self.byblock), (3, self.bylayer)]);
            }
            objects.push((
                h,
                object(codepage, modern, typ, h, 0, &refs, |w| {
                    w.write_bl(entries.len() as u32)?;
                    if i == 8 {
                        w.write_rc(0)?;
                    } // no extra DIMSTYLE handles
                    Ok(())
                })?,
            ));
        }
        let [nod, groups, mlines, layouts, plots, styles] = self.dictionaries;
        let mut names = vec![
            "ACAD_GROUP",
            "ACAD_MLINESTYLE",
            "ACAD_LAYOUT",
            "ACAD_PLOTSETTINGS",
            "ACAD_PLOTSTYLENAME",
        ];
        // Image objects exist only when an IMAGE references a definition.
        let with_images = self.image_dict != 0 && !summary.used_image_defs.is_empty();
        if with_images {
            names.extend(["ACAD_IMAGE_DICT", "ACAD_IMAGE_VARS"]);
        }
        for (i, &h) in self.dictionaries.iter().enumerate() {
            let mut refs: Vec<_> = match i {
                0 => {
                    let mut v = [groups, mlines, layouts, plots, styles]
                        .map(|h| (2, h))
                        .to_vec();
                    if with_images {
                        v.extend([(2, self.image_dict), (2, self.image_vars)]);
                    }
                    v
                }
                3 => vec![(2, self.model_layout), (2, self.paper_layout)],
                5 => vec![(2, self.normal_plotstyle)],
                _ => vec![],
            };
            let count = refs.len();
            if i == 5 {
                refs.push((5, self.normal_plotstyle));
            }
            objects.push((
                h,
                object(
                    codepage,
                    modern,
                    if i == 5 { 500 } else { 0x2A },
                    h,
                    if i == 0 { 0 } else { nod },
                    &refs,
                    |w| {
                        w.write_bl(count as u32)?;
                        w.write_bs(1)?; // duplicate-record cloning: keep existing
                        w.write_rc(0)?; // soft ownership
                        if i == 0 {
                            for name in &names {
                                w.write_tv(name)?;
                            }
                        }
                        if i == 3 {
                            if w.is_unicode() {
                                for v in [0, 2, 300] {
                                    w.write_bs(v)?;
                                }
                            }
                            w.write_tv("Model")?;
                            w.write_tv("Layout1")?;
                        }
                        if i == 5 {
                            w.write_tv("Normal")?;
                        }
                        Ok(())
                    },
                )?,
            ));
        }
        objects.push((
            self.normal_plotstyle,
            object(
                codepage,
                modern,
                501,
                self.normal_plotstyle,
                styles,
                &[],
                |_| Ok(()),
            )?,
        ));
        if with_images {
            use super::classes::{IMAGEDEF_CLASS, RASTERVARIABLES_CLASS};
            use super::entities::image::{encode_image_def_object, encode_raster_variables_object};
            let used: Vec<&ImageDef> = tables
                .image_defs
                .iter()
                .filter(|d| summary.used_image_defs.contains(&d.name.to_uppercase()))
                .collect();
            let mut entries: Vec<(&str, u64)> = used
                .iter()
                .map(|d| (d.name.as_str(), self.image_defs[&d.name.to_uppercase()]))
                .collect();
            entries.sort_by_key(|(_, h)| *h);
            let refs: Vec<(u8, u64)> = entries.iter().map(|(_, h)| (2, *h)).collect();
            objects.push((
                self.image_dict,
                object(codepage, modern, 0x2A, self.image_dict, nod, &refs, |w| {
                    w.write_bl(entries.len() as u32)?;
                    w.write_bs(1)?;
                    w.write_rc(0)?;
                    for (name, _) in &entries {
                        w.write_tv(name)?;
                    }
                    Ok(())
                })?,
            ));
            objects.push((
                self.image_vars,
                encode_raster_variables_object(
                    codepage,
                    RASTERVARIABLES_CLASS,
                    self.image_vars,
                    nod,
                )?,
            ));
            for def in used {
                let handle = self.image_defs[&def.name.to_uppercase()];
                let mut reactors: Vec<u64> = summary
                    .image_reactors
                    .get(&def.name.to_uppercase())
                    .cloned()
                    .unwrap_or_default();
                reactors.sort_unstable();
                objects.push((
                    handle,
                    encode_image_def_object(
                        codepage,
                        IMAGEDEF_CLASS,
                        handle,
                        self.image_dict,
                        &reactors,
                        def,
                    )?,
                ));
            }
        }
        let default_layer = LayerDef::default();
        for layer in std::iter::once(&default_layer)
            .filter(|_| !tables.layers.iter().any(|l| l.name == "0"))
            .chain(tables.layers.iter())
        {
            let handle = self.layer_handle(&layer.name)?;
            let ltype = self.linetype_handle(&layer.linetype)?;
            objects.push((
                handle,
                object(
                    codepage,
                    modern,
                    0x33,
                    handle,
                    self.controls[1],
                    &if modern {
                        vec![
                            (5, 0),
                            (5, self.normal_plotstyle),
                            (5, 0),
                            (5, ltype),
                            (5, 0),
                        ]
                    } else {
                        vec![(5, 0), (5, self.normal_plotstyle), (5, ltype)]
                    },
                    |w| {
                        table_name(w, &layer.name)?;
                        let flags = u16::from(layer.frozen)
                            | (u16::from(layer.off) << 1)
                            | (u16::from(layer.frozen_in_new_viewports) << 2)
                            | (u16::from(layer.locked) << 3)
                            | (u16::from(layer.plot) << 4)
                            | (u16::from(crate::writer::validation::lineweight_index(
                                layer.lineweight,
                            )?) << 5);
                        w.write_bs(flags)?;
                        w.write_cmc(layer.color_index, layer.true_color)
                    },
                )?,
            ));
        }
        objects.push((
            self.dimstyle,
            object(
                codepage,
                modern,
                0x45,
                self.dimstyle,
                self.controls[8],
                &if modern {
                    vec![
                        (5, 0),
                        (5, self.style),
                        (5, 0),
                        (5, 0),
                        (5, 0),
                        (5, 0),
                        (5, 0),
                        (5, 0),
                        (5, 0),
                    ]
                } else {
                    vec![(5, 0), (5, self.style), (5, 0), (5, 0), (5, 0), (5, 0)]
                },
                dimstyle_body,
            )?,
        ));
        let default_style = TextStyleDef::default();
        for st in std::iter::once(&default_style)
            .filter(|_| {
                !tables
                    .text_styles
                    .iter()
                    .any(|s| s.name.eq_ignore_ascii_case("Standard"))
            })
            .chain(tables.text_styles.iter())
        {
            let h = self.style_handle(&st.name)?;
            objects.push((
                h,
                object(
                    codepage,
                    modern,
                    0x35,
                    h,
                    self.controls[2],
                    &[(5, 0)],
                    |w| {
                        table_name(w, &st.name)?;
                        w.write_b(0)?; // shape styles are not text fonts
                        w.write_b(u8::from(st.vertical))?;
                        w.write_bd(st.height)?;
                        w.write_bd(st.width)?;
                        w.write_bd(st.oblique)?;
                        w.write_rc(st.generation)?;
                        w.write_bd(st.last_height)?;
                        w.write_tv(&st.font)?;
                        w.write_tv(&st.bigfont)
                    },
                )?,
            ));
        }
        for (name, h) in &self.linetypes {
            let definition = tables
                .linetypes
                .iter()
                .find(|l| l.name.to_uppercase() == *name);
            let dashes = definition.map(|l| l.dashes.as_slice()).unwrap_or(&[]);
            let mut refs = vec![(5, 0)];
            refs.extend(dashes.iter().map(|_| (5, 0)));
            objects.push((
                *h,
                object(codepage, modern, 0x39, *h, self.controls[3], &refs, |w| {
                    table_name(w, definition.map(|l| l.name.as_str()).unwrap_or(name))?;
                    w.write_tv(definition.map(|l| l.description.as_str()).unwrap_or(""))?;
                    w.write_bd(dashes.iter().map(|v| v.abs()).sum())?;
                    w.write_rc(b'A')?;
                    w.write_rc(dashes.len() as u8)?;
                    for length in dashes {
                        w.write_bd(*length)?;
                        w.write_bs(0)?;
                        w.write_rd(Endian::Little, 0.)?;
                        w.write_rd(Endian::Little, 0.)?;
                        w.write_bd(1.)?;
                        w.write_bd(0.)?;
                        w.write_bs(0)?;
                    }
                    if modern {
                        Ok(())
                    } else {
                        w.write_rcs(&[0; 256])
                    }
                })?,
            ));
        }
        // The paper viewport written by the encoder leads the paper-space chain.
        let paper_viewport = summary.paper_viewport;
        let model = &summary.layouts[0];
        let paper = &summary.layouts[WriterDocument::PAPER_LAYOUT];
        let paper_contents: Vec<u64> = (paper_viewport != 0)
            .then_some(paper_viewport)
            .into_iter()
            .chain(paper.handles.iter().copied())
            .collect();
        let paper_first = if paper_viewport != 0 {
            paper_viewport
        } else {
            paper.first
        };
        let paper_last = if paper.last != 0 {
            paper.last
        } else {
            paper_viewport
        };
        let spaces = [
            (
                self.model,
                "*Model_Space",
                self.model_begin,
                self.model_end,
                (
                    model.first,
                    model.last,
                    model.count,
                    model.handles.as_slice(),
                ),
                self.model_layout,
                (0., 0., 0.),
                "",
                model.has_atts,
                &[][..],
            ),
            (
                self.paper,
                "*Paper_Space",
                self.paper_begin,
                self.paper_end,
                (
                    paper_first,
                    paper_last,
                    paper.count + usize::from(paper_viewport != 0),
                    paper_contents.as_slice(),
                ),
                self.paper_layout,
                (0., 0., 0.),
                "",
                paper.has_atts,
                &[][..],
            ),
        ];
        let definitions = tables.blocks.iter().enumerate().map(|(i, b)| {
            let (h, begin, end) = self.blocks[&b.name.to_uppercase()];
            let contents = &summary.layouts[i + WriterDocument::FIRST_BLOCK_LAYOUT];
            (
                h,
                b.name.as_str(),
                begin,
                end,
                (
                    contents.first,
                    contents.last,
                    contents.count,
                    contents.handles.as_slice(),
                ),
                0,
                b.base_point,
                b.description.as_str(),
                contents.has_atts,
                summary.block_refs[i].as_slice(),
            )
        });
        for (
            h,
            name,
            begin,
            end,
            (first, last, count, all),
            layout,
            base,
            description,
            has_atts,
            inserts,
        ) in spaces.into_iter().chain(definitions)
        {
            let mut refs = vec![(5, 0), (3, begin), (4, first), (4, last), (3, end)];
            if modern {
                refs = vec![(5, 0), (3, begin)];
                refs.extend(all.iter().map(|h| (3, *h)));
                refs.push((3, end));
            }
            refs.extend(inserts.iter().map(|h| (4, *h)));
            refs.push((5, layout));
            objects.push((
                h,
                object(codepage, modern, 0x31, h, self.controls[0], &refs, |w| {
                    table_name(w, name)?;
                    w.write_b(u8::from(name.starts_with('*') && layout == 0))?; // anonymous
                    w.write_b(u8::from(has_atts))?;
                    for _ in 0..3 {
                        w.write_b(0)?;
                    } // xref, overlay, unloaded
                    if modern {
                        w.write_bl(count as u32)?;
                    }
                    w.write_3bd(base.0, base.1, base.2)?;
                    w.write_tv("")?;
                    for _ in inserts {
                        w.write_rc(1)?;
                    }
                    w.write_rc(0)?;
                    w.write_tv(description)?;
                    w.write_bl(0)?;
                    if modern {
                        w.write_bs(0)?;
                        w.write_b(1)?;
                        w.write_rc(0)?;
                    }
                    Ok(())
                })?,
            ));
            if layout != 0 {
                // 330 block record, 331 last active viewport, 346 base UCS, 345 named UCS.
                let active_viewport = if h == self.paper { paper_viewport } else { 0 };
                objects.push((
                    layout,
                    object(
                        codepage,
                        modern,
                        if modern { 0x52 } else { 502 },
                        layout,
                        layouts,
                        &if modern {
                            vec![(4, 0), (4, 0), (4, h), (4, active_viewport), (5, 0), (5, 0)]
                        } else {
                            vec![(4, h), (4, active_viewport), (5, 0), (5, 0)]
                        },
                        |w| {
                            if h == self.model {
                                model_layout_body(w)
                            } else {
                                paper_layout_body(w, &tables.layout, paper_viewport != 0)
                            }
                        },
                    )?,
                ));
            }
            for (handle, typ) in [(begin, 4), (end, 5)] {
                let common = CommonEntityEncodeInput {
                    handle,
                    owner_handle: h,
                    entity_mode: 0,
                    layer_handle: self.layer,
                    color_index: 256,
                    linetype_flags: 0,
                    linetype_handle: 0,
                    codepage,
                    modern,
                    true_color: None,
                    formatting: Default::default(),
                    previous_handle: 0,
                    next_handle: 0,
                    style_handle: 0,
                };
                objects.push((
                    handle,
                    encode_entity_payload(typ, common, |w| {
                        if typ == 4 {
                            w.write_tv(name)?;
                        }
                        Ok(())
                    })?,
                ));
            }
        }
        Ok(objects)
    }
}

/// What a layout's entities contributed to the tables: the first and last
/// entity of its chain, the count and (for AC1032, whose BLOCK_HEADER lists
/// every entity) all handles, plus whether it defines attributes.
#[derive(Debug, Clone)]
pub struct LayoutContents {
    pub first: u64,
    pub last: u64,
    pub count: usize,
    pub handles: Vec<u64>,
    pub has_atts: bool,
    keep_handles: bool,
}

impl LayoutContents {
    pub fn new(keep_handles: bool) -> Self {
        Self {
            first: 0,
            last: 0,
            count: 0,
            handles: Vec::new(),
            has_atts: false,
            keep_handles,
        }
    }

    pub fn push(&mut self, handle: u64) {
        if self.first == 0 {
            self.first = handle;
        }
        self.last = handle;
        self.count += 1;
        if self.keep_handles {
            self.handles.push(handle);
        }
    }
}

/// Everything the table records need from the encoded entities.
pub struct DocumentSummary<'a> {
    pub tables: &'a DocumentTables,
    pub modern: bool,
    /// Per layout in writer order: model space, paper space, blocks.
    pub layouts: &'a [LayoutContents],
    /// Per block: INSERT and DIMENSION handles that reference it.
    pub block_refs: &'a [Vec<u64>],
    /// IMAGEDEF_REACTOR handles per upper-case image definition name.
    pub image_reactors: &'a BTreeMap<String, Vec<u64>>,
    /// Upper-case names of the image definitions an IMAGE references.
    pub used_image_defs: &'a HashSet<String>,
    /// The overall paper viewport (0 when paper space is empty).
    pub paper_viewport: u64,
}

/// `Standard` DIMSTYLE (R2000 layout): the ISO-25 like defaults of the header
/// dimension variables, decimal point `.` and 2 decimals.
fn dimstyle_body(w: &mut BitWriter) -> Result<()> {
    table_name(w, "Standard")?;
    w.write_tv("")?; // DIMPOST
    w.write_tv("")?; // DIMAPOST
                     // DIMSCALE, DIMASZ, DIMEXO, DIMDLI, DIMEXE, DIMRND, DIMDLE, DIMTP, DIMTM
    for v in [1., 2.5, 0.625, 3.75, 1.25, 0., 0., 0., 0.] {
        w.write_bd(v)?;
    }
    // DIMTOL, DIMLIM, DIMTIH, DIMTOH, DIMSE1, DIMSE2
    if w.is_unicode() {
        w.write_bd(1.)?;
        w.write_bd(std::f64::consts::FRAC_PI_4)?;
        w.write_bs(0)?;
        w.write_cmc(0, None)?;
    }
    for v in [0, 0, 0, 0, 0, 0] {
        w.write_b(v)?;
    }
    // DIMTAD, DIMZIN, DIMAZIN
    for v in [1, 8, 0] {
        w.write_bs(v)?;
    }
    if w.is_unicode() {
        w.write_bs(0)?;
    } // DIMARCSYM
      // DIMTXT, DIMCEN, DIMTSZ, DIMALTF, DIMLFAC, DIMTVP, DIMTFAC, DIMGAP, DIMALTRND
    for v in [2.5, 2.5, 0., 0.03937007874015748, 1., 0., 1., 0.625, 0.] {
        w.write_bd(v)?;
    }
    w.write_b(0)?; // DIMALT
    w.write_bs(2)?; // DIMALTD
                    // DIMTOFL, DIMSAH, DIMTIX, DIMSOXD
    for v in [1, 0, 0, 0] {
        w.write_b(v)?;
    }
    // DIMCLRD, DIMCLRE, DIMCLRT (BYBLOCK), DIMADEC, DIMDEC, DIMTDEC, DIMALTU,
    // DIMALTTD, DIMAUNIT, DIMFRAC, DIMLUNIT, DIMDSEP ('.'), DIMTMOVE, DIMJUST
    for _ in 0..3 {
        w.write_cmc(0, None)?;
    }
    for v in [0, 2, 2, 2, 2, 0, 0, 2, 46, 0, 0] {
        w.write_bs(v)?;
    }
    w.write_b(0)?; // DIMSD1
    w.write_b(0)?; // DIMSD2
                   // DIMTOLJ, DIMTZIN, DIMALTZ, DIMALTTZ
    for v in [1, 0, 0, 0] {
        w.write_bs(v)?;
    }
    w.write_b(0)?; // DIMUPT
    w.write_bs(3)?; // DIMFIT
    if w.is_unicode() {
        w.write_b(0)?;
        w.write_b(0)?;
        w.write_bd(100.)?;
        w.write_tv("")?;
        w.write_bd(100.)?;
        w.write_tv("")?;
    }
    w.write_bs((-2i16) as u16)?; // DIMLWD BYBLOCK
    w.write_bs((-2i16) as u16)?; // DIMLWE BYBLOCK
    w.write_b(0) // unknown (70 bit 0)
}

/// The `Model` LAYOUT: default A3 plot settings of a model-space-only drawing.
fn model_layout_body(w: &mut BitWriter) -> Result<()> {
    w.write_tv("")?;
    w.write_tv("")?;
    w.write_bs(1024)?;
    for v in [0., 0., 0., 0., 420., 297.] {
        w.write_bd(v)?;
    }
    w.write_tv("")?;
    w.write_bd(0.)?;
    w.write_bd(0.)?;
    w.write_bs(1)?;
    w.write_bs(0)?;
    w.write_bs(0)?;
    for _ in 0..4 {
        w.write_bd(0.)?;
    }
    if !w.is_unicode() {
        w.write_tv("")?;
    }
    w.write_bd(1.)?;
    w.write_bd(1.)?;
    w.write_tv("")?;
    w.write_bs(16)?;
    w.write_bd(1.)?;
    w.write_bd(0.)?;
    w.write_bd(0.)?;
    if w.is_unicode() {
        for v in [0, 2, 300] {
            w.write_bs(v)?;
        }
    }
    w.write_tv("Model")?;
    w.write_bl(0)?;
    w.write_bs(1)?;
    w.write_3bd(0., 0., 0.)?;
    for v in [0., 0., 420., 297.] {
        w.write_rd(Endian::Little, v)?;
    }
    for p in [(0., 0., 0.), (1., 0., 0.), (0., 1., 0.)] {
        w.write_3bd(p.0, p.1, p.2)?;
    }
    w.write_bd(0.)?;
    w.write_bs(0)?;
    w.write_3bd(0., 0., 0.)?;
    w.write_3bd(0., 0., 0.)?;
    if w.is_unicode() {
        w.write_bl(0)?;
    }
    Ok(())
}

/// The paper-space LAYOUT (ODA 20.4.84, R2000 column): PLOTSETTINGS data from
/// `layout`, then the layout name, tab order 1, PSLTSCALE flag, the world UCS,
/// limits around the paper and, once the sheet holds entities, the paper
/// rectangle as extents.
fn paper_layout_body(w: &mut BitWriter, layout: &LayoutSettings, has_entities: bool) -> Result<()> {
    w.write_tv(&layout.page_setup_name)?;
    w.write_tv(&layout.printer)?;
    w.write_bs(layout.plot_layout_flags)?;
    let (left, bottom, right, top) = layout.margins;
    for v in [
        left,
        bottom,
        right,
        top,
        layout.paper_width,
        layout.paper_height,
    ] {
        w.write_bd(v)?;
    }
    w.write_tv(&layout.paper_size)?;
    w.write_bd(layout.plot_origin.0)?;
    w.write_bd(layout.plot_origin.1)?;
    w.write_bs(layout.paper_units)?;
    w.write_bs(layout.plot_rotation)?;
    w.write_bs(layout.plot_type)?;
    let ((wx0, wy0), (wx1, wy1)) = layout.plot_window;
    for v in [wx0, wy0, wx1, wy1] {
        w.write_bd(v)?;
    }
    if !w.is_unicode() {
        w.write_tv(&layout.plot_view_name)?;
    }
    w.write_bd(layout.scale_numerator)?;
    w.write_bd(layout.scale_denominator)?;
    w.write_tv(&layout.style_sheet)?;
    w.write_bs(layout.scale_type)?;
    w.write_bd(layout.scale_factor())?;
    w.write_bd(layout.paper_image_origin.0)?;
    w.write_bd(layout.paper_image_origin.1)?;
    if w.is_unicode() {
        for v in [0, 2, 300] {
            w.write_bs(v)?;
        }
    }
    w.write_tv(&layout.name)?;
    w.write_bl(1)?; // tab order
    w.write_bs(1)?; // PSLTSCALE
    w.write_3bd(0., 0., 0.)?; // UCS origin
    let ((x0, y0), (x1, y1)) = layout.limits();
    for v in [x0, y0, x1, y1] {
        w.write_rd(Endian::Little, v)?;
    }
    for p in [(0., 0., 0.), (1., 0., 0.), (0., 1., 0.)] {
        w.write_3bd(p.0, p.1, p.2)?; // insertion base, UCS X, UCS Y
    }
    w.write_bd(0.)?; // elevation
    w.write_bs(0)?; // orthographic view type
    if has_entities {
        w.write_3bd(x0, y0, 0.)?;
        w.write_3bd(x1, y1, 0.)?;
    } else {
        w.write_3bd(0., 0., 0.)?;
        w.write_3bd(0., 0., 0.)?;
    }
    if w.is_unicode() {
        w.write_bl(0)?;
    }
    Ok(())
}

fn table_name(w: &mut BitWriter, name: &str) -> Result<()> {
    w.write_tv(name)?;
    if w.is_unicode() {
        w.write_bs(0)
    } else {
        w.write_b(0)?;
        w.write_bs(0)?;
        w.write_b(0)
    }
}

/// Common non-entity record, with an absolute handle and explicit handle stream.
fn object<F>(
    codepage: u16,
    modern: bool,
    typ: u16,
    handle: u64,
    owner: u64,
    refs: &[(u8, u64)],
    body: F,
) -> Result<Vec<u8>>
where
    F: FnOnce(&mut BitWriter) -> Result<()>,
{
    if modern {
        return crate::writer::r2018::streams::object(typ, handle, owner, refs, body);
    }
    let mut w = BitWriter::with_codepage(codepage);
    w.write_bs(typ)?;
    let size_pos = w.tell_bits() as u32;
    w.write_rl(Endian::Little, 0)?;
    w.write_h(0, handle)?;
    w.write_bs(0)?;
    w.write_bl(0)?; // handle, EED, reactors
    body(&mut w)?;
    let end = w.tell_bits() as u32;
    w.set_bit_pos(size_pos);
    w.write_rl(Endian::Little, end)?;
    w.set_bit_pos(end);
    w.write_h(4, owner)?;
    w.write_h(3, 0)?;
    for &(code, h) in refs {
        w.write_h(code, h)?;
    }
    Ok(w.into_bytes())
}
