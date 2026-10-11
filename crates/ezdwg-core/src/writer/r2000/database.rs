//! Minimal R2000 database graph. All handles are allocated after reserving input
//! handles; no drawing/template bytes or external converter are used by the writer.
use super::entities::common::{encode_entity_payload, CommonEntityEncodeInput};
use crate::bit::{BitWriter, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::writer::ir::{LayerDef, TextStyleDef, WriterDocument};
use crate::writer::HandleAllocator;
use std::collections::BTreeMap;

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
}

impl DatabaseHandles {
    pub fn allocate(a: &mut HandleAllocator, doc: &WriterDocument) -> Result<Self> {
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
        };
        result.layers.insert("0".into(), result.layer);
        for layer in &doc.layers {
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
        for lt in &doc.linetypes {
            result
                .linetypes
                .insert(lt.name.to_uppercase(), a.allocate()?);
        }
        result.styles.insert("STANDARD".into(), result.style);
        for st in &doc.text_styles {
            if !st.name.eq_ignore_ascii_case("Standard") {
                result.styles.insert(st.name.to_uppercase(), a.allocate()?);
            }
        }
        for block in &doc.blocks {
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

    pub fn objects(
        &self,
        layout_handles: &[Vec<u64>],
        doc: &WriterDocument,
    ) -> Result<Vec<(u64, Vec<u8>)>> {
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
                object(doc.metadata.codepage, typ, h, 0, &refs, |w| {
                    w.write_bl(entries.len() as u32)?;
                    if i == 8 {
                        w.write_rc(0)?;
                    } // no extra DIMSTYLE handles
                    Ok(())
                })?,
            ));
        }
        let [nod, groups, mlines, layouts, plots, styles] = self.dictionaries;
        let names = [
            "ACAD_GROUP",
            "ACAD_MLINESTYLE",
            "ACAD_LAYOUT",
            "ACAD_PLOTSETTINGS",
            "ACAD_PLOTSTYLENAME",
        ];
        for (i, &h) in self.dictionaries.iter().enumerate() {
            let mut refs: Vec<_> = match i {
                0 => [groups, mlines, layouts, plots, styles]
                    .map(|h| (2, h))
                    .to_vec(),
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
                    doc.metadata.codepage,
                    if i == 5 { 500 } else { 0x2A },
                    h,
                    if i == 0 { 0 } else { nod },
                    &refs,
                    |w| {
                        w.write_bl(count as u32)?;
                        w.write_bs(1)?; // duplicate-record cloning: keep existing
                        w.write_rc(0)?; // soft ownership
                        if i == 0 {
                            for name in names {
                                w.write_tv(name)?;
                            }
                        }
                        if i == 3 {
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
                doc.metadata.codepage,
                501,
                self.normal_plotstyle,
                styles,
                &[],
                |_| Ok(()),
            )?,
        ));
        let default_layer = LayerDef::default();
        for layer in std::iter::once(&default_layer)
            .filter(|_| !doc.layers.iter().any(|l| l.name == "0"))
            .chain(doc.layers.iter())
        {
            let handle = self.layer_handle(&layer.name)?;
            let ltype = self.linetype_handle(&layer.linetype)?;
            objects.push((
                handle,
                object(
                    doc.metadata.codepage,
                    0x33,
                    handle,
                    self.controls[1],
                    &[(5, 0), (5, self.normal_plotstyle), (5, ltype)],
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
                        w.write_bs(layer.color_index)
                    },
                )?,
            ));
        }
        let default_style = TextStyleDef::default();
        for st in std::iter::once(&default_style)
            .filter(|_| {
                !doc.text_styles
                    .iter()
                    .any(|s| s.name.eq_ignore_ascii_case("Standard"))
            })
            .chain(doc.text_styles.iter())
        {
            let h = self.style_handle(&st.name)?;
            objects.push((
                h,
                object(
                    doc.metadata.codepage,
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
            let definition = doc
                .linetypes
                .iter()
                .find(|l| l.name.to_uppercase() == *name);
            let dashes = definition.map(|l| l.dashes.as_slice()).unwrap_or(&[]);
            let mut refs = vec![(5, 0)];
            refs.extend(dashes.iter().map(|_| (5, 0)));
            objects.push((
                *h,
                object(
                    doc.metadata.codepage,
                    0x39,
                    *h,
                    self.controls[3],
                    &refs,
                    |w| {
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
                        w.write_rcs(&[0; 256])
                    },
                )?,
            ));
        }
        let spaces = [
            (
                self.model,
                "*Model_Space",
                self.model_begin,
                self.model_end,
                layout_handles[0].as_slice(),
                self.model_layout,
                (0., 0., 0.),
                "",
                false,
            ),
            (
                self.paper,
                "*Paper_Space",
                self.paper_begin,
                self.paper_end,
                &[][..],
                self.paper_layout,
                (0., 0., 0.),
                "",
                false,
            ),
        ];
        let definitions = doc.blocks.iter().enumerate().map(|(i, b)| {
            let (h, begin, end) = self.blocks[&b.name.to_uppercase()];
            (
                h,
                b.name.as_str(),
                begin,
                end,
                layout_handles[i + 1].as_slice(),
                0,
                b.base_point,
                b.description.as_str(),
                b.entities
                    .iter()
                    .any(|e| matches!(e, crate::writer::WriterEntity::Attdef(_))),
            )
        });
        for (h, name, begin, end, contents, layout, base, description, has_atts) in
            spaces.into_iter().chain(definitions)
        {
            let inserts: Vec<u64> = doc
                .layouts()
                .zip(layout_handles)
                .flat_map(|(entities, handles)| {
                    entities
                        .iter()
                        .zip(handles)
                        .filter_map(|(e, &handle)| match e {
                            crate::writer::WriterEntity::Insert(i)
                                if i.block_name.to_uppercase() == name.to_uppercase() =>
                            {
                                Some(handle)
                            }
                            _ => None,
                        })
                })
                .collect();
            let mut refs = vec![
                (5, 0),
                (3, begin),
                (4, contents.first().copied().unwrap_or(0)),
                (4, contents.last().copied().unwrap_or(0)),
                (3, end),
            ];
            refs.extend(inserts.iter().map(|h| (4, *h)));
            refs.push((5, layout));
            objects.push((
                h,
                object(
                    doc.metadata.codepage,
                    0x31,
                    h,
                    self.controls[0],
                    &refs,
                    |w| {
                        table_name(w, name)?;
                        w.write_b(u8::from(name.starts_with('*') && layout == 0))?; // anonymous
                        w.write_b(u8::from(has_atts))?;
                        for _ in 0..3 {
                            w.write_b(0)?;
                        } // xref, overlay, unloaded
                        w.write_3bd(base.0, base.1, base.2)?;
                        w.write_tv("")?;
                        for _ in &inserts {
                            w.write_rc(1)?;
                        }
                        w.write_rc(0)?;
                        w.write_tv(description)?;
                        w.write_bl(0)
                    },
                )?,
            ));
            if layout != 0 {
                objects.push((
                    layout,
                    object(
                        doc.metadata.codepage,
                        502,
                        layout,
                        layouts,
                        &[(4, h), (4, 0), (5, 0), (5, 0)],
                        |w| layout_body(w, h == self.model),
                    )?,
                ));
            }
            for (handle, typ) in [(begin, 4), (end, 5)] {
                let common = CommonEntityEncodeInput {
                    handle,
                    owner_handle: h,
                    layer_handle: self.layer,
                    color_index: 256,
                    linetype_flags: 0,
                    linetype_handle: 0,
                    codepage: doc.metadata.codepage,
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

fn layout_body(w: &mut BitWriter, model: bool) -> Result<()> {
    w.write_tv("")?;
    w.write_tv("")?;
    w.write_bs(if model { 1024 } else { 0 })?;
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
    w.write_tv("")?;
    w.write_bd(1.)?;
    w.write_bd(1.)?;
    w.write_tv("")?;
    w.write_bs(16)?;
    w.write_bd(1.)?;
    w.write_bd(0.)?;
    w.write_bd(0.)?;
    w.write_tv(if model { "Model" } else { "Layout1" })?;
    w.write_bl(if model { 0 } else { 1 })?;
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
    w.write_3bd(0., 0., 0.)
}

fn table_name(w: &mut BitWriter, name: &str) -> Result<()> {
    w.write_tv(name)?;
    w.write_b(0)?;
    w.write_bs(0)?;
    w.write_b(0)
}

/// Common non-entity record, with an absolute handle and explicit handle stream.
fn object<F>(
    codepage: u16,
    typ: u16,
    handle: u64,
    owner: u64,
    refs: &[(u8, u64)],
    body: F,
) -> Result<Vec<u8>>
where
    F: FnOnce(&mut BitWriter) -> Result<()>,
{
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
