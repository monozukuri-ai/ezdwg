use pyo3::types::{PyBytes, PyDict};
use std::collections::VecDeque;
use writer::encoder::{DocumentEncoder, DocumentTables, LayoutId};
use writer::ir::{AttributeEntity, EntityFormatting, LinetypeDef};

/// Entity groups of `DwgWriter.add`, in the order the legacy `write_dwg`
/// stores them; `order` tags index this list.
const GROUP_NAMES: [&str; 23] = [
    "lines",
    "arcs",
    "circles",
    "lwpolylines",
    "texts",
    "mtexts",
    "points",
    "rays",
    "xlines",
    "ellipses",
    "solids",
    "traces",
    "faces3d",
    "inserts",
    "attdefs",
    "attribs",
    "polylines2d",
    "polylines3d",
    "splines",
    "hatches",
    "dimensions",
    "images",
    "viewports",
];
const GROUP_INSERTS: usize = 13;
const GROUP_ATTDEFS: usize = 14;
const GROUP_ATTRIBS: usize = 15;
const GROUP_POLYLINES2D: usize = 16;
const GROUP_POLYLINES3D: usize = 17;
const GROUP_SPLINES: usize = 18;
const GROUP_HATCHES: usize = 19;
const GROUP_DIMENSIONS: usize = 20;
const GROUP_IMAGES: usize = 21;
const GROUP_VIEWPORTS: usize = 22;

#[pyfunction]
pub fn write_ac1015_line_dwg(output_path: &str, lines: Vec<LineEntityRow>) -> PyResult<()> {
    write_ac1015_dwg(
        output_path,
        lines,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
}

#[pyfunction(signature = (
    output_path,
    lines,
    arcs,
    circles,
    lwpolylines,
    texts,
    mtexts,
    points=None,
    rays=None,
    xlines=None,
    *, entity_attributes=None, layers=None, linetypes=None, metadata=None, text_styles=None,
    blocks=None, inserts=None, attdefs=None, attribs=None, entity_owners=None, ellipses=None,
    polylines2d=None, polylines3d=None, solids=None, traces=None, faces3d=None, splines=None,
    hatches=None, dimensions=None, images=None, image_defs=None, viewports=None, layout=None
))]
#[allow(clippy::too_many_arguments)]
pub fn write_ac1015_dwg(
    output_path: &str,
    lines: Vec<LineEntityRow>,
    arcs: Vec<ArcEntityRow>,
    circles: Vec<CircleEntityRow>,
    lwpolylines: Vec<LwPolylineEntityRow>,
    texts: Vec<TextWriteRow>,
    mtexts: Vec<MTextWriteRow>,
    points: Option<Vec<PointWriteRow>>,
    rays: Option<Vec<RayEntityRow>>,
    xlines: Option<Vec<XLineEntityRow>>,
    entity_attributes: Option<HashMap<u64, Bound<'_, PyDict>>>,
    layers: Option<Vec<Bound<'_, PyDict>>>,
    linetypes: Option<Vec<Bound<'_, PyDict>>>,
    metadata: Option<Bound<'_, PyDict>>,
    text_styles: Option<Vec<Bound<'_, PyDict>>>,
    blocks: Option<Vec<Bound<'_, PyDict>>>,
    inserts: Option<Vec<Bound<'_, PyDict>>>,
    attdefs: Option<Vec<Bound<'_, PyDict>>>,
    attribs: Option<Vec<Bound<'_, PyDict>>>,
    entity_owners: Option<HashMap<u64, Bound<'_, PyAny>>>,
    ellipses: Option<Vec<EllipseEntityRow>>,
    polylines2d: Option<Vec<Bound<'_, PyDict>>>,
    polylines3d: Option<Vec<Bound<'_, PyDict>>>,
    solids: Option<Vec<SolidEntityRow>>,
    traces: Option<Vec<TraceEntityRow>>,
    faces3d: Option<Vec<Face3dEntityRow>>,
    splines: Option<Vec<Bound<'_, PyDict>>>,
    hatches: Option<Vec<Bound<'_, PyDict>>>,
    dimensions: Option<Vec<Bound<'_, PyDict>>>,
    images: Option<Vec<Bound<'_, PyDict>>>,
    image_defs: Option<Vec<Bound<'_, PyDict>>>,
    viewports: Option<Vec<Bound<'_, PyDict>>>,
    layout: Option<Bound<'_, PyDict>>,
) -> PyResult<()> {
    write_dwg(
        output_path,
        lines,
        arcs,
        circles,
        lwpolylines,
        texts,
        mtexts,
        points,
        rays,
        xlines,
        entity_attributes,
        layers,
        linetypes,
        metadata,
        text_styles,
        blocks,
        inserts,
        attdefs,
        attribs,
        entity_owners,
        ellipses,
        polylines2d,
        polylines3d,
        solids,
        traces,
        faces3d,
        splines,
        hatches,
        dimensions,
        images,
        image_defs,
        viewports,
        layout,
        "AC1015",
    )
}

