#![allow(clippy::useless_conversion)] // Triggered by PyO3 #[pyfunction] wrapper expansion.

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
type InsertEntityRow = (u64, f64, f64, f64, f64, f64, f64, f64, Option<String>);
type MInsertEntityRow = (u64, f64, f64, f64, f64, f64, f64, f64, MInsertArrayRow);
type InsertMInsertRows = (Vec<InsertEntityRow>, Vec<MInsertEntityRow>);
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

#[pyfunction]
pub fn detect_version(path: &str) -> PyResult<String> {
    let tag = file_open::read_version_tag(path).map_err(to_py_err)?;
    let version = version::detect_version(&tag).map_err(to_py_err)?;
    Ok(version.as_str().to_string())
}

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
    xlines=None
))]
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
) -> PyResult<()> {
    let points = points.unwrap_or_default();
    let rays = rays.unwrap_or_default();
    let xlines = xlines.unwrap_or_default();
    let mut modelspace = Vec::with_capacity(
        lines.len()
            + arcs.len()
            + circles.len()
            + lwpolylines.len()
            + texts.len()
            + mtexts.len()
            + points.len()
            + rays.len()
            + xlines.len(),
    );
    for (handle, sx, sy, sz, ex, ey, ez) in lines {
        modelspace.push(writer::WriterEntity::Line(writer::LineEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
            },
            start: (sx, sy, sz),
            end: (ex, ey, ez),
        }));
    }
    for (handle, cx, cy, cz, radius, angle_start, angle_end) in arcs {
        modelspace.push(writer::WriterEntity::Arc(writer::ArcEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
            },
            center: (cx, cy, cz),
            radius,
            angle_start_rad: angle_start,
            angle_end_rad: angle_end,
        }));
    }
    for (handle, cx, cy, cz, radius) in circles {
        modelspace.push(writer::WriterEntity::Circle(writer::CircleEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
            },
            center: (cx, cy, cz),
            radius,
        }));
    }
    for (handle, flags, points, bulges, widths, const_width) in lwpolylines {
        modelspace.push(writer::WriterEntity::LwPolyline(writer::LwPolylineEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
            },
            flags,
            vertices: points,
            const_width,
            bulges,
            widths,
        }));
    }
    for (handle, text, insertion, height, rotation) in texts {
        modelspace.push(writer::WriterEntity::Text(writer::TextEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
            },
            text,
            insert: insertion,
            height,
            rotation_rad: rotation,
        }));
    }
    for (
        handle,
        text,
        insertion,
        text_direction,
        rect_width,
        char_height,
        attachment_point,
        drawing_direction,
    ) in mtexts
    {
        modelspace.push(writer::WriterEntity::MText(writer::MTextEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
            },
            text,
            insert: insertion,
            text_direction,
            rect_width,
            char_height,
            attachment_point,
            drawing_direction,
        }));
    }
    for (handle, x, y, z, x_axis_angle) in points {
        modelspace.push(writer::WriterEntity::Point(writer::PointEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
            },
            location: (x, y, z),
            x_axis_angle,
        }));
    }
    for (handle, start, unit_vector) in rays {
        modelspace.push(writer::WriterEntity::Ray(writer::RayEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
            },
            start,
            unit_vector,
        }));
    }
    for (handle, start, unit_vector) in xlines {
        modelspace.push(writer::WriterEntity::XLine(writer::XLineEntity {
            common: writer::CommonEntityProps {
                handle: if handle == 0 { None } else { Some(handle) },
                layer_name: "0".to_string(),
                color_index: Some(7),
                true_color: None,
            },
            start,
            unit_vector,
        }));
    }

    let doc = writer::WriterDocument {
        version: DwgVersion::R2000,
        modelspace,
        ..writer::WriterDocument::default()
    };
    let bytes =
        writer::r2000::write_document(&doc, &writer::WriterConfig::default()).map_err(to_py_err)?;

    let out_path = Path::new(output_path);
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|err| PyIOError::new_err(err.to_string()))?;
        }
    }
    std::fs::write(out_path, bytes).map_err(|err| PyIOError::new_err(err.to_string()))?;
    Ok(())
}

#[pyfunction]
pub fn list_section_locators(path: &str) -> PyResult<Vec<SectionLocatorRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let directory = decoder.section_directory().map_err(to_py_err)?;
    let result = directory
        .records
        .into_iter()
        .map(|record| {
            let label = record.name.clone().unwrap_or_else(|| record.kind().label());
            (label, record.offset, record.size)
        })
        .collect();
    Ok(result)
}

#[pyfunction]
pub fn read_section_bytes(path: &str, index: usize) -> PyResult<Vec<u8>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let directory = decoder.section_directory().map_err(to_py_err)?;
    let section = decoder
        .load_section_by_index(&directory, index)
        .map_err(to_py_err)?;
    Ok(section.data.as_ref().to_vec())
}

#[pyfunction(signature = (path, limit=None))]
pub fn list_object_map_entries(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<ObjectMapEntryRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut entries: Vec<ObjectMapEntryRow> = index
        .objects
        .iter()
        .map(|obj| (obj.handle.0, obj.offset))
        .collect();
    if let Some(limit) = limit {
        if entries.len() > limit {
            entries.truncate(limit);
        }
    }
    Ok(entries)
}

