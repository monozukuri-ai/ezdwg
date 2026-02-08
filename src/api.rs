#![allow(clippy::useless_conversion)] // Triggered by PyO3 #[pyfunction] wrapper expansion.

use pyo3::exceptions::{PyIOError, PyNotImplementedError, PyValueError};
use pyo3::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::bit::{BitReader, Endian};
use crate::core::error::{DwgError, ErrorKind};
use crate::dwg::decoder;
use crate::dwg::file_open;
use crate::dwg::version;
use crate::entities;
use crate::objects;

type Point2 = (f64, f64);
type Point3 = (f64, f64, f64);

type SectionLocatorRow = (String, u32, u32);
type ObjectMapEntryRow = (u64, u32);
type ObjectHeaderRow = (u64, u32, u32, u16);
type ObjectHeaderWithTypeRow = (u64, u32, u32, u16, String, String);
type ObjectRecordBytesRow = (u64, u32, u32, u16, Vec<u8>);
type EntityStyleRow = (u64, Option<u16>, Option<u32>, u64);
type LayerColorRow = (u64, u16, Option<u32>);

type LineEntityRow = (u64, f64, f64, f64, f64, f64, f64);
type PointEntityRow = (u64, f64, f64, f64, f64);
type ArcEntityRow = (u64, f64, f64, f64, f64, f64, f64);
type CircleEntityRow = (u64, f64, f64, f64, f64);
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
    Option<u64>,
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
type InsertEntityRow = (u64, f64, f64, f64, f64, f64, f64, f64);
type MInsertEntityRow = (u64, f64, f64, f64, f64, f64, f64, f64, u16, u16, f64, f64);
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
type PolylineVerticesRow = (u64, u16, Vec<Point3>);
type PolylineInterpolatedRow = (u64, u16, bool, Vec<Point3>);
type Vertex2dEntityRow = (u64, u16, f64, f64, f64, f64, f64, f64, f64);
type VertexDataRow = (f64, f64, f64, f64, f64, f64, f64, u16);
type PolylineVertexDataRow = (u64, u16, Vec<VertexDataRow>);

