//! Incremental document encoder.
//!
//! Entities arrive one at a time, for any layout and in any order, and are
//! encoded to object records as soon as their chain neighbour is known; only
//! the record bytes and a handle index stay in memory until [`DocumentEncoder::finish`]
//! writes the tables, header and object map. A drawing of a million entities
//! therefore costs about a hundred bytes per entity while it is written,
//! instead of a [`WriterDocument`](crate::writer::WriterDocument) of several
//! hundred bytes per entity plus every record.
//!
//! Fresh handles (tables, polyline vertices, SEQENDs, image reactors, the
//! paper viewport) are allocated above `handle_ceiling`, the largest input
//! handle the caller will pass, so they can be assigned before the last entity
//! has arrived.
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;

use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::dwg::version::DwgVersion;
use crate::objects::{Handle, ObjectRef};
use crate::writer::config::WriterConfig;
use crate::writer::ir::{
    AttributeEntity, BlockDef, ImageDef, InsertEntity, LayerDef, LayoutSettings, LinetypeDef,
    TextStyleDef, WriterDocument, WriterEntity, WriterMetadata,
};
use crate::writer::r2000::classes::{
    encode_classes_section, encode_unicode_classes_section, IMAGEDEF_REACTOR_CLASS, IMAGE_CLASS,
};
use crate::writer::r2000::database::{DatabaseHandles, DocumentSummary, LayoutContents};
use crate::writer::r2000::entities::{
    self, common::CommonEntityEncodeInput, encode_arc_entity_payload, encode_circle_entity_payload,
    encode_ellipse_entity_payload, encode_line_entity_payload, encode_lwpolyline_entity_payload,
    encode_mtext_entity_payload, encode_point_entity_payload, encode_ray_entity_payload,
    encode_text_entity_payload, encode_viewport_entity_payload, encode_xline_entity_payload,
    ArcEncodeInput, CircleEncodeInput, EllipseEncodeInput, LineEncodeInput, LwPolylineEncodeInput,
    MTextEncodeInput, PointEncodeInput, RayEncodeInput, TextEncodeInput, XLineEncodeInput,
};
use crate::writer::r2000::{encode_object_map_section, object_record};
use crate::writer::validation::{self, EntityContext, TextEncoding};
use crate::writer::HandleAllocator;

/// Everything of a document but its entities.
#[derive(Debug, Clone)]
pub struct DocumentTables {
    pub version: DwgVersion,
    pub layers: Vec<LayerDef>,
    pub linetypes: Vec<LinetypeDef>,
    pub text_styles: Vec<TextStyleDef>,
    /// Block definitions; their `entities` are ignored here (the contents are
    /// pushed as entities of [`LayoutId::Block`]).
    pub blocks: Vec<BlockDef>,
    /// Image definitions that entities may reference. Only the referenced ones
    /// are written.
    pub image_defs: Vec<ImageDef>,
    pub metadata: WriterMetadata,
    pub layout: LayoutSettings,
}

impl Default for DocumentTables {
    fn default() -> Self {
        let doc = WriterDocument::default();
        Self {
            version: doc.version,
            layers: doc.layers,
            linetypes: doc.linetypes,
            text_styles: doc.text_styles,
            blocks: Vec::new(),
            image_defs: doc.image_defs,
            metadata: doc.metadata,
            layout: doc.layout,
        }
    }
}

impl DocumentTables {
    /// The tables of `doc` (block definitions without their entities).
    pub fn from_document(doc: &WriterDocument) -> Self {
        Self {
            version: doc.version.clone(),
            layers: doc.layers.clone(),
            linetypes: doc.linetypes.clone(),
            text_styles: doc.text_styles.clone(),
            blocks: doc
                .blocks
                .iter()
                .map(|b| BlockDef {
                    name: b.name.clone(),
                    base_point: b.base_point,
                    description: b.description.clone(),
                    entities: Vec::new(),
                })
                .collect(),
            image_defs: doc.image_defs.clone(),
            metadata: doc.metadata.clone(),
            layout: doc.layout.clone(),
        }
    }
}

/// The layout an entity belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutId {
    Model,
    Paper,
    /// Index into [`DocumentTables::blocks`].
    Block(usize),
}

impl LayoutId {
    /// Position in the writer's layout order: model space, paper space, blocks.
    pub fn index(self) -> usize {
        match self {
            Self::Model => 0,
            Self::Paper => WriterDocument::PAPER_LAYOUT,
            Self::Block(i) => WriterDocument::FIRST_BLOCK_LAYOUT + i,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Model,
            WriterDocument::PAPER_LAYOUT => Self::Paper,
            i => Self::Block(i - WriterDocument::FIRST_BLOCK_LAYOUT),
        }
    }
}

/// Input handles seen so far (duplicate detection): a bitmap for the usual
/// dense handle ranges, a hash set for sparse ones.
enum HandleSet {
    Dense(Vec<u64>),
    Sparse(HashSet<u64>),
}

impl HandleSet {
    const DENSE_LIMIT: u64 = 1 << 26;

    fn new(ceiling: u64) -> Self {
        if ceiling <= Self::DENSE_LIMIT {
            Self::Dense(vec![0; (ceiling as usize).div_ceil(64) + 1])
        } else {
            Self::Sparse(HashSet::new())
        }
    }