/// One-shot writer: tables, every entity in one batch, then the file.
#[pyfunction(signature = (
    output_path,
    lines,
    arcs,
    circles,
    lwpolylines,
    texts,
    mtexts,
    points=None,
    rays=None,
    xlines=None,
    *, entity_attributes=None, layers=None, linetypes=None, metadata=None, text_styles=None,
    blocks=None, inserts=None, attdefs=None, attribs=None, entity_owners=None, ellipses=None,
    polylines2d=None, polylines3d=None, solids=None, traces=None, faces3d=None, splines=None,
    hatches=None, dimensions=None, images=None, image_defs=None, viewports=None, layout=None, version="AC1015"
))]
#[allow(clippy::too_many_arguments)]
pub fn write_dwg(
    output_path: &str,
    lines: Vec<LineEntityRow>,
    arcs: Vec<ArcEntityRow>,
    circles: Vec<CircleEntityRow>,
    lwpolylines: Vec<LwPolylineEntityRow>,
    texts: Vec<TextWriteRow>,
    mtexts: Vec<MTextWriteRow>,
    points: Option<Vec<PointWriteRow>>,
    rays: Option<Vec<RayEntityRow>>,
    xlines: Option<Vec<XLineEntityRow>>,
    entity_attributes: Option<HashMap<u64, Bound<'_, PyDict>>>,
    layers: Option<Vec<Bound<'_, PyDict>>>,
    linetypes: Option<Vec<Bound<'_, PyDict>>>,
    metadata: Option<Bound<'_, PyDict>>,
    text_styles: Option<Vec<Bound<'_, PyDict>>>,
    blocks: Option<Vec<Bound<'_, PyDict>>>,
    inserts: Option<Vec<Bound<'_, PyDict>>>,
    attdefs: Option<Vec<Bound<'_, PyDict>>>,
    attribs: Option<Vec<Bound<'_, PyDict>>>,
    entity_owners: Option<HashMap<u64, Bound<'_, PyAny>>>,
    ellipses: Option<Vec<EllipseEntityRow>>,
    polylines2d: Option<Vec<Bound<'_, PyDict>>>,
    polylines3d: Option<Vec<Bound<'_, PyDict>>>,
    solids: Option<Vec<SolidEntityRow>>,
    traces: Option<Vec<TraceEntityRow>>,
    faces3d: Option<Vec<Face3dEntityRow>>,
    splines: Option<Vec<Bound<'_, PyDict>>>,
    hatches: Option<Vec<Bound<'_, PyDict>>>,
    dimensions: Option<Vec<Bound<'_, PyDict>>>,
    images: Option<Vec<Bound<'_, PyDict>>>,
    image_defs: Option<Vec<Bound<'_, PyDict>>>,
    viewports: Option<Vec<Bound<'_, PyDict>>>,
    layout: Option<Bound<'_, PyDict>>,
    version: &str,
) -> PyResult<()> {
    let mut writer = DwgWriter::new(version, None)?;
    writer.set_tables(
        layers,
        linetypes,
        text_styles,
        metadata,
        blocks,
        image_defs,
        layout,
    )?;
    writer.add(
        Some(lines),
        Some(arcs),
        Some(circles),
        Some(lwpolylines),
        Some(texts),
        Some(mtexts),
        points,
        rays,
        xlines,
        ellipses,
        solids,
        traces,
        faces3d,
        inserts,
        attdefs,
        attribs,
        polylines2d,
        polylines3d,
        splines,
        hatches,
        dimensions,
        images,
        viewports,
        entity_attributes,
        entity_owners,
        None,
    )?;
    writer.write(output_path)
}

/// Where the entities of a writer go.
enum Entities {
    /// `max_handle` was given: every batch is encoded to object records at
    /// once by the incremental encoder; nothing but the records stays behind.
    Streaming(DocumentEncoder),
    /// No handle ceiling: entities wait in a document and are encoded on `write`.
    Buffered {
        document: writer::WriterDocument,
        /// ATTRIB rows by owning INSERT handle, attached on `write`.
        attributes: HashMap<u64, Vec<AttributeEntity>>,
    },
    Written,
}

/// Incremental DWG writer behind `ezdwg.raw.write_dwg` and `ezdwg.to_dwg`.
///
/// Tables are set with `set_tables`, entities arrive in any number of `add`
/// batches, and `write` encodes the document and atomically replaces the
/// destination. Each batch is converted to the writer's Rust representation
/// at once, so a caller can drop the Python rows of one batch before building
/// the next.
///
/// With ``max_handle`` (the largest entity handle that will be added) the
/// batches are encoded to object records as they arrive and only those
/// records are kept, about a hundred bytes per entity; without it the entities
/// wait in a document until `write`.
///
/// Entities keep the order of the `add` calls. Within one call ``order``
/// (one group tag per entity, see `GROUP_NAMES`) interleaves the groups;
/// without it the groups follow the legacy `write_dwg` order.
#[pyclass(module = "ezdwg._core")]
pub struct DwgWriter {
    version: String,
    max_handle: Option<u64>,
    /// Collected until the first `add`; fixed from then on.
    tables: Option<DocumentTables>,
    entities: Option<Entities>,
    block_index: HashMap<String, usize>,
    entity_count: usize,
}

#[pymethods]
impl DwgWriter {
    #[new]
    #[pyo3(signature = (version="AC1015", max_handle=None))]
    fn new(version: &str, max_handle: Option<u64>) -> PyResult<Self> {
        let doc_version = match version {
            "AC1015" => DwgVersion::R2000,
            "AC1032" => DwgVersion::R2018,
            _ => {
                return Err(PyValueError::new_err(format!(
                    "unsupported DWG write version: {version}"
                )))
            }
        };
        // Start from the live data: heap the caller already freed (a parsed
        // source document, for example) would otherwise count towards the
        // writer's own peak.
        release_freed_heap();
        Ok(Self {
            version: version.to_owned(),
            max_handle,
            tables: Some(DocumentTables {
                version: doc_version,
                ..DocumentTables::default()
            }),
            entities: None,
            block_index: HashMap::new(),
            entity_count: 0,
        })
    }