#[pyfunction]
pub fn detect_version(path: &str) -> PyResult<String> {
    let tag = file_open::read_version_tag(path).map_err(to_py_err)?;
    let version = version::detect_version(&tag).map_err(to_py_err)?;
    Ok(version.as_str().to_string())
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
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = decoder.parse_object_record(obj.offset).map_err(to_py_err)?;
        let header =
            parse_object_header_for_version(&record, decoder.version()).map_err(to_py_err)?;
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
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = decoder.parse_object_record(obj.offset).map_err(to_py_err)?;
        let header =
            parse_object_header_for_version(&record, decoder.version()).map_err(to_py_err)?;
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
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity =
            match decode_line_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
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
                Err(err) if best_effort => continue,
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
        let entity =
            match decode_mtext_for_version(&mut reader, decoder.version(), &header, obj.handle.0) {
                Ok(entity) => entity,
                Err(err) if best_effort => continue,
                Err(err) => return Err(to_py_err(err)),
            };
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

#[pyfunction(signature = (path, limit=None))]
pub fn decode_insert_entities(path: &str, limit: Option<usize>) -> PyResult<Vec<InsertEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = decoder.parse_object_record(obj.offset).map_err(to_py_err)?;
        let header =
            parse_object_header_for_version(&record, decoder.version()).map_err(to_py_err)?;
        if !matches_type_name(header.type_code, 0x07, "INSERT", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        let _type_code =
            skip_object_type_prefix(&mut reader, decoder.version()).map_err(to_py_err)?;
        let entity = entities::decode_insert(&mut reader).map_err(to_py_err)?;
        result.push((
            entity.handle,
            entity.position.0,
            entity.position.1,
            entity.position.2,
            entity.scale.0,
            entity.scale.1,
            entity.scale.2,
            entity.rotation,
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
pub fn decode_minsert_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<MInsertEntityRow>> {
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
        if !matches_type_name(header.type_code, 0x08, "MINSERT", &dynamic_types) {
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
        result.push((
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
pub fn decode_polyline_2d_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<Polyline2dEntityRow>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = decoder.parse_object_record(obj.offset).map_err(to_py_err)?;
        let header =
            parse_object_header_for_version(&record, decoder.version()).map_err(to_py_err)?;
        if !matches_type_name(header.type_code, 0x0F, "POLYLINE_2D", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        let _type_code =
            skip_object_type_prefix(&mut reader, decoder.version()).map_err(to_py_err)?;
        let entity = entities::decode_polyline_2d(&mut reader).map_err(to_py_err)?;
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
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = decoder.parse_object_record(obj.offset).map_err(to_py_err)?;
        let header =
            parse_object_header_for_version(&record, decoder.version()).map_err(to_py_err)?;
        if !matches_type_name(header.type_code, 0x0F, "POLYLINE_2D", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        let _type_code =
            skip_object_type_prefix(&mut reader, decoder.version()).map_err(to_py_err)?;
        let entity = entities::decode_polyline_2d(&mut reader).map_err(to_py_err)?;
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
            Err(err) if best_effort => continue,
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
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut result = Vec::new();
    for obj in index.objects.iter() {
        let record = decoder.parse_object_record(obj.offset).map_err(to_py_err)?;
        let header =
            parse_object_header_for_version(&record, decoder.version()).map_err(to_py_err)?;
        if !matches_type_name(header.type_code, 0x0A, "VERTEX_2D", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        let _type_code =
            skip_object_type_prefix(&mut reader, decoder.version()).map_err(to_py_err)?;
        let vertex = entities::decode_vertex_2d(&mut reader).map_err(to_py_err)?;
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
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut sorted = index.objects.clone();
    sorted.sort_by_key(|obj| obj.offset);

    let vertex_map = build_vertex_2d_map(&decoder, &sorted, &dynamic_types)?;
    let mut result = Vec::new();
    let mut i = 0usize;
    while i < sorted.len() {
        let obj = sorted[i];
        let record = decoder.parse_object_record(obj.offset).map_err(to_py_err)?;
        let header =
            parse_object_header_for_version(&record, decoder.version()).map_err(to_py_err)?;
        if !matches_type_name(header.type_code, 0x0F, "POLYLINE_2D", &dynamic_types) {
            i += 1;
            continue;
        }

        let mut reader = record.bit_reader();
        let _type_code =
            skip_object_type_prefix(&mut reader, decoder.version()).map_err(to_py_err)?;
        let poly = entities::decode_polyline_2d(&mut reader).map_err(to_py_err)?;
        let (vertices, next_i) =
            collect_polyline_vertices(&decoder, &sorted, &dynamic_types, &vertex_map, &poly, i)?;
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
) -> PyResult<HashMap<u64, entities::Vertex2dEntity>> {
    let mut vertex_map = HashMap::new();
    for obj in sorted {
        let record = decoder.parse_object_record(obj.offset).map_err(to_py_err)?;
        let header =
            parse_object_header_for_version(&record, decoder.version()).map_err(to_py_err)?;
        if !matches_type_name(header.type_code, 0x0A, "VERTEX_2D", dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        let _type_code =
            skip_object_type_prefix(&mut reader, decoder.version()).map_err(to_py_err)?;
        let vertex = entities::decode_vertex_2d(&mut reader).map_err(to_py_err)?;
        vertex_map.insert(vertex.handle, vertex);
    }
    Ok(vertex_map)
}

fn collect_polyline_vertices(
    decoder: &decoder::Decoder<'_>,
    sorted: &[objects::ObjectRef],
    dynamic_types: &HashMap<u16, String>,
    vertex_map: &HashMap<u64, entities::Vertex2dEntity>,
    poly: &entities::Polyline2dEntity,
    start_index: usize,
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

    let mut next_i = start_index + 1;
    while next_i < sorted.len() {
        let next = sorted[next_i];
        let next_record = decoder
            .parse_object_record(next.offset)
            .map_err(to_py_err)?;
        let next_header =
            parse_object_header_for_version(&next_record, decoder.version()).map_err(to_py_err)?;
        let mut next_reader = next_record.bit_reader();
        if matches_type_name(next_header.type_code, 0x0A, "VERTEX_2D", dynamic_types) {
            let _next_type =
                skip_object_type_prefix(&mut next_reader, decoder.version()).map_err(to_py_err)?;
            let vertex = entities::decode_vertex_2d(&mut next_reader).map_err(to_py_err)?;
            vertices.push(vertex);
            next_i += 1;
            continue;
        }
        if matches_type_name(next_header.type_code, 0x06, "SEQEND", dynamic_types) {
            let _next_type =
                skip_object_type_prefix(&mut next_reader, decoder.version()).map_err(to_py_err)?;
            let _seqend = entities::decode_seqend(&mut next_reader).map_err(to_py_err)?;
            next_i += 1;
        }
        break;
    }

    Ok((vertices, next_i))
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

pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(detect_version, module)?)?;
    module.add_function(wrap_pyfunction!(list_section_locators, module)?)?;
    module.add_function(wrap_pyfunction!(read_section_bytes, module)?)?;
    module.add_function(wrap_pyfunction!(list_object_map_entries, module)?)?;
    module.add_function(wrap_pyfunction!(list_object_headers, module)?)?;
    module.add_function(wrap_pyfunction!(list_object_headers_with_type, module)?)?;
    module.add_function(wrap_pyfunction!(list_object_headers_by_type, module)?)?;
    module.add_function(wrap_pyfunction!(read_object_records_by_type, module)?)?;
    module.add_function(wrap_pyfunction!(decode_entity_styles, module)?)?;
    module.add_function(wrap_pyfunction!(decode_layer_colors, module)?)?;
    module.add_function(wrap_pyfunction!(decode_line_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_point_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_arc_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_circle_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_ellipse_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_spline_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_text_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_attrib_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_attdef_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_mtext_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_leader_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_hatch_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_linear_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_ordinate_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_aligned_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_ang3pt_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_ang2ln_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_diameter_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_radius_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_insert_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_minsert_entities, module)?)?;
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
    Ok(())
}

fn is_best_effort_compat_version(decoder: &decoder::Decoder<'_>) -> bool {
    matches!(
        decoder.version(),
        version::DwgVersion::R2000 | version::DwgVersion::R2010 | version::DwgVersion::R2013
    )
}

fn decode_line_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::LineEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_line_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_line_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_line_r2007(reader),
        _ => entities::decode_line(reader),
    }
}

fn decode_point_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
    object_handle: u64,
) -> crate::core::result::Result<entities::PointEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_point_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_arc_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_circle_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_ellipse_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_text_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2013 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_hatch_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_hatch_r2007(reader),
        version::DwgVersion::R2004 => entities::decode_hatch_r2004(reader),
        _ => entities::decode_hatch(reader),
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
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_lwpolyline_r2010(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2013 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_lwpolyline_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_lwpolyline_r2007(reader),
        _ => entities::decode_lwpolyline(reader),
    }
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
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2013 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_vertex_3d_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_vertex_3d_r2007(reader),
        _ => entities::decode_vertex_3d(reader),
    }
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
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2013 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_vertex_pface_face_r2013(reader, object_data_end_bit, object_handle)
        }
        version::DwgVersion::R2007 => entities::decode_vertex_pface_face_r2007(reader),
        _ => entities::decode_vertex_pface_face(reader),
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
    // R2010+/R2013 object header reports handle stream size with an extra
    // one-byte string-stream marker payload. Exclude it from the handle stream
    // bit count when seeking to entity handle references.
    let effective_handle_bits = handle_bits.saturating_sub(8);
    total_bits
        .checked_sub(effective_handle_bits)
        .ok_or_else(|| {
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
        version::DwgVersion::R2010 | version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2010 | version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2010 | version::DwgVersion::R2013
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
        version::DwgVersion::R2013 => {
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
        version::DwgVersion::R2013 => {
            entities::common::parse_common_entity_header_r2013(&mut reader, object_data_end_bit)
                .ok()?
        }
        _ => return None,
    };
    reader.set_bit_pos(header.obj_size);
    entities::common::parse_common_entity_layer_handle(&mut reader, &header).ok()
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

fn decode_layer_color_record(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    expected_handle: u64,
) -> crate::core::result::Result<(u64, u16, Option<u32>)> {
    // R2010+/R2013 objects start with handle directly after OT prefix.
    // Older versions keep ObjSize (RL) before handle.
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013
    ) {
        let _obj_size = reader.read_rl(Endian::Little)?;
    }
    let record_handle = reader.read_h()?.value;
    skip_eed(reader)?;

    let _num_reactors = reader.read_bl()?;
    let _xdic_missing_flag = reader.read_b()?;
    if matches!(version, version::DwgVersion::R2013) {
        let _has_ds_binary_data = reader.read_b()?;
    }
    // R2010+ stores entry name in string stream. The data stream directly
    // continues with layer state flags and color data.
    if !matches!(
        version,
        version::DwgVersion::R2010 | version::DwgVersion::R2013
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