    /// `false` when the handle was already present.
    fn insert(&mut self, handle: u64) -> bool {
        match self {
            Self::Dense(bits) => {
                let (word, bit) = ((handle / 64) as usize, handle % 64);
                if word >= bits.len() {
                    bits.resize(word + 1, 0);
                }
                let present = bits[word] & (1 << bit) != 0;
                bits[word] |= 1 << bit;
                !present
            }
            Self::Sparse(set) => set.insert(handle),
        }
    }
}

/// Encoded object records in arrival order, in fixed pages so that growing
/// never copies what was already encoded.
struct RecordArena {
    pages: Vec<Vec<u8>>,
    len: u64,
}

impl RecordArena {
    const PAGE: usize = 1 << 24;

    fn new() -> Self {
        Self {
            pages: Vec::new(),
            len: 0,
        }
    }

    fn append(&mut self, record: &[u8]) -> Result<u64> {
        let needs_page = match self.pages.last() {
            Some(page) => page.len() + record.len() > Self::PAGE,
            None => true,
        };
        if needs_page {
            if record.len() > Self::PAGE {
                return Err(DwgError::new(
                    ErrorKind::Unsupported,
                    format!("object record too large: {} bytes", record.len()),
                ));
            }
            let mut page = Vec::with_capacity(Self::PAGE);
            page.reserve_exact(Self::PAGE);
            self.pages.push(page);
        }
        let page_index = self.pages.len() - 1;
        let page = &mut self.pages[page_index];
        let offset = (page_index as u64) * Self::PAGE as u64 + page.len() as u64;
        page.extend_from_slice(record);
        self.len += record.len() as u64;
        Ok(offset)
    }

    fn get(&self, offset: u64, len: u32) -> &[u8] {
        let page = &self.pages[(offset / Self::PAGE as u64) as usize];
        let start = (offset % Self::PAGE as u64) as usize;
        &page[start..start + len as usize]
    }
}

#[derive(Clone, Copy)]
struct RecordRef {
    handle: u64,
    offset: u64,
    len: u32,
}

/// An entity whose record waits for the handle of its successor.
struct Pending {
    entity: WriterEntity,
    handle: u64,
    previous: u64,
}

/// An INSERT waits until `finish`, when every ATTRIB has arrived.
struct DeferredInsert {
    insert: Box<InsertEntity>,
    layout: usize,
    handle: u64,
    previous: u64,
    next: u64,
}

struct LayoutState {
    owner: u64,
    entity_mode: u8,
    pending: Option<Pending>,
    contents: LayoutContents,
}

/// The encoder; see the module documentation.
pub struct DocumentEncoder {
    tables: DocumentTables,
    config: WriterConfig,
    modern: bool,
    encoding: TextEncoding,
    ceiling: u64,
    allocator: HandleAllocator,
    seen: HandleSet,
    database: DatabaseHandles,
    records: RecordArena,
    index: Vec<RecordRef>,
    layouts: Vec<LayoutState>,
    inserts: Vec<DeferredInsert>,
    attributes: HashMap<u64, Vec<AttributeEntity>>,
    block_index: HashMap<String, usize>,
    layer_names: HashSet<String>,
    image_def_names: HashSet<String>,
    /// INSERT and DIMENSION handles referencing each block, in arrival order.
    block_refs: Vec<Vec<u64>>,
    /// Block → blocks it references (INSERT/DIMENSION inside a definition).
    block_edges: Vec<Vec<usize>>,
    /// IMAGEDEF_REACTOR handles per referenced image definition (upper-case name).
    image_reactors: BTreeMap<String, Vec<u64>>,
    used_image_defs: HashSet<String>,
    paper_viewport: u64,
    /// First paper-space entity; the paper viewport record that leads the
    /// chain is written at `finish`, when the sheet is final.
    paper_first: u64,
    max_handle: u64,
}

impl DocumentEncoder {
    /// Validate the tables and allocate their handles above `handle_ceiling`,
    /// the largest entity handle that will be pushed (ignored when the
    /// configuration does not preserve input handles).
    pub fn new(tables: DocumentTables, config: WriterConfig, handle_ceiling: u64) -> Result<Self> {
        validation::validate_tables(&tables)?;
        let modern = matches!(tables.version, DwgVersion::R2018);
        let encoding = TextEncoding::new(tables.metadata.codepage, modern);
        let ceiling = if config.preserve_input_handles {
            handle_ceiling
        } else {
            0
        };
        let mut allocator = HandleAllocator::new(ceiling.saturating_add(1));
        let database = DatabaseHandles::allocate(&mut allocator, &tables)?;
        let mut layouts = vec![
            LayoutState {
                owner: database.model,
                entity_mode: 0,
                pending: None,
                contents: LayoutContents::new(modern),
            },
            LayoutState {
                owner: database.paper,
                entity_mode: 1,
                pending: None,
                contents: LayoutContents::new(modern),
            },
        ];
        for block in &tables.blocks {
            layouts.push(LayoutState {
                owner: database.blocks[&block.name.to_uppercase()].0,
                entity_mode: 0,
                pending: None,
                contents: LayoutContents::new(modern),
            });
        }
        let block_index = tables
            .blocks
            .iter()
            .enumerate()
            .map(|(i, b)| (b.name.to_uppercase(), i))
            .collect();
        let layer_names = tables
            .layers
            .iter()
            .map(|l| l.name.to_uppercase())
            .chain(std::iter::once("0".to_string()))
            .collect();
        let image_def_names = tables
            .image_defs
            .iter()
            .map(|d| d.name.to_uppercase())
            .collect();
        Ok(Self {
            seen: HandleSet::new(ceiling),
            block_refs: vec![Vec::new(); tables.blocks.len()],
            block_edges: vec![Vec::new(); tables.blocks.len()],
            tables,
            config,
            modern,
            encoding,
            ceiling,
            allocator,
            database,
            records: RecordArena::new(),
            index: Vec::new(),
            layouts,
            inserts: Vec::new(),
            attributes: HashMap::new(),
            block_index,
            layer_names,
            image_def_names,
            image_reactors: BTreeMap::new(),
            used_image_defs: HashSet::new(),
            paper_viewport: 0,
            paper_first: 0,
            max_handle: 0,
        })
    }

