use crate::dwg::version::DwgVersion;

#[derive(Debug, Clone)]
pub struct WriterDocument {
    pub version: DwgVersion,
    pub modelspace: Vec<WriterEntity>,
    pub layers: Vec<LayerDef>,
    pub metadata: WriterMetadata,
}

impl Default for WriterDocument {
    fn default() -> Self {
        Self {
            version: DwgVersion::R2000,
            modelspace: Vec::new(),
            layers: vec![LayerDef::default()],
            metadata: WriterMetadata::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WriterMetadata {
    pub insertion_base: (f64, f64, f64),
    pub ext_min: Option<(f64, f64, f64)>,
    pub ext_max: Option<(f64, f64, f64)>,
}

impl Default for WriterMetadata {
    fn default() -> Self {
        Self {
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
}

impl Default for LayerDef {
    fn default() -> Self {
        Self {
            name: "0".to_string(),
            color_index: 7,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CommonEntityProps {
    pub handle: Option<u64>,
    pub layer_name: String,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum WriterEntity {
    Line(LineEntity),
    Point(PointEntity),
    Ray(RayEntity),
    XLine(XLineEntity),
    Arc(ArcEntity),
    Circle(CircleEntity),
    LwPolyline(LwPolylineEntity),
    Text(TextEntity),
    MText(MTextEntity),
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