    /// Number of entities added so far.
    #[getter]
    fn entity_count(&self) -> usize {
        self.entity_count
    }

    /// Reserve room for ``count`` more model-space entities of a buffered
    /// writer (without ``max_handle``); a streaming writer keeps no entities,
    /// so this is a no-op there.
    fn reserve(&mut self, count: usize) -> PyResult<()> {
        self.begin()?;
        if let Some(Entities::Buffered { document, .. }) = self.entities.as_mut() {
            document.modelspace.reserve_exact(count);
        }
        Ok(())
    }

    /// Replace the layer, linetype and text style tables and the header
    /// metadata; append block and image definitions; set the paper-space
    /// layout. Once entities have been added, layers, linetypes and text
    /// styles can only be added through `add_tables`; blocks, image
    /// definitions, the layout and the metadata (with the same code page) may
    /// still change, because their records are written last.
    #[pyo3(signature = (*, layers=None, linetypes=None, text_styles=None, metadata=None, blocks=None, image_defs=None, layout=None))]
    #[allow(clippy::too_many_arguments)]
    fn set_tables(
        &mut self,
        layers: Option<Vec<Bound<'_, PyDict>>>,
        linetypes: Option<Vec<Bound<'_, PyDict>>>,
        text_styles: Option<Vec<Bound<'_, PyDict>>>,
        metadata: Option<Bound<'_, PyDict>>,
        blocks: Option<Vec<Bound<'_, PyDict>>>,
        image_defs: Option<Vec<Bound<'_, PyDict>>>,
        layout: Option<Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        if self.entities.is_some() {
            if layers.is_some() || linetypes.is_some() || text_styles.is_some() {
                return Err(PyValueError::new_err(
                    "DwgWriter layers, linetypes and text styles are fixed once entities have \
                     been added; add rows with add_tables",
                ));
            }
            self.add_tables(None, None, None, blocks, image_defs)?;
            if let Some(d) = metadata {
                let mut m = self.current_metadata()?;
                apply_write_metadata(&mut m, &d)?;
                self.store_metadata(m)?;
            }
            if let Some(d) = layout {
                let mut l = self.current_layout()?;
                apply_write_layout(&mut l, Some(d))?;
                self.store_layout(l)?;
            }
            return Ok(());
        }
        let tables = self.tables.as_mut().ok_or_else(spent_writer)?;
        if let Some(rows) = layers {
            tables.layers = parse_layers(&rows)?;
        }
        if let Some(rows) = linetypes {
            tables.linetypes = parse_linetypes(&rows)?;
        }
        if let Some(rows) = text_styles {
            tables.text_styles = parse_text_styles(&rows)?;
        }
        if let Some(d) = metadata {
            apply_write_metadata(&mut tables.metadata, &d)?;
        }
        append_block_defs(&mut tables.blocks, blocks)?;
        append_image_defs(&mut tables.image_defs, image_defs)?;
        apply_write_layout(&mut tables.layout, layout)
    }

    /// Add table rows at any time, also between batches of entities: a layer,
    /// linetype, text style or image definition with a known (case-insensitive)
    /// name replaces that row; a block definition must be new. Entities of a
    /// batch may reference rows added before it.
    #[pyo3(signature = (*, layers=None, linetypes=None, text_styles=None, blocks=None, image_defs=None))]
    fn add_tables(
        &mut self,
        layers: Option<Vec<Bound<'_, PyDict>>>,
        linetypes: Option<Vec<Bound<'_, PyDict>>>,
        text_styles: Option<Vec<Bound<'_, PyDict>>>,
        blocks: Option<Vec<Bound<'_, PyDict>>>,
        image_defs: Option<Vec<Bound<'_, PyDict>>>,
    ) -> PyResult<()> {
        let layers = layers
            .map(|r| parse_layers(&r))
            .transpose()?
            .unwrap_or_default();
        let linetypes = linetypes
            .map(|r| parse_linetypes(&r))
            .transpose()?
            .unwrap_or_default();
        let text_styles = text_styles
            .map(|r| parse_text_styles(&r))
            .transpose()?
            .unwrap_or_default();
        let mut block_rows = Vec::new();
        append_block_defs(&mut block_rows, blocks)?;
        let mut image_def_rows = Vec::new();
        append_image_defs(&mut image_def_rows, image_defs)?;
        let Self {
            entities,
            tables,
            block_index,
            ..
        } = self;
        match entities.as_mut() {
            None => {
                let tables = tables.as_mut().ok_or_else(spent_writer)?;
                for row in layers {
                    upsert(&mut tables.layers, row, |l| l.name.to_uppercase());
                }
                for row in linetypes {
                    upsert(&mut tables.linetypes, row, |l| l.name.to_uppercase());
                }
                for row in text_styles {
                    upsert(&mut tables.text_styles, row, |st| st.name.to_uppercase());
                }
                tables.blocks.extend(block_rows);
                for row in image_def_rows {
                    upsert(&mut tables.image_defs, row, |d| d.name.to_uppercase());
                }
            }
            Some(Entities::Streaming(encoder)) => {
                for row in layers {
                    encoder.add_layer(row).map_err(to_py_err)?;
                }
                for row in linetypes {
                    encoder.add_linetype(row).map_err(to_py_err)?;
                }
                for row in text_styles {
                    encoder.add_text_style(row).map_err(to_py_err)?;
                }
                for row in block_rows {
                    let key = row.name.to_uppercase();
                    let index = encoder.add_block(row).map_err(to_py_err)?;
                    block_index.insert(key, index);
                }
                for row in image_def_rows {
                    encoder.add_image_def(row).map_err(to_py_err)?;
                }
            }
            Some(Entities::Buffered { document, .. }) => {
                for row in layers {
                    upsert(&mut document.layers, row, |l| l.name.to_uppercase());
                }
                for row in linetypes {
                    upsert(&mut document.linetypes, row, |l| l.name.to_uppercase());
                }
                for row in text_styles {
                    upsert(&mut document.text_styles, row, |st| st.name.to_uppercase());
                }
                for row in block_rows {
                    block_index.insert(row.name.to_uppercase(), document.blocks.len());
                    document.blocks.push(row);
                }
                for row in image_def_rows {
                    upsert(&mut document.image_defs, row, |d| d.name.to_uppercase());
                }
            }
            Some(Entities::Written) => return Err(spent_writer()),
        }
        Ok(())
    }