#[pyfunction(signature = (path, limit=None))]
pub fn list_object_headers(path: &str, limit: Option<usize>) -> PyResult<Vec<ObjectHeaderRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = match decoder.parse_object_record(obj.offset) {
            Ok(record) => record,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let header = match parse_object_header_for_version(&record, decoder.version()) {
            Ok(header) => header,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((obj.handle.0, obj.offset, header.data_size, header.type_code));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn list_object_headers_with_type(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<ObjectHeaderWithTypeRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = match decoder.parse_object_record(obj.offset) {
            Ok(record) => record,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let header = match parse_object_header_for_version(&record, decoder.version()) {
            Ok(header) => header,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let type_name = resolved_type_name(header.type_code, &dynamic_types);
        let type_class = resolved_type_class(header.type_code, &type_name);
        result.push((
            obj.handle.0,
            obj.offset,
            header.data_size,
            header.type_code,
            type_name,
            type_class,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, type_codes, limit=None))]
pub fn list_object_headers_by_type(
    path: &str,
    type_codes: Vec<u16>,
    limit: Option<usize>,
) -> PyResult<Vec<ObjectHeaderWithTypeRow>> {
    if type_codes.is_empty() {
        return Ok(Vec::new());
    }
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let filter: HashSet<u16> = type_codes.into_iter().collect();
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = match decoder.parse_object_record(obj.offset) {
            Ok(record) => record,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let header = match parse_object_header_for_version(&record, decoder.version()) {
            Ok(header) => header,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let type_name = resolved_type_name(header.type_code, &dynamic_types);
        if !matches_type_filter(&filter, header.type_code, &type_name) {
            continue;
        }
        let type_class = resolved_type_class(header.type_code, &type_name);
        result.push((
            obj.handle.0,
            obj.offset,
            header.data_size,
            header.type_code,
            type_name,
            type_class,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, type_codes, limit=None))]
pub fn read_object_records_by_type(
    path: &str,
    type_codes: Vec<u16>,
    limit: Option<usize>,
) -> PyResult<Vec<ObjectRecordBytesRow>> {
    if type_codes.is_empty() {
        return Ok(Vec::new());
    }
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let filter: HashSet<u16> = type_codes.into_iter().collect();
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = decoder.parse_object_record(obj.offset).map_err(to_py_err)?;
        let header =
            parse_object_header_for_version(&record, decoder.version()).map_err(to_py_err)?;
        let type_name = resolved_type_name(header.type_code, &dynamic_types);
        if !matches_type_filter(&filter, header.type_code, &type_name) {
            continue;
        }
        let record = record.raw.as_ref().to_vec();
        result.push((
            obj.handle.0,
            obj.offset,
            header.data_size,
            header.type_code,
            record,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, handles, limit=None))]
pub fn read_object_records_by_handle(
    path: &str,
    handles: Vec<u64>,
    limit: Option<usize>,
) -> PyResult<Vec<ObjectRecordBytesRow>> {
    if handles.is_empty() {
        return Ok(Vec::new());
    }

    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let target_handles: HashSet<u64> = handles.iter().copied().collect();
    let mut found_rows: HashMap<u64, ObjectRecordBytesRow> = HashMap::new();

    for obj in index.objects.iter() {
        let handle = obj.handle.0;
        if !target_handles.contains(&handle) || found_rows.contains_key(&handle) {
            continue;
        }
        let record = decoder.parse_object_record(obj.offset).map_err(to_py_err)?;
        let header =
            parse_object_header_for_version(&record, decoder.version()).map_err(to_py_err)?;
        found_rows.insert(
            handle,
            (
                handle,
                obj.offset,
                header.data_size,
                header.type_code,
                record.raw.as_ref().to_vec(),
            ),
        );
        if found_rows.len() >= target_handles.len() {
            break;
        }
    }

    let mut result = Vec::new();
    for handle in handles {
        if let Some(row) = found_rows.remove(&handle) {
            result.push(row);
            if let Some(limit) = limit {
                if result.len() >= limit {
                    break;
                }
            }
        }
    }
    Ok(result)
}

fn start_delta_candidates_for_type(type_code: u16) -> &'static [i32] {
    const DEFAULT: &[i32] = &[-8, -4, 0, 4, 8];
    const HEADER_LIKE: &[i32] = &[-16, -8, -4, 0, 4, 8, 16];
    const PAYLOAD_LIKE: &[i32] = &[-32, -24, -16, -8, -4, 0, 4, 8, 16, 24, 32];
    match type_code {
        0x214 | 0x221 => HEADER_LIKE,
        0x222 | 0x223 | 0x224 | 0x225 => PAYLOAD_LIKE,
        _ => DEFAULT,
    }
}

fn preferred_ref_type_codes_for_acis_unknown(type_code: u16) -> &'static [u16] {
    match type_code {
        // HEADER-like records usually point back to owner 3DSOLID/BODY/REGION or link table.
        0x221 => &[0x26, 0x27, 0x25, 0x214],
        // Link table records are expected to link to header/payload records.
        0x214 => &[0x221, 0x222, 0x223, 0x224, 0x225],
        // Payload chunks often refer to link table/header and sometimes sibling payload chunks.
        0x222 | 0x223 | 0x224 | 0x225 => &[0x214, 0x221, 0x222, 0x223, 0x224, 0x225],
        _ => &[],
    }
}

fn resolve_handle_stream_start_candidates(
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    type_code: u16,
) -> Vec<u32> {
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        return Vec::new();
    }
    let total_bits = header.data_size.saturating_mul(8);
    let mut bases = resolve_r2010_object_data_end_bit_candidates(header);
    if let Ok(canonical) = resolve_r2010_object_data_end_bit(header) {
        bases.push(canonical);
    }
    let mut out = Vec::new();
    for base in bases {
        for delta in start_delta_candidates_for_type(type_code).iter().copied() {
            let candidate_i64 = i64::from(base) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            let Ok(candidate) = u32::try_from(candidate_i64) else {
                continue;
            };
            if candidate >= total_bits {
                continue;
            }
            out.push(candidate);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[derive(Default)]
struct KnownHandleRefsDecode {
    refs: Vec<u64>,
    confidence: u8,
}

fn derive_known_handle_refs_confidence(
    refs_len: usize,
    quality_score: i64,
    best_score: i64,
    second_score: Option<i64>,
) -> u8 {
    if refs_len == 0 {
        return 0;
    }
    let mut confidence = 8i64;
    confidence = confidence.saturating_add(i64::try_from(refs_len.min(8)).unwrap_or(0) * 7);
    if quality_score > 0 {
        confidence = confidence.saturating_add(quality_score.min(12) * 3);
    }
    if let Some(second) = second_score {
        let margin = best_score.saturating_sub(second);
        let margin_boost = if margin >= 48 {
            26
        } else if margin >= 24 {
            18
        } else if margin >= 12 {
            12
        } else if margin >= 6 {
            7
        } else if margin > 0 {
            3
        } else {
            0
        };
        confidence = confidence.saturating_add(margin_boost);
    } else {
        // Only one candidate decoded successfully: moderate confidence, not maximal.
        confidence = confidence.saturating_add(14);
    }
    confidence.clamp(0, 100) as u8
}

fn decode_known_handle_refs_from_object_record(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
    known_handles: &HashSet<u64>,
    object_type_codes: Option<&HashMap<u64, u16>>,
    max_refs: usize,
) -> KnownHandleRefsDecode {
    let total_bits = u64::from(header.data_size.saturating_mul(8));
    let start_candidates =
        resolve_handle_stream_start_candidates(version, header, header.type_code);
    if start_candidates.is_empty() {
        return KnownHandleRefsDecode::default();
    }
    let canonical_start = resolve_r2010_object_data_end_bit(header).ok();
    let preferred_ref_types = preferred_ref_type_codes_for_acis_unknown(header.type_code);
    let mut best: Option<(i64, i64, usize, u32, Vec<u64>)> = None;
    let mut second_score: Option<i64> = None;

    for start_bit in start_candidates {
        let mut reader = record.bit_reader();
        if skip_object_type_prefix(&mut reader, version).is_err() {
            continue;
        }
        reader.set_bit_pos(start_bit);

        let mut refs: Vec<u64> = Vec::new();
        let mut seen: HashSet<u64> = HashSet::new();
        let mut quality_score: i64 = 0;
        for _ in 0..128usize {
            if reader.tell_bits() >= total_bits {
                break;
            }
            let before_bits = reader.tell_bits();
            let value = match entities::common::read_handle_reference(&mut reader, object_handle) {
                Ok(value) => value,
                Err(_) => break,
            };
            if reader.tell_bits() <= before_bits {
                break;
            }
            if value == 0 || value == object_handle || !known_handles.contains(&value) {
                continue;
            }
            if seen.insert(value) {
                refs.push(value);
                if let Some(type_codes) = object_type_codes {
                    if let Some(ref_type_code) = type_codes.get(&value) {
                        if preferred_ref_types.contains(ref_type_code) {
                            quality_score += 6;
                        } else if (0x214..=0x225).contains(ref_type_code) {
                            quality_score += 3;
                        } else if matches!(*ref_type_code, 0x25 | 0x26 | 0x27) {
                            quality_score += 2;
                        } else if *ref_type_code == 0x33 {
                            quality_score -= 2;
                        }
                    }
                }
                if refs.len() >= max_refs {
                    break;
                }
            }
        }

        let delta = canonical_start
            .map(|canonical| canonical.abs_diff(start_bit))
            .unwrap_or(0);
        let score = quality_score
            .saturating_mul(32)
            .saturating_add((refs.len() as i64).saturating_mul(4))
            .saturating_sub(i64::from(delta));

        let should_replace_best = match &best {
            Some((best_score, _best_quality, best_len, best_delta, _))
                if score < *best_score
                    || (score == *best_score
                        && (refs.len() < *best_len
                            || (refs.len() == *best_len && delta >= *best_delta))) =>
            {
                false
            }
            _ => true,
        };
        if should_replace_best {
            if let Some((prev_best_score, _, _, _, _)) = &best {
                second_score = Some(
                    second_score
                        .map(|value| value.max(*prev_best_score))
                        .unwrap_or(*prev_best_score),
                );
            }
            best = Some((score, quality_score, refs.len(), delta, refs));
        } else {
            second_score = Some(second_score.map(|value| value.max(score)).unwrap_or(score));
        }
    }

    if let Some((best_score, quality_score, _len, _delta, refs)) = best {
        let confidence = derive_known_handle_refs_confidence(
            refs.len(),
            quality_score,
            best_score,
            second_score,
        );
        KnownHandleRefsDecode { refs, confidence }
    } else {
        KnownHandleRefsDecode::default()
    }
}

#[pyfunction(signature = (path, handles, limit=None))]
pub fn decode_object_handle_stream_refs(
    path: &str,
    handles: Vec<u64>,
    limit: Option<usize>,
) -> PyResult<Vec<HandleStreamRefsRow>> {
    if handles.is_empty() {
        return Ok(Vec::new());
    }

    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let known_handles: HashSet<u64> = index.objects.iter().map(|obj| obj.handle.0).collect();
    let object_offsets: HashMap<u64, u32> = index
        .objects
        .iter()
        .map(|obj| (obj.handle.0, obj.offset))
        .collect();

    let mut result = Vec::new();
    for handle in handles {
        let Some(offset) = object_offsets.get(&handle).copied() else {
            continue;
        };
        let Some((record, header)) = parse_record_and_header(&decoder, offset, best_effort)? else {
            continue;
        };
        let decoded = decode_known_handle_refs_from_object_record(
            &record,
            decoder.version(),
            &header,
            handle,
            &known_handles,
            None,
            16,
        );
        result.push((handle, decoded.refs));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

fn acis_unknown_role_hint_from_type_code(type_code: u16, data_size: u32) -> &'static str {
    match type_code {
        0x214 => "acis-link-table",
        0x221 => "acis-header",
        0x222 => "acis-payload-chunk",
        0x223 | 0x224 | 0x225 => {
            if data_size >= 128 {
                "acis-payload-main"
            } else {
                "acis-payload-chunk"
            }
        }
        0x215..=0x220 => "acis-aux",
        _ if (0x214..=0x225).contains(&type_code) => "acis-aux",
        _ => "unknown",
    }
}

#[pyfunction(signature = (path, handles, limit=None))]
pub fn decode_acis_candidate_infos(
    path: &str,
    handles: Vec<u64>,
    limit: Option<usize>,
) -> PyResult<Vec<AcisCandidateInfoRow>> {
    if handles.is_empty() {
        return Ok(Vec::new());
    }

    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let known_handles: HashSet<u64> = index.objects.iter().map(|obj| obj.handle.0).collect();
    let object_type_codes = collect_object_type_codes(&decoder, &index, best_effort)?;
    let object_offsets: HashMap<u64, u32> = index
        .objects
        .iter()
        .map(|obj| (obj.handle.0, obj.offset))
        .collect();

    let mut result = Vec::new();
    for handle in handles {
        let Some(offset) = object_offsets.get(&handle).copied() else {
            continue;
        };
        let Some((record, header)) = parse_record_and_header(&decoder, offset, best_effort)? else {
            continue;
        };
        let decoded = decode_known_handle_refs_from_object_record(
            &record,
            decoder.version(),
            &header,
            handle,
            &known_handles,
            Some(&object_type_codes),
            16,
        );
        let role = acis_unknown_role_hint_from_type_code(header.type_code, header.data_size);
        result.push((
            handle,
            header.type_code,
            header.data_size,
            role.to_string(),
            decoded.refs,
            decoded.confidence,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_entity_styles(path: &str, limit: Option<usize>) -> PyResult<Vec<EntityStyleRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let decoded_layer_rows = decode_layer_colors(path, None)?;
    let decoded_layer_handles: Vec<u64> = decoded_layer_rows.iter().map(|(h, _, _)| *h).collect();
    let raw_layer_handles =
        collect_known_layer_handles_in_order(&decoder, &dynamic_types, &index, best_effort)?;
    let mut layer_handle_remap = HashMap::new();
    if raw_layer_handles.len() == decoded_layer_handles.len() {
        for (raw, decoded) in raw_layer_handles
            .iter()
            .copied()
            .zip(decoded_layer_handles.iter().copied())
        {
            layer_handle_remap.insert(raw, decoded);
        }
    }
    let mut known_layer_handles: HashSet<u64> = decoded_layer_handles.into_iter().collect();
    known_layer_handles.extend(raw_layer_handles.iter().copied());
    let mut result = Vec::new();

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        if matches_type_name(header.type_code, 0x13, "LINE", &dynamic_types) {
            let entity = match decode_line_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x1B, "POINT", &dynamic_types) {
            let entity = match decode_point_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x11, "ARC", &dynamic_types) {
            let entity =
                match decode_arc_for_version(&mut reader, decoder.version(), &header, obj.handle.0)
                {
                    Ok(entity) => entity,
                    Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                    Err(err) => return Err(to_py_err(err)),
                };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x12, "CIRCLE", &dynamic_types) {
            let entity = match decode_circle_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x23, "ELLIPSE", &dynamic_types) {
            let entity = match decode_ellipse_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x24, "SPLINE", &dynamic_types) {
            let entity = match decode_spline_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x01, "TEXT", &dynamic_types) {
            let entity = match decode_text_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x02, "ATTRIB", &dynamic_types) {
            let entity = match decode_attrib_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x03, "ATTDEF", &dynamic_types) {
            let entity = match decode_attdef_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x2C, "MTEXT", &dynamic_types) {
            let entity = match decode_mtext_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x2D, "LEADER", &dynamic_types) {
            let entity = match decode_leader_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x4E, "HATCH", &dynamic_types) {
            let entity = match decode_hatch_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x2E, "TOLERANCE", &dynamic_types) {
            let entity = match decode_tolerance_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x2F, "MLINE", &dynamic_types) {
            let entity = match decode_mline_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x4D, "LWPOLYLINE", &dynamic_types) {
            let entity = match decode_lwpolyline_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x10, "POLYLINE_3D", &dynamic_types) {
            let entity = match decode_polyline_3d_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x1E, "POLYLINE_MESH", &dynamic_types) {
            let entity = match decode_polyline_mesh_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x1D, "POLYLINE_PFACE", &dynamic_types) {
            let entity = match decode_polyline_pface_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x1C, "3DFACE", &dynamic_types) {
            let entity = match decode_3dface_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x1F, "SOLID", &dynamic_types) {
            let entity = match decode_solid_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x20, "TRACE", &dynamic_types) {
            let entity = match decode_trace_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x21, "SHAPE", &dynamic_types) {
            let entity = match decode_shape_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x22, "VIEWPORT", &dynamic_types) {
            let entity = match decode_viewport_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x2B, "OLEFRAME", &dynamic_types) {
            let entity = match decode_oleframe_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x4A, "OLE2FRAME", &dynamic_types) {
            let entity = match decode_ole2frame_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x4C, "LONG_TRANSACTION", &dynamic_types) {
            let entity = match decode_long_transaction_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x25, "REGION", &dynamic_types) {
            let entity = match decode_region_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x26, "3DSOLID", &dynamic_types) {
            let entity = match decode_3dsolid_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x27, "BODY", &dynamic_types) {
            let entity = match decode_body_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x28, "RAY", &dynamic_types) {
            let entity =
                match decode_ray_for_version(&mut reader, decoder.version(), &header, obj.handle.0)
                {
                    Ok(entity) => entity,
                    Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                    Err(err) => return Err(to_py_err(err)),
                };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x29, "XLINE", &dynamic_types) {
            let entity = match decode_xline_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x15, "DIM_LINEAR", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                common.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x14, "DIM_ORDINATE", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                common.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x16, "DIM_ALIGNED", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                common.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x17, "DIM_ANG3PT", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                common.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x18, "DIM_ANG2LN", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                common.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x1A, "DIM_DIAMETER", &dynamic_types) {
            let entity = match decode_dim_diameter_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                common.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x19, "DIM_RADIUS", &dynamic_types) {
            let entity = match decode_dim_radius_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            let layer_handle = recover_entity_layer_handle_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                common.layer_handle,
                &known_layer_handles,
            );
            let layer_handle = layer_handle_remap
                .get(&layer_handle)
                .copied()
                .unwrap_or(layer_handle);
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                layer_handle,
            ));
        } else {
            continue;
        }

        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_layer_colors(path: &str, limit: Option<usize>) -> PyResult<Vec<LayerColorRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x33, "LAYER", &dynamic_types) {
            continue;
        }

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let (handle, color_index, true_color) =
            match decode_layer_color_record(&mut reader, decoder.version(), obj.handle.0) {
                Ok(decoded) => decoded,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((handle, color_index, true_color));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_line_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<LineEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x13, "LINE", &dynamic_types) {
            continue;
        }
        let mut entity: Option<entities::LineEntity> = None;
        let mut last_err = None;
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix {
                if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
                    last_err = Some(err);
                    continue;
                }
            }
            match decode_line_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(decoded) => {
                    if !is_plausible_line_entity_candidate(&decoded) {
                        continue;
                    }
                    entity = Some(decoded);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        let entity = match entity {
            Some(entity) => entity,
            None if best_effort => continue,
            None => {
                if let Some(err) = last_err {
                    return Err(to_py_err(err));
                }
                continue;
            }
        };
        result.push((
            entity.handle,
            entity.start.0,
            entity.start.1,
            entity.start.2,
            entity.end.0,
            entity.end.1,
            entity.end.2,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

fn is_plausible_line_entity_candidate(entity: &entities::LineEntity) -> bool {
    let values = [
        entity.start.0,
        entity.start.1,
        entity.start.2,
        entity.end.0,
        entity.end.1,
        entity.end.2,
    ];
    if values.iter().any(|value| !value.is_finite()) {
        return false;
    }
    let max_abs = values
        .iter()
        .fold(0.0_f64, |acc, value| acc.max(value.abs()));
    if max_abs > 1.0e8 {
        return false;
    }
    true
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_point_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<PointEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x1B, "POINT", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_point_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => {
                    if std::env::var("EZDWG_DEBUG_POINT_DECODE")
                        .ok()
                        .is_some_and(|value| value != "0")
                    {
                        eprintln!(
                            "[point-decode] handle={} type=0x{:X} offset={} error={}",
                            obj.handle.0, header.type_code, obj.offset, err
                        );
                    }
                    continue;
                }
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((
            entity.handle,
            entity.location.0,
            entity.location.1,
            entity.location.2,
            entity.x_axis_angle,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_3dface_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<Face3dEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x1C, "3DFACE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_3dface_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.p1,
            entity.p2,
            entity.p3,
            entity.p4,
            entity.invisible_edge_flags,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_arc_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<ArcEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x11, "ARC", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_arc_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((
            entity.handle,
            entity.center.0,
            entity.center.1,
            entity.center.2,
            entity.radius,
            entity.angle_start,
            entity.angle_end,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_circle_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<CircleEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x12, "CIRCLE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_circle_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.center.0,
            entity.center.1,
            entity.center.2,
            entity.radius,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_line_arc_circle_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<LineArcCircleRows> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut lines: Vec<LineEntityRow> = Vec::new();
    let mut arcs: Vec<ArcEntityRow> = Vec::new();
    let mut circles: Vec<CircleEntityRow> = Vec::new();
    let mut total = 0usize;

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        let is_line = matches_type_name(header.type_code, 0x13, "LINE", &dynamic_types);
        let is_arc = matches_type_name(header.type_code, 0x11, "ARC", &dynamic_types);
        let is_circle = matches_type_name(header.type_code, 0x12, "CIRCLE", &dynamic_types);
        if !(is_line || is_arc || is_circle) {
            continue;
        }

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }

        if is_line {
            let entity = match decode_line_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            lines.push((
                entity.handle,
                entity.start.0,
                entity.start.1,
                entity.start.2,
                entity.end.0,
                entity.end.1,
                entity.end.2,
            ));
            total += 1;
        } else if is_arc {
            let entity =
                match decode_arc_for_version(&mut reader, decoder.version(), &header, obj.handle.0)
                {
                    Ok(entity) => entity,
                    Err(err) if best_effort => continue,
                    Err(err) => return Err(to_py_err(err)),
                };
            arcs.push((
                entity.handle,
                entity.center.0,
                entity.center.1,
                entity.center.2,
                entity.radius,
                entity.angle_start,
                entity.angle_end,
            ));
            total += 1;
        } else if is_circle {
            let entity = match decode_circle_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            circles.push((
                entity.handle,
                entity.center.0,
                entity.center.1,
                entity.center.2,
                entity.radius,
            ));
            total += 1;
        }

        if let Some(limit) = limit {
            if total >= limit {
                break;
            }
        }
    }

    Ok((lines, arcs, circles))
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_ellipse_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<EllipseEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x23, "ELLIPSE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_ellipse_for_version(&mut reader, decoder.version(), &header, obj.handle.0)
            {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((
            entity.handle,
            entity.center,
            entity.major_axis,
            entity.extrusion,
            entity.axis_ratio,
            entity.start_angle,
            entity.end_angle,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_spline_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<SplineEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x24, "SPLINE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_spline_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            (
                entity.scenario,
                entity.degree,
                entity.rational,
                entity.closed,
                entity.periodic,
            ),
            (
                entity.fit_tolerance,
                entity.knot_tolerance,
                entity.ctrl_tolerance,
            ),
            entity.knots,
            entity.control_points,
            entity.weights,
            entity.fit_points,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_text_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<TextEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x01, "TEXT", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_text_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((
            entity.handle,
            entity.text,
            entity.insertion,
            entity.alignment,
            entity.extrusion,
            (
                entity.thickness,
                entity.oblique_angle,
                entity.height,
                entity.rotation,
                entity.width_factor,
            ),
            (
                entity.generation,
                entity.horizontal_alignment,
                entity.vertical_alignment,
            ),
            entity.style_handle,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_attrib_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<AttribEntityRow>> {
    decode_attrib_like_entities_by_type(
        path,
        limit,
        0x02,
        "ATTRIB",
        |reader, version, header, object_handle| {
            decode_attrib_for_version(reader, version, header, object_handle)
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_attdef_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<AttribEntityRow>> {
    decode_attrib_like_entities_by_type(
        path,
        limit,
        0x03,
        "ATTDEF",
        |reader, version, header, object_handle| {
            decode_attdef_for_version(reader, version, header, object_handle)
        },
    )
}

fn decode_attrib_like_entities_by_type<F>(
    path: &str,
    limit: Option<usize>,
    type_code: u16,
    type_name: &str,
    mut decode_entity: F,
) -> PyResult<Vec<AttribEntityRow>>
where
    F: FnMut(
        &mut BitReader<'_>,
        &version::DwgVersion,
        &ApiObjectHeader,
        u64,
    ) -> crate::core::result::Result<entities::AttribEntity>,
{
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, type_code, type_name, &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_entity(&mut reader, decoder.version(), &header, obj.handle.0) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.text,
            entity.tag,
            entity.prompt,
            entity.insertion,
            entity.alignment,
            entity.extrusion,
            (
                entity.thickness,
                entity.oblique_angle,
                entity.height,
                entity.rotation,
                entity.width_factor,
            ),
            (
                entity.generation,
                entity.horizontal_alignment,
                entity.vertical_alignment,
            ),
            entity.flags,
            entity.lock_position,
            (entity.style_handle, entity.owner_handle),
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_mtext_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<MTextEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x2C, "MTEXT", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let reader_after_prefix = reader.clone();
        let mut entity =
            match decode_mtext_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        if matches!(
            decoder.version(),
            version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
        ) {
            if let Some(recovered_text) =
                recover_r2010_mtext_text(&reader_after_prefix, &header, entity.text.as_str())
            {
                entity.text = recovered_text;
            }
        }
        result.push((
            entity.handle,
            entity.text,
            entity.insertion,
            entity.extrusion,
            entity.x_axis_dir,
            entity.rect_width,
            entity.text_height,
            entity.attachment,
            entity.drawing_dir,
            (
                entity.background_flags,
                entity.background_scale_factor,
                entity.background_color_index,
                entity.background_true_color,
                entity.background_transparency,
            ),
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_leader_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<LeaderEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x2D, "LEADER", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_leader_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.annotation_type,
            entity.path_type,
            entity.points,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_hatch_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<HatchEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x4E, "HATCH", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_hatch_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        let paths: Vec<HatchPathRow> = entity
            .paths
            .into_iter()
            .map(|path| (path.closed, path.points))
            .collect();
        result.push((
            entity.handle,
            entity.name,
            entity.solid_fill,
            entity.associative,
            entity.elevation,
            entity.extrusion,
            paths,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_tolerance_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<ToleranceEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x2E, "TOLERANCE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_tolerance_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.text,
            entity.insertion,
            entity.x_direction,
            entity.extrusion,
            entity.height,
            entity.dimgap,
            entity.dimstyle_handle,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_mline_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<MLineEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x2F, "MLINE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_mline_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        let vertices: Vec<MLineVertexRow> = entity
            .vertices
            .iter()
            .map(|vertex| {
                (
                    vertex.position,
                    vertex.vertex_direction,
                    vertex.miter_direction,
                )
            })
            .collect();
        result.push((
            entity.handle,
            entity.scale,
            entity.justification,
            entity.base_point,
            entity.extrusion,
            entity.open_closed,
            entity.lines_in_style,
            vertices,
            entity.mlinestyle_handle,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dim_linear_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<DimEntityRow>> {
    decode_dim_entities_by_type(
        path,
        limit,
        0x15,
        "DIM_LINEAR",
        |reader, version, header, object_handle| {
            let entity = decode_dim_linear_for_version(reader, version, header, object_handle)?;
            Ok(dim_entity_row_from_linear_like(&entity))
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dim_ordinate_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<DimEntityRow>> {
    decode_dim_entities_by_type(
        path,
        limit,
        0x14,
        "DIM_ORDINATE",
        |reader, version, header, object_handle| {
            let entity = decode_dim_linear_for_version(reader, version, header, object_handle)?;
            Ok(dim_entity_row_from_linear_like(&entity))
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dim_diameter_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<DimEntityRow>> {
    decode_dim_entities_by_type(
        path,
        limit,
        0x1A,
        "DIM_DIAMETER",
        |reader, version, header, object_handle| {
            let entity = decode_dim_diameter_for_version(reader, version, header, object_handle)?;
            Ok(dim_entity_row_from_linear_like(&entity))
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dim_aligned_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<DimEntityRow>> {
    decode_dim_entities_by_type(
        path,
        limit,
        0x16,
        "DIM_ALIGNED",
        |reader, version, header, object_handle| {
            let entity = decode_dim_linear_for_version(reader, version, header, object_handle)?;
            Ok(dim_entity_row_from_linear_like(&entity))
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dim_ang3pt_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<DimEntityRow>> {
    decode_dim_entities_by_type(
        path,
        limit,
        0x17,
        "DIM_ANG3PT",
        |reader, version, header, object_handle| {
            let entity = decode_dim_linear_for_version(reader, version, header, object_handle)?;
            Ok(dim_entity_row_from_linear_like(&entity))
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dim_ang2ln_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<DimEntityRow>> {
    decode_dim_entities_by_type(
        path,
        limit,
        0x18,
        "DIM_ANG2LN",
        |reader, version, header, object_handle| {
            let entity = decode_dim_linear_for_version(reader, version, header, object_handle)?;
            Ok(dim_entity_row_from_linear_like(&entity))
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dim_radius_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<DimEntityRow>> {
    decode_dim_entities_by_type(
        path,
        limit,
        0x19,
        "DIM_RADIUS",
        |reader, version, header, object_handle| {
            let entity = decode_dim_radius_for_version(reader, version, header, object_handle)?;
            Ok(dim_entity_row_from_linear_like(&entity))
        },
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_dimension_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<DimTypedEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result: Vec<DimTypedEntityRow> = Vec::new();

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }

        let maybe_row = if matches_type_name(header.type_code, 0x15, "DIM_LINEAR", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            Some(("LINEAR", dim_entity_row_from_linear_like(&entity)))
        } else if matches_type_name(header.type_code, 0x14, "DIM_ORDINATE", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            Some(("ORDINATE", dim_entity_row_from_linear_like(&entity)))
        } else if matches_type_name(header.type_code, 0x16, "DIM_ALIGNED", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            Some(("ALIGNED", dim_entity_row_from_linear_like(&entity)))
        } else if matches_type_name(header.type_code, 0x17, "DIM_ANG3PT", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            Some(("ANG3PT", dim_entity_row_from_linear_like(&entity)))
        } else if matches_type_name(header.type_code, 0x18, "DIM_ANG2LN", &dynamic_types) {
            let entity = match decode_dim_linear_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            Some(("ANG2LN", dim_entity_row_from_linear_like(&entity)))
        } else if matches_type_name(header.type_code, 0x1A, "DIM_DIAMETER", &dynamic_types) {
            let entity = match decode_dim_diameter_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            Some(("DIAMETER", dim_entity_row_from_linear_like(&entity)))
        } else if matches_type_name(header.type_code, 0x19, "DIM_RADIUS", &dynamic_types) {
            let entity = match decode_dim_radius_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            Some(("RADIUS", dim_entity_row_from_linear_like(&entity)))
        } else {
            None
        };

        if let Some((dimtype, row)) = maybe_row {
            result.push((dimtype.to_string(), row));
            if let Some(limit) = limit {
                if result.len() >= limit {
                    break;
                }
            }
        }
    }

    Ok(result)
}

fn decode_dim_entities_by_type<F>(
    path: &str,
    limit: Option<usize>,
    type_code: u16,
    type_name: &str,
    mut decode_entity_row: F,
) -> PyResult<Vec<DimEntityRow>>
where
    F: FnMut(
        &mut BitReader<'_>,
        &version::DwgVersion,
        &ApiObjectHeader,
        u64,
    ) -> crate::core::result::Result<DimEntityRow>,
{
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();

    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, type_code, type_name, &dynamic_types) {
            continue;
        }

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }

        let row = match decode_entity_row(&mut reader, decoder.version(), &header, obj.handle.0) {
            Ok(row) => row,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push(row);

        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

fn dim_entity_row_from_linear_like(entity: &entities::DimLinearEntity) -> DimEntityRow {
    let common = &entity.common;
    (
        common.handle,
        common.user_text.clone(),
        entity.point10,
        entity.point13,
        entity.point14,
        common.text_midpoint,
        common.insert_point,
        (common.extrusion, common.insert_scale),
        (
            common.text_rotation,
            common.horizontal_direction,
            entity.ext_line_rotation,
            entity.dim_rotation,
        ),
        (
            common.dim_flags,
            common.actual_measurement,
            common.attachment_point,
            common.line_spacing_style,
            common.line_spacing_factor,
            common.insert_rotation,
        ),
        (common.dimstyle_handle, common.anonymous_block_handle),
    )
}

struct InsertNameResolutionState {
    known_block_handles: HashSet<u64>,
    block_header_names: HashMap<u64, String>,
    named_block_handles: HashSet<u64>,
}

fn prepare_insert_name_resolution_state(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
) -> PyResult<InsertNameResolutionState> {
    let block_header_entries =
        collect_block_header_name_entries_in_order(decoder, dynamic_types, index, best_effort)?;
    let mut known_block_handles: HashSet<u64> = HashSet::new();
    let mut block_header_names: HashMap<u64, String> = HashMap::new();
    let mut block_header_decoded_by_raw: HashMap<u64, u64> = HashMap::new();
    for (raw_handle, decoded_handle, name) in block_header_entries {
        block_header_decoded_by_raw.insert(raw_handle, decoded_handle);
        known_block_handles.insert(raw_handle);
        known_block_handles.insert(decoded_handle);
        if name.is_empty() {
            continue;
        }
        block_header_names
            .entry(raw_handle)
            .or_insert_with(|| name.clone());
        block_header_names.entry(decoded_handle).or_insert(name);
    }
    let (block_name_aliases, recovered_header_names) = collect_block_name_aliases_in_order(
        decoder,
        dynamic_types,
        index,
        best_effort,
        &block_header_names,
    )?;
    for (header_handle, name) in recovered_header_names {
        if name.is_empty() {
            continue;
        }
        known_block_handles.insert(header_handle);
        block_header_names
            .entry(header_handle)
            .or_insert_with(|| name.clone());
        if let Some(decoded_handle) = block_header_decoded_by_raw.get(&header_handle).copied() {
            known_block_handles.insert(decoded_handle);
            block_header_names.entry(decoded_handle).or_insert(name);
        }
    }
    for (alias_handle, name) in block_name_aliases {
        known_block_handles.insert(alias_handle);
        block_header_names.entry(alias_handle).or_insert(name);
    }
    let block_record_aliases = collect_block_record_handle_aliases_in_order(
        decoder,
        dynamic_types,
        index,
        best_effort,
        &block_header_names,
    )?;
    for (alias_handle, name) in block_record_aliases {
        known_block_handles.insert(alias_handle);
        block_header_names.entry(alias_handle).or_insert(name);
    }
    let object_type_codes = collect_object_type_codes(decoder, index, best_effort)?;
    let known_layer_handles: HashSet<u64> =
        collect_known_layer_handles_in_order(decoder, dynamic_types, index, best_effort)?
            .into_iter()
            .collect();
    let stream_aliases = collect_block_header_stream_aliases_in_order(
        decoder,
        dynamic_types,
        index,
        best_effort,
        &block_header_names,
        &object_type_codes,
        &known_layer_handles,
    )?;
    for (alias_handle, name) in stream_aliases {
        known_block_handles.insert(alias_handle);
        block_header_names.entry(alias_handle).or_insert(name);
    }
    let named_block_handles: HashSet<u64> = block_header_names.keys().copied().collect();
    Ok(InsertNameResolutionState {
        known_block_handles,
        block_header_names,
        named_block_handles,
    })
}

fn decode_insert_entities_with_state(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    state: &mut InsertNameResolutionState,
    limit: Option<usize>,
) -> PyResult<Vec<InsertEntityRow>> {
    let mut decoded_rows: Vec<(u64, f64, f64, f64, f64, f64, f64, f64, Option<u64>)> = Vec::new();
    let mut unresolved_insert_candidates: HashMap<u64, Vec<u64>> = HashMap::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x07, "INSERT", dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_insert_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let resolved_block_handle = recover_insert_block_header_handle_r2010_plus(
            &record,
            decoder.version(),
            &header,
            obj.handle.0,
            entity.block_header_handle,
            &state.known_block_handles,
            &state.named_block_handles,
        );
        decoded_rows.push((
            entity.handle,
            entity.position.0,
            entity.position.1,
            entity.position.2,
            entity.scale.0,
            entity.scale.1,
            entity.scale.2,
            entity.rotation,
            resolved_block_handle,
        ));
        if let Some(limit) = limit {
            if decoded_rows.len() >= limit {
                break;
            }
        }
    }
    let unresolved_handles: HashSet<u64> = decoded_rows
        .iter()
        .filter_map(|row| row.8)
        .filter(|handle| !state.block_header_names.contains_key(handle))
        .collect();
    if !unresolved_handles.is_empty() {
        let targeted_aliases = collect_block_header_targeted_aliases_in_order(
            decoder,
            dynamic_types,
            index,
            best_effort,
            &state.block_header_names,
            &unresolved_handles,
        )?;
        for (alias_handle, name) in targeted_aliases {
            state.known_block_handles.insert(alias_handle);
            state.block_header_names.entry(alias_handle).or_insert(name);
        }
    }
    let unresolved_insert_handles: HashSet<u64> = decoded_rows
        .iter()
        .filter_map(|row| {
            let missing = row
                .8
                .and_then(|handle| state.block_header_names.get(&handle))
                .is_none();
            if missing {
                Some(row.0)
            } else {
                None
            }
        })
        .collect();
    if !unresolved_insert_handles.is_empty() {
        let mut extra_targets: HashSet<u64> = HashSet::new();
        for obj in index.objects.iter() {
            if !unresolved_insert_handles.contains(&obj.handle.0) {
                continue;
            }
            let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
            else {
                continue;
            };
            if !matches_type_name(header.type_code, 0x07, "INSERT", dynamic_types) {
                continue;
            }
            let mut reader = record.bit_reader();
            if skip_object_type_prefix(&mut reader, decoder.version()).is_err() {
                continue;
            }
            let Ok(entity) =
                decode_insert_for_version(&mut reader, decoder.version(), &header, obj.handle.0)
            else {
                continue;
            };
            let candidates = collect_insert_block_handle_candidates_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                entity.block_header_handle,
                Some(&state.known_block_handles),
                8,
            );
            if candidates.is_empty() {
                continue;
            }
            for candidate in candidates.iter().copied().take(4) {
                if !state.block_header_names.contains_key(&candidate) {
                    extra_targets.insert(candidate);
                }
            }
            unresolved_insert_candidates.insert(obj.handle.0, candidates);
        }
        if !extra_targets.is_empty() {
            let targeted_aliases = collect_block_header_targeted_aliases_in_order(
                decoder,
                dynamic_types,
                index,
                best_effort,
                &state.block_header_names,
                &extra_targets,
            )?;
            for (alias_handle, name) in targeted_aliases {
                state.known_block_handles.insert(alias_handle);
                state.block_header_names.entry(alias_handle).or_insert(name);
            }
        }
    }

    let available_named_handles: Vec<u64> = state.block_header_names.keys().copied().collect();
    let mut result = Vec::with_capacity(decoded_rows.len());
    let debug_insert_names = std::env::var("EZDWG_DEBUG_INSERT_NAMES")
        .ok()
        .is_some_and(|v| v != "0");
    for (handle, px, py, pz, sx, sy, sz, rotation, block_handle) in decoded_rows {
        let mut resolved_name =
            block_handle.and_then(|h| state.block_header_names.get(&h).cloned());
        if resolved_name.is_none() {
            if let Some(candidates) = unresolved_insert_candidates.get(&handle) {
                resolved_name = candidates
                    .iter()
                    .find_map(|candidate| state.block_header_names.get(candidate).cloned());
                if resolved_name.is_none() {
                    let mut nearby_names: HashSet<String> = HashSet::new();
                    for candidate in candidates {
                        for known in &available_named_handles {
                            if known.abs_diff(*candidate) <= 8 {
                                if let Some(name) = state.block_header_names.get(known) {
                                    nearby_names.insert(name.clone());
                                }
                            }
                        }
                    }
                    if nearby_names.len() == 1 {
                        resolved_name = nearby_names.into_iter().next();
                    }
                }
            }
        }
        if debug_insert_names {
            let candidate_debug = unresolved_insert_candidates.get(&handle);
            eprintln!(
                "[insert-name] insert={} block_handle={:?} name={:?} candidates={:?}",
                handle, block_handle, resolved_name, candidate_debug
            );
        }
        result.push((handle, px, py, pz, sx, sy, sz, rotation, resolved_name));
    }
    Ok(result)
}

fn decode_minsert_entities_with_state(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    state: &mut InsertNameResolutionState,
    limit: Option<usize>,
) -> PyResult<Vec<MInsertEntityRow>> {
    let mut decoded_rows: Vec<(
        u64,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        u16,
        u16,
        f64,
        f64,
        Option<u64>,
    )> = Vec::new();
    let mut unresolved_minsert_candidates: HashMap<u64, Vec<u64>> = HashMap::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x08, "MINSERT", dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match entities::decode_minsert(&mut reader) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let block_handle = recover_insert_block_header_handle_r2010_plus(
            &record,
            decoder.version(),
            &header,
            obj.handle.0,
            None,
            &state.known_block_handles,
            &state.named_block_handles,
        );
        decoded_rows.push((
            entity.handle,
            entity.position.0,
            entity.position.1,
            entity.position.2,
            entity.scale.0,
            entity.scale.1,
            entity.scale.2,
            entity.rotation,
            entity.num_columns,
            entity.num_rows,
            entity.column_spacing,
            entity.row_spacing,
            block_handle,
        ));
        if let Some(limit) = limit {
            if decoded_rows.len() >= limit {
                break;
            }
        }
    }

    let unresolved_handles: HashSet<u64> = decoded_rows
        .iter()
        .filter_map(|row| row.12)
        .filter(|handle| !state.block_header_names.contains_key(handle))
        .collect();
    if !unresolved_handles.is_empty() {
        let targeted_aliases = collect_block_header_targeted_aliases_in_order(
            decoder,
            dynamic_types,
            index,
            best_effort,
            &state.block_header_names,
            &unresolved_handles,
        )?;
        for (alias_handle, name) in targeted_aliases {
            state.known_block_handles.insert(alias_handle);
            state.block_header_names.entry(alias_handle).or_insert(name);
        }
    }
    let unresolved_minsert_handles: HashSet<u64> = decoded_rows
        .iter()
        .filter_map(|row| {
            let missing = row
                .12
                .and_then(|handle| state.block_header_names.get(&handle))
                .is_none();
            if missing {
                Some(row.0)
            } else {
                None
            }
        })
        .collect();
    if !unresolved_minsert_handles.is_empty() {
        let mut extra_targets: HashSet<u64> = HashSet::new();
        for obj in index.objects.iter() {
            if !unresolved_minsert_handles.contains(&obj.handle.0) {
                continue;
            }
            let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
            else {
                continue;
            };
            if !matches_type_name(header.type_code, 0x08, "MINSERT", dynamic_types) {
                continue;
            }
            let mut reader = record.bit_reader();
            if skip_object_type_prefix(&mut reader, decoder.version()).is_err() {
                continue;
            }
            let Ok(_entity) = entities::decode_minsert(&mut reader) else {
                continue;
            };
            let candidates = collect_insert_block_handle_candidates_r2010_plus(
                &record,
                decoder.version(),
                &header,
                obj.handle.0,
                None,
                Some(&state.known_block_handles),
                8,
            );
            if candidates.is_empty() {
                continue;
            }
            for candidate in candidates.iter().copied().take(4) {
                if !state.block_header_names.contains_key(&candidate) {
                    extra_targets.insert(candidate);
                }
            }
            unresolved_minsert_candidates.insert(obj.handle.0, candidates);
        }
        if !extra_targets.is_empty() {
            let targeted_aliases = collect_block_header_targeted_aliases_in_order(
                decoder,
                dynamic_types,
                index,
                best_effort,
                &state.block_header_names,
                &extra_targets,
            )?;
            for (alias_handle, name) in targeted_aliases {
                state.known_block_handles.insert(alias_handle);
                state.block_header_names.entry(alias_handle).or_insert(name);
            }
        }
    }

    let available_named_handles: Vec<u64> = state.block_header_names.keys().copied().collect();
    let mut result = Vec::with_capacity(decoded_rows.len());
    for (
        handle,
        px,
        py,
        pz,
        sx,
        sy,
        sz,
        rotation,
        num_columns,
        num_rows,
        column_spacing,
        row_spacing,
        block_handle,
    ) in decoded_rows
    {
        let mut resolved_name =
            block_handle.and_then(|h| state.block_header_names.get(&h).cloned());
        if resolved_name.is_none() {
            if let Some(candidates) = unresolved_minsert_candidates.get(&handle) {
                resolved_name = candidates
                    .iter()
                    .find_map(|candidate| state.block_header_names.get(candidate).cloned());
                if resolved_name.is_none() {
                    let mut nearby_names: HashSet<String> = HashSet::new();
                    for candidate in candidates {
                        for known in &available_named_handles {
                            if known.abs_diff(*candidate) <= 8 {
                                if let Some(name) = state.block_header_names.get(known) {
                                    nearby_names.insert(name.clone());
                                }
                            }
                        }
                    }
                    if nearby_names.len() == 1 {
                        resolved_name = nearby_names.into_iter().next();
                    }
                }
            }
        }
        result.push((
            handle,
            px,
            py,
            pz,
            sx,
            sy,
            sz,
            rotation,
            (
                num_columns,
                num_rows,
                column_spacing,
                row_spacing,
                resolved_name,
            ),
        ));
    }

    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_insert_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<InsertEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut state =
        prepare_insert_name_resolution_state(&decoder, &dynamic_types, &index, best_effort)?;
    decode_insert_entities_with_state(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        &mut state,
        limit,
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_minsert_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<MInsertEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut state =
        prepare_insert_name_resolution_state(&decoder, &dynamic_types, &index, best_effort)?;
    decode_minsert_entities_with_state(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        &mut state,
        limit,
    )
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_insert_minsert_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<InsertMInsertRows> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut state =
        prepare_insert_name_resolution_state(&decoder, &dynamic_types, &index, best_effort)?;
    let inserts = decode_insert_entities_with_state(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        &mut state,
        limit,
    )?;
    let minserts = decode_minsert_entities_with_state(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        &mut state,
        limit,
    )?;
    Ok((inserts, minserts))
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_block_header_names(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<BlockHeaderNameRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let names =
        collect_block_header_names_in_order(&decoder, &dynamic_types, &index, best_effort, None)?;
    let mut rows: Vec<BlockHeaderNameRow> = names.into_iter().collect();
    rows.sort_by_key(|(handle, _)| *handle);
    if let Some(limit) = limit {
        rows.truncate(limit);
    }
    Ok(rows)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_block_entity_names(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<BlockEntityNameRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut ordered_objects: Vec<_> = index.objects.iter().collect();
    ordered_objects.sort_by_key(|obj| obj.offset);
    let is_r2010_plus = matches!(
        decoder.version(),
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    );
    let header_entries =
        collect_block_header_name_entries_in_order(&decoder, &dynamic_types, &index, best_effort)?;
    let mut header_entry_name_by_handle: HashMap<u64, String> = HashMap::new();
    for (raw_handle, decoded_handle, name) in header_entries.iter() {
        if name.is_empty() {
            continue;
        }
        header_entry_name_by_handle
            .entry(*raw_handle)
            .or_insert_with(|| name.clone());
        header_entry_name_by_handle
            .entry(*decoded_handle)
            .or_insert_with(|| name.clone());
    }
    let block_header_names =
        collect_block_header_names_in_order(&decoder, &dynamic_types, &index, best_effort, None)?;
    let (mut block_aliases, mut endblk_aliases) = collect_block_and_endblk_handle_aliases_in_order(
        &decoder,
        &dynamic_types,
        &index,
        best_effort,
        &block_header_names,
    )?;
    let header_names_in_order: Vec<String> = if is_r2010_plus {
        header_entries
            .into_iter()
            .filter_map(
                |(_raw_handle, _decoded_handle, name)| {
                    if name.is_empty() {
                        None
                    } else {
                        Some(name)
                    }
                },
            )
            .collect()
    } else {
        let mut names = Vec::new();
        for obj in ordered_objects.iter().copied() {
            let Some((_record, header)) =
                parse_record_and_header(&decoder, obj.offset, best_effort)?
            else {
                continue;
            };
            if !matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", &dynamic_types) {
                continue;
            }
            if let Some(name) = block_header_names
                .get(&obj.handle.0)
                .cloned()
                .filter(|value| !value.is_empty())
            {
                names.push(name);
                continue;
            }
            if let Some(name) = header_entry_name_by_handle
                .get(&obj.handle.0)
                .cloned()
                .filter(|value| !value.is_empty())
            {
                names.push(name);
            }
        }
        names
    };
    let mut block_handles_in_order: Vec<u64> = Vec::new();
    let mut endblk_handles_in_order: Vec<u64> = Vec::new();
    for obj in ordered_objects.iter().copied() {
        let Some((_record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if matches_type_name(header.type_code, 0x04, "BLOCK", &dynamic_types) {
            block_handles_in_order.push(obj.handle.0);
            continue;
        }
        if matches_type_name(header.type_code, 0x05, "ENDBLK", &dynamic_types) {
            endblk_handles_in_order.push(obj.handle.0);
        }
    }

    if is_r2010_plus {
        let block_targets: HashSet<u64> = block_handles_in_order.iter().copied().collect();
        let endblk_targets: HashSet<u64> = endblk_handles_in_order.iter().copied().collect();
        if !block_targets.is_empty() {
            for (handle, name) in collect_block_header_targeted_aliases_in_order(
                &decoder,
                &dynamic_types,
                &index,
                best_effort,
                &block_header_names,
                &block_targets,
            )? {
                if !name.is_empty() {
                    block_aliases.insert(handle, name);
                }
            }
        }
        if !endblk_targets.is_empty() {
            for (handle, name) in collect_block_header_targeted_aliases_in_order(
                &decoder,
                &dynamic_types,
                &index,
                best_effort,
                &block_header_names,
                &endblk_targets,
            )? {
                if !name.is_empty() {
                    endblk_aliases.insert(handle, name);
                }
            }
        }
    }

    if !header_names_in_order.is_empty() {
        if !is_r2010_plus && block_handles_in_order.len() == header_names_in_order.len() {
            block_aliases = HashMap::new();
            for (handle, name) in block_handles_in_order
                .iter()
                .copied()
                .zip(header_names_in_order.iter())
            {
                block_aliases.insert(handle, name.clone());
            }
        } else {
            for (index, handle) in block_handles_in_order.iter().copied().enumerate() {
                if block_aliases.contains_key(&handle) {
                    continue;
                }
                if let Some(name) = header_names_in_order.get(index) {
                    block_aliases.insert(handle, name.clone());
                }
            }
        }

        if !is_r2010_plus && endblk_handles_in_order.len() == header_names_in_order.len() {
            endblk_aliases = HashMap::new();
            for (handle, name) in endblk_handles_in_order
                .iter()
                .copied()
                .zip(header_names_in_order.iter())
            {
                endblk_aliases.insert(handle, name.clone());
            }
        } else {
            for (index, handle) in endblk_handles_in_order.iter().copied().enumerate() {
                if endblk_aliases.contains_key(&handle) {
                    continue;
                }
                if let Some(name) = header_names_in_order.get(index) {
                    endblk_aliases.insert(handle, name.clone());
                }
            }
        }
    }

    let mut rows: Vec<BlockEntityNameRow> = Vec::new();
    rows.reserve(block_aliases.len().saturating_add(endblk_aliases.len()));
    for (handle, name) in block_aliases {
        rows.push((handle, "BLOCK".to_string(), name));
    }
    for (handle, name) in endblk_aliases {
        rows.push((handle, "ENDBLK".to_string(), name));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    if let Some(limit) = limit {
        rows.truncate(limit);
    }
    Ok(rows)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_2d_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Polyline2dEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        let declared_match =
            matches_type_name(header.type_code, 0x0F, "POLYLINE_2D", &dynamic_types);
        if !declared_match
            && !is_r14_polyline_2d_speculative_type(decoder.version(), header.type_code)
        {
            continue;
        }
        let mut entity = None;
        let mut last_err = None;
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix {
                if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
                    last_err = Some(err);
                    continue;
                }
            }
            match decode_polyline_2d_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(value) => {
                    entity = Some(value);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        let entity = match entity {
            Some(entity) => entity,
            None if !declared_match => continue,
            None if best_effort => continue,
            None => {
                if let Some(err) = last_err {
                    return Err(to_py_err(err));
                }
                continue;
            }
        };
        if !declared_match
            && std::env::var("EZDWG_DEBUG_R14_POLY2D")
                .ok()
                .is_some_and(|value| value != "0")
        {
            eprintln!(
                "[r14-poly2d] handle={} type=0x{:X} flags={} curve_type={} owned={} width=({:.6},{:.6}) thickness={:.6} elevation={:.6}",
                obj.handle.0,
                header.type_code,
                entity.flags,
                entity.curve_type,
                entity.owned_handles.len(),
                entity.width_start,
                entity.width_end,
                entity.thickness,
                entity.elevation,
            );
        }
        if !declared_match && !is_plausible_polyline_2d_entity(&entity) {
            continue;
        }
        result.push((
            entity.handle,
            entity.flags,
            entity.curve_type,
            entity.width_start,
            entity.width_end,
            entity.thickness,
            entity.elevation,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_2d_entities_interpreted(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Polyline2dInterpretedRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        let declared_match =
            matches_type_name(header.type_code, 0x0F, "POLYLINE_2D", &dynamic_types);
        if !declared_match
            && !is_r14_polyline_2d_speculative_type(decoder.version(), header.type_code)
        {
            continue;
        }
        let mut entity = None;
        let mut last_err = None;
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix {
                if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
                    last_err = Some(err);
                    continue;
                }
            }
            match decode_polyline_2d_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(value) => {
                    entity = Some(value);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        let entity = match entity {
            Some(entity) => entity,
            None if !declared_match => continue,
            None if best_effort => continue,
            None => {
                if let Some(err) = last_err {
                    return Err(to_py_err(err));
                }
                continue;
            }
        };
        if !declared_match
            && std::env::var("EZDWG_DEBUG_R14_POLY2D")
                .ok()
                .is_some_and(|value| value != "0")
        {
            eprintln!(
                "[r14-poly2d] handle={} type=0x{:X} flags={} curve_type={} owned={} width=({:.6},{:.6}) thickness={:.6} elevation={:.6}",
                obj.handle.0,
                header.type_code,
                entity.flags,
                entity.curve_type,
                entity.owned_handles.len(),
                entity.width_start,
                entity.width_end,
                entity.thickness,
                entity.elevation,
            );
        }
        if !declared_match && !is_plausible_polyline_2d_entity(&entity) {
            continue;
        }
        let info = entity.flags_info;
        let curve_label = entity.curve_type_info.label().to_string();
        result.push((
            entity.handle,
            entity.flags,
            entity.curve_type,
            curve_label,
            info.closed,
            info.curve_fit,
            info.spline_fit,
            info.is_3d_polyline,
            info.is_3d_mesh,
            info.is_closed_mesh,
            info.is_polyface_mesh,
            info.continuous_linetype,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_lwpolyline_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<LwPolylineEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x4D, "LWPOLYLINE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_lwpolyline_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => {
                if std::env::var("EZDWG_DEBUG_LWPOLYLINE")
                    .ok()
                    .is_some_and(|value| value != "0")
                {
                    eprintln!(
                        "[lwpolyline] skip handle={} type=0x{:X} err={}",
                        obj.handle.0, header.type_code, err
                    );
                }
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.flags,
            entity.vertices,
            entity.bulges,
            entity.widths,
            entity.const_width,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_3d_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Polyline3dEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x10, "POLYLINE_3D", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_polyline_3d_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((entity.handle, entity.flags_75_bits, entity.flags_70_bits));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_vertex_3d_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Vertex3dEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x0B, "VERTEX_3D", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_vertex_3d_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.flags,
            entity.position.0,
            entity.position.1,
            entity.position.2,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_3d_with_vertices(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Polyline3dVerticesRow>> {
    let decoded_rows = decode_polyline_3d_vertex_rows(path, limit)?;
    let mut result = Vec::with_capacity(decoded_rows.len());
    for row in decoded_rows {
        let mut vertices: Vec<Point3> = row.vertices.iter().map(|vertex| vertex.position).collect();
        if row.closed && vertices.len() > 1 {
            let first = vertices[0];
            let last = *vertices.last().unwrap();
            if !points_equal_3d(first, last) {
                vertices.push(first);
            }
        }
        result.push((row.handle, row.flags_70_bits, row.closed, vertices));
    }
    Ok(result)
}

#[derive(Debug, Clone)]
struct Polyline3dVertexRow {
    handle: u64,
    flags_70_bits: u8,
    closed: bool,
    vertices: Vec<entities::Vertex3dEntity>,
}

fn decode_polyline_3d_vertex_rows(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Polyline3dVertexRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut sorted = index.objects.clone();
    sorted.sort_by_key(|obj| obj.offset);

    let vertex_map = build_vertex_3d_map(&decoder, &sorted, &dynamic_types, best_effort)?;
    let mut result = Vec::new();
    let mut i = 0usize;
    while i < sorted.len() {
        let Some((record, header)) =
            parse_record_and_header(&decoder, sorted[i].offset, best_effort)?
        else {
            i += 1;
            continue;
        };
        if !matches_type_name(header.type_code, 0x10, "POLYLINE_3D", &dynamic_types) {
            i += 1;
            continue;
        }

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                i += 1;
                continue;
            }
            return Err(to_py_err(err));
        }
        let poly = match decode_polyline_3d_for_version(
            &mut reader,
            decoder.version(),
            &header,
            sorted[i].handle.0,
        ) {
            Ok(poly) => poly,
            Err(err) if best_effort => {
                i += 1;
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        let (vertices, next_i) = collect_polyline_3d_vertices(
            &decoder,
            &sorted,
            &dynamic_types,
            &vertex_map,
            &poly,
            i,
            best_effort,
        )?;
        i = next_i;
        result.push(Polyline3dVertexRow {
            handle: poly.handle,
            flags_70_bits: poly.flags_70_bits,
            closed: (poly.flags_70_bits & 0x01) != 0,
            vertices,
        });
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

fn build_vertex_3d_map(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    best_effort: bool,
) -> PyResult<HashMap<u64, entities::Vertex3dEntity>> {
    let mut vertex_map = HashMap::new();
    for obj in sorted {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x0B, "VERTEX_3D", dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let vertex = match decode_vertex_3d_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(vertex) => vertex,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        vertex_map.insert(vertex.handle, vertex);
    }
    Ok(vertex_map)
}

fn collect_polyline_3d_vertices(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    vertex_map: &HashMap<u64, entities::Vertex3dEntity>,
    poly: &entities::Polyline3dEntity,
    start_index: usize,
    best_effort: bool,
) -> PyResult<(Vec<entities::Vertex3dEntity>, usize)> {
    let mut vertices = Vec::new();

    if !poly.owned_handles.is_empty() {
        for handle in &poly.owned_handles {
            if let Some(vertex) = vertex_map.get(handle) {
                vertices.push(vertex.clone());
            }
        }
        return Ok((vertices, start_index + 1));
    }

    let mut next_i = start_index + 1;
    while next_i < sorted.len() {
        let Some((next_record, next_header)) =
            parse_record_and_header(decoder, sorted[next_i].offset, best_effort)?
        else {
            next_i += 1;
            continue;
        };
        let mut next_reader = next_record.bit_reader();
        if matches_type_name(next_header.type_code, 0x0B, "VERTEX_3D", dynamic_types) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            let vertex = match decode_vertex_3d_for_version(
                &mut next_reader,
                decoder.version(),
                &next_header,
                sorted[next_i].handle.0,
            ) {
                Ok(vertex) => vertex,
                Err(err) if best_effort => {
                    next_i += 1;
                    continue;
                }
                Err(err) => return Err(to_py_err(err)),
            };
            vertices.push(vertex);
            next_i += 1;
            continue;
        }
        if matches_type_name(next_header.type_code, 0x06, "SEQEND", dynamic_types) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            if let Err(err) = entities::decode_seqend(&mut next_reader) {
                if !best_effort {
                    return Err(to_py_err(err));
                }
            }
            next_i += 1;
        }
        break;
    }

    Ok((vertices, next_i))
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_mesh_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineMeshEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x1E, "POLYLINE_MESH", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_polyline_mesh_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.flags,
            entity.curve_type,
            entity.m_vertex_count,
            entity.n_vertex_count,
            entity.m_density,
            entity.n_density,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_vertex_mesh_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<VertexMeshEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x0C, "VERTEX_MESH", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_vertex_3d_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.flags,
            entity.position.0,
            entity.position.1,
            entity.position.2,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_mesh_with_vertices(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineMeshVerticesRow>> {
    let decoded_rows = decode_polyline_mesh_vertex_rows(path, limit)?;
    let mut result = Vec::with_capacity(decoded_rows.len());
    for row in decoded_rows {
        let mut vertices: Vec<Point3> = row.vertices.iter().map(|vertex| vertex.position).collect();
        if row.closed && vertices.len() > 1 {
            let first = vertices[0];
            let last = *vertices.last().unwrap();
            if !points_equal_3d(first, last) {
                vertices.push(first);
            }
        }
        result.push((
            row.handle,
            row.flags,
            row.m_vertex_count,
            row.n_vertex_count,
            row.closed,
            vertices,
        ));
    }
    Ok(result)
}

#[derive(Debug, Clone)]
struct PolylineMeshVertexRow {
    handle: u64,
    flags: u16,
    m_vertex_count: u16,
    n_vertex_count: u16,
    closed: bool,
    vertices: Vec<entities::Vertex3dEntity>,
}

fn decode_polyline_mesh_vertex_rows(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineMeshVertexRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut sorted = index.objects.clone();
    sorted.sort_by_key(|obj| obj.offset);

    let vertex_map = build_vertex_mesh_map(&decoder, &sorted, &dynamic_types, best_effort)?;
    let mut result = Vec::new();
    let mut i = 0usize;
    while i < sorted.len() {
        let Some((record, header)) =
            parse_record_and_header(&decoder, sorted[i].offset, best_effort)?
        else {
            i += 1;
            continue;
        };
        if !matches_type_name(header.type_code, 0x1E, "POLYLINE_MESH", &dynamic_types) {
            i += 1;
            continue;
        }

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                i += 1;
                continue;
            }
            return Err(to_py_err(err));
        }
        let poly = match decode_polyline_mesh_for_version(
            &mut reader,
            decoder.version(),
            &header,
            sorted[i].handle.0,
        ) {
            Ok(poly) => poly,
            Err(err) if best_effort => {
                i += 1;
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        let (vertices, next_i) = collect_polyline_mesh_vertices(
            &decoder,
            &sorted,
            &dynamic_types,
            &vertex_map,
            &poly,
            i,
            best_effort,
        )?;
        i = next_i;
        result.push(PolylineMeshVertexRow {
            handle: poly.handle,
            flags: poly.flags,
            m_vertex_count: poly.m_vertex_count,
            n_vertex_count: poly.n_vertex_count,
            closed: (poly.flags & 0x01) != 0,
            vertices,
        });
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

fn build_vertex_mesh_map(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    best_effort: bool,
) -> PyResult<HashMap<u64, entities::Vertex3dEntity>> {
    let mut vertex_map = HashMap::new();
    for obj in sorted {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x0C, "VERTEX_MESH", dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let vertex = match decode_vertex_3d_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(vertex) => vertex,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        vertex_map.insert(vertex.handle, vertex);
    }
    Ok(vertex_map)
}

fn collect_polyline_mesh_vertices(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    vertex_map: &HashMap<u64, entities::Vertex3dEntity>,
    poly: &entities::PolylineMeshEntity,
    start_index: usize,
    best_effort: bool,
) -> PyResult<(Vec<entities::Vertex3dEntity>, usize)> {
    let mut vertices = Vec::new();

    if !poly.owned_handles.is_empty() {
        for handle in &poly.owned_handles {
            if let Some(vertex) = vertex_map.get(handle) {
                vertices.push(vertex.clone());
            }
        }
        return Ok((vertices, start_index + 1));
    }

    let mut next_i = start_index + 1;
    while next_i < sorted.len() {
        let Some((next_record, next_header)) =
            parse_record_and_header(decoder, sorted[next_i].offset, best_effort)?
        else {
            next_i += 1;
            continue;
        };
        let mut next_reader = next_record.bit_reader();
        if matches_type_name(next_header.type_code, 0x0C, "VERTEX_MESH", dynamic_types) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            let vertex = match decode_vertex_3d_for_version(
                &mut next_reader,
                decoder.version(),
                &next_header,
                sorted[next_i].handle.0,
            ) {
                Ok(vertex) => vertex,
                Err(err) if best_effort => {
                    next_i += 1;
                    continue;
                }
                Err(err) => return Err(to_py_err(err)),
            };
            vertices.push(vertex);
            next_i += 1;
            continue;
        }
        if matches_type_name(next_header.type_code, 0x06, "SEQEND", dynamic_types) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            if let Err(err) = entities::decode_seqend(&mut next_reader) {
                if !best_effort {
                    return Err(to_py_err(err));
                }
            }
            next_i += 1;
        }
        break;
    }

    Ok((vertices, next_i))
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_pface_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylinePFaceEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x1D, "POLYLINE_PFACE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_polyline_pface_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((entity.handle, entity.num_vertices, entity.num_faces));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_vertex_pface_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<VertexPFaceEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x0D, "VERTEX_PFACE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_vertex_3d_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.flags,
            entity.position.0,
            entity.position.1,
            entity.position.2,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_vertex_pface_face_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<VertexPFaceFaceEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x0E, "VERTEX_PFACE_FACE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_vertex_pface_face_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.index1,
            entity.index2,
            entity.index3,
            entity.index4,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_pface_with_faces(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylinePFaceFacesRow>> {
    let decoded_rows = decode_polyline_pface_rows(path, limit)?;
    let mut result = Vec::with_capacity(decoded_rows.len());
    for row in decoded_rows {
        let vertices: Vec<Point3> = row.vertices.iter().map(|vertex| vertex.position).collect();
        let faces: Vec<PFaceFaceRow> = row
            .faces
            .iter()
            .map(|face| (face.index1, face.index2, face.index3, face.index4))
            .collect();
        result.push((row.handle, row.num_vertices, row.num_faces, vertices, faces));
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_solid_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<SolidEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x1F, "SOLID", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_solid_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((
            entity.handle,
            entity.p1,
            entity.p2,
            entity.p3,
            entity.p4,
            entity.thickness,
            entity.extrusion,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_trace_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<TraceEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x20, "TRACE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_trace_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((
            entity.handle,
            entity.p1,
            entity.p2,
            entity.p3,
            entity.p4,
            entity.thickness,
            entity.extrusion,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_shape_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<ShapeEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x21, "SHAPE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_shape_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((
            entity.handle,
            entity.insertion,
            entity.scale,
            entity.rotation,
            entity.width_factor,
            entity.oblique,
            entity.thickness,
            entity.shape_no,
            entity.extrusion,
            entity.shapefile_handle,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_viewport_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<ViewportEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x22, "VIEWPORT", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_viewport_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((entity.handle,));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_oleframe_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<OleFrameEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x2B, "OLEFRAME", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_oleframe_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((entity.handle,));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_ole2frame_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<OleFrameEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x4A, "OLE2FRAME", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_ole2frame_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((entity.handle,));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_long_transaction_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<LongTransactionEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x4C, "LONG_TRANSACTION", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_long_transaction_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((
            entity.handle,
            entity.owner_handle,
            entity.reactor_handles,
            entity.xdic_obj_handle,
            entity.ltype_handle,
            entity.plotstyle_handle,
            entity.material_handle,
            entity.extra_handles,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_region_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<RegionEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let known_layer_handles: HashSet<u64> =
        collect_known_layer_handles_in_order(&decoder, &dynamic_types, &index, best_effort)?
            .into_iter()
            .collect();
    let known_handles: HashSet<u64> = index.objects.iter().map(|obj| obj.handle.0).collect();
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x25, "REGION", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match decode_region_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let layer_handle = entity.layer_handle;
        let mut acis_handles = entity.acis_handles;
        acis_handles.retain(|handle| {
            *handle != layer_handle
                && known_handles.contains(handle)
                && !known_layer_handles.contains(handle)
        });
        result.push((entity.handle, acis_handles));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_3dsolid_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Solid3dEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let known_layer_handles: HashSet<u64> =
        collect_known_layer_handles_in_order(&decoder, &dynamic_types, &index, best_effort)?
            .into_iter()
            .collect();
    let known_handles: HashSet<u64> = index.objects.iter().map(|obj| obj.handle.0).collect();
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x26, "3DSOLID", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_3dsolid_for_version(&mut reader, decoder.version(), &header, obj.handle.0)
            {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        let layer_handle = entity.layer_handle;
        let mut acis_handles = entity.acis_handles;
        acis_handles.retain(|handle| {
            *handle != layer_handle
                && known_handles.contains(handle)
                && !known_layer_handles.contains(handle)
        });
        result.push((entity.handle, acis_handles));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_body_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<BodyEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let known_layer_handles: HashSet<u64> =
        collect_known_layer_handles_in_order(&decoder, &dynamic_types, &index, best_effort)?
            .into_iter()
            .collect();
    let known_handles: HashSet<u64> = index.objects.iter().map(|obj| obj.handle.0).collect();
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x27, "BODY", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_body_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        let layer_handle = entity.layer_handle;
        let mut acis_handles = entity.acis_handles;
        acis_handles.retain(|handle| {
            *handle != layer_handle
                && known_handles.contains(handle)
                && !known_layer_handles.contains(handle)
        });
        result.push((entity.handle, acis_handles));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_ray_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<RayEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x28, "RAY", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_ray_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((entity.handle, entity.start, entity.unit_vector));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_xline_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<XLineEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x29, "XLINE", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_xline_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
        result.push((entity.handle, entity.start, entity.unit_vector));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[derive(Debug, Clone)]
struct PolylinePFaceRow {
    handle: u64,
    num_vertices: u16,
    num_faces: u16,
    vertices: Vec<entities::Vertex3dEntity>,
    faces: Vec<entities::VertexPFaceFaceEntity>,
}

fn decode_polyline_pface_rows(path: &str, limit: Option<usize>) -> PyResult<Vec<PolylinePFaceRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut sorted = index.objects.clone();
    sorted.sort_by_key(|obj| obj.offset);

    let vertex_map = build_vertex_pface_map(&decoder, &sorted, &dynamic_types, best_effort)?;
    let face_map = build_vertex_pface_face_map(&decoder, &sorted, &dynamic_types, best_effort)?;
    let mut result = Vec::new();
    let mut i = 0usize;
    while i < sorted.len() {
        let Some((record, header)) =
            parse_record_and_header(&decoder, sorted[i].offset, best_effort)?
        else {
            i += 1;
            continue;
        };
        if !matches_type_name(header.type_code, 0x1D, "POLYLINE_PFACE", &dynamic_types) {
            i += 1;
            continue;
        }

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                i += 1;
                continue;
            }
            return Err(to_py_err(err));
        }
        let poly = match decode_polyline_pface_for_version(
            &mut reader,
            decoder.version(),
            &header,
            sorted[i].handle.0,
        ) {
            Ok(poly) => poly,
            Err(err) if best_effort => {
                i += 1;
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        let (vertices, faces, next_i) = collect_polyline_pface_data(
            &decoder,
            &sorted,
            &dynamic_types,
            &vertex_map,
            &face_map,
            &poly,
            i,
            best_effort,
        )?;
        i = next_i;
        result.push(PolylinePFaceRow {
            handle: poly.handle,
            num_vertices: poly.num_vertices,
            num_faces: poly.num_faces,
            vertices,
            faces,
        });
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

fn build_vertex_pface_map(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    best_effort: bool,
) -> PyResult<HashMap<u64, entities::Vertex3dEntity>> {
    let mut vertex_map = HashMap::new();
    for obj in sorted {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x0D, "VERTEX_PFACE", dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let vertex = match decode_vertex_3d_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(vertex) => vertex,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        vertex_map.insert(vertex.handle, vertex);
    }
    Ok(vertex_map)
}

fn build_vertex_pface_face_map(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    best_effort: bool,
) -> PyResult<HashMap<u64, entities::VertexPFaceFaceEntity>> {
    let mut face_map = HashMap::new();
    for obj in sorted {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x0E, "VERTEX_PFACE_FACE", dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let face = match decode_vertex_pface_face_for_version(
            &mut reader,
            decoder.version(),
            &header,
            obj.handle.0,
        ) {
            Ok(face) => face,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        face_map.insert(face.handle, face);
    }
    Ok(face_map)
}

fn collect_polyline_pface_data(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    vertex_map: &HashMap<u64, entities::Vertex3dEntity>,
    face_map: &HashMap<u64, entities::VertexPFaceFaceEntity>,
    poly: &entities::PolylinePFaceEntity,
    start_index: usize,
    best_effort: bool,
) -> PyResult<(
    Vec<entities::Vertex3dEntity>,
    Vec<entities::VertexPFaceFaceEntity>,
    usize,
)> {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();

    if !poly.owned_handles.is_empty() {
        for handle in &poly.owned_handles {
            if let Some(vertex) = vertex_map.get(handle) {
                vertices.push(vertex.clone());
                continue;
            }
            if let Some(face) = face_map.get(handle) {
                faces.push(face.clone());
            }
        }
        return Ok((vertices, faces, start_index + 1));
    }

    let mut next_i = start_index + 1;
    while next_i < sorted.len() {
        let Some((next_record, next_header)) =
            parse_record_and_header(decoder, sorted[next_i].offset, best_effort)?
        else {
            next_i += 1;
            continue;
        };
        let mut next_reader = next_record.bit_reader();
        if matches_type_name(next_header.type_code, 0x0D, "VERTEX_PFACE", dynamic_types) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            let vertex = match decode_vertex_3d_for_version(
                &mut next_reader,
                decoder.version(),
                &next_header,
                sorted[next_i].handle.0,
            ) {
                Ok(vertex) => vertex,
                Err(err) if best_effort => {
                    next_i += 1;
                    continue;
                }
                Err(err) => return Err(to_py_err(err)),
            };
            vertices.push(vertex);
            next_i += 1;
            continue;
        }
        if matches_type_name(
            next_header.type_code,
            0x0E,
            "VERTEX_PFACE_FACE",
            dynamic_types,
        ) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            let face = match decode_vertex_pface_face_for_version(
                &mut next_reader,
                decoder.version(),
                &next_header,
                sorted[next_i].handle.0,
            ) {
                Ok(face) => face,
                Err(err) if best_effort => {
                    next_i += 1;
                    continue;
                }
                Err(err) => return Err(to_py_err(err)),
            };
            faces.push(face);
            next_i += 1;
            continue;
        }
        if matches_type_name(next_header.type_code, 0x06, "SEQEND", dynamic_types) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            if let Err(err) = entities::decode_seqend(&mut next_reader) {
                if !best_effort {
                    return Err(to_py_err(err));
                }
            }
            next_i += 1;
        }
        break;
    }

    Ok((vertices, faces, next_i))
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_2d_with_vertices(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineVerticesRow>> {
    let decoded_rows = decode_polyline_2d_vertex_rows(path, limit)?;
    let mut result = Vec::with_capacity(decoded_rows.len());

    for row in decoded_rows {
        let use_vertex_z = polyline_uses_vertex_z(row.flags_info);
        let mut vertices: Vec<Point3> = row
            .vertices
            .iter()
            .map(|vertex| vertex_position_for_polyline(vertex, row.elevation, use_vertex_z))
            .collect();
        if row.flags_info.closed && vertices.len() > 1 {
            let first = vertices[0];
            let last = *vertices.last().unwrap();
            if !points_equal_3d(first, last) {
                vertices.push(first);
            }
        }
        result.push((row.handle, row.flags, vertices));
    }

    Ok(result)
}

#[pyfunction(signature = (path, segments_per_span=8, limit=None))]
pub fn decode_polyline_2d_with_vertices_interpolated(
    path: &str,
    segments_per_span: usize,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineInterpolatedRow>> {
    let decoded_rows = decode_polyline_2d_vertex_rows(path, limit)?;
    let mut result = Vec::with_capacity(decoded_rows.len());

    for row in decoded_rows {
        let use_vertex_z = polyline_uses_vertex_z(row.flags_info);
        let mut vertices: Vec<Point3> = row
            .vertices
            .iter()
            .map(|vertex| vertex_position_for_polyline(vertex, row.elevation, use_vertex_z))
            .collect();
        let mut applied = false;
        let should_interpolate = row.flags_info.curve_fit
            || row.flags_info.spline_fit
            || matches!(
                row.curve_type_info,
                entities::PolylineCurveType::QuadraticBSpline
                    | entities::PolylineCurveType::CubicBSpline
                    | entities::PolylineCurveType::Bezier
            );

        if should_interpolate && vertices.len() > 1 {
            let base = strip_closure(vertices);
            let interpolated =
                entities::catmull_rom_spline(&base, row.flags_info.closed, segments_per_span)
                    .map_err(to_py_err)?;
            vertices = interpolated;
            applied = true;
        } else if row.flags_info.closed && vertices.len() > 1 {
            let first = vertices[0];
            let last = *vertices.last().unwrap();
            if !points_equal_3d(first, last) {
                vertices.push(first);
            }
        }

        result.push((row.handle, row.flags, applied, vertices));
    }

    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_vertex_2d_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Vertex2dEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = match decoder.parse_object_record(obj.offset) {
            Ok(record) => record,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let header = match parse_object_header_for_version(&record, decoder.version()) {
            Ok(header) => header,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        if !matches_type_name(header.type_code, 0x0A, "VERTEX_2D", &dynamic_types) {
            continue;
        }
        let mut decoded: Option<entities::Vertex2dEntity> = None;
        let mut last_err = None;
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix {
                if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
                    last_err = Some(err);
                    continue;
                }
            }
            match decode_vertex_2d_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(vertex) => {
                    decoded = Some(vertex);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        let vertex = match decoded {
            Some(vertex) => vertex,
            None if best_effort => continue,
            None => {
                if let Some(err) = last_err {
                    return Err(to_py_err(err));
                }
                continue;
            }
        };
        result.push((
            vertex.handle,
            vertex.flags,
            vertex.position.0,
            vertex.position.1,
            vertex.position.2,
            vertex.start_width,
            vertex.end_width,
            vertex.bulge,
            vertex.tangent_dir,
        ));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_2d_with_vertex_data(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineVertexDataRow>> {
    let decoded_rows = decode_polyline_2d_vertex_rows(path, limit)?;
    let mut result = Vec::with_capacity(decoded_rows.len());

    for row in decoded_rows {
        let use_vertex_z = polyline_uses_vertex_z(row.flags_info);
        let mut vertices: Vec<VertexDataRow> = row
            .vertices
            .iter()
            .map(|vertex| vertex_data_for_polyline(vertex, row.elevation, use_vertex_z))
            .collect();
        if row.flags_info.closed && vertices.len() > 1 {
            let first = vertices[0];
            let last = *vertices.last().unwrap();
            if !points_equal_3d_with_data(first, last) {
                vertices.push(first);
            }
        }
        result.push((row.handle, row.flags, vertices));
    }

    Ok(result)
}

#[derive(Debug, Clone)]
struct PolylineVertexRow {
    handle: u64,
    flags: u16,
    flags_info: entities::PolylineFlagsInfo,
    curve_type_info: entities::PolylineCurveType,
    elevation: f64,
    vertices: Vec<entities::Vertex2dEntity>,
}

fn decode_polyline_2d_vertex_rows(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineVertexRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut sorted = index.objects.clone();
    sorted.sort_by_key(|obj| obj.offset);

    let vertex_map = build_vertex_2d_map(&decoder, &sorted, &dynamic_types, best_effort)?;
    let mut vertices_by_owner: HashMap<u64, Vec<entities::Vertex2dEntity>> = HashMap::new();
    for vertex in vertex_map.values() {
        let Some(owner_handle) = vertex.owner_handle else {
            continue;
        };
        vertices_by_owner
            .entry(owner_handle)
            .or_default()
            .push(vertex.clone());
    }
    for owned_vertices in vertices_by_owner.values_mut() {
        owned_vertices.sort_by_key(|vertex| vertex.handle);
    }
    let mut result = Vec::new();
    let mut i = 0usize;
    while i < sorted.len() {
        let obj = sorted[i];
        let record = match decoder.parse_object_record(obj.offset) {
            Ok(record) => record,
            Err(err) if best_effort => {
                i += 1;
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        let header = match parse_object_header_for_version(&record, decoder.version()) {
            Ok(header) => header,
            Err(err) if best_effort => {
                i += 1;
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        let declared_match =
            matches_type_name(header.type_code, 0x0F, "POLYLINE_2D", &dynamic_types);
        if !declared_match
            && !is_r14_polyline_2d_speculative_type(decoder.version(), header.type_code)
        {
            i += 1;
            continue;
        }

        let mut poly: Option<entities::Polyline2dEntity> = None;
        let mut last_err = None;
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix {
                if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
                    last_err = Some(err);
                    continue;
                }
            }
            let decoded = decode_polyline_2d_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            );
            match decoded {
                Ok(entity) => {
                    poly = Some(entity);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        let poly = match poly {
            Some(poly) => poly,
            None if !declared_match => {
                i += 1;
                continue;
            }
            None if best_effort => {
                i += 1;
                continue;
            }
            None => {
                if let Some(err) = last_err {
                    return Err(to_py_err(err));
                }
                i += 1;
                continue;
            }
        };
        if !declared_match
            && std::env::var("EZDWG_DEBUG_R14_POLY2D")
                .ok()
                .is_some_and(|value| value != "0")
        {
            eprintln!(
                "[r14-poly2d] handle={} type=0x{:X} flags={} curve_type={} owned={} width=({:.6},{:.6}) thickness={:.6} elevation={:.6}",
                obj.handle.0,
                header.type_code,
                poly.flags,
                poly.curve_type,
                poly.owned_handles.len(),
                poly.width_start,
                poly.width_end,
                poly.thickness,
                poly.elevation,
            );
        }
        if !declared_match && !is_plausible_polyline_2d_entity(&poly) {
            i += 1;
            continue;
        }
        let (vertices, next_i) = collect_polyline_vertices(
            &decoder,
            &sorted,
            &dynamic_types,
            &vertex_map,
            &vertices_by_owner,
            &poly,
            i,
            best_effort,
        )?;
        let vertices = sanitize_polyline_2d_vertices(vertices);
        i = next_i;

        result.push(PolylineVertexRow {
            handle: poly.handle,
            flags: poly.flags,
            flags_info: poly.flags_info,
            curve_type_info: poly.curve_type_info,
            elevation: poly.elevation,
            vertices,
        });
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

fn build_vertex_2d_map(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    best_effort: bool,
) -> PyResult<HashMap<u64, entities::Vertex2dEntity>> {
    let mut vertex_map = HashMap::new();
    for obj in sorted {
        let record = match decoder.parse_object_record(obj.offset) {
            Ok(record) => record,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let header = match parse_object_header_for_version(&record, decoder.version()) {
            Ok(header) => header,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        if !matches_type_name(header.type_code, 0x0A, "VERTEX_2D", dynamic_types) {
            continue;
        }
        let mut decoded: Option<entities::Vertex2dEntity> = None;
        let mut last_err = None;
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix {
                if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
                    last_err = Some(err);
                    continue;
                }
            }
            match decode_vertex_2d_for_version(
                &mut reader,
                decoder.version(),
                &header,
                obj.handle.0,
            ) {
                Ok(vertex) => {
                    decoded = Some(vertex);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        let vertex = match decoded {
            Some(vertex) => vertex,
            None if best_effort => continue,
            None => {
                if let Some(err) = last_err {
                    return Err(to_py_err(err));
                }
                continue;
            }
        };
        vertex_map.insert(vertex.handle, vertex);
    }
    Ok(vertex_map)
}

fn collect_polyline_vertices(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    vertex_map: &HashMap<u64, entities::Vertex2dEntity>,
    vertices_by_owner: &HashMap<u64, Vec<entities::Vertex2dEntity>>,
    poly: &entities::Polyline2dEntity,
    start_index: usize,
    best_effort: bool,
) -> PyResult<(Vec<entities::Vertex2dEntity>, usize)> {
    let mut vertices = Vec::new();

    if !poly.owned_handles.is_empty() {
        for handle in &poly.owned_handles {
            if let Some(vertex) = vertex_map.get(handle) {
                vertices.push(vertex.clone());
            }
        }
        return Ok((vertices, start_index + 1));
    }

    if let Some(owned_vertices) = vertices_by_owner.get(&poly.handle) {
        return Ok((owned_vertices.clone(), start_index + 1));
    }

    // Legacy POLYLINE_2D often stores VERTEX/SEQEND far from parent in object-offset
    // order, but keeps handle adjacency: POLYLINE -> VERTEX* -> SEQEND.
    let mut handle_cursor = poly.handle.saturating_add(1);
    while let Some(vertex) = vertex_map.get(&handle_cursor) {
        vertices.push(vertex.clone());
        handle_cursor = handle_cursor.saturating_add(1);
    }
    if !vertices.is_empty() {
        return Ok((vertices, start_index + 1));
    }

    let mut next_i = start_index + 1;
    while next_i < sorted.len() {
        let next = sorted[next_i];
        let next_record = match decoder.parse_object_record(next.offset) {
            Ok(record) => record,
            Err(err) if best_effort => {
                next_i += 1;
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        let next_header = match parse_object_header_for_version(&next_record, decoder.version()) {
            Ok(header) => header,
            Err(err) if best_effort => {
                next_i += 1;
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        let mut next_reader = next_record.bit_reader();
        if matches_type_name(next_header.type_code, 0x0A, "VERTEX_2D", dynamic_types) {
            let mut decoded: Option<entities::Vertex2dEntity> = None;
            let mut last_err = None;
            for with_prefix in [true, false] {
                let mut candidate_reader = if with_prefix {
                    let mut prefixed = next_record.bit_reader();
                    if let Err(err) = skip_object_type_prefix(&mut prefixed, decoder.version()) {
                        last_err = Some(err);
                        continue;
                    }
                    prefixed
                } else {
                    next_record.bit_reader()
                };
                match decode_vertex_2d_for_version(
                    &mut candidate_reader,
                    decoder.version(),
                    &next_header,
                    next.handle.0,
                ) {
                    Ok(vertex) => {
                        decoded = Some(vertex);
                        break;
                    }
                    Err(err) => last_err = Some(err),
                }
            }
            let vertex = match decoded {
                Some(vertex) => vertex,
                None if best_effort => {
                    next_i += 1;
                    continue;
                }
                None => {
                    if let Some(err) = last_err {
                        return Err(to_py_err(err));
                    }
                    next_i += 1;
                    continue;
                }
            };
            vertices.push(vertex);
            next_i += 1;
            continue;
        }
        if matches_type_name(next_header.type_code, 0x06, "SEQEND", dynamic_types) {
            if let Err(err) = skip_object_type_prefix(&mut next_reader, decoder.version()) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            if let Err(err) = entities::decode_seqend(&mut next_reader) {
                if best_effort {
                    next_i += 1;
                    continue;
                }
                return Err(to_py_err(err));
            }
            next_i += 1;
        }
        break;
    }

    Ok((vertices, next_i))
}

fn sanitize_polyline_2d_vertices(
    vertices: Vec<entities::Vertex2dEntity>,
) -> Vec<entities::Vertex2dEntity> {
    if vertices.len() < 3 {
        return vertices;
    }

    let far_count = vertices
        .iter()
        .filter(|vertex| polyline_vertex_extent(vertex) >= 1000.0)
        .count();
    if far_count == 0 {
        return vertices;
    }

    let candidate_count = vertices
        .iter()
        .filter(|vertex| is_origin_like_polyline_vertex(vertex))
        .count();
    if candidate_count == 0 {
        return vertices;
    }

    let mut remove = vec![false; vertices.len()];
    for index in 0..vertices.len() {
        if !is_origin_like_polyline_vertex(&vertices[index]) {
            continue;
        }
        if candidate_count >= 2 || has_large_adjacent_jump(&vertices, index) {
            remove[index] = true;
        }
    }

    let mut cleaned: Vec<entities::Vertex2dEntity> = vertices
        .iter()
        .enumerate()
        .filter_map(|(index, vertex)| {
            if remove[index] {
                None
            } else {
                Some(vertex.clone())
            }
        })
        .collect();
    if cleaned.len() < 2 {
        if candidate_count >= vertices.len().saturating_sub(1) {
            return Vec::new();
        }
        return vertices;
    }

    let mut deduped: Vec<entities::Vertex2dEntity> = Vec::with_capacity(cleaned.len());
    for vertex in cleaned.drain(..) {
        if deduped
            .last()
            .is_some_and(|prev| points_equal_3d(prev.position, vertex.position))
        {
            continue;
        }
        deduped.push(vertex);
    }
    if deduped.len() < 2 {
        if candidate_count >= vertices.len().saturating_sub(1) {
            return Vec::new();
        }
        return vertices;
    }
    deduped
}

fn polyline_vertex_extent(vertex: &entities::Vertex2dEntity) -> f64 {
    vertex.position.0.abs().max(vertex.position.1.abs())
}

fn is_origin_like_polyline_vertex(vertex: &entities::Vertex2dEntity) -> bool {
    let x = vertex.position.0;
    let y = vertex.position.1;
    if !x.is_finite() || !y.is_finite() {
        return true;
    }
    if x.abs() + y.abs() <= 1.0e-120 {
        return true;
    }
    x.abs() <= 1.5 && y.abs() <= 1.5
}

fn has_large_adjacent_jump(vertices: &[entities::Vertex2dEntity], index: usize) -> bool {
    let mut max_jump = 0.0f64;
    if index > 0 {
        max_jump = max_jump.max(distance_2d(
            vertices[index - 1].position,
            vertices[index].position,
        ));
    }
    if index + 1 < vertices.len() {
        max_jump = max_jump.max(distance_2d(
            vertices[index].position,
            vertices[index + 1].position,
        ));
    }
    max_jump >= 1000.0
}

fn distance_2d(a: Point3, b: Point3) -> f64 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    (dx * dx + dy * dy).sqrt()
}

fn polyline_uses_vertex_z(flags_info: entities::PolylineFlagsInfo) -> bool {
    flags_info.is_3d_polyline || flags_info.is_3d_mesh || flags_info.is_polyface_mesh
}

fn vertex_position_for_polyline(
    vertex: &entities::Vertex2dEntity,
    polyline_elevation: f64,
    use_vertex_z: bool,
) -> Point3 {
    let z = if use_vertex_z {
        vertex.position.2
    } else {
        polyline_elevation
    };
    (vertex.position.0, vertex.position.1, z)
}

fn vertex_data_for_polyline(
    vertex: &entities::Vertex2dEntity,
    polyline_elevation: f64,
    use_vertex_z: bool,
) -> VertexDataRow {
    let z = if use_vertex_z {
        vertex.position.2
    } else {
        polyline_elevation
    };
    (
        vertex.position.0,
        vertex.position.1,
        z,
        vertex.start_width,
        vertex.end_width,
        vertex.bulge,
        vertex.tangent_dir,
        vertex.flags,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolylineSequenceKind {
    Polyline2d,
    Polyline3d,
    PolylineMesh,
    PolylinePFace,
}

impl PolylineSequenceKind {
    fn label(self) -> &'static str {
        match self {
            Self::Polyline2d => "POLYLINE_2D",
            Self::Polyline3d => "POLYLINE_3D",
            Self::PolylineMesh => "POLYLINE_MESH",
            Self::PolylinePFace => "POLYLINE_PFACE",
        }
    }
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_sequence_members(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<PolylineSequenceMembersRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let best_effort = is_best_effort_compat_version(&decoder);
    let dynamic_types = load_dynamic_types(&decoder, best_effort)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut sorted = index.objects.clone();
    sorted.sort_by_key(|obj| obj.offset);

    let mut vertex_2d_handles: HashSet<u64> = HashSet::new();
    let mut vertex_3d_handles: HashSet<u64> = HashSet::new();
    let mut vertex_mesh_handles: HashSet<u64> = HashSet::new();
    let mut vertex_pface_handles: HashSet<u64> = HashSet::new();
    let mut vertex_pface_face_handles: HashSet<u64> = HashSet::new();
    let mut seqend_handles: HashSet<u64> = HashSet::new();

    for obj in sorted.iter() {
        let Some((_record, header)) = parse_record_and_header(&decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if matches_type_name(header.type_code, 0x0A, "VERTEX_2D", &dynamic_types) {
            vertex_2d_handles.insert(obj.handle.0);
            continue;
        }
        if matches_type_name(header.type_code, 0x0B, "VERTEX_3D", &dynamic_types) {
            vertex_3d_handles.insert(obj.handle.0);
            continue;
        }
        if matches_type_name(header.type_code, 0x0C, "VERTEX_MESH", &dynamic_types) {
            vertex_mesh_handles.insert(obj.handle.0);
            continue;
        }
        if matches_type_name(header.type_code, 0x0D, "VERTEX_PFACE", &dynamic_types) {
            vertex_pface_handles.insert(obj.handle.0);
            continue;
        }
        if matches_type_name(header.type_code, 0x0E, "VERTEX_PFACE_FACE", &dynamic_types) {
            vertex_pface_face_handles.insert(obj.handle.0);
            continue;
        }
        if matches_type_name(header.type_code, 0x06, "SEQEND", &dynamic_types) {
            seqend_handles.insert(obj.handle.0);
        }
    }

    let mut result = Vec::new();
    let mut i = 0usize;
    while i < sorted.len() {
        let Some((record, header)) =
            parse_record_and_header(&decoder, sorted[i].offset, best_effort)?
        else {
            i += 1;
            continue;
        };
        let kind = if matches_type_name(header.type_code, 0x0F, "POLYLINE_2D", &dynamic_types) {
            Some(PolylineSequenceKind::Polyline2d)
        } else if matches_type_name(header.type_code, 0x10, "POLYLINE_3D", &dynamic_types) {
            Some(PolylineSequenceKind::Polyline3d)
        } else if matches_type_name(header.type_code, 0x1E, "POLYLINE_MESH", &dynamic_types) {
            Some(PolylineSequenceKind::PolylineMesh)
        } else if matches_type_name(header.type_code, 0x1D, "POLYLINE_PFACE", &dynamic_types) {
            Some(PolylineSequenceKind::PolylinePFace)
        } else {
            None
        };
        let Some(kind) = kind else {
            i += 1;
            continue;
        };

        let polyline_handle = sorted[i].handle.0;
        let mut vertex_handles: Vec<u64> = Vec::new();
        let mut face_handles: Vec<u64> = Vec::new();
        let mut seqend_handle: Option<u64> = None;

        let mut owned_handles: Option<Vec<u64>> = None;
        let mut last_err = None;
        for with_prefix in [true, false] {
            let mut reader = record.bit_reader();
            if with_prefix {
                if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
                    last_err = Some(err);
                    continue;
                }
            }
            let decoded = match kind {
                PolylineSequenceKind::Polyline2d => decode_polyline_2d_for_version(
                    &mut reader,
                    decoder.version(),
                    &header,
                    polyline_handle,
                )
                .map(|polyline| polyline.owned_handles),
                PolylineSequenceKind::Polyline3d => decode_polyline_3d_for_version(
                    &mut reader,
                    decoder.version(),
                    &header,
                    polyline_handle,
                )
                .map(|polyline| polyline.owned_handles),
                PolylineSequenceKind::PolylineMesh => decode_polyline_mesh_for_version(
                    &mut reader,
                    decoder.version(),
                    &header,
                    polyline_handle,
                )
                .map(|polyline| polyline.owned_handles),
                PolylineSequenceKind::PolylinePFace => decode_polyline_pface_for_version(
                    &mut reader,
                    decoder.version(),
                    &header,
                    polyline_handle,
                )
                .map(|polyline| polyline.owned_handles),
            };
            match decoded {
                Ok(handles) => {
                    owned_handles = Some(handles);
                    break;
                }
                Err(err) => last_err = Some(err),
            }
        }
        if owned_handles.is_none() && !best_effort {
            if let Some(err) = last_err {
                return Err(to_py_err(err));
            }
        }

        let mut next_i = i + 1;
        if let Some(owned_handles) = owned_handles {
            for owned_handle in owned_handles {
                match kind {
                    PolylineSequenceKind::Polyline2d => {
                        if vertex_2d_handles.contains(&owned_handle) {
                            vertex_handles.push(owned_handle);
                            continue;
                        }
                    }
                    PolylineSequenceKind::Polyline3d => {
                        if vertex_3d_handles.contains(&owned_handle) {
                            vertex_handles.push(owned_handle);
                            continue;
                        }
                    }
                    PolylineSequenceKind::PolylineMesh => {
                        if vertex_mesh_handles.contains(&owned_handle) {
                            vertex_handles.push(owned_handle);
                            continue;
                        }
                    }
                    PolylineSequenceKind::PolylinePFace => {
                        if vertex_pface_handles.contains(&owned_handle) {
                            vertex_handles.push(owned_handle);
                            continue;
                        }
                        if vertex_pface_face_handles.contains(&owned_handle) {
                            face_handles.push(owned_handle);
                            continue;
                        }
                    }
                }
                if seqend_handle.is_none() && seqend_handles.contains(&owned_handle) {
                    seqend_handle = Some(owned_handle);
                }
            }
        } else {
            while next_i < sorted.len() {
                let Some((_next_record, next_header)) =
                    parse_record_and_header(&decoder, sorted[next_i].offset, best_effort)?
                else {
                    next_i += 1;
                    continue;
                };
                let next_handle = sorted[next_i].handle.0;
                let is_member = match kind {
                    PolylineSequenceKind::Polyline2d => {
                        matches_type_name(next_header.type_code, 0x0A, "VERTEX_2D", &dynamic_types)
                    }
                    PolylineSequenceKind::Polyline3d => {
                        matches_type_name(next_header.type_code, 0x0B, "VERTEX_3D", &dynamic_types)
                    }
                    PolylineSequenceKind::PolylineMesh => matches_type_name(
                        next_header.type_code,
                        0x0C,
                        "VERTEX_MESH",
                        &dynamic_types,
                    ),
                    PolylineSequenceKind::PolylinePFace => matches_type_name(
                        next_header.type_code,
                        0x0D,
                        "VERTEX_PFACE",
                        &dynamic_types,
                    ),
                };
                if is_member {
                    vertex_handles.push(next_handle);
                    next_i += 1;
                    continue;
                }
                if kind == PolylineSequenceKind::PolylinePFace
                    && matches_type_name(
                        next_header.type_code,
                        0x0E,
                        "VERTEX_PFACE_FACE",
                        &dynamic_types,
                    )
                {
                    face_handles.push(next_handle);
                    next_i += 1;
                    continue;
                }
                if matches_type_name(next_header.type_code, 0x06, "SEQEND", &dynamic_types) {
                    seqend_handle = Some(next_handle);
                    next_i += 1;
                }
                break;
            }
        }

        result.push((
            polyline_handle,
            kind.label().to_string(),
            vertex_handles,
            face_handles,
            seqend_handle,
        ));
        i = next_i;
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(detect_version, module)?)?;
    module.add_function(wrap_pyfunction!(write_ac1015_dwg, module)?)?;
    module.add_function(wrap_pyfunction!(write_ac1015_line_dwg, module)?)?;
    module.add_function(wrap_pyfunction!(list_section_locators, module)?)?;
    module.add_function(wrap_pyfunction!(read_section_bytes, module)?)?;
    module.add_function(wrap_pyfunction!(list_object_map_entries, module)?)?;
    module.add_function(wrap_pyfunction!(list_object_headers, module)?)?;
    module.add_function(wrap_pyfunction!(list_object_headers_with_type, module)?)?;
    module.add_function(wrap_pyfunction!(list_object_headers_by_type, module)?)?;
    module.add_function(wrap_pyfunction!(read_object_records_by_type, module)?)?;
    module.add_function(wrap_pyfunction!(read_object_records_by_handle, module)?)?;
    module.add_function(wrap_pyfunction!(decode_object_handle_stream_refs, module)?)?;
    module.add_function(wrap_pyfunction!(decode_acis_candidate_infos, module)?)?;
    module.add_function(wrap_pyfunction!(decode_entity_styles, module)?)?;
    module.add_function(wrap_pyfunction!(decode_layer_colors, module)?)?;
    module.add_function(wrap_pyfunction!(decode_line_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_point_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_3dface_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_arc_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_circle_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_line_arc_circle_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_ellipse_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_spline_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_text_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_attrib_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_attdef_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_mtext_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_leader_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_hatch_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_tolerance_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_mline_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dimension_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_linear_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_ordinate_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_aligned_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_ang3pt_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_ang2ln_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_diameter_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_radius_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_insert_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_minsert_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_insert_minsert_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_block_header_names, module)?)?;
    module.add_function(wrap_pyfunction!(decode_block_entity_names, module)?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_2d_entities, module)?)?;
    module.add_function(wrap_pyfunction!(
        decode_polyline_2d_entities_interpreted,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(decode_lwpolyline_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_3d_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_3d_with_vertices, module)?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_mesh_entities, module)?)?;
    module.add_function(wrap_pyfunction!(
        decode_polyline_mesh_with_vertices,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_pface_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_pface_with_faces, module)?)?;
    module.add_function(wrap_pyfunction!(decode_solid_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_trace_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_shape_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_viewport_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_oleframe_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_ole2frame_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_long_transaction_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_region_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_3dsolid_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_body_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_ray_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_xline_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_2d_with_vertices, module)?)?;
    module.add_function(wrap_pyfunction!(
        decode_polyline_2d_with_vertices_interpolated,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(decode_vertex_2d_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_vertex_3d_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_vertex_mesh_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_vertex_pface_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_vertex_pface_face_entities, module)?)?;
    module.add_function(wrap_pyfunction!(
        decode_polyline_2d_with_vertex_data,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_sequence_members, module)?)?;
    Ok(())
}

fn is_best_effort_compat_version(decoder: &decoder::Decoder<'_>) -> bool {
    matches!(
        decoder.version(),
        version::DwgVersion::R14
            | version::DwgVersion::R2000
            | version::DwgVersion::R2010
            | version::DwgVersion::R2013
            | version::DwgVersion::R2018
    )
}

fn decode_line_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::LineEntity> {
    let start = reader.get_pos();
    let primary = match version {
        version::DwgVersion::R14 => entities::decode_line_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_line_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_line_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_line_r2007(reader),
        _ => entities::decode_line(reader),
    };
    if let Ok(entity) = primary {
        return Ok(entity);
    }
    let primary_err = primary.unwrap_err();

    reader.set_pos(start.0, start.1);
    if let Ok(entity) = entities::decode_line(reader) {
        return Ok(entity);
    }

    reader.set_pos(start.0, start.1);
    if let Ok(entity) = entities::decode_line_r14(reader, object_handle) {
        return Ok(entity);
    }

    Err(primary_err)
}

fn decode_point_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::PointEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_point_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_point_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_point_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_point_r2007(reader),
        _ => entities::decode_point(reader),
    }
}

fn decode_arc_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::ArcEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_arc_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_arc_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_arc_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_arc_r2007(reader),
        _ => entities::decode_arc(reader),
    }
}

fn decode_circle_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::CircleEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_circle_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_circle_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_circle_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_circle_r2007(reader),
        _ => entities::decode_circle(reader),
    }
}

fn decode_ellipse_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::EllipseEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_ellipse_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_ellipse_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_ellipse_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_ellipse_r2007(reader),
        _ => entities::decode_ellipse(reader),
    }
}

fn decode_spline_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::SplineEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_spline_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_spline_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_spline_r2007(reader),
        _ => entities::decode_spline(reader),
    }
}

fn decode_text_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::TextEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_text_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_text_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_text_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_text_r2007(reader),
        _ => entities::decode_text(reader),
    }
}

fn decode_attrib_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::AttribEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_attrib_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_attrib_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_attrib_r2007(reader),
        _ => entities::decode_attrib(reader),
    }
}

fn decode_attdef_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::AttribEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_attdef_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_attdef_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_attdef_r2007(reader),
        _ => entities::decode_attdef(reader),
    }
}

fn decode_mtext_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::MTextEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_mtext_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_mtext_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_mtext_r2007(reader),
        version::DwgVersion::R2004 => entities::decode_mtext_r2004(reader),
        _ => entities::decode_mtext(reader),
    }
}

fn decode_leader_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::LeaderEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_leader_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_leader_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_leader_r2007(reader),
        _ => entities::decode_leader(reader),
    }
}

fn decode_hatch_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::HatchEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_hatch_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_hatch_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_hatch_r2007(reader),
        version::DwgVersion::R2004 => entities::decode_hatch_r2004(reader),
        _ => entities::decode_hatch(reader),
    }
}

fn decode_tolerance_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::ToleranceEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_tolerance_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_tolerance_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_tolerance_r2007(reader),
        _ => entities::decode_tolerance(reader),
    }
}

fn decode_mline_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::MLineEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_mline_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_mline_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_mline_r2007(reader),
        _ => entities::decode_mline(reader),
    }
}

fn decode_insert_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::InsertEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_insert_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_insert_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_insert_r2007(reader),
        _ => entities::decode_insert(reader),
    }
}

fn decode_dim_linear_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::DimLinearEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_dim_linear_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_dim_linear_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_dim_linear_r2007(reader),
        _ => entities::decode_dim_linear(reader),
    }
}

fn decode_dim_radius_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::DimRadiusEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_dim_radius_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_dim_radius_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_dim_radius_r2007(reader),
        _ => entities::decode_dim_radius(reader),
    }
}

fn decode_dim_diameter_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::DimDiameterEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_dim_diameter_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_dim_diameter_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_dim_diameter_r2007(reader),
        _ => entities::decode_dim_diameter(reader),
    }
}

fn decode_lwpolyline_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::LwPolylineEntity> {
    match version {
        version::DwgVersion::R14 => {
            entities::decode_lwpolyline_r14(reader, object_handle, header.type_code)
        }
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_lwpolyline_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_lwpolyline_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_lwpolyline_r2007(reader),
        _ => entities::decode_lwpolyline(reader),
    }
}

fn decode_polyline_2d_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::Polyline2dEntity> {
    let start = reader.get_pos();
    match version {
        version::DwgVersion::R14 => entities::decode_polyline_2d_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            match entities::decode_polyline_2d_r2010(reader, object_data_end_bit, object_handle) {
                Ok(entity) => Ok(entity),
                Err(primary_err) => {
                    reader.set_pos(start.0, start.1);
                    entities::decode_polyline_2d(reader).or(Err(primary_err))
                }
            }
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            match entities::decode_polyline_2d_r2013(reader, object_data_end_bit, object_handle) {
                Ok(entity) => Ok(entity),
                Err(primary_err) => {
                    reader.set_pos(start.0, start.1);
                    entities::decode_polyline_2d(reader).or(Err(primary_err))
                }
            }
        }
        version::DwgVersion::R2007 => match entities::decode_polyline_2d_r2007(reader) {
            Ok(entity) => Ok(entity),
            Err(primary_err) => {
                reader.set_pos(start.0, start.1);
                entities::decode_polyline_2d(reader).or(Err(primary_err))
            }
        },
        _ => entities::decode_polyline_2d(reader),
    }
}

fn is_r14_polyline_2d_speculative_type(version: &version::DwgVersion, type_code: u16) -> bool {
    matches!(version, version::DwgVersion::R14) && type_code >= 0x01F4
}

fn is_plausible_polyline_2d_entity(entity: &entities::Polyline2dEntity) -> bool {
    if entity.handle == 0 {
        return false;
    }
    if !matches!(entity.curve_type, 0 | 5 | 6 | 8) {
        return false;
    }
    if !entity.width_start.is_finite()
        || !entity.width_end.is_finite()
        || !entity.thickness.is_finite()
        || !entity.elevation.is_finite()
    {
        return false;
    }
    if entity.width_start.abs() > 1.0e9
        || entity.width_end.abs() > 1.0e9
        || entity.thickness.abs() > 1.0e9
        || entity.elevation.abs() > 1.0e9
    {
        return false;
    }
    if entity.flags > 0x03FF {
        return false;
    }
    let owned_len = entity.owned_handles.len();
    owned_len > 0 && owned_len <= 4096
}

fn decode_polyline_3d_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::Polyline3dEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_polyline_3d_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_polyline_3d_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_polyline_3d_r2007(reader),
        _ => entities::decode_polyline_3d(reader),
    }
}

fn decode_vertex_3d_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::Vertex3dEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_vertex_3d_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_vertex_3d_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_vertex_3d_r2007(reader),
        _ => entities::decode_vertex_3d(reader),
    }
}

fn decode_vertex_2d_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::Vertex2dEntity> {
    let start = reader.get_pos();
    let primary = match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_vertex_2d_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_vertex_2d_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_vertex_2d_r2007(reader),
        _ => entities::decode_vertex_2d(reader),
    };
    if let Ok(entity) = primary {
        return Ok(entity);
    }
    let primary_err = primary.unwrap_err();

    reader.set_pos(start.0, start.1);
    if let Ok(entity) = entities::decode_vertex_2d(reader) {
        return Ok(entity);
    }

    // Some drawings tag 2D vertices with legacy 3D-like payloads.
    reader.set_pos(start.0, start.1);
    if let Ok(vertex3d) = decode_vertex_3d_for_version(reader, version, header, object_handle) {
        return Ok(entities::Vertex2dEntity {
            handle: vertex3d.handle,
            flags: u16::from(vertex3d.flags),
            position: vertex3d.position,
            start_width: 0.0,
            end_width: 0.0,
            bulge: 0.0,
            tangent_dir: 0.0,
            owner_handle: None,
        });
    }
    Err(primary_err)
}

fn decode_polyline_mesh_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::PolylineMeshEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_polyline_mesh_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_polyline_mesh_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_polyline_mesh_r2007(reader),
        _ => entities::decode_polyline_mesh(reader),
    }
}

fn decode_polyline_pface_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::PolylinePFaceEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_polyline_pface_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_polyline_pface_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_polyline_pface_r2007(reader),
        _ => entities::decode_polyline_pface(reader),
    }
}

fn decode_vertex_pface_face_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::VertexPFaceFaceEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_vertex_pface_face_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_vertex_pface_face_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_vertex_pface_face_r2007(reader),
        _ => entities::decode_vertex_pface_face(reader),
    }
}

fn decode_3dface_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::Face3dEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_3dface_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_3dface_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_3dface_r2007(reader),
        _ => entities::decode_3dface(reader),
    }
}

fn decode_solid_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::SolidEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_solid_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_solid_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_solid_r2007(reader),
        _ => entities::decode_solid(reader),
    }
}

fn decode_trace_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::TraceEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_trace_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_trace_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_trace_r2007(reader),
        _ => entities::decode_trace(reader),
    }
}

fn decode_shape_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::ShapeEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_shape_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_shape_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_shape_r2007(reader),
        _ => entities::decode_shape(reader),
    }
}

fn decode_viewport_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::ViewportEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_viewport_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_viewport_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_viewport_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_viewport_r2007(reader),
        _ => entities::decode_viewport(reader),
    }
}

fn decode_oleframe_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::OleFrameEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_oleframe_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_oleframe_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_oleframe_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_oleframe_r2007(reader),
        _ => entities::decode_oleframe(reader),
    }
}

fn decode_ole2frame_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::OleFrameEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_ole2frame_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_ole2frame_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_ole2frame_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_ole2frame_r2007(reader),
        _ => entities::decode_ole2frame(reader),
    }
}

fn decode_long_transaction_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::LongTransactionEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_long_transaction_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_long_transaction_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_long_transaction_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_long_transaction_r2007(reader),
        _ => entities::decode_long_transaction(reader),
    }
}

fn decode_region_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::RegionEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_region_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_region_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_region_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_region_r2007(reader),
        _ => entities::decode_region(reader),
    }
}

fn decode_3dsolid_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::Solid3dEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_3dsolid_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_3dsolid_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_3dsolid_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_3dsolid_r2007(reader),
        _ => entities::decode_3dsolid(reader),
    }
}

fn decode_body_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::BodyEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_body_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_body_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_body_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_body_r2007(reader),
        _ => entities::decode_body(reader),
    }
}

fn decode_ray_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::RayEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_ray_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_ray_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_ray_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_ray_r2007(reader),
        _ => entities::decode_ray(reader),
    }
}

fn decode_xline_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::XLineEntity> {
    match version {
        version::DwgVersion::R14 => entities::decode_xline_r14(reader, object_handle),
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_xline_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_xline_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_xline_r2007(reader),
        _ => entities::decode_xline(reader),
    }
}

fn resolve_r2010_object_data_end_bit(header: &ApiObjectHeader) -> crate::core::result::Result<u32> {
    let total_bits = header
        .data_size
        .checked_mul(8)
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "object size bits overflow"))?;
    let handle_bits = header
        .handle_stream_size_bits
        .ok_or_else(|| DwgError::new(ErrorKind::Format, "missing R2010 handle stream size"))?;
    total_bits.checked_sub(handle_bits).ok_or_else(|| {
        DwgError::new(
            ErrorKind::Format,
            "R2010 handle stream exceeds object data size",
        )
    })
}

fn resolve_r2010_object_data_end_bit_candidates(header: &ApiObjectHeader) -> Vec<u32> {
    let total_bits = header.data_size.saturating_mul(8);
    let Some(handle_bits) = header.handle_stream_size_bits else {
        return Vec::new();
    };

    let bases = [
        total_bits.saturating_sub(handle_bits),
        total_bits.saturating_sub(handle_bits.saturating_sub(8)),
    ];
    let deltas = [-16i32, -8, 0, 8, 16];

    let mut out = Vec::new();
    for base in bases {
        for delta in deltas {
            let candidate_i64 = i64::from(base) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            let Ok(candidate) = u32::try_from(candidate_i64) else {
                continue;
            };
            if candidate > total_bits {
                continue;
            }
            out.push(candidate);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn skip_object_type_prefix(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
) -> crate::core::result::Result<u16> {
    match version {
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let _handle_stream_size_bits = reader.read_umc()?;
            let type_code = reader.read_ot_r2010()?;
            if type_code == 0 {
                return Err(DwgError::new(ErrorKind::Format, "object type code is zero"));
            }
            Ok(type_code)
        }
        _ => {
            let type_code = reader.read_bs()?;
            if type_code == 0 {
                return Err(DwgError::new(ErrorKind::Format, "object type code is zero"));
            }
            Ok(type_code)
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ApiObjectHeader {
    data_size: u32,
    type_code: u16,
    handle_stream_size_bits: Option<u32>,
}

fn parse_object_header_for_version(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
) -> crate::core::result::Result<ApiObjectHeader> {
    match version {
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let header = objects::object_header_r2010::parse_from_record(record)?;
            Ok(ApiObjectHeader {
                data_size: header.data_size,
                type_code: header.type_code,
                handle_stream_size_bits: Some(header.handle_stream_size_bits),
            })
        }
        _ => {
            let header = objects::object_header_r2000::parse_from_record(record)?;
            Ok(ApiObjectHeader {
                data_size: header.data_size,
                type_code: header.type_code,
                handle_stream_size_bits: None,
            })
        }
    }
}

fn parse_record_and_header<'a>(
    decoder: &decoder::Decoder<'a>,
    offset: u32,
    best_effort: bool,
) -> PyResult<Option<(objects::ObjectRecord<'a>, ApiObjectHeader)>> {
    let record = match decoder.parse_object_record(offset) {
        Ok(record) => record,
        Err(err) if best_effort => return Ok(None),
        Err(err) => return Err(to_py_err(err)),
    };
    let header = match parse_object_header_for_version(&record, decoder.version()) {
        Ok(header) => header,
        Err(err) if best_effort => return Ok(None),
        Err(err) => return Err(to_py_err(err)),
    };
    Ok(Some((record, header)))
}

fn load_dynamic_types(
    decoder: &decoder::Decoder<'_>,
    best_effort: bool,
) -> PyResult<HashMap<u16, String>> {
    match decoder.dynamic_type_map() {
        Ok(map) => Ok(map),
        Err(_) if best_effort => Ok(HashMap::new()),
        Err(err) => Err(to_py_err(err)),
    }
}

fn collect_known_layer_handles_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
) -> PyResult<Vec<u64>> {
    let mut layer_handles = Vec::new();
    for obj in index.objects.iter() {
        let Some((_record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if matches_type_name(header.type_code, 0x33, "LAYER", dynamic_types) {
            layer_handles.push(obj.handle.0);
        }
    }
    Ok(layer_handles)
}

fn collect_object_type_codes(
    decoder: &decoder::Decoder<'_>,
    index: &objects::ObjectIndex,
    best_effort: bool,
) -> PyResult<HashMap<u64, u16>> {
    let mut object_types: HashMap<u64, u16> = HashMap::new();
    for obj in index.objects.iter() {
        let Some((_record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        object_types.insert(obj.handle.0, header.type_code);
    }
    Ok(object_types)
}

fn collect_block_header_name_entries_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
) -> PyResult<Vec<(u64, u64, String)>> {
    let mut entries: Vec<(u64, u64, String)> = Vec::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", dynamic_types) {
            continue;
        }
        let prefer_prefixed = matches!(
            decoder.version(),
            version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
        );
        let mut decoded_handle_fallback = obj.handle.0;
        if prefer_prefixed {
            let mut prefixed_reader = record.bit_reader();
            if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
                if let Ok(handle) =
                    decode_block_header_record_handle(&mut prefixed_reader, obj.handle.0)
                {
                    decoded_handle_fallback = handle;
                }
            } else {
                let mut reader = record.bit_reader();
                if let Ok(handle) = decode_block_header_record_handle(&mut reader, obj.handle.0) {
                    decoded_handle_fallback = handle;
                }
            }
        } else {
            let mut reader = record.bit_reader();
            if let Ok(handle) = decode_block_header_record_handle(&mut reader, obj.handle.0) {
                decoded_handle_fallback = handle;
            }
        }

        let mut parsed = if prefer_prefixed {
            let mut prefixed_reader = record.bit_reader();
            if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
                decode_block_header_name_record(
                    &mut prefixed_reader,
                    decoder.version(),
                    obj.handle.0,
                    Some(&header),
                )
            } else {
                let mut reader = record.bit_reader();
                decode_block_header_name_record(
                    &mut reader,
                    decoder.version(),
                    obj.handle.0,
                    Some(&header),
                )
            }
        } else {
            let mut reader = record.bit_reader();
            decode_block_header_name_record(
                &mut reader,
                decoder.version(),
                obj.handle.0,
                Some(&header),
            )
        };

        let retry_alternate = parsed
            .as_ref()
            .map(|(_handle, name)| name.is_empty())
            .unwrap_or(true);
        if retry_alternate {
            if prefer_prefixed {
                let mut reader = record.bit_reader();
                if let Ok(row) = decode_block_header_name_record(
                    &mut reader,
                    decoder.version(),
                    obj.handle.0,
                    Some(&header),
                ) {
                    parsed = Ok(row);
                }
            } else {
                let mut prefixed_reader = record.bit_reader();
                if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
                    if let Ok(row) = decode_block_header_name_record(
                        &mut prefixed_reader,
                        decoder.version(),
                        obj.handle.0,
                        Some(&header),
                    ) {
                        parsed = Ok(row);
                    }
                }
            }
        }
        let (decoded_handle, mut name) = match parsed {
            Ok(row) => row,
            Err(err) if best_effort || is_recoverable_decode_error(&err) => {
                let recovered_name =
                    recover_block_header_name_from_record(&record, decoder.version(), &header)
                        .unwrap_or_default();
                entries.push((obj.handle.0, decoded_handle_fallback, recovered_name));
                continue;
            }
            Err(err) => return Err(to_py_err(err)),
        };
        if name.is_empty() {
            if let Some(recovered_name) =
                recover_block_header_name_from_record(&record, decoder.version(), &header)
            {
                name = recovered_name;
            }
        }
        entries.push((obj.handle.0, decoded_handle, name));
    }
    Ok(entries)
}

fn collect_block_header_names_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    only_handles: Option<&HashSet<u64>>,
) -> PyResult<HashMap<u64, String>> {
    let entries =
        collect_block_header_name_entries_in_order(decoder, dynamic_types, index, best_effort)?;
    let mut block_names: HashMap<u64, String> = HashMap::new();
    let mut raw_to_decoded: HashMap<u64, u64> = HashMap::new();
    for (raw_handle, decoded_handle, name) in entries {
        raw_to_decoded.insert(raw_handle, decoded_handle);
        if name.is_empty() {
            continue;
        }
        if let Some(handles) = only_handles {
            if !handles.contains(&raw_handle) && !handles.contains(&decoded_handle) {
                continue;
            }
        }
        block_names
            .entry(raw_handle)
            .or_insert_with(|| name.clone());
        block_names.entry(decoded_handle).or_insert(name);
    }
    let (_aliases, recovered_header_names) = collect_block_name_aliases_in_order(
        decoder,
        dynamic_types,
        index,
        best_effort,
        &block_names,
    )?;
    for (raw_handle, name) in recovered_header_names {
        if name.is_empty() {
            continue;
        }
        let decoded_handle = raw_to_decoded
            .get(&raw_handle)
            .copied()
            .unwrap_or(raw_handle);
        if let Some(handles) = only_handles {
            if !handles.contains(&raw_handle) && !handles.contains(&decoded_handle) {
                continue;
            }
        }
        block_names
            .entry(raw_handle)
            .or_insert_with(|| name.clone());
        block_names.entry(decoded_handle).or_insert(name);
    }
    Ok(block_names)
}

fn collect_block_name_aliases_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    block_header_names: &HashMap<u64, String>,
) -> PyResult<(HashMap<u64, String>, HashMap<u64, String>)> {
    let mut aliases: HashMap<u64, String> = HashMap::new();
    let mut recovered_header_names: HashMap<u64, String> = HashMap::new();
    let mut pending_name: Option<String> = None;
    let mut pending_header_handle: Option<u64> = None;
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", dynamic_types) {
            pending_name = block_header_names.get(&obj.handle.0).cloned();
            pending_header_handle = Some(obj.handle.0);
            continue;
        }
        if matches_type_name(header.type_code, 0x04, "BLOCK", dynamic_types) {
            let mut block_name = pending_name.clone();
            if block_name.is_none() || block_name.as_ref().is_some_and(|name| name.is_empty()) {
                block_name =
                    recover_block_name_from_block_record(&record, decoder.version(), &header);
                if let (Some(header_handle), Some(name)) =
                    (pending_header_handle, block_name.clone())
                {
                    if !name.is_empty() {
                        recovered_header_names.entry(header_handle).or_insert(name);
                    }
                }
            }
            if let Some(name) = block_name {
                aliases.insert(obj.handle.0, name);
            }
            continue;
        }
        if matches_type_name(header.type_code, 0x05, "ENDBLK", dynamic_types) {
            pending_name = None;
            pending_header_handle = None;
        }
    }
    Ok((aliases, recovered_header_names))
}

fn decode_block_record_handle(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
    expected_handle: u64,
) -> crate::core::result::Result<u64> {
    let decoded = match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(api_header)?;
            entities::common::parse_common_entity_header_r2010(reader, object_data_end_bit)?.handle
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(api_header)?;
            entities::common::parse_common_entity_header_r2013(reader, object_data_end_bit)?.handle
        }
        _ => entities::common::parse_common_entity_header(reader)?.handle,
    };
    Ok(if decoded != 0 {
        decoded
    } else {
        expected_handle
    })
}

fn collect_block_record_handle_aliases_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    block_header_names: &HashMap<u64, String>,
) -> PyResult<HashMap<u64, String>> {
    let mut aliases: HashMap<u64, String> = HashMap::new();
    let mut pending_name: Option<String> = None;
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", dynamic_types) {
            pending_name = block_header_names.get(&obj.handle.0).cloned();
            if pending_name.is_none() || pending_name.as_ref().is_some_and(|name| name.is_empty()) {
                let prefer_prefixed = matches!(
                    decoder.version(),
                    version::DwgVersion::R2010
                        | version::DwgVersion::R2013
                        | version::DwgVersion::R2018
                );
                let parsed = if prefer_prefixed {
                    let mut prefixed_reader = record.bit_reader();
                    if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
                        decode_block_header_name_record(
                            &mut prefixed_reader,
                            decoder.version(),
                            obj.handle.0,
                            Some(&header),
                        )
                    } else {
                        let mut reader = record.bit_reader();
                        decode_block_header_name_record(
                            &mut reader,
                            decoder.version(),
                            obj.handle.0,
                            Some(&header),
                        )
                    }
                } else {
                    let mut reader = record.bit_reader();
                    decode_block_header_name_record(
                        &mut reader,
                        decoder.version(),
                        obj.handle.0,
                        Some(&header),
                    )
                };
                if let Ok((decoded_handle, decoded_name)) = parsed {
                    let mapped = block_header_names.get(&decoded_handle).cloned();
                    if mapped.is_some() {
                        pending_name = mapped;
                    } else if !decoded_name.is_empty() {
                        pending_name = Some(decoded_name);
                    }
                }
            }
            continue;
        }
        if matches_type_name(header.type_code, 0x04, "BLOCK", dynamic_types) {
            let Some(name) = pending_name.clone() else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            aliases.entry(obj.handle.0).or_insert_with(|| name.clone());

            let prefer_prefixed = matches!(
                decoder.version(),
                version::DwgVersion::R2010
                    | version::DwgVersion::R2013
                    | version::DwgVersion::R2018
            );
            let decoded_handle = if prefer_prefixed {
                let mut prefixed_reader = record.bit_reader();
                if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
                    decode_block_record_handle(
                        &mut prefixed_reader,
                        decoder.version(),
                        &header,
                        obj.handle.0,
                    )
                } else {
                    let mut reader = record.bit_reader();
                    decode_block_record_handle(
                        &mut reader,
                        decoder.version(),
                        &header,
                        obj.handle.0,
                    )
                }
            } else {
                let mut reader = record.bit_reader();
                decode_block_record_handle(&mut reader, decoder.version(), &header, obj.handle.0)
            };
            if let Ok(decoded_handle) = decoded_handle {
                aliases.entry(decoded_handle).or_insert(name);
            }
            continue;
        }
        if matches_type_name(header.type_code, 0x05, "ENDBLK", dynamic_types) {
            pending_name = None;
        }
    }
    Ok(aliases)
}