    /// Add a layer, or replace the one with the same (case-insensitive) name;
    /// its record is written at `finish`, so this may follow entities.
    pub fn add_layer(&mut self, layer: LayerDef) -> Result<()> {
        validation::validate_layer(&layer, self.encoding)?;
        let key = layer.name.to_uppercase();
        if let Some(existing) = self
            .tables
            .layers
            .iter_mut()
            .find(|l| l.name.to_uppercase() == key)
        {
            *existing = layer;
            return Ok(());
        }
        if key != "0" {
            let handle = self.allocator.allocate()?;
            self.database.layers.insert(key.clone(), handle);
        }
        self.layer_names.insert(key);
        self.tables.layers.push(layer);
        Ok(())
    }

    /// Add or replace a linetype (see [`add_layer`](Self::add_layer)).
    pub fn add_linetype(&mut self, linetype: LinetypeDef) -> Result<()> {
        validation::validate_linetype(&linetype, self.encoding)?;
        let key = linetype.name.to_uppercase();
        if let Some(existing) = self
            .tables
            .linetypes
            .iter_mut()
            .find(|l| l.name.to_uppercase() == key)
        {
            *existing = linetype;
            return Ok(());
        }
        let handle = self.allocator.allocate()?;
        self.database.linetypes.insert(key, handle);
        self.tables.linetypes.push(linetype);
        Ok(())
    }

    /// Add or replace a text style (see [`add_layer`](Self::add_layer)).
    pub fn add_text_style(&mut self, style: TextStyleDef) -> Result<()> {
        validation::validate_text_style(&style, self.encoding)?;
        let key = style.name.to_uppercase();
        if let Some(existing) = self
            .tables
            .text_styles
            .iter_mut()
            .find(|st| st.name.to_uppercase() == key)
        {
            *existing = style;
            return Ok(());
        }
        if key != "STANDARD" {
            let handle = self.allocator.allocate()?;
            self.database.styles.insert(key, handle);
        }
        self.tables.text_styles.push(style);
        Ok(())
    }

    /// Add a block definition and return its index for [`LayoutId::Block`];
    /// its entities are pushed afterwards. Block names cannot be reused.
    pub fn add_block(&mut self, block: BlockDef) -> Result<usize> {
        validation::validate_block(&block, self.encoding)?;
        let key = block.name.to_uppercase();
        if self.block_index.contains_key(&key) {
            return Err(DwgError::new(ErrorKind::Format, "duplicate block name"));
        }
        let handles = (
            self.allocator.allocate()?,
            self.allocator.allocate()?,
            self.allocator.allocate()?,
        );
        self.database.blocks.insert(key.clone(), handles);
        self.layouts.push(LayoutState {
            owner: handles.0,
            entity_mode: 0,
            pending: None,
            contents: LayoutContents::new(self.modern),
        });
        let index = self.tables.blocks.len();
        self.block_index.insert(key, index);
        self.block_refs.push(Vec::new());
        self.block_edges.push(Vec::new());
        self.tables.blocks.push(BlockDef {
            entities: Vec::new(),
            ..block
        });
        Ok(index)
    }

    /// Add or replace an image definition (see [`add_layer`](Self::add_layer)).
    pub fn add_image_def(&mut self, def: ImageDef) -> Result<()> {
        if self.modern {
            return Err(DwgError::new(
                ErrorKind::Unsupported,
                "AC1032 writer does not support image definitions",
            ));
        }
        validation::validate_image_def(&def, self.encoding)?;
        let key = def.name.to_uppercase();
        if let Some(existing) = self
            .tables
            .image_defs
            .iter_mut()
            .find(|d| d.name.to_uppercase() == key)
        {
            *existing = def;
            return Ok(());
        }
        if self.database.image_dict == 0 {
            self.database.image_dict = self.allocator.allocate()?;
            self.database.image_vars = self.allocator.allocate()?;
        }
        let handle = self.allocator.allocate()?;
        self.database.image_defs.insert(key.clone(), handle);
        self.image_def_names.insert(key);
        self.tables.image_defs.push(def);
        Ok(())
    }

    /// Replace the paper-space layout settings; they are used at `finish`.
    pub fn set_layout(&mut self, layout: LayoutSettings) -> Result<()> {
        validation::validate_layout_settings(&layout, self.encoding)?;
        self.tables.layout = layout;
        Ok(())
    }