    /// Add one batch of entities and return how many were added.
    ///
    /// ``entity_attributes`` (common properties by nonzero handle) must only
    /// name entities of this batch; ``entity_owners`` routes entities to a block
    /// (by name), to paper space (`*PAPER_SPACE`) or, for ATTRIB, to the owning
    /// INSERT (by handle).
    #[pyo3(signature = (*, lines=None, arcs=None, circles=None, lwpolylines=None, texts=None, mtexts=None,
        points=None, rays=None, xlines=None, ellipses=None, solids=None, traces=None, faces3d=None,
        inserts=None, attdefs=None, attribs=None, polylines2d=None, polylines3d=None, splines=None,
        hatches=None, dimensions=None, images=None, viewports=None,
        entity_attributes=None, entity_owners=None, order=None))]
    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        lines: Option<Vec<LineEntityRow>>,
        arcs: Option<Vec<ArcEntityRow>>,
        circles: Option<Vec<CircleEntityRow>>,
        lwpolylines: Option<Vec<LwPolylineEntityRow>>,
        texts: Option<Vec<TextWriteRow>>,
        mtexts: Option<Vec<MTextWriteRow>>,
        points: Option<Vec<PointWriteRow>>,
        rays: Option<Vec<RayEntityRow>>,
        xlines: Option<Vec<XLineEntityRow>>,
        ellipses: Option<Vec<EllipseEntityRow>>,
        solids: Option<Vec<SolidEntityRow>>,
        traces: Option<Vec<TraceEntityRow>>,
        faces3d: Option<Vec<Face3dEntityRow>>,
        inserts: Option<Vec<Bound<'_, PyDict>>>,
        attdefs: Option<Vec<Bound<'_, PyDict>>>,
        attribs: Option<Vec<Bound<'_, PyDict>>>,
        polylines2d: Option<Vec<Bound<'_, PyDict>>>,
        polylines3d: Option<Vec<Bound<'_, PyDict>>>,
        splines: Option<Vec<Bound<'_, PyDict>>>,
        hatches: Option<Vec<Bound<'_, PyDict>>>,
        dimensions: Option<Vec<Bound<'_, PyDict>>>,
        images: Option<Vec<Bound<'_, PyDict>>>,
        viewports: Option<Vec<Bound<'_, PyDict>>>,
        entity_attributes: Option<HashMap<u64, Bound<'_, PyDict>>>,
        entity_owners: Option<HashMap<u64, Bound<'_, PyAny>>>,
        order: Option<Bound<'_, PyBytes>>,
    ) -> PyResult<usize> {
        if matches!(self.entities, Some(Entities::Written)) {
            return Err(spent_writer());
        }
        let mut groups: Vec<Vec<writer::WriterEntity>> =
            (0..GROUP_NAMES.len()).map(|_| Vec::new()).collect();
        build_basic_rows(
            &mut groups,
            lines.unwrap_or_default(),
            arcs.unwrap_or_default(),
            circles.unwrap_or_default(),
            lwpolylines.unwrap_or_default(),
            texts.unwrap_or_default(),
            mtexts.unwrap_or_default(),
            points.unwrap_or_default(),
            rays.unwrap_or_default(),
            xlines.unwrap_or_default(),
            ellipses.unwrap_or_default(),
            solids.unwrap_or_default(),
            traces.unwrap_or_default(),
            faces3d.unwrap_or_default(),
        );
        build_write_inserts(&mut groups[GROUP_INSERTS], inserts)?;
        build_write_attributes(&mut groups[GROUP_ATTDEFS], attdefs, true)?;
        build_write_attributes(&mut groups[GROUP_ATTRIBS], attribs, false)?;
        build_write_polylines(&mut groups[GROUP_POLYLINES2D], polylines2d, false)?;
        build_write_polylines(&mut groups[GROUP_POLYLINES3D], polylines3d, true)?;
        build_write_splines(&mut groups[GROUP_SPLINES], splines)?;
        build_write_hatches(&mut groups[GROUP_HATCHES], hatches)?;
        build_write_dimensions(&mut groups[GROUP_DIMENSIONS], dimensions)?;
        build_write_images(&mut groups[GROUP_IMAGES], images)?;
        build_write_viewports(&mut groups[GROUP_VIEWPORTS], viewports)?;
        let mut entities = interleave(groups, order.as_ref().map(|o| o.as_bytes()))?;
        apply_entity_attributes(&mut entities, entity_attributes)?;
        let added = entities.len();
        self.begin()?;
        self.distribute(entities, entity_owners)?;
        self.entity_count += added;
        Ok(added)
    }

    /// Validate and encode the document, then atomically replace ``output_path``.
    ///
    /// The document is consumed: its memory is released (and returned to the
    /// operating system where the allocator supports it) before the file is
    /// written, so the writer cannot be used again.
    fn write(&mut self, output_path: &str) -> PyResult<()> {
        self.begin()?;
        let out_path = Path::new(output_path);
        match self.entities.replace(Entities::Written) {
            Some(Entities::Streaming(encoder)) => {
                let encoded = encoder.finish().map_err(to_py_err)?;
                crate::atomic_write::atomic_write_streaming(out_path, |file| {
                    // Records are small; buffer them into page-sized writes.
                    let mut out = std::io::BufWriter::with_capacity(1 << 20, file);
                    encoded
                        .write_to(&mut out)
                        .map_err(|err| std::io::Error::other(err.to_string()))?;
                    std::io::Write::flush(&mut out)
                })
                .map_err(|err| PyIOError::new_err(err.to_string()))?;
            }
            Some(Entities::Buffered {
                mut document,
                attributes,
            }) => {
                attach_attributes(&mut document, attributes)?;
                let encode = match self.version.as_str() {
                    "AC1032" => writer::r2018::write_document,
                    _ => writer::r2000::write_document,
                };
                let bytes =
                    encode(&document, &writer::WriterConfig::default()).map_err(to_py_err)?;
                drop(document);
                crate::atomic_write::atomic_write(out_path, &bytes)
                    .map_err(|err| PyIOError::new_err(err.to_string()))?;
            }
            _ => return Err(spent_writer()),
        }
        clear_decode_cache()?;
        release_freed_heap();
        Ok(())
    }
}