fn collect_block_and_endblk_handle_aliases_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    block_header_names: &HashMap<u64, String>,
) -> PyResult<(HashMap<u64, String>, HashMap<u64, String>)> {
    let is_r2010_plus = matches!(
        decoder.version(),
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    );
    let mut block_aliases: HashMap<u64, String> = HashMap::new();
    let mut endblk_aliases: HashMap<u64, String> = HashMap::new();
    let mut pending_name: Option<String> = None;
    let mut current_block_name: Option<String> = None;
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", dynamic_types) {
            pending_name = block_header_names.get(&obj.handle.0).cloned();
            continue;
        }
        if matches_type_name(header.type_code, 0x04, "BLOCK", dynamic_types) {
            let recovered_name =
                recover_block_name_from_block_record(&record, decoder.version(), &header);
            let mut block_name = pending_name.clone();
            if !is_r2010_plus && recovered_name.as_ref().is_some_and(|name| !name.is_empty()) {
                block_name = recovered_name.clone();
            }
            if block_name.is_none() || block_name.as_ref().is_some_and(|name| name.is_empty()) {
                block_name = recovered_name;
            }
            if let Some(name) = block_name {
                if !name.is_empty() {
                    current_block_name = Some(name.clone());
                    block_aliases
                        .entry(obj.handle.0)
                        .or_insert_with(|| name.clone());
                    let prefer_prefixed = matches!(
                        decoder.version(),
                        version::DwgVersion::R2010
                            | version::DwgVersion::R2013
                            | version::DwgVersion::R2018
                    );
                    let decoded_handle = if prefer_prefixed {
                        let mut prefixed_reader = record.bit_reader();
                        if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok()
                        {
                            decode_block_record_handle(
                                &mut prefixed_reader,
                                decoder.version(),
                                &header,
                                obj.handle.0,
                            )
                        } else {
                            let mut reader = record.bit_reader();
                            decode_block_record_handle(
                                &mut reader,
                                decoder.version(),
                                &header,
                                obj.handle.0,
                            )
                        }
                    } else {
                        let mut reader = record.bit_reader();
                        decode_block_record_handle(
                            &mut reader,
                            decoder.version(),
                            &header,
                            obj.handle.0,
                        )
                    };
                    if let Ok(decoded_handle) = decoded_handle {
                        block_aliases.entry(decoded_handle).or_insert(name);
                    }
                } else {
                    current_block_name = None;
                }
            } else {
                current_block_name = None;
            }
            continue;
        }
        if matches_type_name(header.type_code, 0x05, "ENDBLK", dynamic_types) {
            let mut endblk_name = current_block_name.clone();
            if endblk_name.is_none() || endblk_name.as_ref().is_some_and(|name| name.is_empty()) {
                endblk_name = pending_name.clone();
            }
            let Some(name) = endblk_name else {
                pending_name = None;
                current_block_name = None;
                continue;
            };
            if !name.is_empty() {
                endblk_aliases
                    .entry(obj.handle.0)
                    .or_insert_with(|| name.clone());
                let prefer_prefixed = matches!(
                    decoder.version(),
                    version::DwgVersion::R2010
                        | version::DwgVersion::R2013
                        | version::DwgVersion::R2018
                );
                let decoded_handle = if prefer_prefixed {
                    let mut prefixed_reader = record.bit_reader();
                    if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
                        decode_block_record_handle(
                            &mut prefixed_reader,
                            decoder.version(),
                            &header,
                            obj.handle.0,
                        )
                    } else {
                        let mut reader = record.bit_reader();
                        decode_block_record_handle(
                            &mut reader,
                            decoder.version(),
                            &header,
                            obj.handle.0,
                        )
                    }
                } else {
                    let mut reader = record.bit_reader();
                    decode_block_record_handle(
                        &mut reader,
                        decoder.version(),
                        &header,
                        obj.handle.0,
                    )
                };
                if let Ok(decoded_handle) = decoded_handle {
                    endblk_aliases.entry(decoded_handle).or_insert(name);
                }
            }
            pending_name = None;
            current_block_name = None;
        }
    }
    Ok((block_aliases, endblk_aliases))
}

