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
        }
    }
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
        }
    }
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