impl DwgWriter {
    fn current_metadata(&self) -> PyResult<writer::WriterMetadata> {
        match (&self.tables, &self.entities) {
            (Some(tables), _) => Ok(tables.metadata.clone()),
            (None, Some(Entities::Streaming(encoder))) => Ok(encoder.metadata().clone()),
            (None, Some(Entities::Buffered { document, .. })) => Ok(document.metadata.clone()),
            _ => Err(spent_writer()),
        }
    }

    fn store_metadata(&mut self, metadata: writer::WriterMetadata) -> PyResult<()> {
        match (&mut self.tables, &mut self.entities) {
            (Some(tables), _) => tables.metadata = metadata,
            (None, Some(Entities::Streaming(encoder))) => {
                encoder.set_metadata(metadata).map_err(to_py_err)?
            }
            (None, Some(Entities::Buffered { document, .. })) => document.metadata = metadata,
            _ => return Err(spent_writer()),
        }
        Ok(())
    }

    fn current_layout(&self) -> PyResult<writer::LayoutSettings> {
        match (&self.tables, &self.entities) {
            (Some(tables), _) => Ok(tables.layout.clone()),
            (None, Some(Entities::Streaming(encoder))) => Ok(encoder.layout().clone()),
            (None, Some(Entities::Buffered { document, .. })) => Ok(document.layout.clone()),
            _ => Err(spent_writer()),
        }
    }

    fn store_layout(&mut self, layout: writer::LayoutSettings) -> PyResult<()> {
        match (&mut self.tables, &mut self.entities) {
            (Some(tables), _) => tables.layout = layout,
            (None, Some(Entities::Streaming(encoder))) => {
                encoder.set_layout(layout).map_err(to_py_err)?
            }
            (None, Some(Entities::Buffered { document, .. })) => document.layout = layout,
            _ => return Err(spent_writer()),
        }
        Ok(())
    }

    /// Fix the tables and open the entity store on the first batch.
    fn begin(&mut self) -> PyResult<()> {
        if self.entities.is_some() {
            return Ok(());
        }
        let tables = self.tables.take().ok_or_else(spent_writer)?;
        self.block_index = tables
            .blocks
            .iter()
            .enumerate()
            .map(|(i, b)| (b.name.to_uppercase(), i))
            .collect();
        self.entities = Some(match self.max_handle {
            Some(ceiling) => Entities::Streaming(
                DocumentEncoder::new(tables, writer::WriterConfig::default(), ceiling)
                    .map_err(to_py_err)?,
            ),
            None => Entities::Buffered {
                document: writer::WriterDocument {
                    version: tables.version.clone(),
                    modelspace: Vec::new(),
                    blocks: tables.blocks,
                    layers: tables.layers,
                    linetypes: tables.linetypes,
                    text_styles: tables.text_styles,
                    metadata: tables.metadata,
                    image_defs: tables.image_defs,
                    paperspace: Vec::new(),
                    layout: tables.layout,
                },
                attributes: HashMap::new(),
            },
        });
        Ok(())
    }