fn collect_block_header_stream_aliases_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    block_header_names: &HashMap<u64, String>,
    object_type_codes: &HashMap<u64, u16>,
    known_layer_handles: &HashSet<u64>,
) -> PyResult<HashMap<u64, String>> {
    if !matches!(
        decoder.version(),
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        return Ok(HashMap::new());
    }
    let mut aliases: HashMap<u64, String> = HashMap::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", dynamic_types) {
            continue;
        }
        let Some(name) = block_header_names.get(&obj.handle.0).cloned() else {
            continue;
        };
        if name.is_empty() {
            continue;
        }

        let expected_end_bit = resolve_r2010_object_data_end_bit(&header).ok();
        let mut end_bit_candidates = resolve_r2010_object_data_end_bit_candidates(&header);
        if end_bit_candidates.is_empty() {
            if let Some(expected) = expected_end_bit {
                end_bit_candidates.push(expected);
            }
        }
        let mut best_known: Option<(u64, u64)> = None;
        let mut best_unknown: Option<(u64, u64)> = None;
        for end_bit in end_bit_candidates {
            let mut reader = record.bit_reader();
            if skip_object_type_prefix(&mut reader, decoder.version()).is_err() {
                continue;
            }
            reader.set_bit_pos(end_bit);
            for index in 0..96u64 {
                let Ok(candidate) =
                    entities::common::read_handle_reference(&mut reader, obj.handle.0)
                else {
                    break;
                };
                if candidate == 0 || candidate == obj.handle.0 {
                    continue;
                }
                if known_layer_handles.contains(&candidate) {
                    continue;
                }

                let mut score = index.saturating_mul(16);
                if let Some(expected) = expected_end_bit {
                    score = score.saturating_add(expected.abs_diff(end_bit) as u64);
                }
                if let Some(type_code) = object_type_codes.get(&candidate) {
                    score = score.saturating_add(match *type_code {
                        0x04 => 0,   // BLOCK entity
                        0x05 => 40,  // ENDBLK
                        0x31 => 120, // BLOCK_HEADER itself
                        0x30 => 160, // BLOCK_CONTROL
                        0x33 => 240, // LAYER
                        _ => 80,
                    });
                } else {
                    // Unknown handle ids may still point to valid block-related records.
                    score = score.saturating_add(48);
                }
                if block_header_names.contains_key(&candidate) {
                    score = score.saturating_add(120);
                }

                let known_like = object_type_codes.contains_key(&candidate)
                    || block_header_names.contains_key(&candidate);
                if known_like {
                    match best_known {
                        Some((best_score, _)) if best_score <= score => {}
                        _ => best_known = Some((score, candidate)),
                    }
                } else {
                    match best_unknown {
                        Some((best_score, _)) if best_score <= score => {}
                        _ => best_unknown = Some((score, candidate)),
                    }
                }
            }
        }

        if let Some((_, alias_handle)) = best_known {
            aliases.entry(alias_handle).or_insert_with(|| name.clone());
        }
        if let Some((unknown_score, alias_handle)) = best_unknown {
            let allow_unknown = match best_known {
                Some((known_score, _)) => unknown_score <= known_score.saturating_add(128),
                None => true,
            };
            if allow_unknown {
                aliases.entry(alias_handle).or_insert(name);
            }
        }
    }
    Ok(aliases)
}