    /// Replace the header metadata. The code page cannot change once entities
    /// have been encoded with it.
    pub fn set_metadata(&mut self, metadata: WriterMetadata) -> Result<()> {
        if metadata.codepage != self.tables.metadata.codepage {
            return Err(DwgError::new(
                ErrorKind::Format,
                "the code page is fixed once the encoder has started",
            ));
        }
        validation::validate_metadata(&metadata)?;
        self.tables.metadata = metadata;
        Ok(())
    }

    /// Index of the block definition `name` (case-insensitive).
    pub fn block_index(&self, name: &str) -> Option<usize> {
        self.block_index.get(&name.to_uppercase()).copied()
    }

    /// The header metadata the document is being encoded with.
    pub fn metadata(&self) -> &WriterMetadata {
        &self.tables.metadata
    }

    /// The paper-space layout settings that `finish` will write.
    pub fn layout(&self) -> &LayoutSettings {
        &self.tables.layout
    }

    /// Append `entity` to `layout`. It is validated now; its record is
    /// written once the next entity of the layout (or `finish`) fixes the
    /// chain. An INSERT waits for `finish`, so its ATTRIBs may still arrive
    /// through [`push_attribute`](Self::push_attribute).
    pub fn push(&mut self, layout: LayoutId, entity: WriterEntity) -> Result<()> {
        let index = layout.index();
        if index >= self.layouts.len() {
            return Err(DwgError::new(
                ErrorKind::Format,
                format!("undefined block layout {index}"),
            ));
        }
        let context = EntityContext {
            blocks: &self.block_index,
            layer_names: &self.layer_names,
            image_defs: &self.image_def_names,
            is_paper: layout == LayoutId::Paper,
            encoding: self.encoding,
        };
        let target = validation::validate_layout_entity(&entity, &context)?;
        let handle = self.input_handle(entity.common().handle)?;
        if let Some(target) = target {
            self.block_refs[target].push(handle);
            if let LayoutId::Block(source) = layout {
                self.block_edges[source].push(target);
            }
        }
        if let WriterEntity::Image(image) = &entity {
            self.used_image_defs.insert(image.image_def.to_uppercase());
        }
        if matches!(entity, WriterEntity::Attdef(_)) {
            self.layouts[index].contents.has_atts = true;
        }
        let previous = match self.layouts[index].pending.take() {
            Some(pending) => {
                let previous = pending.handle;
                self.encode_pending(index, pending, handle)?;
                previous
            }
            None if layout == LayoutId::Paper => {
                self.paper_viewport = self.allocator.allocate()?;
                self.paper_first = handle;
                self.paper_viewport
            }
            None => 0,
        };
        self.layouts[index].contents.push(handle);
        self.layouts[index].pending = Some(Pending {
            entity,
            handle,
            previous,
        });
        Ok(())
    }

    /// Attach an ATTRIB to the INSERT with handle `insert`, which may arrive
    /// before or after it.
    pub fn push_attribute(&mut self, insert: u64, attribute: AttributeEntity) {
        self.attributes.entry(insert).or_default().push(attribute);
    }

    /// Write the pending entities, the deferred INSERTs with their ATTRIBs,
    /// the tables and the header, and lay the records out by handle.
    pub fn finish(mut self) -> Result<EncodedDocument> {
        for index in 0..self.layouts.len() {
            if let Some(pending) = self.layouts[index].pending.take() {
                self.encode_pending(index, pending, 0)?;
            }
        }
        if self.paper_viewport != 0 {
            self.paper_viewport_record()?;
        }
        let inserts = std::mem::take(&mut self.inserts);
        for deferred in inserts {
            self.encode_insert(deferred)?;
        }
        if let Some(handle) = self.attributes.keys().next() {
            return Err(DwgError::new(
                ErrorKind::Format,
                format!("unknown entity/INSERT owner handle {handle}"),
            ));
        }
        validation::check_block_cycles(&self.block_edges)?;

        let layouts: Vec<LayoutContents> =
            self.layouts.iter().map(|l| l.contents.clone()).collect();
        let summary = DocumentSummary {
            tables: &self.tables,
            modern: self.modern,
            layouts: &layouts,
            block_refs: &self.block_refs,
            image_reactors: &self.image_reactors,
            used_image_defs: &self.used_image_defs,
            paper_viewport: self.paper_viewport,
        };
        for (handle, payload) in self.database.objects(&summary)? {
            self.push_record(handle, &payload)?;
        }
        let handseed = self
            .max_handle
            .checked_add(1)
            .ok_or_else(|| DwgError::new(ErrorKind::Unsupported, "handle space exhausted"))?;
        self.database.paper_viewport = self.paper_viewport;
        let header = if self.modern {
            crate::writer::r2018::encode_header(
                &self.database,
                handseed,
                &self.tables.metadata,
                &self.tables.layout,
            )?
        } else {
            crate::writer::r2000::header::encode_header(
                &self.database,
                handseed,
                &self.tables.metadata,
                &self.tables.layout,
            )?
        };
        let classes = if self.modern {
            encode_unicode_classes_section()?
        } else {
            encode_classes_section(!self.used_image_defs.is_empty())?
        };
        let mut index = std::mem::take(&mut self.index);
        index.sort_by_key(|r| r.handle);
        Ok(EncodedDocument {
            modern: self.modern,
            codepage: self.tables.metadata.codepage,
            header,
            classes,
            records: self.records,
            index,
        })
    }