    /// Owner values are block names for definition contents, `*PAPER_SPACE`
    /// for paper-space entities, or INSERT handles for ATTRIB.
    fn distribute(
        &mut self,
        entities: Vec<writer::WriterEntity>,
        owners: Option<HashMap<u64, Bound<'_, PyAny>>>,
    ) -> PyResult<()> {
        let mut owners = owners.unwrap_or_default();
        let store = self.entities.as_mut().ok_or_else(spent_writer)?;
        for entity in entities {
            if let writer::WriterEntity::Attrib(attr) = entity {
                let owner = attr
                    .text
                    .common
                    .handle
                    .and_then(|h| owners.remove(&h))
                    .ok_or_else(|| PyValueError::new_err("ATTRIB requires an INSERT owner"))?;
                let handle = owner
                    .extract::<u64>()
                    .map_err(|_| PyValueError::new_err("ATTRIB owner must be an INSERT handle"))?;
                match store {
                    Entities::Streaming(encoder) => encoder.push_attribute(handle, *attr),
                    Entities::Buffered { attributes, .. } => {
                        attributes.entry(handle).or_default().push(*attr)
                    }
                    Entities::Written => return Err(spent_writer()),
                }
                continue;
            }
            let layout = match entity.common().handle.and_then(|h| owners.remove(&h)) {
                None => LayoutId::Model,
                Some(owner) => {
                    let name = owner
                        .extract::<String>()
                        .map_err(|_| PyValueError::new_err("entity owner must be a block name"))?;
                    if name.eq_ignore_ascii_case("*PAPER_SPACE") {
                        LayoutId::Paper
                    } else {
                        LayoutId::Block(
                            self.block_index
                                .get(&name.to_uppercase())
                                .copied()
                                .ok_or_else(|| {
                                    PyValueError::new_err(format!("undefined owner block: {name}"))
                                })?,
                        )
                    }
                }
            };
            match store {
                Entities::Streaming(encoder) => {
                    encoder.push(layout, entity).map_err(to_py_err)?;
                }
                Entities::Buffered { document, .. } => match layout {
                    LayoutId::Model => document.modelspace.push(entity),
                    LayoutId::Paper => document.paperspace.push(entity),
                    LayoutId::Block(index) => document.blocks[index].entities.push(entity),
                },
                Entities::Written => return Err(spent_writer()),
            }
        }
        if !owners.is_empty() {
            return Err(PyValueError::new_err("unknown entity/INSERT owner handle"));
        }
        Ok(())
    }
}

/// Attach the late ATTRIBs of a buffered document to their INSERTs.
fn attach_attributes(
    document: &mut writer::WriterDocument,
    mut pending: HashMap<u64, Vec<AttributeEntity>>,
) -> PyResult<()> {
    if pending.is_empty() {
        return Ok(());
    }
    let blocks = document
        .blocks
        .iter_mut()
        .flat_map(|b| b.entities.iter_mut());
    for entity in document
        .modelspace
        .iter_mut()
        .chain(document.paperspace.iter_mut())
        .chain(blocks)
    {
        if let writer::WriterEntity::Insert(insert) = entity {
            if let Some(attrs) = insert.common.handle.and_then(|h| pending.remove(&h)) {
                insert.attributes = attrs;
            }
        }
    }
    if !pending.is_empty() {
        return Err(PyValueError::new_err("unknown entity/INSERT owner handle"));
    }
    Ok(())
}

fn spent_writer() -> PyErr {
    PyValueError::new_err("DwgWriter has already written its document")
}

/// Replace the row with the same key, or append.
fn upsert<T>(rows: &mut Vec<T>, row: T, key: impl Fn(&T) -> String) {
    let name = key(&row);
    match rows.iter_mut().find(|r| key(r) == name) {
        Some(existing) => *existing = row,
        None => rows.push(row),
    }
}