fn collect_block_header_targeted_aliases_in_order(
    decoder: &decoder::Decoder<'_>,
    dynamic_types: &HashMap<u16, String>,
    index: &objects::ObjectIndex,
    best_effort: bool,
    block_header_names: &HashMap<u64, String>,
    targets: &HashSet<u64>,
) -> PyResult<HashMap<u64, String>> {
    if targets.is_empty()
        || !matches!(
            decoder.version(),
            version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
        )
    {
        return Ok(HashMap::new());
    }
    let mut best: HashMap<u64, (u64, String)> = HashMap::new();
    for obj in index.objects.iter() {
        let Some((record, header)) = parse_record_and_header(decoder, obj.offset, best_effort)?
        else {
            continue;
        };
        if !matches_type_name(header.type_code, 0x31, "BLOCK_HEADER", dynamic_types) {
            continue;
        }
        let mut block_name = block_header_names.get(&obj.handle.0).cloned();
        if block_name.is_none() {
            let mut prefixed_reader = record.bit_reader();
            if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
                if let Ok((decoded_handle, _)) = decode_block_header_name_record(
                    &mut prefixed_reader,
                    decoder.version(),
                    obj.handle.0,
                    Some(&header),
                ) {
                    block_name = block_header_names.get(&decoded_handle).cloned();
                }
            }
        }
        let Some(block_name) = block_name else {
            continue;
        };
        if block_name.is_empty() {
            continue;
        }

        let expected_end_bit = resolve_r2010_object_data_end_bit(&header).ok();
        let mut end_bit_candidates = resolve_r2010_object_data_end_bit_candidates(&header);
        if let Some(expected) = expected_end_bit {
            for delta in (-64i32..=64).step_by(8) {
                let candidate_i64 = i64::from(expected) + i64::from(delta);
                if candidate_i64 < 0 {
                    continue;
                }
                if let Ok(candidate) = u32::try_from(candidate_i64) {
                    end_bit_candidates.push(candidate);
                }
            }
        }
        end_bit_candidates.sort_unstable();
        end_bit_candidates.dedup();

        let mut base_handles = vec![obj.handle.0];
        let mut prefixed_reader = record.bit_reader();
        if skip_object_type_prefix(&mut prefixed_reader, decoder.version()).is_ok() {
            if let Ok(record_handle) = prefixed_reader.read_h() {
                if record_handle.value != 0 {
                    base_handles.push(record_handle.value);
                }
            }
        }
        base_handles.sort_unstable();
        base_handles.dedup();

        for end_bit in end_bit_candidates {
            for base_handle in base_handles.iter().copied() {
                for chained_base in [false, true] {
                    let mut reader = record.bit_reader();
                    if skip_object_type_prefix(&mut reader, decoder.version()).is_err() {
                        continue;
                    }
                    reader.set_bit_pos(end_bit);
                    let mut prev_handle = base_handle;
                    for index in 0..256u64 {
                        let candidate = if chained_base {
                            match read_handle_reference_chained(&mut reader, &mut prev_handle) {
                                Ok(value) => value,
                                Err(_) => break,
                            }
                        } else {
                            match entities::common::read_handle_reference(&mut reader, base_handle)
                            {
                                Ok(value) => value,
                                Err(_) => break,
                            }
                        };
                        if candidate == 0 {
                            continue;
                        }
                        if !targets.contains(&candidate) {
                            continue;
                        }
                        let mut score = index.saturating_mul(8);
                        if let Some(expected) = expected_end_bit {
                            score = score.saturating_add(expected.abs_diff(end_bit) as u64);
                        }
                        if base_handle != obj.handle.0 {
                            score = score.saturating_add(12);
                        }
                        if chained_base {
                            score = score.saturating_add(8);
                        }
                        match best.get(&candidate) {
                            Some((best_score, _)) if *best_score <= score => {}
                            _ => {
                                best.insert(candidate, (score, block_name.clone()));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(best
        .into_iter()
        .map(|(handle, (_score, name))| (handle, name))
        .collect())
}

fn collect_insert_block_handle_candidates_r2010_plus(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
    object_handle: u64,
    parsed_block_handle: Option<u64>,
    known_block_handles: Option<&HashSet<u64>>,
    limit: usize,
) -> Vec<u64> {
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        return parsed_block_handle
            .filter(|handle| *handle != 0)
            .into_iter()
            .collect();
    }

    let parsed_block_handle = parsed_block_handle.filter(|handle| *handle != 0);
    let mut candidate_scores: HashMap<u64, u64> = HashMap::new();

    let mut base_handles = vec![object_handle];
    if object_handle > 1 {
        base_handles.push(object_handle - 1);
    }
    base_handles.push(object_handle.saturating_add(1));
    if object_handle > 2 {
        base_handles.push(object_handle - 2);
    }
    base_handles.push(object_handle.saturating_add(2));

    let mut base_reader = record.bit_reader();
    if skip_object_type_prefix(&mut base_reader, version).is_ok() {
        if let Ok(record_handle) = base_reader.read_h() {
            if record_handle.value != 0 {
                base_handles.push(record_handle.value);
                if record_handle.value > 1 {
                    base_handles.push(record_handle.value - 1);
                }
                base_handles.push(record_handle.value.saturating_add(1));
            }
        }
    }
    let mut base_reader_with_size = record.bit_reader();
    if skip_object_type_prefix(&mut base_reader_with_size, version).is_ok()
        && base_reader_with_size.read_rl(Endian::Little).is_ok()
    {
        if let Ok(record_handle) = base_reader_with_size.read_h() {
            if record_handle.value != 0 {
                base_handles.push(record_handle.value);
                if record_handle.value > 1 {
                    base_handles.push(record_handle.value - 1);
                }
                base_handles.push(record_handle.value.saturating_add(1));
            }
        }
    }
    base_handles.sort_unstable();
    base_handles.dedup();

    let mut expanded_end_bits = Vec::new();
    for base in resolve_r2010_object_data_end_bit_candidates(api_header) {
        for delta in (-256i32..=256).step_by(8) {
            let candidate_i64 = i64::from(base) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            let Ok(candidate) = u32::try_from(candidate_i64) else {
                continue;
            };
            expanded_end_bits.push(candidate);
        }
    }
    let mut stream_size_reader = record.bit_reader();
    if skip_object_type_prefix(&mut stream_size_reader, version).is_ok() {
        if let Ok(obj_size_bits) = stream_size_reader.read_rl(Endian::Little) {
            for delta in (-128i32..=128).step_by(8) {
                let candidate_i64 = i64::from(obj_size_bits) + i64::from(delta);
                if candidate_i64 < 0 {
                    continue;
                }
                if let Ok(candidate) = u32::try_from(candidate_i64) {
                    expanded_end_bits.push(candidate);
                }
            }
        }
    }
    if let Some(expected) = resolve_r2010_object_data_end_bit(api_header).ok() {
        for delta in -48i32..=48i32 {
            let candidate_i64 = i64::from(expected) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            if let Ok(candidate) = u32::try_from(candidate_i64) {
                expanded_end_bits.push(candidate);
            }
        }
    }
    expanded_end_bits.sort_unstable();
    expanded_end_bits.dedup();

    let expected_end_bit = resolve_r2010_object_data_end_bit(api_header).ok();
    let is_known = |candidate: u64| -> bool {
        known_block_handles
            .map(|handles| handles.contains(&candidate))
            .unwrap_or(false)
    };
    for object_data_end_bit in expanded_end_bits.iter().copied() {
        for base_handle in base_handles.iter().copied() {
            let Some(candidate) = parse_insert_block_header_handle_from_common_header(
                record,
                version,
                object_data_end_bit,
                base_handle,
            ) else {
                continue;
            };
            if candidate == 0 || candidate == object_handle {
                continue;
            }
            let mut score = expected_end_bit
                .map(|expected| expected.abs_diff(object_data_end_bit) as u64)
                .unwrap_or(0)
                .saturating_mul(4);
            if base_handle != object_handle {
                score = score.saturating_add(32);
            }
            if Some(candidate) == parsed_block_handle {
                score = score.saturating_sub(16);
            }
            if is_known(candidate) {
                score = score.saturating_sub(80);
            }
            match candidate_scores.get(&candidate).copied() {
                Some(best_score) if best_score <= score => {}
                _ => {
                    candidate_scores.insert(candidate, score);
                }
            }
        }
    }

    for object_data_end_bit in expanded_end_bits {
        for base_handle in base_handles.iter().copied() {
            for chained_base in [false, true] {
                let mut reader = record.bit_reader();
                if skip_object_type_prefix(&mut reader, version).is_err() {
                    continue;
                }
                reader.set_bit_pos(object_data_end_bit);
                let mut prev_handle = base_handle;
                for index in 0..64u64 {
                    let candidate = if chained_base {
                        match read_handle_reference_chained(&mut reader, &mut prev_handle) {
                            Ok(value) => value,
                            Err(_) => break,
                        }
                    } else {
                        match entities::common::read_handle_reference(&mut reader, base_handle) {
                            Ok(value) => value,
                            Err(_) => break,
                        }
                    };
                    if candidate == 0 || candidate == object_handle {
                        continue;
                    }
                    let mut score = index.saturating_mul(64);
                    if let Some(expected) = expected_end_bit {
                        score = score.saturating_add(expected.abs_diff(object_data_end_bit) as u64);
                    }
                    if base_handle != object_handle {
                        score = score.saturating_add(40);
                    }
                    if !chained_base {
                        score = score.saturating_add(8);
                    } else {
                        score = score.saturating_add(24);
                    }
                    if Some(candidate) == parsed_block_handle {
                        score = score.saturating_sub(12);
                    }
                    if is_known(candidate) {
                        score = score.saturating_sub(72);
                    }
                    match candidate_scores.get(&candidate).copied() {
                        Some(best_score) if best_score <= score => {}
                        _ => {
                            candidate_scores.insert(candidate, score);
                        }
                    }
                }
            }
        }
    }

    let mut scored: Vec<(u64, u64)> = candidate_scores.into_iter().collect();
    scored.sort_by_key(|(candidate, score)| (*score, *candidate));
    if let Some(parsed) = parsed_block_handle {
        if !scored.iter().any(|(candidate, _)| *candidate == parsed) {
            scored.insert(0, (parsed, 0));
        }
    }
    scored
        .into_iter()
        .take(limit.max(1))
        .map(|(candidate, _score)| candidate)
        .collect()
}

fn recover_insert_block_header_handle_r2010_plus(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
    object_handle: u64,
    parsed_block_handle: Option<u64>,
    known_block_handles: &HashSet<u64>,
    named_block_handles: &HashSet<u64>,
) -> Option<u64> {
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        return parsed_block_handle;
    }
    if known_block_handles.is_empty() {
        return parsed_block_handle;
    }
    let parsed_block_handle = parsed_block_handle.filter(|handle| *handle != 0);
    let mut best: Option<(u64, u64)> = None;
    if let Some(handle) = parsed_block_handle {
        if known_block_handles.contains(&handle) {
            if named_block_handles.is_empty() || named_block_handles.contains(&handle) {
                return Some(handle);
            }
            best = Some((10, handle));
        }
    }
    let mut base_handles = vec![object_handle];
    if object_handle > 1 {
        base_handles.push(object_handle - 1);
    }
    base_handles.push(object_handle.saturating_add(1));

    let mut base_reader = record.bit_reader();
    if skip_object_type_prefix(&mut base_reader, version).is_ok() {
        if let Ok(record_handle) = base_reader.read_h() {
            if record_handle.value != 0 {
                base_handles.push(record_handle.value);
                if record_handle.value > 1 {
                    base_handles.push(record_handle.value - 1);
                }
                base_handles.push(record_handle.value.saturating_add(1));
            }
        }
    }
    let mut base_reader_with_size = record.bit_reader();
    if skip_object_type_prefix(&mut base_reader_with_size, version).is_ok()
        && base_reader_with_size.read_rl(Endian::Little).is_ok()
    {
        if let Ok(record_handle) = base_reader_with_size.read_h() {
            if record_handle.value != 0 {
                base_handles.push(record_handle.value);
                if record_handle.value > 1 {
                    base_handles.push(record_handle.value - 1);
                }
                base_handles.push(record_handle.value.saturating_add(1));
            }
        }
    }

    let mut ordered_base_handles = Vec::with_capacity(base_handles.len());
    let mut seen_base_handles = HashSet::with_capacity(base_handles.len());
    for handle in base_handles {
        if seen_base_handles.insert(handle) {
            ordered_base_handles.push(handle);
        }
    }

    let mut expanded_end_bits = Vec::new();
    for base in resolve_r2010_object_data_end_bit_candidates(api_header) {
        for delta in (-256i32..=256).step_by(8) {
            let candidate_i64 = i64::from(base) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            let Ok(candidate) = u32::try_from(candidate_i64) else {
                continue;
            };
            expanded_end_bits.push(candidate);
        }
    }
    let mut stream_size_reader = record.bit_reader();
    if skip_object_type_prefix(&mut stream_size_reader, version).is_ok() {
        if let Ok(obj_size_bits) = stream_size_reader.read_rl(Endian::Little) {
            for delta in (-128i32..=128).step_by(8) {
                let candidate_i64 = i64::from(obj_size_bits) + i64::from(delta);
                if candidate_i64 < 0 {
                    continue;
                }
                if let Ok(candidate) = u32::try_from(candidate_i64) {
                    expanded_end_bits.push(candidate);
                }
            }
        }
    }
    expanded_end_bits.sort_unstable();
    expanded_end_bits.dedup();

    let expected_end_bit = resolve_r2010_object_data_end_bit(api_header).ok();
    for object_data_end_bit in expanded_end_bits.iter().copied() {
        for base_handle in ordered_base_handles.iter().copied() {
            let Some(candidate) = parse_insert_block_header_handle_from_common_header(
                record,
                version,
                object_data_end_bit,
                base_handle,
            ) else {
                continue;
            };
            let mut score = expected_end_bit
                .map(|expected| expected.abs_diff(object_data_end_bit) as u64)
                .unwrap_or(0)
                .saturating_mul(4);
            if base_handle != object_handle {
                score = score.saturating_add(32);
            }
            if named_block_handles.contains(&candidate) {
                score = score.saturating_sub(24);
            }
            if Some(candidate) == parsed_block_handle {
                score = score.saturating_sub(16);
            }
            if !known_block_handles.contains(&candidate) {
                continue;
            }
            match best {
                Some((best_score, _)) if best_score <= score => {}
                _ => best = Some((score, candidate)),
            }
        }
    }

    for object_data_end_bit in expanded_end_bits {
        for base_handle in ordered_base_handles.iter().copied() {
            for chained_base in [false, true] {
                let mut reader = record.bit_reader();
                if skip_object_type_prefix(&mut reader, version).is_err() {
                    continue;
                }
                reader.set_bit_pos(object_data_end_bit);
                let mut prev_handle = base_handle;
                for index in 0..64u64 {
                    let candidate = if chained_base {
                        match read_handle_reference_chained(&mut reader, &mut prev_handle) {
                            Ok(value) => value,
                            Err(_) => break,
                        }
                    } else {
                        match entities::common::read_handle_reference(&mut reader, base_handle) {
                            Ok(value) => value,
                            Err(_) => break,
                        }
                    };
                    let mut score = index.saturating_mul(64);
                    if let Some(expected) = expected_end_bit {
                        score = score.saturating_add(expected.abs_diff(object_data_end_bit) as u64);
                    }
                    if base_handle != object_handle {
                        score = score.saturating_add(40);
                    }
                    if !chained_base {
                        score = score.saturating_add(8);
                    } else {
                        score = score.saturating_add(24);
                    }
                    if named_block_handles.contains(&candidate) {
                        score = score.saturating_sub(20);
                    }
                    if Some(candidate) == parsed_block_handle {
                        score = score.saturating_sub(12);
                    }
                    if !known_block_handles.contains(&candidate) {
                        continue;
                    }
                    match best {
                        Some((best_score, _)) if best_score <= score => {}
                        _ => best = Some((score, candidate)),
                    }
                }
            }
        }
    }
    best.map(|(_, handle)| handle).or(parsed_block_handle)
}

fn recover_entity_layer_handle_r2010_plus(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
    object_handle: u64,
    parsed_layer_handle: u64,
    known_layer_handles: &HashSet<u64>,
) -> u64 {
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        return parsed_layer_handle;
    }
    if known_layer_handles.is_empty() {
        return parsed_layer_handle;
    }

    let expected_layer_index =
        parse_expected_entity_layer_ref_index(record, version, api_header, object_handle);
    let common_parsed_layer =
        parse_common_entity_layer_handle_from_common_header(record, version, api_header);
    let mut parsed_score = layer_handle_score(parsed_layer_handle, known_layer_handles);
    if known_layer_handles.contains(&parsed_layer_handle) {
        // Allow handle-stream candidates to override parsed value.
        parsed_score = parsed_score.saturating_add(1);
    }
    let mut best = (parsed_score, parsed_layer_handle);
    let default_layer = known_layer_handles.iter().copied().min();
    let debug_entity_handle = std::env::var("EZDWG_DEBUG_ENTITY_LAYER")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let debug_this = debug_entity_handle == Some(object_handle);
    if debug_this {
        eprintln!(
            "[entity-layer] handle={} parsed_layer={} parsed_score={}",
            object_handle, parsed_layer_handle, parsed_score
        );
        if let Some(layer) = common_parsed_layer {
            eprintln!(
                "[entity-layer] handle={} common_header_layer={}",
                object_handle, layer
            );
        }
    }
    if let Some(layer) = common_parsed_layer {
        let score = layer_handle_score(layer, known_layer_handles);
        if score < best.0 {
            best = (score, layer);
        }
    }
    let mut base_handles = vec![object_handle];
    if object_handle > 1 {
        base_handles.push(object_handle - 1);
    }
    base_handles.push(object_handle.saturating_add(1));
    let mut base_reader = record.bit_reader();
    if skip_object_type_prefix(&mut base_reader, version).is_ok() {
        if let Ok(record_handle) = base_reader.read_h() {
            let record_base = record_handle.value;
            if record_base != 0 && record_base != object_handle {
                base_handles.push(record_base);
                if record_base > 1 {
                    base_handles.push(record_base - 1);
                }
                base_handles.push(record_base.saturating_add(1));
            }
        }
    }
    let mut base_reader_with_size = record.bit_reader();
    if skip_object_type_prefix(&mut base_reader_with_size, version).is_ok()
        && base_reader_with_size.read_rl(Endian::Little).is_ok()
    {
        if let Ok(record_handle) = base_reader_with_size.read_h() {
            let record_base = record_handle.value;
            if record_base != 0 && !base_handles.contains(&record_base) {
                base_handles.push(record_base);
                if record_base > 1 {
                    base_handles.push(record_base - 1);
                }
                base_handles.push(record_base.saturating_add(1));
            }
        }
    }
    let mut ordered_base_handles = Vec::with_capacity(base_handles.len());
    let mut seen_base_handles = HashSet::with_capacity(base_handles.len());
    for handle in base_handles {
        if seen_base_handles.insert(handle) {
            ordered_base_handles.push(handle);
        }
    }

    let mut expanded_end_bits = Vec::new();
    for base in resolve_r2010_object_data_end_bit_candidates(api_header) {
        for delta in (-256i32..=256).step_by(8) {
            let candidate_i64 = i64::from(base) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            let Ok(candidate) = u32::try_from(candidate_i64) else {
                continue;
            };
            expanded_end_bits.push(candidate);
        }
    }
    let mut stream_size_reader = record.bit_reader();
    if skip_object_type_prefix(&mut stream_size_reader, version).is_ok() {
        if let Ok(obj_size_bits) = stream_size_reader.read_rl(Endian::Little) {
            for delta in (-128i32..=128).step_by(8) {
                let candidate_i64 = i64::from(obj_size_bits) + i64::from(delta);
                if candidate_i64 < 0 {
                    continue;
                }
                if let Ok(candidate) = u32::try_from(candidate_i64) {
                    expanded_end_bits.push(candidate);
                }
            }
        }
    }
    expanded_end_bits.sort_unstable();
    expanded_end_bits.dedup();

    for object_data_end_bit in expanded_end_bits {
        for base_handle in ordered_base_handles.iter().copied() {
            for chained_base in [false, true] {
                let mut reader = record.bit_reader();
                if skip_object_type_prefix(&mut reader, version).is_err() {
                    continue;
                }
                reader.set_bit_pos(object_data_end_bit);
                let mut prev_handle = base_handle;
                let mut handle_index = 0u64;
                while handle_index < 64 {
                    let layer_handle = if chained_base {
                        match read_handle_reference_chained(&mut reader, &mut prev_handle) {
                            Ok(handle) => handle,
                            Err(_) => break,
                        }
                    } else {
                        match entities::common::read_handle_reference(&mut reader, base_handle) {
                            Ok(handle) => handle,
                            Err(_) => break,
                        }
                    };
                    let mut score = layer_handle_score(layer_handle, known_layer_handles)
                        .saturating_add(handle_index);
                    if let Some(expected) = expected_layer_index {
                        let distance = handle_index.abs_diff(expected as u64);
                        score = score.saturating_add(distance.saturating_mul(16));
                        if handle_index == expected as u64 {
                            score = score.saturating_sub(120);
                        }
                    }
                    if handle_index == 0 {
                        // First handle is often owner-related; avoid overfitting to it.
                        score = score.saturating_add(200);
                    }
                    if chained_base {
                        // Relative-to-previous mode is speculative; keep fixed-base preference.
                        score = score.saturating_add(20);
                    }
                    if layer_handle == parsed_layer_handle
                        && known_layer_handles.contains(&layer_handle)
                    {
                        score = score.saturating_sub(80);
                    }
                    if Some(layer_handle) == default_layer {
                        score = score.saturating_add(150);
                    }
                    if debug_this && known_layer_handles.contains(&layer_handle) {
                        eprintln!(
                            "[entity-layer] handle={} end_bit={} base={} chained={} idx={} layer={} score={}",
                            object_handle,
                            object_data_end_bit,
                            base_handle,
                            chained_base,
                            handle_index,
                            layer_handle,
                            score
                        );
                    } else if debug_this && handle_index < 16 {
                        eprintln!(
                            "[entity-layer] handle={} end_bit={} base={} chained={} idx={} raw_layer={} score={}",
                            object_handle,
                            object_data_end_bit,
                            base_handle,
                            chained_base,
                            handle_index,
                            layer_handle,
                            score
                        );
                    }
                    if score < best.0 {
                        best = (score, layer_handle);
                        if score == 0 {
                            break;
                        }
                    }
                    handle_index += 1;
                }
                if best.0 == 0 {
                    break;
                }
            }
            if best.0 == 0 {
                break;
            }
        }
        if best.0 == 0 {
            break;
        }
    }

    if known_layer_handles.contains(&best.1) {
        if debug_this {
            eprintln!(
                "[entity-layer] handle={} selected={}",
                object_handle, best.1
            );
        }
        return best.1;
    }
    if known_layer_handles.contains(&parsed_layer_handle) {
        return parsed_layer_handle;
    }
    if let Some(default_layer) = known_layer_handles.iter().copied().min() {
        return default_layer;
    }
    best.1
}

fn parse_expected_entity_layer_ref_index(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
    object_handle: u64,
) -> Option<usize> {
    let object_data_end_bit = resolve_r2010_object_data_end_bit(api_header).ok()?;
    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return None;
    }
    let header = match version {
        version::DwgVersion::R2010 => {
            entities::common::parse_common_entity_header_r2010(&mut reader, object_data_end_bit)
                .ok()?
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            entities::common::parse_common_entity_header_r2013(&mut reader, object_data_end_bit)
                .ok()?
        }
        _ => return None,
    };

    let mut index = 0usize;
    if header.entity_mode == 0 {
        index = index.saturating_add(1);
    }
    index = index.saturating_add(header.num_of_reactors as usize);
    if header.xdic_missing_flag == 0 {
        index = index.saturating_add(1);
    }
    if matches!(api_header.type_code, 0x15 | 0x19 | 0x1A) {
        // R2010+ dimensions keep dimstyle and anonymous block handles
        // before common entity handles.
        index = index.saturating_add(2);
    }

    let debug_entity_handle = std::env::var("EZDWG_DEBUG_ENTITY_LAYER")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    if debug_entity_handle == Some(object_handle) {
        eprintln!(
            "[entity-layer] handle={} expected_index={} entity_mode={} reactors={} xdic_missing={} ltype_flags={} plotstyle_flags={} material_flags={} type=0x{:X}",
            object_handle,
            index,
            header.entity_mode,
            header.num_of_reactors,
            header.xdic_missing_flag,
            header.ltype_flags,
            header.plotstyle_flags,
            header.material_flags,
            api_header.type_code
        );
    }

    Some(index)
}

fn parse_common_entity_layer_handle_from_common_header(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
) -> Option<u64> {
    let object_data_end_bit = resolve_r2010_object_data_end_bit(api_header).ok()?;
    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return None;
    }
    let header = match version {
        version::DwgVersion::R2010 => {
            entities::common::parse_common_entity_header_r2010(&mut reader, object_data_end_bit)
                .ok()?
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            entities::common::parse_common_entity_header_r2013(&mut reader, object_data_end_bit)
                .ok()?
        }
        _ => return None,
    };
    reader.set_bit_pos(header.obj_size);
    entities::common::parse_common_entity_layer_handle(&mut reader, &header).ok()
}

fn parse_insert_block_header_handle_from_common_header(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    object_data_end_bit: u32,
    base_handle: u64,
) -> Option<u64> {
    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return None;
    }
    let mut header = match version {
        version::DwgVersion::R2010 => {
            entities::common::parse_common_entity_header_r2010(&mut reader, object_data_end_bit)
                .ok()?
        }
        version::DwgVersion::R2013 | version::DwgVersion::R2018 => {
            entities::common::parse_common_entity_header_r2013(&mut reader, object_data_end_bit)
                .ok()?
        }
        _ => return None,
    };
    header.handle = base_handle;
    reader.set_bit_pos(header.obj_size);
    entities::common::parse_common_entity_handles(&mut reader, &header).ok()?;
    entities::common::read_handle_reference(&mut reader, header.handle).ok()
}

