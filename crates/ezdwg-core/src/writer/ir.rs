use crate::dwg::version::DwgVersion;

#[derive(Debug, Clone)]
pub struct WriterDocument {
    pub version: DwgVersion,
    pub modelspace: Vec<WriterEntity>,
    pub blocks: Vec<BlockDef>,
    pub layers: Vec<LayerDef>,
    pub linetypes: Vec<LinetypeDef>,
    pub text_styles: Vec<TextStyleDef>,
    pub metadata: WriterMetadata,
    /// Raster image definitions (`ACAD_IMAGE_DICT`) referenced by IMAGE entities.
    pub image_defs: Vec<ImageDef>,
}

impl Default for WriterDocument {
    fn default() -> Self {
        Self {
            version: DwgVersion::R2000,
            modelspace: Vec::new(),
            blocks: Vec::new(),
            layers: vec![LayerDef::default()],
            linetypes: Vec::new(),
            text_styles: Vec::new(),
            metadata: WriterMetadata::default(),
            image_defs: Vec::new(),
        }
    }
}

/// IMAGEDEF object: the external raster file of one or more IMAGE entities.
#[derive(Debug, Clone)]
pub struct ImageDef {
    /// Key in `ACAD_IMAGE_DICT`.
    pub name: String,
    pub file_path: String,
    /// Image size in pixels.
    pub size: (f64, f64),
    pub is_loaded: bool,
    /// Resolution units: 0 none, 2 centimeters, 5 inches.
    pub resolution_units: u8,
    /// Size of one pixel in drawing units.
    pub pixel_size: (f64, f64),
}

#[derive(Debug, Clone)]
pub struct WriterMetadata {
    pub codepage: u16,
    pub insunits: u16,
    pub lunits: u16,
    pub luprec: u16,
    pub aunits: u16,
    pub auprec: u16,
    pub ltscale: f64,
    pub insertion_base: (f64, f64, f64),
    pub ext_min: Option<(f64, f64, f64)>,
    pub ext_max: Option<(f64, f64, f64)>,
}

