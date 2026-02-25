use pyo3::exceptions::{PyIOError, PyNotImplementedError, PyValueError};
use pyo3::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::dwg::decoder;
use crate::dwg::file_open;
use crate::dwg::version;
use crate::dwg::version::DwgVersion;
use crate::entities;
use crate::objects;
use crate::writer;

type Point2 = (f64, f64);
type Point3 = (f64, f64, f64);

type SectionLocatorRow = (String, u32, u32);
type ObjectMapEntryRow = (u64, u32);
type ObjectHeaderRow = (u64, u32, u32, u16);
type ObjectHeaderWithTypeRow = (u64, u32, u32, u16, String, String);
type ObjectRecordBytesRow = (u64, u32, u32, u16, Vec<u8>);
type HandleStreamRefsRow = (u64, Vec<u64>);
type AcisCandidateInfoRow = (u64, u16, u32, String, Vec<u64>, u8);
type EntityStyleRow = (u64, Option<u16>, Option<u32>, u64);
type LayerColorRow = (u64, u16, Option<u32>);

type LineEntityRow = (u64, f64, f64, f64, f64, f64, f64);
type PointEntityRow = (u64, f64, f64, f64, f64);
type ArcEntityRow = (u64, f64, f64, f64, f64, f64, f64);
type CircleEntityRow = (u64, f64, f64, f64, f64);
type LineArcCircleRows = (Vec<LineEntityRow>, Vec<ArcEntityRow>, Vec<CircleEntityRow>);
type EllipseEntityRow = (u64, Point3, Point3, Point3, f64, f64, f64);
type SplineFlagsRow = (u32, u32, bool, bool, bool);
type SplineToleranceRow = (Option<f64>, Option<f64>, Option<f64>);
type SplineEntityRow = (
    u64,
    SplineFlagsRow,
    SplineToleranceRow,
    Vec<f64>,
    Vec<Point3>,
    Vec<f64>,
    Vec<Point3>,
);
type TextMetricsRow = (f64, f64, f64, f64, f64);
type TextAlignmentRow = (u16, u16, u16);
type TextEntityRow = (
    u64,
    String,
    Point3,
    Option<Point3>,
    Point3,
    TextMetricsRow,
    TextAlignmentRow,
    Option<u64>,
);
type AttribEntityRow = (
    u64,
    String,
    Option<String>,
    Option<String>,
    Point3,
    Option<Point3>,
    Point3,
    TextMetricsRow,
    TextAlignmentRow,
    u8,
    bool,
    (Option<u64>, Option<u64>),
);
type MTextBackgroundRow = (u32, Option<f64>, Option<u16>, Option<u32>, Option<u32>);
type MTextEntityRow = (
    u64,
    String,
    Point3,
    Point3,
    Point3,
    f64,
    f64,
    u16,
    u16,
    MTextBackgroundRow,
);
type LeaderEntityRow = (u64, u16, u16, Vec<Point3>);
type HatchPathRow = (bool, Vec<Point2>);
type HatchEntityRow = (u64, String, bool, bool, f64, Point3, Vec<HatchPathRow>);
type ToleranceEntityRow = (u64, String, Point3, Point3, Point3, f64, f64, Option<u64>);
type MLineVertexRow = (Point3, Point3, Point3);
type MLineEntityRow = (
    u64,
    f64,
    u8,
    Point3,
    Point3,
    u16,
    u8,
    Vec<MLineVertexRow>,
    Option<u64>,
);
type MInsertArrayRow = (u16, u16, f64, f64, Option<String>);
type DimExtrusionScaleRow = (Point3, Point3);
type DimAnglesRow = (f64, f64, f64, f64);
type DimStyleRow = (u8, Option<f64>, Option<u16>, Option<u16>, Option<f64>, f64);
type DimHandlesRow = (Option<u64>, Option<u64>);
type DimEntityRow = (
    u64,
    String,
    Point3,
    Point3,
    Point3,
    Point3,
    Option<Point3>,
    DimExtrusionScaleRow,
    DimAnglesRow,
    DimStyleRow,
    DimHandlesRow,
);
type DimTypedEntityRow = (String, DimEntityRow);
type DimLinearDecodeFn = for<'a> fn(
    &mut BitReader<'a>,
    &version::DwgVersion,
    &ApiObjectHeader,
    u64,
) -> crate::core::result::Result<entities::DimLinearEntity>;
type InsertEntityRow = (u64, f64, f64, f64, f64, f64, f64, f64, Option<String>);
type MInsertEntityRow = (u64, f64, f64, f64, f64, f64, f64, f64, MInsertArrayRow);
type InsertMInsertRows = (Vec<InsertEntityRow>, Vec<MInsertEntityRow>);
type InsertMInsertDimensionRows = (
    Vec<InsertEntityRow>,
    Vec<MInsertEntityRow>,
    Vec<DimTypedEntityRow>,
);
type BlockHeaderNameRow = (u64, String);
type BlockEntityNameRow = (u64, String, String);
type Polyline2dEntityRow = (u64, u16, u16, f64, f64, f64, f64);
type Polyline2dInterpretedRow = (
    u64,
    u16,
    u16,
    String,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
);
type LwPolylineEntityRow = (u64, u16, Vec<Point2>, Vec<f64>, Vec<Point2>, Option<f64>);
type Polyline3dEntityRow = (u64, u8, u8);
type Vertex3dEntityRow = (u64, u8, f64, f64, f64);
type Polyline3dVerticesRow = (u64, u8, bool, Vec<Point3>);
type PolylineMeshEntityRow = (u64, u16, u16, u16, u16, u16, u16);
type VertexMeshEntityRow = (u64, u8, f64, f64, f64);
type PolylineMeshVerticesRow = (u64, u16, u16, u16, bool, Vec<Point3>);
type PolylinePFaceEntityRow = (u64, u16, u16);
type VertexPFaceEntityRow = (u64, u8, f64, f64, f64);
type VertexPFaceFaceEntityRow = (u64, u16, u16, u16, u16);
type PFaceFaceRow = (u16, u16, u16, u16);
type PolylinePFaceFacesRow = (u64, u16, u16, Vec<Point3>, Vec<PFaceFaceRow>);
type Face3dEntityRow = (u64, Point3, Point3, Point3, Point3, u16);
type SolidEntityRow = (u64, Point3, Point3, Point3, Point3, f64, Point3);
type TraceEntityRow = (u64, Point3, Point3, Point3, Point3, f64, Point3);
type ShapeEntityRow = (
    u64,
    Point3,
    f64,
    f64,
    f64,
    f64,
    f64,
    u16,
    Point3,
    Option<u64>,
);
type ViewportEntityRow = (u64,);
type OleFrameEntityRow = (u64,);
type LongTransactionEntityRow = (
    u64,
    Option<u64>,
    Vec<u64>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
    Option<u64>,
    Vec<u64>,
);
type RegionEntityRow = (u64, Vec<u64>);
type Solid3dEntityRow = (u64, Vec<u64>);
type BodyEntityRow = (u64, Vec<u64>);
type RayEntityRow = (u64, Point3, Point3);
type XLineEntityRow = (u64, Point3, Point3);
type PolylineVerticesRow = (u64, u16, Vec<Point3>);
type PolylineInterpolatedRow = (u64, u16, bool, Vec<Point3>);
type Vertex2dEntityRow = (u64, u16, f64, f64, f64, f64, f64, f64, f64);
type VertexDataRow = (f64, f64, f64, f64, f64, f64, f64, u16);
type PolylineVertexDataRow = (u64, u16, Vec<VertexDataRow>);
type PolylineSequenceMembersRow = (u64, String, Vec<u64>, Vec<u64>, Option<u64>);
type TextWriteRow = (u64, String, Point3, f64, f64);
type MTextWriteRow = (u64, String, Point3, Point3, f64, f64, u16, u16);
type PointWriteRow = (u64, f64, f64, f64, f64);