fn read_handle_reference_chained(
    reader: &mut BitReader<'_>,
    prev_handle: &mut u64,
) -> crate::core::result::Result<u64> {
    let handle = reader.read_h()?;
    let absolute = match handle.code {
        0x06 => prev_handle.saturating_add(1),
        0x08 => prev_handle.saturating_sub(1),
        0x0A => prev_handle.saturating_add(handle.value),
        0x0C => prev_handle.saturating_sub(handle.value),
        0x02..=0x05 => handle.value,
        _ => handle.value,
    };
    *prev_handle = absolute;
    Ok(absolute)
}

fn layer_handle_score(layer_handle: u64, known_layer_handles: &HashSet<u64>) -> u64 {
    if known_layer_handles.contains(&layer_handle) {
        0
    } else if layer_handle == 0 {
        10_000
    } else {
        50_000
    }
}

fn decode_block_header_record_handle(
    reader: &mut BitReader<'_>,
    expected_handle: u64,
) -> crate::core::result::Result<u64> {
    let _obj_size_bits = reader.read_rl(Endian::Little)?;
    let record_handle = reader.read_h()?.value;
    Ok(if record_handle != 0 {
        record_handle
    } else {
        expected_handle
    })
}

fn recover_block_header_name_from_record(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
) -> Option<String> {
    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return None;
    }
    let start_bit = reader.tell_bits() as u32;
    let total_bits = api_header.data_size.saturating_mul(8);
    if start_bit >= total_bits {
        return None;
    }

    let mut end_bit_candidates: Vec<u32> = Vec::new();
    end_bit_candidates.extend(resolve_r2010_object_data_end_bit_candidates(api_header));
    let mut size_reader = reader.clone();
    if let Ok(obj_size_bits) = size_reader.read_rl(Endian::Little) {
        for delta in (-64i32..=64).step_by(8) {
            let candidate_i64 = i64::from(obj_size_bits) + i64::from(delta);
            if candidate_i64 < 0 {
                continue;
            }
            let Ok(candidate) = u32::try_from(candidate_i64) else {
                continue;
            };
            end_bit_candidates.push(candidate);
        }
    }
    end_bit_candidates.push(total_bits);
    end_bit_candidates.retain(|candidate| *candidate > start_bit && *candidate <= total_bits);
    end_bit_candidates.sort_unstable();
    end_bit_candidates.dedup();
    if end_bit_candidates.is_empty() {
        return None;
    }

    let canonical_end_bit = resolve_r2010_object_data_end_bit(api_header).ok();
    let mut best: Option<(u64, String)> = None;
    for end_bit in end_bit_candidates {
        let Some(name) = scan_block_header_name_in_string_stream(&reader, start_bit, end_bit)
        else {
            continue;
        };
        let mut score = score_block_name_candidate(&name);
        if let Some(canonical_end) = canonical_end_bit {
            score = score.saturating_add(canonical_end.abs_diff(end_bit) as u64);
        }
        match &best {
            Some((best_score, _)) if score >= *best_score => {}
            _ => best = Some((score, name)),
        }
    }
    best.map(|(_, name)| name)
}