    fn input_handle(&mut self, handle: Option<u64>) -> Result<u64> {
        match handle.filter(|_| self.config.preserve_input_handles) {
            Some(h) => {
                if h == 0 {
                    return Err(DwgError::new(
                        ErrorKind::Format,
                        "handle 0 is reserved and cannot be allocated",
                    ));
                }
                if h > self.ceiling {
                    return Err(DwgError::new(
                        ErrorKind::Format,
                        format!("handle {h} exceeds the declared maximum {}", self.ceiling),
                    ));
                }
                if !self.seen.insert(h) {
                    return Err(DwgError::new(
                        ErrorKind::Resolve,
                        format!("duplicate handle reservation: {h}"),
                    ));
                }
                Ok(h)
            }
            None => self.allocator.allocate(),
        }
    }

    fn push_record(&mut self, handle: u64, payload: &[u8]) -> Result<()> {
        let record = if self.modern {
            crate::writer::r2018::streams::record(payload)?
        } else {
            object_record::encode_object_record(payload)?
        };
        let offset = self.records.append(&record)?;
        self.index.push(RecordRef {
            handle,
            offset,
            len: u32::try_from(record.len())
                .map_err(|_| DwgError::new(ErrorKind::Unsupported, "object record too large"))?,
        });
        self.max_handle = self.max_handle.max(handle);
        Ok(())
    }

    fn common(
        &self,
        layout: usize,
        handle: u64,
        previous: u64,
        next: u64,
        props: &crate::writer::ir::CommonEntityProps,
    ) -> Result<CommonEntityEncodeInput> {
        let state = &self.layouts[layout];
        Ok(CommonEntityEncodeInput {
            handle,
            owner_handle: state.owner,
            entity_mode: state.entity_mode,
            layer_handle: self.database.layer_handle(&props.layer_name)?,
            previous_handle: previous,
            next_handle: next,
            style_handle: self.database.style_handle(&props.style_name)?,
            color_index: props.color_index.unwrap_or(256),
            linetype_flags: linetype_flags(&props.linetype),
            linetype_handle: self.database.linetype_handle(&props.linetype)?,
            codepage: self.tables.metadata.codepage,
            modern: self.modern,
            true_color: props.true_color,
            formatting: props.formatting,
        })
    }

    /// The overall paper viewport leads the paper-space chain (its successor
    /// is the first paper-space entity). Written at `finish`, when the sheet
    /// settings are final.
    fn paper_viewport_record(&mut self) -> Result<()> {
        let handle = self.paper_viewport;
        let common = CommonEntityEncodeInput {
            handle,
            owner_handle: self.database.paper,
            entity_mode: 1,
            layer_handle: self.database.layer,
            previous_handle: 0,
            next_handle: self.paper_first,
            style_handle: 0,
            color_index: 256,
            linetype_flags: 0,
            linetype_handle: 0,
            codepage: self.tables.metadata.codepage,
            modern: self.modern,
            true_color: None,
            formatting: Default::default(),
        };
        let payload =
            encode_viewport_entity_payload(common, &self.tables.layout.paper_viewport(), &[])?;
        self.push_record(handle, &payload)
    }