struct DimDecodeSpec {
    type_code: u16,
    type_name: &'static str,
    dimtype: &'static str,
    decode_entity: DimLinearDecodeFn,
}

const DIM_DECODE_SPECS: [DimDecodeSpec; 7] = [
    DimDecodeSpec {
        type_code: 0x15,
        type_name: "DIM_LINEAR",
        dimtype: "LINEAR",
        decode_entity: decode_dim_linear_for_version,
    },
    DimDecodeSpec {
        type_code: 0x14,
        type_name: "DIM_ORDINATE",
        dimtype: "ORDINATE",
        decode_entity: decode_dim_linear_for_version,
    },
    DimDecodeSpec {
        type_code: 0x16,
        type_name: "DIM_ALIGNED",
        dimtype: "ALIGNED",
        decode_entity: decode_dim_linear_for_version,
    },
    DimDecodeSpec {
        type_code: 0x17,
        type_name: "DIM_ANG3PT",
        dimtype: "ANG3PT",
        decode_entity: decode_dim_linear_for_version,
    },
    DimDecodeSpec {
        type_code: 0x18,
        type_name: "DIM_ANG2LN",
        dimtype: "ANG2LN",
        decode_entity: decode_dim_linear_for_version,
    },
    DimDecodeSpec {
        type_code: 0x1A,
        type_name: "DIM_DIAMETER",
        dimtype: "DIAMETER",
        decode_entity: decode_dim_diameter_for_version,
    },
    DimDecodeSpec {
        type_code: 0x19,
        type_name: "DIM_RADIUS",
        dimtype: "RADIUS",
        decode_entity: decode_dim_radius_for_version,
    },
];

struct InsertNameResolutionState {
    known_block_handles: HashSet<u64>,
    block_header_names: HashMap<u64, String>,
    named_block_handles: HashSet<u64>,
}