fn recover_block_name_from_block_record(
    record: &objects::ObjectRecord<'_>,
    version: &version::DwgVersion,
    api_header: &ApiObjectHeader,
) -> Option<String> {
    let mut reader = record.bit_reader();
    if skip_object_type_prefix(&mut reader, version).is_err() {
        return None;
    }
    let start_bit = reader.tell_bits() as u32;
    let total_bits = api_header.data_size.saturating_mul(8);
    if start_bit >= total_bits {
        return None;
    }

    let mut best: Option<(u64, String)> = None;
    let mut consider = |name: String, score_bias: u64| {
        if !is_plausible_block_name(&name) {
            return;
        }
        let score = score_block_name_candidate(&name).saturating_add(score_bias);
        match &best {
            Some((best_score, _)) if score >= *best_score => {}
            _ => best = Some((score, name)),
        }
    };

    if matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        let mut end_bit_candidates: Vec<u32> = Vec::new();
        end_bit_candidates.extend(resolve_r2010_object_data_end_bit_candidates(api_header));
        end_bit_candidates.push(total_bits);
        end_bit_candidates.retain(|candidate| *candidate > start_bit && *candidate <= total_bits);
        end_bit_candidates.sort_unstable();
        end_bit_candidates.dedup();

        let canonical_end_bit = resolve_r2010_object_data_end_bit(api_header).ok();
        let base_reader = reader.clone();
        for end_bit in end_bit_candidates {
            for (stream_start_bit, stream_end_bit) in
                resolve_r2010_string_stream_ranges(&base_reader, end_bit)
            {
                if let Some(name) = scan_block_header_name_in_string_stream(
                    &base_reader,
                    stream_start_bit,
                    stream_end_bit,
                ) {
                    let end_bias = canonical_end_bit
                        .map(|canonical| canonical.abs_diff(end_bit) as u64)
                        .unwrap_or(0);
                    consider(name, end_bias);
                }
            }
        }
    }

    if let Some(name) = scan_block_header_name_in_string_stream(&reader, start_bit, total_bits) {
        consider(name, 32);
    }

    best.map(|(_, name)| name)
}