/// Common properties of the raw tuple rows: layer `0`, ACI 7 (the attribute
/// dictionary overrides them).
fn basic_common(handle: u64) -> writer::CommonEntityProps {
    writer::CommonEntityProps {
        handle: (handle != 0).then_some(handle),
        layer_name: "0".to_string(),
        color_index: Some(7),
        true_color: None,
        ..writer::CommonEntityProps::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn build_basic_rows(
    groups: &mut [Vec<writer::WriterEntity>],
    lines: Vec<LineEntityRow>,
    arcs: Vec<ArcEntityRow>,
    circles: Vec<CircleEntityRow>,
    lwpolylines: Vec<LwPolylineEntityRow>,
    texts: Vec<TextWriteRow>,
    mtexts: Vec<MTextWriteRow>,
    points: Vec<PointWriteRow>,
    rays: Vec<RayEntityRow>,
    xlines: Vec<XLineEntityRow>,
    ellipses: Vec<EllipseEntityRow>,
    solids: Vec<SolidEntityRow>,
    traces: Vec<TraceEntityRow>,
    faces3d: Vec<Face3dEntityRow>,
) {
    use writer::WriterEntity;
    groups[0].extend(lines.into_iter().map(|(handle, sx, sy, sz, ex, ey, ez)| {
        WriterEntity::Line(writer::LineEntity {
            common: basic_common(handle),
            start: (sx, sy, sz),
            end: (ex, ey, ez),
        })
    }));
    groups[1].extend(arcs.into_iter().map(
        |(handle, cx, cy, cz, radius, angle_start, angle_end)| {
            WriterEntity::Arc(writer::ArcEntity {
                common: basic_common(handle),
                center: (cx, cy, cz),
                radius,
                angle_start_rad: angle_start,
                angle_end_rad: angle_end,
            })
        },
    ));
    groups[2].extend(circles.into_iter().map(|(handle, cx, cy, cz, radius)| {
        WriterEntity::Circle(writer::CircleEntity {
            common: basic_common(handle),
            center: (cx, cy, cz),
            radius,
        })
    }));
    groups[3].extend(lwpolylines.into_iter().map(
        |(handle, flags, points, bulges, widths, const_width)| {
            WriterEntity::LwPolyline(writer::LwPolylineEntity {
                common: basic_common(handle),
                flags,
                vertices: points,
                const_width,
                bulges,
                widths,
            })
        },
    ));
    groups[4].extend(
        texts
            .into_iter()
            .map(|(handle, text, insertion, height, rotation)| {
                WriterEntity::Text(writer::TextEntity {
                    common: basic_common(handle),
                    text,
                    insert: insertion,
                    height,
                    rotation_rad: rotation,
                })
            }),
    );
    groups[5].extend(mtexts.into_iter().map(
        |(
            handle,
            text,
            insertion,
            text_direction,
            rect_width,
            char_height,
            attachment_point,
            drawing_direction,
        )| {
            WriterEntity::MText(writer::MTextEntity {
                common: basic_common(handle),
                text,
                insert: insertion,
                text_direction,
                rect_width,
                char_height,
                attachment_point,
                drawing_direction,
            })
        },
    ));
    groups[6].extend(points.into_iter().map(|(handle, x, y, z, x_axis_angle)| {
        WriterEntity::Point(writer::PointEntity {
            common: basic_common(handle),
            location: (x, y, z),
            x_axis_angle,
        })
    }));
    groups[7].extend(rays.into_iter().map(|(handle, start, unit_vector)| {
        WriterEntity::Ray(writer::RayEntity {
            common: basic_common(handle),
            start,
            unit_vector,
        })
    }));
    groups[8].extend(xlines.into_iter().map(|(handle, start, unit_vector)| {
        WriterEntity::XLine(writer::XLineEntity {
            common: basic_common(handle),
            start,
            unit_vector,
        })
    }));
    groups[9].extend(ellipses.into_iter().map(
        |(handle, center, major_axis, extrusion, axis_ratio, start_param, end_param)| {
            WriterEntity::Ellipse(writer::EllipseEntity {
                common: writer::CommonEntityProps {
                    formatting: EntityFormatting {
                        extrusion,
                        ..Default::default()
                    },
                    ..basic_common(handle)
                },
                center,
                major_axis,
                axis_ratio,
                start_param,
                end_param,
            })
        },
    ));
    for (group, rows, is_trace) in [(10, solids, false), (11, traces, true)] {
        groups[group].extend(rows.into_iter().map(
            |(handle, p1, p2, p3, p4, thickness, extrusion)| {
                let face = writer::PlanarFaceEntity {
                    common: writer::CommonEntityProps {
                        formatting: EntityFormatting {
                            thickness,
                            extrusion,
                            ..Default::default()
                        },
                        ..basic_common(handle)
                    },
                    points: [p1, p2, p3, p4],
                };
                if is_trace {
                    WriterEntity::Trace(face)
                } else {
                    WriterEntity::Solid(face)
                }
            },
        ));
    }
    groups[12].extend(
        faces3d
            .into_iter()
            .map(|(handle, p1, p2, p3, p4, invisible_edge_flags)| {
                WriterEntity::Face3d(Box::new(writer::Face3dEntity {
                    common: basic_common(handle),
                    points: [p1, p2, p3, p4],
                    invisible_edge_flags,
                }))
            }),
    );
}

/// Flatten the groups in their legacy order, or follow `order` (one group tag
/// per entity) when the caller wants the drawing order preserved.
fn interleave(
    groups: Vec<Vec<writer::WriterEntity>>,
    order: Option<&[u8]>,
) -> PyResult<Vec<writer::WriterEntity>> {
    let total: usize = groups.iter().map(Vec::len).sum();
    let Some(order) = order else {
        return Ok(groups.into_iter().flatten().collect());
    };
    if order.len() != total {
        return Err(PyValueError::new_err(format!(
            "order lists {} entities but the rows hold {total}",
            order.len()
        )));
    }
    let mut queues: Vec<VecDeque<writer::WriterEntity>> =
        groups.into_iter().map(VecDeque::from).collect();
    let mut out = Vec::with_capacity(total);
    for &tag in order {
        let queue = queues.get_mut(usize::from(tag)).ok_or_else(|| {
            PyValueError::new_err(format!("order tag {tag} is not an entity group"))
        })?;
        out.push(queue.pop_front().ok_or_else(|| {
            PyValueError::new_err(format!(
                "order lists more {} than the rows hold",
                GROUP_NAMES[usize::from(tag)]
            ))
        })?);
    }
    Ok(out)
}

fn apply_entity_attributes(
    entities: &mut [writer::WriterEntity],
    attributes: Option<HashMap<u64, Bound<'_, PyDict>>>,
) -> PyResult<()> {
    let Some(mut attributes) = attributes else {
        return Ok(());
    };
    for entity in entities.iter_mut() {
        if let Some(values) = entity.common().handle.and_then(|h| attributes.remove(&h)) {
            read_write_attributes(entity.common_mut(), &values)?;
        }
    }
    if !attributes.is_empty() {
        return Err(PyValueError::new_err(
            "entity_attributes contains unknown/zero handles",
        ));
    }
    Ok(())
}

fn parse_layers(rows: &[Bound<'_, PyDict>]) -> PyResult<Vec<writer::LayerDef>> {
    rows.iter()
        .map(|d| {
            check_write_keys(
                d,
                &[
                    "name",
                    "color_index",
                    "true_color",
                    "linetype",
                    "lineweight",
                    "frozen",
                    "off",
                    "frozen_in_new_viewports",
                    "locked",
                    "plot",
                ],
            )?;
            Ok(writer::LayerDef {
                name: write_value(d, "name", "0".to_owned())?,
                color_index: write_value(d, "color_index", 7)?,
                true_color: write_value(d, "true_color", None)?,
                linetype: write_value(d, "linetype", "Continuous".to_owned())?,
                lineweight: write_value(d, "lineweight", -3)?,
                frozen: write_value(d, "frozen", false)?,
                off: write_value(d, "off", false)?,
                frozen_in_new_viewports: write_value(d, "frozen_in_new_viewports", false)?,
                locked: write_value(d, "locked", false)?,
                plot: write_value(d, "plot", true)?,
            })
        })
        .collect()
}

fn parse_linetypes(rows: &[Bound<'_, PyDict>]) -> PyResult<Vec<LinetypeDef>> {
    rows.iter()
        .map(|d| {
            check_write_keys(d, &["name", "description", "dashes"])?;
            Ok(LinetypeDef {
                name: write_value(d, "name", String::new())?,
                description: write_value(d, "description", String::new())?,
                dashes: write_value(d, "dashes", Vec::new())?,
            })
        })
        .collect()
}

fn parse_text_styles(rows: &[Bound<'_, PyDict>]) -> PyResult<Vec<writer::ir::TextStyleDef>> {
    rows.iter()
        .map(|d| {
            check_write_keys(
                d,
                &[
                    "name",
                    "vertical",
                    "height",
                    "width",
                    "oblique",
                    "generation",
                    "last_height",
                    "font",
                    "bigfont",
                ],
            )?;
            let st = writer::ir::TextStyleDef::default();
            Ok(writer::ir::TextStyleDef {
                name: write_value(d, "name", st.name)?,
                vertical: write_value(d, "vertical", st.vertical)?,
                height: write_value(d, "height", st.height)?,
                width: write_value(d, "width", st.width)?,
                oblique: write_value(d, "oblique", st.oblique)?,
                generation: write_value(d, "generation", st.generation)?,
                last_height: write_value(d, "last_height", st.last_height)?,
                font: write_value(d, "font", st.font)?,
                bigfont: write_value(d, "bigfont", st.bigfont)?,
            })
        })
        .collect()
}

fn apply_write_metadata(m: &mut writer::WriterMetadata, d: &Bound<'_, PyDict>) -> PyResult<()> {
    check_write_keys(
        d,
        &[
            "codepage", "insunits", "lunits", "luprec", "aunits", "auprec", "ltscale",
        ],
    )?;
    m.codepage = write_value(d, "codepage", m.codepage)?;
    m.insunits = write_value(d, "insunits", m.insunits)?;
    m.lunits = write_value(d, "lunits", m.lunits)?;
    m.luprec = write_value(d, "luprec", m.luprec)?;
    m.aunits = write_value(d, "aunits", m.aunits)?;
    m.auprec = write_value(d, "auprec", m.auprec)?;
    m.ltscale = write_value(d, "ltscale", m.ltscale)?;
    Ok(())
}

/// Return freed heap memory to the operating system.
///
/// glibc keeps freed chunks mapped for reuse, and CPython's own allocator
/// cannot reuse them, so a process that just dropped a large writer document
/// would otherwise keep its resident size. A no-op on other platforms.
#[pyfunction]
pub fn release_freed_heap() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        extern "C" {
            fn malloc_trim(pad: usize) -> std::os::raw::c_int;
        }
        // SAFETY: malloc_trim only releases memory the allocator already
        // considers free; it takes no pointers from us.
        unsafe {
            malloc_trim(0);
        }
    }
}

fn write_value<'py, T: FromPyObject<'py>>(
    d: &Bound<'py, PyDict>,
    name: &str,
    default: T,
) -> PyResult<T> {
    match d.get_item(name)? {
        Some(v) => v.extract(),
        None => Ok(default),
    }
}
fn check_write_keys(d: &Bound<'_, PyDict>, allowed: &[&str]) -> PyResult<()> {
    for key in d.keys() {
        let name: String = key.extract()?;
        if !allowed.contains(&name.as_str()) {
            return Err(PyValueError::new_err(format!(
                "unsupported writer attribute: {name}"
            )));
        }
    }
    Ok(())
}
fn read_write_attributes(c: &mut writer::CommonEntityProps, d: &Bound<'_, PyDict>) -> PyResult<()> {
    check_write_keys(
        d,
        &[
            "style",
            "layer",
            "color_index",
            "true_color",
            "linetype",
            "linetype_scale",
            "lineweight",
            "invisible",
            "extrusion",
            "thickness",
            "elevation",
            "align_point",
            "oblique",
            "width",
            "text_generation_flag",
            "halign",
            "valign",
            "line_spacing_style",
            "line_spacing_factor",
        ],
    )?;
    c.style_name = write_value(d, "style", c.style_name.clone())?;
    c.layer_name = write_value(d, "layer", c.layer_name.clone())?;
    c.color_index = write_value(d, "color_index", c.color_index)?;
    c.true_color = write_value(d, "true_color", None)?;
    c.linetype = write_value(d, "linetype", c.linetype.clone())?;
    // Optional display attributes must not reset geometry supplied in raw rows.
    let f = c.formatting;
    c.formatting = EntityFormatting {
        linetype_scale: write_value(d, "linetype_scale", f.linetype_scale)?,
        lineweight: write_value(d, "lineweight", f.lineweight)?,
        invisible: write_value(d, "invisible", f.invisible)?,
        extrusion: write_value(d, "extrusion", f.extrusion)?,
        thickness: write_value(d, "thickness", f.thickness)?,
        elevation: write_value(d, "elevation", f.elevation)?,
        alignment: write_value(d, "align_point", f.alignment)?,
        oblique: write_value::<f64>(d, "oblique", 0.)?.to_radians(),
        width_factor: write_value(d, "width", f.width_factor)?,
        generation: write_value(d, "text_generation_flag", f.generation)?,
        halign: write_value(d, "halign", f.halign)?,
        valign: write_value(d, "valign", f.valign)?,
        line_spacing_style: write_value(d, "line_spacing_style", f.line_spacing_style)?,
        line_spacing_factor: write_value(d, "line_spacing_factor", f.line_spacing_factor)?,
    };
    Ok(())
}