    fn encode_pending(&mut self, layout: usize, pending: Pending, next: u64) -> Result<()> {
        let Pending {
            entity,
            handle,
            previous,
        } = pending;
        let common = self.common(layout, handle, previous, next, entity.common())?;
        match entity {
            WriterEntity::Line(line) => {
                let payload = encode_line_entity_payload(LineEncodeInput {
                    common,
                    start: line.start,
                    end: line.end,
                })?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Point(point) => {
                let payload = encode_point_entity_payload(PointEncodeInput {
                    common,
                    location: point.location,
                    x_axis_angle: point.x_axis_angle,
                })?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Ray(ray) => {
                let payload = encode_ray_entity_payload(RayEncodeInput {
                    common,
                    start: ray.start,
                    unit_vector: ray.unit_vector,
                })?;
                self.push_record(handle, &payload)
            }
            WriterEntity::XLine(xline) => {
                let payload = encode_xline_entity_payload(XLineEncodeInput {
                    common,
                    start: xline.start,
                    unit_vector: xline.unit_vector,
                })?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Arc(arc) => {
                let payload = encode_arc_entity_payload(ArcEncodeInput {
                    common,
                    center: arc.center,
                    radius: arc.radius,
                    angle_start: arc.angle_start_rad,
                    angle_end: arc.angle_end_rad,
                })?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Circle(circle) => {
                let payload = encode_circle_entity_payload(CircleEncodeInput {
                    common,
                    center: circle.center,
                    radius: circle.radius,
                })?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Ellipse(ellipse) => {
                let payload = encode_ellipse_entity_payload(EllipseEncodeInput {
                    common,
                    center: ellipse.center,
                    major_axis: ellipse.major_axis,
                    axis_ratio: ellipse.axis_ratio,
                    start_param: ellipse.start_param,
                    end_param: ellipse.end_param,
                })?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Solid(face) => {
                let payload =
                    entities::face::encode_planar_face_entity_payload(common, face.points, false)?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Trace(face) => {
                let payload =
                    entities::face::encode_planar_face_entity_payload(common, face.points, true)?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Spline(spline) => {
                let payload = entities::spline::encode_spline_entity_payload(common, &spline)?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Face3d(face) => {
                let payload = entities::face::encode_3dface_entity_payload(
                    common,
                    face.points,
                    face.invisible_edge_flags,
                )?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Polyline2d(poly) => self.encode_polyline(handle, common, &poly, false),
            WriterEntity::Polyline3d(poly) => self.encode_polyline(handle, common, &poly, true),
            WriterEntity::LwPolyline(poly) => {
                let payload = encode_lwpolyline_entity_payload(LwPolylineEncodeInput {
                    common,
                    flags: poly.flags,
                    vertices: poly.vertices,
                    const_width: poly.const_width,
                    bulges: poly.bulges,
                    widths: poly.widths,
                })?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Text(text) => {
                let payload = encode_text_entity_payload(&TextEncodeInput {
                    common,
                    text: text.text,
                    insertion: text.insert,
                    height: text.height,
                    rotation: text.rotation_rad,
                })?;
                self.push_record(handle, &payload)
            }
            WriterEntity::MText(mtext) => {
                let payload = encode_mtext_entity_payload(&MTextEncodeInput {
                    common,
                    text: mtext.text,
                    insertion: mtext.insert,
                    text_direction: mtext.text_direction,
                    rect_width: mtext.rect_width,
                    text_height: mtext.char_height,
                    attachment: mtext.attachment_point,
                    drawing_dir: mtext.drawing_direction,
                })?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Dimension(dim) => {
                let block = self.database.blocks[&dim.block_name.to_uppercase()].0;
                let payload = entities::dimension::encode_dimension_entity_payload(
                    common,
                    &dim,
                    self.database.dimstyle,
                    block,
                )?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Image(image) => {
                let reactor = self.allocator.allocate()?;
                let def = self.database.image_def_handle(&image.image_def)?;
                let payload = entities::image::encode_image_entity_payload(
                    IMAGE_CLASS,
                    common,
                    &image,
                    def,
                    reactor,
                )?;
                self.push_record(handle, &payload)?;
                let reactor_payload = entities::image::encode_image_def_reactor_object(
                    self.tables.metadata.codepage,
                    IMAGEDEF_REACTOR_CLASS,
                    reactor,
                    handle,
                )?;
                self.push_record(reactor, &reactor_payload)?;
                self.image_reactors
                    .entry(image.image_def.to_uppercase())
                    .or_default()
                    .push(reactor);
                Ok(())
            }
            WriterEntity::Viewport(viewport) => {
                let frozen = viewport
                    .frozen_layers
                    .iter()
                    .map(|name| self.database.layer_handle(name))
                    .collect::<Result<Vec<_>>>()?;
                let payload = encode_viewport_entity_payload(common, &viewport, &frozen)?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Hatch(hatch) => {
                let payload = entities::hatch::encode_hatch_entity_payload(common, &hatch)?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Attdef(attr) => {
                let payload = entities::block::encode_attribute(common, &attr, true)?;
                self.push_record(handle, &payload)
            }
            WriterEntity::Attrib(_) => Err(DwgError::new(
                ErrorKind::Format,
                "ATTRIB must belong to INSERT",
            )),
            WriterEntity::Insert(insert) => {
                self.inserts.push(DeferredInsert {
                    insert,
                    layout,
                    handle,
                    previous,
                    next,
                });
                Ok(())
            }
        }
    }

    fn encode_polyline(
        &mut self,
        handle: u64,
        common: CommonEntityEncodeInput,
        poly: &crate::writer::ir::PolylineEntity,
        is_3d: bool,
    ) -> Result<()> {
        let vertices = poly
            .vertices
            .iter()
            .map(|_| self.allocator.allocate())
            .collect::<Result<Vec<_>>>()?;
        let seqend = self.allocator.allocate()?;
        let payload = entities::polyline::encode_polyline(common, poly, is_3d, &vertices, seqend)?;
        self.push_record(handle, &payload)?;
        for (j, vertex) in poly.vertices.iter().enumerate() {
            let child = CommonEntityEncodeInput {
                handle: vertices[j],
                owner_handle: handle,
                entity_mode: 0,
                previous_handle: j.checked_sub(1).map(|k| vertices[k]).unwrap_or(0),
                next_handle: vertices.get(j + 1).copied().unwrap_or(0),
                color_index: 256,
                linetype_flags: 0,
                linetype_handle: 0,
                formatting: Default::default(),
                ..common
            };
            let payload = entities::polyline::encode_vertex(child, vertex, is_3d)?;
            self.push_record(vertices[j], &payload)?;
        }
        let child = CommonEntityEncodeInput {
            handle: seqend,
            owner_handle: handle,
            entity_mode: 0,
            previous_handle: 0,
            next_handle: 0,
            color_index: 256,
            linetype_flags: 0,
            linetype_handle: 0,
            formatting: Default::default(),
            ..common
        };
        let payload = entities::common::encode_entity_payload(6, child, |_| Ok(()))?;
        self.push_record(seqend, &payload)
    }

    fn encode_insert(&mut self, deferred: DeferredInsert) -> Result<()> {
        let DeferredInsert {
            mut insert,
            layout,
            handle,
            previous,
            next,
        } = deferred;
        if let Some(late) = self.attributes.remove(&handle) {
            insert.attributes.extend(late);
        }
        validation::validate_insert_attributes(&insert, self.encoding)?;
        let common = self.common(layout, handle, previous, next, &insert.common)?;
        let attrs = insert
            .attributes
            .iter()
            .map(|a| self.input_handle(a.text.common.handle))
            .collect::<Result<Vec<_>>>()?;
        let seqend = if attrs.is_empty() {
            0
        } else {
            self.allocator.allocate()?
        };
        let block = self.database.blocks[&insert.block_name.to_uppercase()].0;
        let payload = entities::block::encode_insert(common, &insert, block, &attrs, seqend)?;
        self.push_record(handle, &payload)?;
        for (i, attr) in insert.attributes.iter().enumerate() {
            let c = &attr.text.common;
            let child = CommonEntityEncodeInput {
                handle: attrs[i],
                owner_handle: handle,
                entity_mode: 0,
                previous_handle: i.checked_sub(1).map(|j| attrs[j]).unwrap_or(0),
                next_handle: attrs.get(i + 1).copied().unwrap_or(0),
                layer_handle: self.database.layer_handle(&c.layer_name)?,
                color_index: c.color_index.unwrap_or(256),
                true_color: c.true_color,
                style_handle: self.database.style_handle(&c.style_name)?,
                linetype_flags: linetype_flags(&c.linetype),
                linetype_handle: self.database.linetype_handle(&c.linetype)?,
                formatting: c.formatting,
                ..common
            };
            let payload = entities::block::encode_attribute(child, attr, false)?;
            self.push_record(attrs[i], &payload)?;
        }
        if seqend != 0 {
            let child = CommonEntityEncodeInput {
                handle: seqend,
                owner_handle: handle,
                entity_mode: 0,
                previous_handle: 0,
                next_handle: 0,
                layer_handle: self.database.layer,
                formatting: Default::default(),
                linetype_flags: 0,
                color_index: 256,
                ..common
            };
            let payload = entities::common::encode_entity_payload(6, child, |_| Ok(()))?;
            self.push_record(seqend, &payload)?;
        }
        Ok(())
    }
}

fn linetype_flags(linetype: &str) -> u8 {
    match linetype.to_uppercase().as_str() {
        "" | "BYLAYER" => 0,
        "BYBLOCK" => 1,
        "CONTINUOUS" => 2,
        _ => 3,
    }
}

const SECTION_DIRECTORY_SENTINEL: [u8; 16] = [
    0x95, 0xA0, 0x4E, 0x28, 0x99, 0x82, 0x1A, 0xE5, 0x5E, 0x41, 0xE0, 0x5F, 0x9D, 0x3A, 0x4D, 0x00,
];

/// The encoded sections of a document, ready to be laid out in a file.
pub struct EncodedDocument {
    modern: bool,
    codepage: u16,
    header: Vec<u8>,
    classes: Vec<u8>,
    records: RecordArena,
    /// Records sorted by handle.
    index: Vec<RecordRef>,
}

impl EncodedDocument {
    /// The complete file.
    pub fn into_bytes(self) -> Result<Vec<u8>> {
        if self.modern {
            return self.container();
        }
        let mut bytes = Vec::with_capacity(self.r2000_layout()?.total);
        self.write_to(&mut bytes)?;
        Ok(bytes)
    }

    /// Write the complete file to `out` (AC1015 is streamed section by
    /// section; AC1032 is assembled in memory first).
    pub fn write_to<W: Write>(&self, out: &mut W) -> Result<()> {
        if self.modern {
            let bytes = self.container()?;
            return out
                .write_all(&bytes)
                .map_err(|e| DwgError::new(ErrorKind::Format, e.to_string()));
        }
        let layout = self.r2000_layout()?;
        let mut written = 0usize;
        let mut emit = |out: &mut W, at: usize, bytes: &[u8]| -> Result<()> {
            if at < written {
                return Err(DwgError::new(ErrorKind::Format, "section offset overflow"));
            }
            let pad = vec![0u8; at - written];
            out.write_all(&pad)
                .and_then(|_| out.write_all(bytes))
                .map_err(|e| DwgError::new(ErrorKind::Format, e.to_string()))?;
            written = at + bytes.len();
            Ok(())
        };
        emit(out, 0, &layout.directory)?;
        emit(out, layout.header_offset, &self.header)?;
        emit(out, layout.classes_offset, &self.classes)?;
        for (record, reference) in self.index.iter().zip(&layout.refs) {
            emit(
                out,
                reference.offset as usize,
                self.records.get(record.offset, record.len),
            )?;
        }
        emit(out, layout.map_offset, &layout.map)?;
        if written < layout.total {
            let pad = vec![0u8; layout.total - written];
            out.write_all(&pad)
                .map_err(|e| DwgError::new(ErrorKind::Format, e.to_string()))?;
        }
        Ok(())
    }

    fn r2000_layout(&self) -> Result<R2000Layout> {
        let record_count = 3usize;
        let directory_size =
            0x15usize + 4 + record_count * 9 + 2 + SECTION_DIRECTORY_SENTINEL.len();
        let mut cursor = align_up(directory_size, 4);
        let header_offset = cursor;
        cursor += self.header.len();
        let classes_offset = cursor;
        cursor = cursor.saturating_add(self.classes.len() + 0x200); // R13C3+ padding
        cursor = align_up(cursor, 4);
        let too_large = || DwgError::new(ErrorKind::Unsupported, "AC1015 file exceeds 4 GiB");
        let mut refs = Vec::with_capacity(self.index.len());
        for record in &self.index {
            refs.push(ObjectRef {
                handle: Handle(record.handle),
                offset: u32::try_from(cursor).map_err(|_| too_large())?,
            });
            cursor = cursor.saturating_add(record.len as usize);
        }
        cursor = align_up(cursor, 4);
        let map = encode_object_map_section(&refs)?;
        let map_offset = cursor;
        cursor = cursor.saturating_add(map.len());
        u32::try_from(cursor).map_err(|_| too_large())?;

        let mut directory = vec![0u8; align_up(directory_size, 4)];
        directory[0..6].copy_from_slice(b"AC1015");
        directory[0x0C] = 1;
        directory[0x11] = 0x17;
        directory[0x12] = 0x0F;
        write_u16_le(&mut directory, 0x13, self.codepage);
        write_u32_le(&mut directory, 0x15, record_count as u32);
        let mut entry_off = 0x15usize + 4;
        for (record_no, offset, size) in [
            (0u8, header_offset, self.header.len()),
            (1, classes_offset, self.classes.len()),
            (2, map_offset, map.len()),
        ] {
            directory[entry_off] = record_no;
            write_u32_le(&mut directory, entry_off + 1, offset as u32);
            write_u32_le(&mut directory, entry_off + 5, size as u32);
            entry_off += 9;
        }
        let crc = crate::writer::r2000::checksum::crc16(0, &directory[..entry_off]) ^ 0xA598;
        write_u16_le(&mut directory, entry_off, crc);
        entry_off += 2;
        directory[entry_off..entry_off + SECTION_DIRECTORY_SENTINEL.len()]
            .copy_from_slice(&SECTION_DIRECTORY_SENTINEL);
        Ok(R2000Layout {
            directory,
            header_offset,
            classes_offset,
            refs,
            map,
            map_offset,
            total: cursor,
        })
    }

    fn container(&self) -> Result<Vec<u8>> {
        let mut objects = 0x0dcau32.to_le_bytes().to_vec();
        let mut refs = Vec::with_capacity(self.index.len());
        for record in &self.index {
            refs.push(ObjectRef {
                handle: Handle(record.handle),
                offset: u32::try_from(objects.len()).map_err(|_| {
                    DwgError::new(ErrorKind::Unsupported, "AC1032 objects exceed 4 GiB")
                })?,
            });
            objects.extend_from_slice(self.records.get(record.offset, record.len));
        }
        objects.extend_from_slice(&[0, 0]);
        crate::writer::r2018::write_container(vec![
            ("AcDb:Header", self.header.clone()),
            ("AcDb:Classes", self.classes.clone()),
            ("AcDb:AcDbObjects", objects),
            ("AcDb:Handles", encode_object_map_section(&refs)?),
            ("AcDb:Template", vec![0, 0, 1, 0]),
        ])
    }
}

struct R2000Layout {
    directory: Vec<u8>,
    header_offset: usize,
    classes_offset: usize,
    refs: Vec<ObjectRef>,
    map: Vec<u8>,
    map_offset: usize,
    total: usize,
}

fn align_up(value: usize, align: usize) -> usize {
    match value % align {
        0 => value,
        rem => value + (align - rem),
    }
}

fn write_u16_le(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32_le(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

/// Encode a whole document through the incremental encoder: tables first, then
/// every layout's entities in order.
pub(crate) fn encode_document(
    doc: &WriterDocument,
    config: &WriterConfig,
) -> Result<EncodedDocument> {
    let tables = DocumentTables::from_document(doc);
    let ceiling = doc
        .layouts()
        .flatten()
        .flat_map(|e| {
            let attributes = match e {
                WriterEntity::Insert(insert) => insert.attributes.as_slice(),
                _ => &[],
            };
            std::iter::once(e.common().handle)
                .chain(attributes.iter().map(|a| a.text.common.handle))
                .flatten()
        })
        .max()
        .unwrap_or(0);
    let mut encoder = DocumentEncoder::new(tables, config.clone(), ceiling)?;
    for (index, entities) in doc.layouts().enumerate() {
        let layout = LayoutId::from_index(index);
        for entity in entities {
            encoder.push(layout, entity.clone())?;
        }
    }
    encoder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_set_detects_duplicates_in_both_representations() {
        let mut dense = HandleSet::new(100);
        assert!(dense.insert(5));
        assert!(!dense.insert(5));
        assert!(dense.insert(100));
        let mut sparse = HandleSet::new(u64::MAX);
        assert!(sparse.insert(u64::MAX - 1));
        assert!(!sparse.insert(u64::MAX - 1));
    }

    #[test]
    fn record_arena_spans_pages() {
        let mut arena = RecordArena::new();
        let big = vec![1u8; RecordArena::PAGE - 10];
        let first = arena.append(&big).unwrap();
        let second = arena.append(&[2u8; 20]).unwrap();
        assert_eq!(first, 0);
        assert_eq!(second, RecordArena::PAGE as u64);
        assert_eq!(arena.get(second, 20), &[2u8; 20]);
        assert_eq!(arena.pages.len(), 2);
    }
}