fn resolve_r2010_string_stream_ranges(
    base_reader: &BitReader<'_>,
    end_bit: u32,
) -> Vec<(u32, u32)> {
    if end_bit <= 1 {
        return Vec::new();
    }
    let mut present_reader = base_reader.clone();
    present_reader.set_bit_pos(end_bit.saturating_sub(1));
    let Ok(has_string_stream) = present_reader.read_b() else {
        return Vec::new();
    };
    if has_string_stream == 0 {
        return Vec::new();
    }

    let mut size_field_start = end_bit.saturating_sub(1);
    if size_field_start < 16 {
        return Vec::new();
    }
    size_field_start = size_field_start.saturating_sub(16);
    let mut size_reader = base_reader.clone();
    size_reader.set_bit_pos(size_field_start);
    let Ok(low_size_signed) = size_reader.read_rs(Endian::Little) else {
        return Vec::new();
    };
    let mut stream_size = u32::from(low_size_signed as u16);
    if (stream_size & 0x8000) != 0 {
        if size_field_start < 16 {
            return Vec::new();
        }
        size_field_start = size_field_start.saturating_sub(16);
        let mut hi_reader = base_reader.clone();
        hi_reader.set_bit_pos(size_field_start);
        let Ok(high_size_signed) = hi_reader.read_rs(Endian::Little) else {
            return Vec::new();
        };
        let high_size = u32::from(high_size_signed as u16);
        stream_size = (stream_size & 0x7FFF) | (high_size << 15);
    }

    let mut ranges = Vec::new();
    for multiplier in [1u32, 8u32] {
        let Some(size_bits) = stream_size.checked_mul(multiplier) else {
            continue;
        };
        if size_field_start < size_bits {
            continue;
        }
        let start_bit = size_field_start.saturating_sub(size_bits);
        if start_bit >= size_field_start {
            continue;
        }
        ranges.push((start_bit, size_field_start));
    }
    ranges.sort_unstable();
    ranges.dedup();
    ranges
}

fn decode_block_header_name_record(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    expected_handle: u64,
    api_header: Option<&ApiObjectHeader>,
) -> crate::core::result::Result<(u64, String)> {
    let obj_size_bits = reader.read_rl(Endian::Little)?;
    let record_handle = reader.read_h()?.value;
    skip_eed(reader)?;
    let _num_reactors = reader.read_bl()?;
    let _xdic_missing_flag = reader.read_b()?;
    if matches!(
        version,
        version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        let _has_ds_binary_data = reader.read_b()?;
    }

    let entry_name = if matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        let start_bit = reader.tell_bits() as u32;
        let total_bits = api_header
            .map(|header| header.data_size.saturating_mul(8))
            .unwrap_or(u32::MAX);
        let mut end_bit_candidates: Vec<u32> = Vec::new();
        if obj_size_bits > 0 {
            for delta in (-64i32..=64).step_by(8) {
                let candidate_i64 = i64::from(obj_size_bits) + i64::from(delta);
                if candidate_i64 < 0 {
                    continue;
                }
                let Ok(candidate) = u32::try_from(candidate_i64) else {
                    continue;
                };
                end_bit_candidates.push(candidate);
            }
        }
        if let Some(header) = api_header {
            for base in resolve_r2010_object_data_end_bit_candidates(header) {
                for delta in (-64i32..=64).step_by(8) {
                    let candidate_i64 = i64::from(base) + i64::from(delta);
                    if candidate_i64 < 0 {
                        continue;
                    }
                    let Ok(candidate) = u32::try_from(candidate_i64) else {
                        continue;
                    };
                    end_bit_candidates.push(candidate);
                }
            }
        }
        end_bit_candidates.retain(|candidate| *candidate > start_bit && *candidate <= total_bits);
        end_bit_candidates.sort_unstable();
        end_bit_candidates.dedup();
        if end_bit_candidates.is_empty() {
            if obj_size_bits > start_bit {
                end_bit_candidates.push(obj_size_bits.min(total_bits));
            } else if total_bits > start_bit {
                end_bit_candidates.push(total_bits);
            }
        }

        let canonical_end_bit =
            api_header.and_then(|header| resolve_r2010_object_data_end_bit(header).ok());
        let base_reader = reader.clone();
        let mut best_name: Option<(u64, String)> = None;
        for end_bit in end_bit_candidates {
            let mut candidate_name = String::new();
            let mut stream_best: Option<(u64, String)> = None;
            for (stream_start_bit, stream_end_bit) in
                resolve_r2010_string_stream_ranges(&base_reader, end_bit)
            {
                let mut stream_reader = base_reader.clone();
                stream_reader.set_bit_pos(stream_start_bit);
                if let Ok(name) = read_tu(&mut stream_reader) {
                    if stream_reader.tell_bits() <= stream_end_bit as u64
                        && is_plausible_block_name(&name)
                    {
                        let score = score_block_name_candidate(&name);
                        match &stream_best {
                            Some((best_score, _)) if score >= *best_score => {}
                            _ => stream_best = Some((score, name)),
                        }
                    }
                }
                if let Some(name) = scan_block_header_name_in_string_stream(
                    &base_reader,
                    stream_start_bit,
                    stream_end_bit,
                ) {
                    let score = score_block_name_candidate(&name);
                    match &stream_best {
                        Some((best_score, _)) if score >= *best_score => {}
                        _ => stream_best = Some((score, name)),
                    }
                }
            }
            if let Some((_, name)) = stream_best {
                candidate_name = name;
            }

            if candidate_name.is_empty() {
                let mut parsed_reader = reader.clone();
                if parse_block_header_nonstring_data_r2010_plus(&mut parsed_reader).is_ok() {
                    if let Ok(name) = read_tu(&mut parsed_reader) {
                        if parsed_reader.tell_bits() <= end_bit as u64
                            && is_plausible_block_name(&name)
                        {
                            candidate_name = name;
                        }
                    }
                }
            }
            if candidate_name.is_empty() {
                if let Some(name) =
                    scan_block_header_name_in_string_stream(&base_reader, start_bit, end_bit)
                {
                    candidate_name = name;
                }
            }
            if candidate_name.is_empty() {
                continue;
            }
            let mut score = score_block_name_candidate(&candidate_name);
            if let Some(canonical_end) = canonical_end_bit {
                score = score.saturating_add(canonical_end.abs_diff(end_bit) as u64);
            }
            match &best_name {
                Some((best_score, _)) if score >= *best_score => {}
                _ => best_name = Some((score, candidate_name)),
            }
        }
        best_name.map(|(_, name)| name).unwrap_or_default()
    } else {
        reader.read_tv()?
    };

    let handle = if record_handle != 0 {
        record_handle
    } else {
        expected_handle
    };
    Ok((handle, entry_name))
}

fn parse_block_header_nonstring_data_r2010_plus(
    reader: &mut BitReader<'_>,
) -> crate::core::result::Result<()> {
    let _flag_64 = reader.read_b()?;
    let _xref_index_plus1 = reader.read_bs()?;
    let _xdep = reader.read_b()?;
    let _anonymous = reader.read_b()?;
    let _has_atts = reader.read_b()?;
    let _blk_is_xref = reader.read_b()?;
    let _xref_overlaid = reader.read_b()?;
    let _loaded_bit = reader.read_b()?;
    let _owned_obj_count = reader.read_bl()?;
    let _base_pt = reader.read_3bd()?;
    loop {
        let marker = reader.read_rc()?;
        if marker == 0 {
            break;
        }
    }
    let preview_data_size = reader.read_bl()? as usize;
    let _preview_data = reader.read_rcs(preview_data_size)?;
    let _insert_units = reader.read_bs()?;
    let _explodable = reader.read_b()?;
    let _block_scaling = reader.read_rc()?;
    Ok(())
}

fn is_plausible_block_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 255 {
        return false;
    }
    let mut has_meaningful = false;
    for ch in name.chars() {
        if ch.is_control() {
            return false;
        }
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '*' | '-') {
            has_meaningful = true;
            continue;
        }
        if ch == ' ' {
            continue;
        }
        if !ch.is_ascii_graphic() {
            return false;
        }
    }
    has_meaningful
}

fn score_block_name_candidate(name: &str) -> u64 {
    let mut score = 0u64;
    if name.len() <= 2 {
        score = score.saturating_add(24);
    } else if name.len() <= 4 {
        score = score.saturating_add(8);
    }
    if name.len() > 96 {
        score = score.saturating_add((name.len() - 96) as u64);
    }
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '*' | '-' | '.') {
            continue;
        }
        if ch == ' ' {
            score = score.saturating_add(120);
        } else if ch.is_ascii_graphic() {
            score = score.saturating_add(240);
        } else {
            score = score.saturating_add(500);
        }
    }
    if name.starts_with('*') {
        score = score.saturating_add(8);
    }
    if name.chars().all(|ch| ch.is_ascii_digit()) {
        score = score.saturating_add(64);
    }
    score
}

fn scan_block_header_name_in_string_stream(
    base_reader: &BitReader<'_>,
    start_bit: u32,
    end_bit: u32,
) -> Option<String> {
    if start_bit >= end_bit {
        return None;
    }
    // String stream alignment is byte-based; scanning the full range is cheap
    // and more robust than relying on a fixed tail window.
    let scan_start = start_bit;
    let mut best: Option<(u64, String)> = None;
    let mut bit = scan_start;
    let mut tried = 0u32;
    let max_tries = end_bit
        .saturating_sub(scan_start)
        .saturating_div(8)
        .saturating_add(2)
        .min(65_536);
    while bit + 16 <= end_bit && tried < max_tries {
        let mut candidate_reader = base_reader.clone();
        candidate_reader.set_bit_pos(bit);
        let Ok(name) = read_tu(&mut candidate_reader) else {
            bit = bit.saturating_add(8);
            tried = tried.saturating_add(1);
            continue;
        };
        if candidate_reader.tell_bits() > end_bit as u64 || !is_plausible_block_name(&name) {
            bit = bit.saturating_add(8);
            tried = tried.saturating_add(1);
            continue;
        }
        let trailing_gap_bits = end_bit as u64 - candidate_reader.tell_bits();
        let mut score = score_block_name_candidate(&name);
        score = score.saturating_add(trailing_gap_bits / 128);
        match &best {
            Some((best_score, _)) if score >= *best_score => {}
            _ => best = Some((score, name)),
        }
        bit = bit.saturating_add(8);
        tried = tried.saturating_add(1);
    }
    best.map(|(_, name)| name)
}

fn recover_r2010_mtext_text(
    reader_after_prefix: &BitReader<'_>,
    header: &ApiObjectHeader,
    inline_text: &str,
) -> Option<String> {
    let total_bits = header.data_size.saturating_mul(8);
    let start_bit = reader_after_prefix.tell_bits() as u32;
    if total_bits <= start_bit.saturating_add(16) {
        return None;
    }

    let mut end_bit_candidates = resolve_r2010_object_data_end_bit_candidates(header);
    end_bit_candidates.push(total_bits);
    end_bit_candidates.retain(|candidate| *candidate > start_bit && *candidate <= total_bits);
    end_bit_candidates.sort_unstable();
    end_bit_candidates.dedup();
    if end_bit_candidates.is_empty() {
        return None;
    }

    let current_score = score_mtext_text_quality(inline_text);
    let canonical_end_bit = resolve_r2010_object_data_end_bit(header).ok();
    let mut best: Option<(u64, String)> = None;
    for end_bit in end_bit_candidates {
        for (stream_start_bit, stream_end_bit) in
            resolve_r2010_string_stream_ranges(reader_after_prefix, end_bit)
        {
            if let Some((mut score, text)) = scan_mtext_text_in_string_stream(
                reader_after_prefix,
                stream_start_bit,
                stream_end_bit,
            ) {
                if let Some(canonical) = canonical_end_bit {
                    score = score.saturating_add(canonical.abs_diff(end_bit) as u64);
                }
                match &best {
                    Some((best_score, _)) if score >= *best_score => {}
                    _ => best = Some((score, text)),
                }
            }
        }
    }
    let Some((best_score, best_text)) = best else {
        return None;
    };
    if best_score.saturating_add(32) < current_score {
        Some(best_text)
    } else {
        None
    }
}

fn scan_mtext_text_in_string_stream(
    base_reader: &BitReader<'_>,
    start_bit: u32,
    end_bit: u32,
) -> Option<(u64, String)> {
    if start_bit >= end_bit {
        return None;
    }
    let mut best: Option<(u64, String)> = None;
    let mut bit = start_bit;
    let mut tried = 0u32;
    let max_tries = end_bit
        .saturating_sub(start_bit)
        .saturating_div(8)
        .saturating_add(2)
        .min(65_536);
    while bit + 16 <= end_bit && tried < max_tries {
        let mut candidate_reader = base_reader.clone();
        candidate_reader.set_bit_pos(bit);
        let Ok(candidate) = read_tu(&mut candidate_reader) else {
            bit = bit.saturating_add(8);
            tried = tried.saturating_add(1);
            continue;
        };
        if candidate_reader.tell_bits() > end_bit as u64 || !is_plausible_mtext_text(&candidate) {
            bit = bit.saturating_add(8);
            tried = tried.saturating_add(1);
            continue;
        }

        let trailing_gap_bits = end_bit as u64 - candidate_reader.tell_bits();
        let score = score_mtext_text_quality(&candidate).saturating_add(trailing_gap_bits / 64);
        match &best {
            Some((best_score, _)) if score >= *best_score => {}
            _ => best = Some((score, candidate)),
        }
        bit = bit.saturating_add(8);
        tried = tried.saturating_add(1);
    }
    best
}

fn is_plausible_mtext_text(text: &str) -> bool {
    let len = text.chars().count();
    if !(2..=4096).contains(&len) {
        return false;
    }
    if text.contains('\u{0000}') || text.contains('\u{fffd}') {
        return false;
    }
    let mut has_meaningful = false;
    for ch in text.chars() {
        if ch.is_control() && !matches!(ch, '\n' | '\r' | '\t') {
            return false;
        }
        if ch.is_alphanumeric() || ch.is_whitespace() || ch.is_ascii_punctuation() {
            has_meaningful = true;
        }
    }
    has_meaningful
}

fn score_mtext_text_quality(text: &str) -> u64 {
    if text.is_empty() {
        return 1_000_000;
    }
    let len = text.chars().count() as u64;
    let mut score = 0u64;
    if len <= 1 {
        score = score.saturating_add(50_000);
    } else if len == 2 {
        score = score.saturating_add(5_000);
    }
    if len > 4096 {
        score = score.saturating_add((len - 4096) * 10);
    }

    let mut meaningful = 0u64;
    for ch in text.chars() {
        if ch == '\u{fffd}' || ch == '\u{0000}' {
            score = score.saturating_add(10_000);
            continue;
        }
        if ch.is_control() && !matches!(ch, '\n' | '\r' | '\t') {
            score = score.saturating_add(5_000);
            continue;
        }
        if ch.is_alphanumeric() || ch.is_whitespace() || ch.is_ascii_punctuation() {
            meaningful = meaningful.saturating_add(1);
        } else if !ch.is_control() {
            // Treat non-ASCII printable glyphs (e.g. CJK, symbols) as meaningful.
            meaningful = meaningful.saturating_add(1);
        }
    }
    if meaningful == 0 {
        score = score.saturating_add(25_000);
    }
    score
}

fn decode_layer_color_record(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    expected_handle: u64,
) -> crate::core::result::Result<(u64, u16, Option<u32>)> {
    // R2010+/R2013 objects start with handle directly after OT prefix.
    // Older versions keep ObjSize (RL) before handle.
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        let _obj_size = reader.read_rl(Endian::Little)?;
    }
    let record_handle = reader.read_h()?.value;
    skip_eed(reader)?;

    let _num_reactors = reader.read_bl()?;
    let _xdic_missing_flag = reader.read_b()?;
    if matches!(
        version,
        version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        let _has_ds_binary_data = reader.read_b()?;
    }
    // R2010+ stores entry name in string stream. The data stream directly
    // continues with layer state flags and color data.
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013 | version::DwgVersion::R2018
    ) {
        let _entry_name = reader.read_tv()?;
    }

    let style_start = reader.get_pos();
    let variants = [
        LayerColorParseVariant {
            pre_flag_bits: 0,
            post_flag_bits: 0,
            pre_values_bits: 0,
        },
        LayerColorParseVariant {
            pre_flag_bits: 2,
            post_flag_bits: 0,
            pre_values_bits: 0,
        },
        LayerColorParseVariant {
            pre_flag_bits: 0,
            post_flag_bits: 2,
            pre_values_bits: 0,
        },
        LayerColorParseVariant {
            pre_flag_bits: 0,
            post_flag_bits: 0,
            pre_values_bits: 2,
        },
        LayerColorParseVariant {
            pre_flag_bits: 2,
            post_flag_bits: 2,
            pre_values_bits: 0,
        },
        LayerColorParseVariant {
            pre_flag_bits: 2,
            post_flag_bits: 0,
            pre_values_bits: 2,
        },
        LayerColorParseVariant {
            pre_flag_bits: 0,
            post_flag_bits: 2,
            pre_values_bits: 2,
        },
        LayerColorParseVariant {
            pre_flag_bits: 2,
            post_flag_bits: 2,
            pre_values_bits: 2,
        },
    ];

    let mut best: Option<(u64, (u16, Option<u32>))> = None;
    for variant in variants {
        reader.set_pos(style_start.0, style_start.1);
        let Ok((color_index, true_color, color_byte)) = decode_layer_color_cmc(reader, variant)
        else {
            continue;
        };
        let score = layer_color_candidate_score(color_index, true_color, color_byte);
        match &best {
            Some((best_score, _)) if score >= *best_score => {}
            _ => best = Some((score, (color_index, true_color))),
        }
    }

    if let Some((_, (color_index, true_color))) = best {
        let handle = if record_handle != 0 {
            record_handle
        } else {
            expected_handle
        };
        return Ok((handle, color_index, true_color));
    }

    // Last resort: parse in the simplest form to keep progress.
    reader.set_pos(style_start.0, style_start.1);
    let (color_index, true_color, _) = decode_layer_color_cmc(reader, variants[0])?;
    let handle = if record_handle != 0 {
        record_handle
    } else {
        expected_handle
    };
    Ok((handle, color_index, true_color))
}

#[derive(Clone, Copy)]
struct LayerColorParseVariant {
    pre_flag_bits: u8,
    post_flag_bits: u8,
    pre_values_bits: u8,
}

fn decode_layer_color_cmc(
    reader: &mut BitReader<'_>,
    variant: LayerColorParseVariant,
) -> crate::core::result::Result<(u16, Option<u32>, u8)> {
    if variant.pre_flag_bits > 0 {
        let _unknown = reader.read_bits_msb(variant.pre_flag_bits)?;
    }
    let _flag_64 = reader.read_b()?;
    if variant.post_flag_bits > 0 {
        let _unknown = reader.read_bits_msb(variant.post_flag_bits)?;
    }
    let _xref_index_plus_one = reader.read_bs()?;
    let _xdep = reader.read_b()?;
    let _frozen = reader.read_b()?;
    let _on = reader.read_b()?;
    let _frozen_new = reader.read_b()?;
    let _locked = reader.read_b()?;
    if variant.pre_values_bits > 0 {
        let _unknown = reader.read_bits_msb(variant.pre_values_bits)?;
    }
    let _values = reader.read_bs()?;

    let color_index = reader.read_bs()?;
    let color_rgb = reader.read_bl()?;
    let color_byte = reader.read_rc()?;
    if (color_byte & 0x01) != 0 {
        let _color_name = reader.read_tv()?;
    }
    if (color_byte & 0x02) != 0 {
        let _book_name = reader.read_tv()?;
    }

    let true_color = if color_rgb == 0 || (color_rgb >> 24) == 0 {
        // Keep only true 24-bit payload with marker byte present.
        // If high byte is zero, treat as unset to prefer indexed color.
        None
    } else {
        let rgb = color_rgb & 0x00FF_FFFF;
        if rgb == 0 {
            None
        } else {
            Some(rgb)
        }
    };
    Ok((color_index, true_color, color_byte))
}

fn layer_color_candidate_score(color_index: u16, true_color: Option<u32>, color_byte: u8) -> u64 {
    let mut score = 0u64;

    if color_index <= 257 {
        score += 0;
    } else if color_index <= 4096 {
        score += 1_000;
    } else {
        score += 100_000;
    }

    if color_byte <= 3 {
        score += 0;
    } else {
        score += 10_000;
    }

    if let Some(rgb) = true_color {
        if rgb == 0 || rgb > 0x00FF_FFFF {
            score += 10_000;
        }
    }

    score
}

fn skip_eed(reader: &mut BitReader<'_>) -> crate::core::result::Result<()> {
    let mut ext_size = reader.read_bs()?;
    while ext_size > 0 {
        let _app_handle = reader.read_h()?;
        for _ in 0..ext_size {
            let _ = reader.read_rc()?;
        }
        ext_size = reader.read_bs()?;
    }
    Ok(())
}

fn read_tu(reader: &mut BitReader<'_>) -> crate::core::result::Result<String> {
    let length = reader.read_bs()? as usize;
    let mut units = Vec::with_capacity(length);
    for _ in 0..length {
        units.push(reader.read_rs(Endian::Little)?);
    }
    Ok(String::from_utf16_lossy(&units))
}

fn is_recoverable_decode_error(err: &DwgError) -> bool {
    matches!(
        err.kind,
        ErrorKind::NotImplemented | ErrorKind::Decode | ErrorKind::Format
    )
}

fn build_decoder(bytes: &[u8]) -> crate::core::result::Result<decoder::Decoder<'_>> {
    decoder::Decoder::new(bytes, Default::default())
}

fn to_py_err(err: DwgError) -> PyErr {
    let message = err.to_string();
    match err.kind {
        ErrorKind::Io => PyIOError::new_err(message),
        ErrorKind::Format | ErrorKind::Decode | ErrorKind::Resolve | ErrorKind::Unsupported => {
            PyValueError::new_err(message)
        }
        ErrorKind::NotImplemented => PyNotImplementedError::new_err(message),
    }
}

fn points_equal_3d(a: (f64, f64, f64), b: (f64, f64, f64)) -> bool {
    const EPS: f64 = 1e-9;
    (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() < EPS && (a.2 - b.2).abs() < EPS
}

fn strip_closure(mut points: Vec<(f64, f64, f64)>) -> Vec<(f64, f64, f64)> {
    if points.len() > 1 {
        let first = points[0];
        let last = *points.last().unwrap();
        if points_equal_3d(first, last) {
            points.pop();
        }
    }
    points
}

fn points_equal_3d_with_data(
    a: (f64, f64, f64, f64, f64, f64, f64, u16),
    b: (f64, f64, f64, f64, f64, f64, f64, u16),
) -> bool {
    points_equal_3d((a.0, a.1, a.2), (b.0, b.1, b.2))
}

fn resolved_type_name(type_code: u16, dynamic_types: &HashMap<u16, String>) -> String {
    dynamic_types
        .get(&type_code)
        .cloned()
        .unwrap_or_else(|| objects::object_type_name(type_code))
}

fn resolved_type_class(type_code: u16, resolved_name: &str) -> String {
    let class = objects::object_type_class(type_code).as_str();
    if !class.is_empty() {
        return class.to_string();
    }
    if is_known_entity_type_name(resolved_name) {
        return "E".to_string();
    }
    String::new()
}

fn matches_type_name(
    type_code: u16,
    builtin_code: u16,
    builtin_name: &str,
    dynamic_types: &HashMap<u16, String>,
) -> bool {
    if type_code == builtin_code {
        return true;
    }
    dynamic_types
        .get(&type_code)
        .map(|name| name == builtin_name)
        .unwrap_or(false)
}

fn matches_type_filter(filter: &HashSet<u16>, type_code: u16, resolved_name: &str) -> bool {
    if filter.contains(&type_code) {
        return true;
    }
    if let Some(builtin_code) = builtin_code_from_name(resolved_name) {
        return filter.contains(&builtin_code);
    }
    false
}

fn builtin_code_from_name(name: &str) -> Option<u16> {
    match name {
        "TEXT" => Some(0x01),
        "SEQEND" => Some(0x06),
        "INSERT" => Some(0x07),
        "VERTEX_2D" => Some(0x0A),
        "CIRCLE" => Some(0x12),
        "POLYLINE_2D" => Some(0x0F),
        "ARC" => Some(0x11),
        "LINE" => Some(0x13),
        "POINT" => Some(0x1B),
        "ELLIPSE" => Some(0x23),
        "MTEXT" => Some(0x2C),
        "LWPOLYLINE" => Some(0x4D),
        "DIM_LINEAR" => Some(0x15),
        "DIM_RADIUS" => Some(0x19),
        "DIM_DIAMETER" => Some(0x1A),
        "DIMENSION" => Some(0x15),
        _ => None,
    }
}

fn is_known_entity_type_name(name: &str) -> bool {
    builtin_code_from_name(name).is_some()
}