impl Default for WriterMetadata {
    fn default() -> Self {
        Self {
            codepage: 30,
            insunits: 0,
            lunits: 2,
            luprec: 4,
            aunits: 0,
            auprec: 0,
            ltscale: 1.0,
            insertion_base: (0.0, 0.0, 0.0),
            ext_min: None,
            ext_max: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayerDef {
    pub name: String,
    pub color_index: u16,
    pub linetype: String,
    pub lineweight: i16,
    pub frozen: bool,
    pub off: bool,
    pub frozen_in_new_viewports: bool,
    pub locked: bool,
    pub plot: bool,
}

impl Default for LayerDef {
    fn default() -> Self {
        Self {
            name: "0".to_string(),
            color_index: 7,
            linetype: "Continuous".into(),
            lineweight: -3,
            frozen: false,
            off: false,
            frozen_in_new_viewports: false,
            locked: false,
            plot: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LinetypeDef {
    pub name: String,
    pub description: String,
    pub dashes: Vec<f64>,
}

/// Additional per-entity values shared by the writer and its FFI adapter.
#[derive(Debug, Clone, Copy)]
pub struct EntityFormatting {
    pub linetype_scale: f64,
    pub lineweight: i16,
    pub invisible: bool,
    pub extrusion: (f64, f64, f64),
    pub thickness: f64,
    pub elevation: f64,
    pub alignment: Option<(f64, f64, f64)>,
    pub oblique: f64,
    pub width_factor: f64,
    pub generation: u16,
    pub halign: u16,
    pub valign: u16,
    pub line_spacing_style: u16,
    pub line_spacing_factor: f64,
}
impl Default for EntityFormatting {
    fn default() -> Self {
        Self {
            linetype_scale: 1.,
            lineweight: -1,
            invisible: false,
            extrusion: (0., 0., 1.),
            thickness: 0.,
            elevation: 0.,
            alignment: None,
            oblique: 0.,
            width_factor: 1.,
            generation: 0,
            halign: 0,
            valign: 0,
            line_spacing_style: 1,
            line_spacing_factor: 1.,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CommonEntityProps {
    pub handle: Option<u64>,
    pub layer_name: String,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub linetype: String,
    pub style_name: String,
    pub formatting: EntityFormatting,
}

#[derive(Debug, Clone)]
pub enum WriterEntity {
    Line(LineEntity),
    Point(PointEntity),
    Ray(RayEntity),
    XLine(XLineEntity),
    Arc(ArcEntity),
    Circle(CircleEntity),
    Ellipse(EllipseEntity),
    Spline(SplineEntity),
    Solid(PlanarFaceEntity),
    Trace(PlanarFaceEntity),
    Face3d(Face3dEntity),
    Polyline2d(PolylineEntity),
    Polyline3d(PolylineEntity),
    LwPolyline(LwPolylineEntity),
    Text(TextEntity),
    MText(MTextEntity),
    Insert(InsertEntity),
    Attdef(AttributeEntity),
    Attrib(AttributeEntity),
    Hatch(HatchEntity),
    Dimension(DimensionEntity),
    Image(ImageEntity),
}

impl WriterEntity {
    pub fn common(&self) -> &CommonEntityProps {
        match self {
            Self::Line(e) => &e.common,
            Self::Point(e) => &e.common,
            Self::Ray(e) => &e.common,
            Self::XLine(e) => &e.common,
            Self::Arc(e) => &e.common,
            Self::Circle(e) => &e.common,
            Self::Ellipse(e) => &e.common,
            Self::Spline(e) => &e.common,
            Self::Solid(e) | Self::Trace(e) => &e.common,
            Self::Face3d(e) => &e.common,
            Self::Polyline2d(e) | Self::Polyline3d(e) => &e.common,
            Self::LwPolyline(e) => &e.common,
            Self::Text(e) => &e.common,
            Self::MText(e) => &e.common,
            Self::Insert(e) => &e.common,
            Self::Attdef(e) | Self::Attrib(e) => &e.text.common,
            Self::Hatch(e) => &e.common,
            Self::Dimension(e) => &e.common,
            Self::Image(e) => &e.common,
        }
    }
    pub fn common_mut(&mut self) -> &mut CommonEntityProps {
        match self {
            Self::Line(e) => &mut e.common,
            Self::Point(e) => &mut e.common,
            Self::Ray(e) => &mut e.common,
            Self::XLine(e) => &mut e.common,
            Self::Arc(e) => &mut e.common,
            Self::Circle(e) => &mut e.common,
            Self::Ellipse(e) => &mut e.common,
            Self::Spline(e) => &mut e.common,
            Self::Solid(e) | Self::Trace(e) => &mut e.common,
            Self::Face3d(e) => &mut e.common,
            Self::Polyline2d(e) | Self::Polyline3d(e) => &mut e.common,
            Self::LwPolyline(e) => &mut e.common,
            Self::Text(e) => &mut e.common,
            Self::MText(e) => &mut e.common,
            Self::Insert(e) => &mut e.common,
            Self::Attdef(e) | Self::Attrib(e) => &mut e.text.common,
            Self::Hatch(e) => &mut e.common,
            Self::Dimension(e) => &mut e.common,
            Self::Image(e) => &mut e.common,
        }
    }
}

/// IMAGE entity (class `AcDbRasterImage`, ODA 20.4.80).
#[derive(Debug, Clone)]
pub struct ImageEntity {
    pub common: CommonEntityProps,
    /// Name of the `ImageDef` in `WriterDocument::image_defs`.
    pub image_def: String,
    pub insert: (f64, f64, f64),
    /// Drawing-unit vectors of one pixel step along the image rows/columns.
    pub u_vector: (f64, f64, f64),
    pub v_vector: (f64, f64, f64),
    /// Image size in pixels (DXF 13).
    pub size: (f64, f64),
    /// DXF 70: 1 show image, 2 show when not aligned, 4 use clipping, 8 transparency.
    pub display_props: u16,
    pub clipping: bool,
    pub brightness: u8,
    pub contrast: u8,
    pub fade: u8,
    /// DXF 71: 1 rectangular (two corners), 2 polygonal.
    pub clip_type: u16,
    pub clip_vertices: Vec<(f64, f64)>,
}

/// DIMENSION (types 20-26) with the common dimension data of ODA 20.4.22.
///
/// Angles are radians; the extrusion comes from `common.formatting`. The
/// rendered geometry lives in the referenced anonymous block (`*D...`).
#[derive(Debug, Clone)]
pub struct DimensionEntity {
    pub common: CommonEntityProps,
    pub kind: DimensionKind,
    /// Text midpoint (DXF 11) in OCS; its Z is `elevation`.
    pub text_midpoint: (f64, f64),
    pub elevation: f64,
    /// "Flags 1" (RC): bit 0 = text at the default position, bit 1 = unique block.
    pub flags: u8,
    pub user_text: String,
    pub text_rotation: f64,
    pub horizontal_direction: f64,
    pub insert_scale: (f64, f64, f64),
    pub insert_rotation: f64,
    pub attachment_point: u16,
    pub line_spacing_style: u16,
    pub line_spacing_factor: f64,
    pub actual_measurement: f64,
    /// Insertion point of the anonymous block (DXF 12), OCS.
    pub insert_point: (f64, f64),
    pub dimstyle: String,
    pub block_name: String,
}

#[derive(Debug, Clone)]
pub enum DimensionKind {
    Linear {
        point13: (f64, f64, f64),
        point14: (f64, f64, f64),
        point10: (f64, f64, f64),
        ext_line_rotation: f64,
        dim_rotation: f64,
    },
    Aligned {
        point13: (f64, f64, f64),
        point14: (f64, f64, f64),
        point10: (f64, f64, f64),
        ext_line_rotation: f64,
    },
    Angular3Pt {
        point10: (f64, f64, f64),
        point13: (f64, f64, f64),
        point14: (f64, f64, f64),
        point15: (f64, f64, f64),
    },
    Angular2Ln {
        point16: (f64, f64),
        point13: (f64, f64, f64),
        point14: (f64, f64, f64),
        point15: (f64, f64, f64),
        point10: (f64, f64, f64),
    },
    Radius {
        point10: (f64, f64, f64),
        point15: (f64, f64, f64),
        leader_length: f64,
    },
    Diameter {
        point15: (f64, f64, f64),
        point10: (f64, f64, f64),
        leader_length: f64,
    },
    Ordinate {
        point10: (f64, f64, f64),
        point13: (f64, f64, f64),
        point14: (f64, f64, f64),
        /// "Flags 2" (RC): bit 0 set for an X-datum ordinate.
        flags2: u8,
    },
}

impl DimensionKind {
    /// Internal DWG type code (ODA 20.4.23-20.4.29).
    pub fn type_code(&self) -> u16 {
        match self {
            Self::Ordinate { .. } => 20,
            Self::Linear { .. } => 21,
            Self::Aligned { .. } => 22,
            Self::Angular3Pt { .. } => 23,
            Self::Angular2Ln { .. } => 24,
            Self::Radius { .. } => 25,
            Self::Diameter { .. } => 26,
        }
    }
}

/// HATCH boundary and pattern definition in OCS coordinates (ODA 20.4.75).
///
/// The elevation and extrusion come from `common.formatting`. Angles are radians.
#[derive(Debug, Clone)]
pub struct HatchEntity {
    pub common: CommonEntityProps,
    /// Pattern name (DXF 2); `SOLID` for solid fills.
    pub name: String,
    pub solid_fill: bool,
    pub paths: Vec<HatchPathDef>,
    /// Hatch style (DXF 75): 0 odd parity, 1 outermost, 2 whole area.
    pub style: u16,
    /// Pattern type (DXF 76): 0 user-defined, 1 predefined, 2 custom.
    pub pattern_type: u16,
    /// Required for pattern fills, ignored for solid fills.
    pub pattern: Option<HatchPatternDef>,
    /// Written when any path carries the derived flag (DXF 92 bit 4).
    pub pixel_size: Option<f64>,
    pub seed_points: Vec<(f64, f64)>,
}

#[derive(Debug, Clone)]
pub struct HatchPathDef {
    /// Boundary path type flags (DXF 92): 1 external, 2 polyline, 4 derived,
    /// 8 textbox, 16 outermost. Bit 2 selects the polyline layout.
    pub flags: u32,
    pub data: HatchPathData,
}

#[derive(Debug, Clone)]
pub enum HatchPathData {
    Polyline {
        closed: bool,
        vertices: Vec<(f64, f64)>,
        bulges: Option<Vec<f64>>,
    },
    Edges(Vec<HatchEdge>),
}

#[derive(Debug, Clone)]
pub enum HatchEdge {
    Line {
        start: (f64, f64),
        end: (f64, f64),
    },
    Arc {
        center: (f64, f64),
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        ccw: bool,
    },
    Ellipse {
        center: (f64, f64),
        /// Endpoint of the major axis relative to the center.
        major_endpoint: (f64, f64),
        ratio: f64,
        start_angle: f64,
        end_angle: f64,
        ccw: bool,
    },
    Spline {
        degree: u32,
        rational: bool,
        periodic: bool,
        knots: Vec<f64>,
        control_points: Vec<(f64, f64)>,
        weights: Vec<f64>,
    },
}

#[derive(Debug, Clone)]
pub struct HatchPatternDef {
    pub angle: f64,
    pub scale: f64,
    pub double: bool,
    pub lines: Vec<HatchPatternLineDef>,
}

/// One family of pattern lines, already rotated and scaled (as stored in DWG).
#[derive(Debug, Clone)]
pub struct HatchPatternLineDef {
    pub angle: f64,
    pub base: (f64, f64),
    pub offset: (f64, f64),
    pub dashes: Vec<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct LineEntity {
    pub common: CommonEntityProps,
    pub start: (f64, f64, f64),
    pub end: (f64, f64, f64),
}

#[derive(Debug, Clone, Default)]
pub struct PointEntity {
    pub common: CommonEntityProps,
    pub location: (f64, f64, f64),
    pub x_axis_angle: f64,
}

#[derive(Debug, Clone, Default)]
pub struct RayEntity {
    pub common: CommonEntityProps,
    pub start: (f64, f64, f64),
    pub unit_vector: (f64, f64, f64),
}

#[derive(Debug, Clone, Default)]
pub struct XLineEntity {
    pub common: CommonEntityProps,
    pub start: (f64, f64, f64),
    pub unit_vector: (f64, f64, f64),
}

#[derive(Debug, Clone, Default)]
pub struct ArcEntity {
    pub common: CommonEntityProps,
    pub center: (f64, f64, f64),
    pub radius: f64,
    pub angle_start_rad: f64,
    pub angle_end_rad: f64,
}

#[derive(Debug, Clone, Default)]
pub struct CircleEntity {
    pub common: CommonEntityProps,
    pub center: (f64, f64, f64),
    pub radius: f64,
}

#[derive(Debug, Clone)]
pub struct EllipseEntity {
    pub common: CommonEntityProps,
    /// Center and semi-major axis vector are WCS coordinates.
    pub center: (f64, f64, f64),
    pub major_axis: (f64, f64, f64),
    pub axis_ratio: f64,
    /// Eccentric anomaly parameters in radians, not polar angles.
    pub start_param: f64,
    pub end_param: f64,
}

/// WCS spline geometry, preserving its native control or fit representation.
#[derive(Debug, Clone)]
pub struct SplineEntity {
    pub common: CommonEntityProps,
    pub degree: u32,
    pub data: SplineData,
}

#[derive(Debug, Clone)]
pub enum SplineData {
    Control {
        rational: bool,
        closed: bool,
        periodic: bool,
        knot_tolerance: f64,
        ctrl_tolerance: f64,
        knots: Vec<f64>,
        control_points: Vec<(f64, f64, f64)>,
        weights: Vec<f64>,
    },
    Fit {
        fit_tolerance: f64,
        start_tangent: (f64, f64, f64),
        end_tangent: (f64, f64, f64),
        fit_points: Vec<(f64, f64, f64)>,
    },
}

/// SOLID/TRACE corners in stored OCS order, with a shared Z elevation.
#[derive(Debug, Clone)]
pub struct PlanarFaceEntity {
    pub common: CommonEntityProps,
    pub points: [(f64, f64, f64); 4],
}

/// 3DFACE corners in perimeter order in WCS.
#[derive(Debug, Clone)]
pub struct Face3dEntity {
    pub common: CommonEntityProps,
    pub points: [(f64, f64, f64); 4],
    pub invisible_edge_flags: u16,
}

#[derive(Debug, Clone, Default)]
pub struct LwPolylineEntity {
    pub common: CommonEntityProps,
    pub flags: u16,
    pub vertices: Vec<(f64, f64)>,
    pub const_width: Option<f64>,
    pub bulges: Vec<f64>,
    pub widths: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Default)]
pub struct PolylineEntity {
    pub common: CommonEntityProps,
    pub flags: u16,
    pub width_start: f64,
    pub width_end: f64,
    pub vertices: Vec<PolylineVertex>,
}

#[derive(Debug, Clone, Default)]
pub struct PolylineVertex {
    pub position: (f64, f64, f64),
    pub start_width: f64,
    pub end_width: f64,
    pub bulge: f64,
}

#[derive(Debug, Clone, Default)]
pub struct TextEntity {
    pub common: CommonEntityProps,
    pub text: String,
    pub insert: (f64, f64, f64),
    pub height: f64,
    pub rotation_rad: f64,
}

#[derive(Debug, Clone, Default)]
pub struct MTextEntity {
    pub common: CommonEntityProps,
    pub text: String,
    pub insert: (f64, f64, f64),
    pub text_direction: (f64, f64, f64),
    pub rect_width: f64,
    pub char_height: f64,
    pub attachment_point: u16,
    pub drawing_direction: u16,
}

#[derive(Debug, Clone)]
pub struct TextStyleDef {
    pub name: String,
    pub vertical: bool,
    pub height: f64,
    pub width: f64,
    pub oblique: f64,
    pub generation: u8,
    pub last_height: f64,
    pub font: String,
    pub bigfont: String,
}
impl Default for TextStyleDef {
    fn default() -> Self {
        Self {
            name: "Standard".into(),
            vertical: false,
            height: 0.,
            width: 1.,
            oblique: 0.,
            generation: 0,
            last_height: 2.5,
            font: "txt".into(),
            bigfont: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BlockDef {
    pub name: String,
    pub base_point: (f64, f64, f64),
    pub description: String,
    pub entities: Vec<WriterEntity>,
}

#[derive(Debug, Clone)]
pub struct InsertEntity {
    pub common: CommonEntityProps,
    pub block_name: String,
    pub insert: (f64, f64, f64),
    pub scale: (f64, f64, f64),
    pub rotation_rad: f64,
    pub attributes: Vec<AttributeEntity>,
}

#[derive(Debug, Clone, Default)]
pub struct AttributeEntity {
    pub text: TextEntity,
    pub tag: String,
    pub prompt: String,
    pub flags: u8,
    pub field_length: u16,
}

impl WriterDocument {
    pub fn layouts(&self) -> impl Iterator<Item = &[WriterEntity]> {
        std::iter::once(self.modelspace.as_slice())
            .chain(self.blocks.iter().map(|b| b.entities.as_slice()))
    }
}
