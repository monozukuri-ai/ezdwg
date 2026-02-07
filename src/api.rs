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

#[pyfunction]
pub fn hello_from_bin() -> String {
    "Hello from ezdwg!".to_string()
}

#[pyfunction]
pub fn detect_version(path: &str) -> PyResult<String> {
    let tag = file_open::read_version_tag(path).map_err(to_py_err)?;
    let version = version::detect_version(&tag).map_err(to_py_err)?;
    Ok(version.as_str().to_string())
}

#[pyfunction]
pub fn list_section_locators(path: &str) -> PyResult<Vec<(String, u32, u32)>> {
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
pub fn list_object_map_entries(path: &str, limit: Option<usize>) -> PyResult<Vec<(u64, u32)>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut entries: Vec<(u64, u32)> = index
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
pub fn list_object_headers(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<(u64, u32, u32, u16)>> {
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
) -> PyResult<Vec<(u64, u32, u32, u16, String, String)>> {
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
) -> PyResult<Vec<(u64, u32, u32, u16, String, String)>> {
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
) -> PyResult<Vec<(u64, u32, u32, u16, Vec<u8>)>> {
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
pub fn decode_entity_styles(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<(u64, Option<u16>, Option<u32>, u64)>> {
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

        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        if matches_type_name(header.type_code, 0x13, "LINE", &dynamic_types) {
            let entity = match decode_line_for_version(&mut reader, decoder.version(), &header) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                entity.layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x1B, "POINT", &dynamic_types) {
            let entity = match entities::decode_point(&mut reader) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                entity.layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x11, "ARC", &dynamic_types) {
            let entity = match decode_arc_for_version(&mut reader, decoder.version(), &header) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                entity.layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x12, "CIRCLE", &dynamic_types) {
            let entity = match entities::decode_circle(&mut reader) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                entity.layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x23, "ELLIPSE", &dynamic_types) {
            let entity = match entities::decode_ellipse(&mut reader) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                entity.layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x01, "TEXT", &dynamic_types) {
            let entity = match entities::decode_text(&mut reader) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                entity.layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x2C, "MTEXT", &dynamic_types) {
            let entity = match entities::decode_mtext(&mut reader) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                entity.layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x4D, "LWPOLYLINE", &dynamic_types) {
            let entity =
                match decode_lwpolyline_for_version(&mut reader, decoder.version(), &header) {
                    Ok(entity) => entity,
                    Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                    Err(err) => return Err(to_py_err(err)),
                };
            result.push((
                entity.handle,
                entity.color_index,
                entity.true_color,
                entity.layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x15, "DIM_LINEAR", &dynamic_types) {
            let entity = match entities::decode_dim_linear(&mut reader) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                common.layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x1A, "DIM_DIAMETER", &dynamic_types) {
            let entity = match entities::decode_dim_diameter(&mut reader) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                common.layer_handle,
            ));
        } else if matches_type_name(header.type_code, 0x19, "DIM_RADIUS", &dynamic_types) {
            let entity = match entities::decode_dim_radius(&mut reader) {
                Ok(entity) => entity,
                Err(err) if best_effort || is_recoverable_decode_error(&err) => continue,
                Err(err) => return Err(to_py_err(err)),
            };
            let common = &entity.common;
            result.push((
                common.handle,
                common.color_index,
                common.true_color,
                common.layer_handle,
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
pub fn decode_layer_colors(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<(u64, u16, Option<u32>)>> {
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
        let (handle, color_index, true_color) = match decode_layer_color_record(&mut reader) {
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
pub fn decode_line_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<(u64, f64, f64, f64, f64, f64, f64)>> {
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
        let entity = match decode_line_for_version(&mut reader, decoder.version(), &header) {
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
pub fn decode_point_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<(u64, f64, f64, f64, f64)>> {
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
        let entity = match entities::decode_point(&mut reader) {
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
pub fn decode_arc_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<(u64, f64, f64, f64, f64, f64, f64)>> {
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
        let entity = match decode_arc_for_version(&mut reader, decoder.version(), &header) {
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
pub fn decode_circle_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<(u64, f64, f64, f64, f64)>> {
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
        let entity = match entities::decode_circle(&mut reader) {
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
) -> PyResult<
    Vec<(
        u64,
        (f64, f64, f64),
        (f64, f64, f64),
        (f64, f64, f64),
        f64,
        f64,
        f64,
    )>,
> {
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
        let entity = match entities::decode_ellipse(&mut reader) {
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
pub fn decode_text_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<
    Vec<(
        u64,
        String,
        (f64, f64, f64),
        Option<(f64, f64, f64)>,
        (f64, f64, f64),
        (f64, f64, f64, f64, f64),
        (u16, u16, u16),
        Option<u64>,
    )>,
> {
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
        let entity = match entities::decode_text(&mut reader) {
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
pub fn decode_mtext_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<
    Vec<(
        u64,
        String,
        (f64, f64, f64),
        (f64, f64, f64),
        (f64, f64, f64),
        f64,
        f64,
        u16,
        u16,
    )>,
> {
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
        let entity = match entities::decode_mtext(&mut reader) {
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
pub fn decode_dim_linear_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<
    Vec<(
        u64,
        String,
        (f64, f64, f64),
        (f64, f64, f64),
        (f64, f64, f64),
        (f64, f64, f64),
        Option<(f64, f64, f64)>,
        ((f64, f64, f64), (f64, f64, f64)),
        (f64, f64, f64, f64),
        (u8, Option<f64>, Option<u16>, Option<u16>, Option<f64>, f64),
        (Option<u64>, Option<u64>),
    )>,
> {
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
        if !matches_type_name(header.type_code, 0x15, "DIM_LINEAR", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match entities::decode_dim_linear(&mut reader) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let common = &entity.common;
        result.push((
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
pub fn decode_dim_diameter_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<
    Vec<(
        u64,
        String,
        (f64, f64, f64),
        (f64, f64, f64),
        (f64, f64, f64),
        (f64, f64, f64),
        Option<(f64, f64, f64)>,
        ((f64, f64, f64), (f64, f64, f64)),
        (f64, f64, f64, f64),
        (u8, Option<f64>, Option<u16>, Option<u16>, Option<f64>, f64),
        (Option<u64>, Option<u64>),
    )>,
> {
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
        if !matches_type_name(header.type_code, 0x1A, "DIM_DIAMETER", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match entities::decode_dim_diameter(&mut reader) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let common = &entity.common;
        result.push((
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
pub fn decode_dim_radius_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<
    Vec<(
        u64,
        String,
        (f64, f64, f64),
        (f64, f64, f64),
        (f64, f64, f64),
        (f64, f64, f64),
        Option<(f64, f64, f64)>,
        ((f64, f64, f64), (f64, f64, f64)),
        (f64, f64, f64, f64),
        (u8, Option<f64>, Option<u16>, Option<u16>, Option<f64>, f64),
        (Option<u64>, Option<u64>),
    )>,
> {
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
        if !matches_type_name(header.type_code, 0x19, "DIM_RADIUS", &dynamic_types) {
            continue;
        }
        let mut reader = record.bit_reader();
        if let Err(err) = skip_object_type_prefix(&mut reader, decoder.version()) {
            if best_effort {
                continue;
            }
            return Err(to_py_err(err));
        }
        let entity = match entities::decode_dim_radius(&mut reader) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        let common = &entity.common;
        result.push((
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
pub fn decode_insert_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<(u64, f64, f64, f64, f64, f64, f64, f64)>> {
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
pub fn decode_polyline_2d_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<(u64, u16, u16, f64, f64, f64, f64)>> {
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
) -> PyResult<
    Vec<(
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
    )>,
> {
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
) -> PyResult<Vec<(u64, u16, Vec<(f64, f64)>)>> {
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
        let entity = match decode_lwpolyline_for_version(&mut reader, decoder.version(), &header) {
            Ok(entity) => entity,
            Err(err) if best_effort => continue,
            Err(err) => return Err(to_py_err(err)),
        };
        result.push((entity.handle, entity.flags, entity.vertices));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }
    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_polyline_2d_with_vertices(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<(u64, u16, Vec<(f64, f64, f64)>)>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut sorted = index.objects.clone();
    sorted.sort_by_key(|obj| obj.offset);

    let mut vertex_map = std::collections::HashMap::new();
    for obj in sorted.iter() {
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
        vertex_map.insert(vertex.handle, vertex);
    }

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
        let mut vertices: Vec<(f64, f64, f64)> = Vec::new();
        let use_vertex_z = poly.flags_info.is_3d_polyline
            || poly.flags_info.is_3d_mesh
            || poly.flags_info.is_polyface_mesh;

        if !poly.owned_handles.is_empty() {
            for handle in &poly.owned_handles {
                if let Some(vertex) = vertex_map.get(handle) {
                    let z = if use_vertex_z {
                        vertex.position.2
                    } else {
                        poly.elevation
                    };
                    vertices.push((vertex.position.0, vertex.position.1, z));
                }
            }
            i += 1;
        } else {
            let mut j = i + 1;
            while j < sorted.len() {
                let next = sorted[j];
                let next_record = decoder
                    .parse_object_record(next.offset)
                    .map_err(to_py_err)?;
                let next_header = parse_object_header_for_version(&next_record, decoder.version())
                    .map_err(to_py_err)?;
                let mut next_reader = next_record.bit_reader();
                if matches_type_name(next_header.type_code, 0x0A, "VERTEX_2D", &dynamic_types) {
                    let _next_type = skip_object_type_prefix(&mut next_reader, decoder.version())
                        .map_err(to_py_err)?;
                    let vertex = entities::decode_vertex_2d(&mut next_reader).map_err(to_py_err)?;
                    let z = if use_vertex_z {
                        vertex.position.2
                    } else {
                        poly.elevation
                    };
                    vertices.push((vertex.position.0, vertex.position.1, z));
                    j += 1;
                    continue;
                }
                if matches_type_name(next_header.type_code, 0x06, "SEQEND", &dynamic_types) {
                    let _next_type = skip_object_type_prefix(&mut next_reader, decoder.version())
                        .map_err(to_py_err)?;
                    let _seqend = entities::decode_seqend(&mut next_reader).map_err(to_py_err)?;
                    j += 1;
                }
                break;
            }
            i = j;
        }

        if poly.flags_info.closed && vertices.len() > 1 {
            let first = vertices[0];
            let last = *vertices.last().unwrap();
            if !points_equal_3d(first, last) {
                vertices.push(first);
            }
        }

        result.push((poly.handle, poly.flags, vertices));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

#[pyfunction(signature = (path, segments_per_span=8, limit=None))]
pub fn decode_polyline_2d_with_vertices_interpolated(
    path: &str,
    segments_per_span: usize,
    limit: Option<usize>,
) -> PyResult<Vec<(u64, u16, bool, Vec<(f64, f64, f64)>)>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut sorted = index.objects.clone();
    sorted.sort_by_key(|obj| obj.offset);

    let mut vertex_map = std::collections::HashMap::new();
    for obj in sorted.iter() {
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
        vertex_map.insert(vertex.handle, vertex);
    }

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
        let mut vertices: Vec<(f64, f64, f64)> = Vec::new();
        let use_vertex_z = poly.flags_info.is_3d_polyline
            || poly.flags_info.is_3d_mesh
            || poly.flags_info.is_polyface_mesh;

        if !poly.owned_handles.is_empty() {
            for handle in &poly.owned_handles {
                if let Some(vertex) = vertex_map.get(handle) {
                    let z = if use_vertex_z {
                        vertex.position.2
                    } else {
                        poly.elevation
                    };
                    vertices.push((vertex.position.0, vertex.position.1, z));
                }
            }
            i += 1;
        } else {
            let mut j = i + 1;
            while j < sorted.len() {
                let next = sorted[j];
                let next_record = decoder
                    .parse_object_record(next.offset)
                    .map_err(to_py_err)?;
                let next_header = parse_object_header_for_version(&next_record, decoder.version())
                    .map_err(to_py_err)?;
                let mut next_reader = next_record.bit_reader();
                if matches_type_name(next_header.type_code, 0x0A, "VERTEX_2D", &dynamic_types) {
                    let _next_type = skip_object_type_prefix(&mut next_reader, decoder.version())
                        .map_err(to_py_err)?;
                    let vertex = entities::decode_vertex_2d(&mut next_reader).map_err(to_py_err)?;
                    let z = if use_vertex_z {
                        vertex.position.2
                    } else {
                        poly.elevation
                    };
                    vertices.push((vertex.position.0, vertex.position.1, z));
                    j += 1;
                    continue;
                }
                if matches_type_name(next_header.type_code, 0x06, "SEQEND", &dynamic_types) {
                    let _next_type = skip_object_type_prefix(&mut next_reader, decoder.version())
                        .map_err(to_py_err)?;
                    let _seqend = entities::decode_seqend(&mut next_reader).map_err(to_py_err)?;
                    j += 1;
                }
                break;
            }
            i = j;
        }

        let mut applied = false;
        let should_interpolate = poly.flags_info.curve_fit
            || poly.flags_info.spline_fit
            || matches!(
                poly.curve_type_info,
                entities::PolylineCurveType::QuadraticBSpline
                    | entities::PolylineCurveType::CubicBSpline
                    | entities::PolylineCurveType::Bezier
            );

        if should_interpolate && vertices.len() > 1 {
            let base = strip_closure(vertices);
            let interpolated =
                entities::catmull_rom_spline(&base, poly.flags_info.closed, segments_per_span)
                    .map_err(to_py_err)?;
            vertices = interpolated;
            applied = true;
        } else if poly.flags_info.closed && vertices.len() > 1 {
            let first = vertices[0];
            let last = *vertices.last().unwrap();
            if !points_equal_3d(first, last) {
                vertices.push(first);
            }
        }

        result.push((poly.handle, poly.flags, applied, vertices));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

#[pyfunction(signature = (path, limit=None))]
pub fn decode_vertex_2d_entities(
    path: &str,
    limit: Option<usize>,
) -> PyResult<Vec<(u64, u16, f64, f64, f64, f64, f64, f64, f64)>> {
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
) -> PyResult<Vec<(u64, u16, Vec<(f64, f64, f64, f64, f64, f64, f64, u16)>)>> {
    let bytes = file_open::read_file(path).map_err(to_py_err)?;
    let decoder = build_decoder(&bytes).map_err(to_py_err)?;
    let dynamic_types = decoder.dynamic_type_map().map_err(to_py_err)?;
    let index = decoder.build_object_index().map_err(to_py_err)?;
    let mut sorted = index.objects.clone();
    sorted.sort_by_key(|obj| obj.offset);

    let mut vertex_map = std::collections::HashMap::new();
    for obj in sorted.iter() {
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
        vertex_map.insert(vertex.handle, vertex);
    }

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
        let mut vertices: Vec<(f64, f64, f64, f64, f64, f64, f64, u16)> = Vec::new();
        let use_vertex_z = poly.flags_info.is_3d_polyline
            || poly.flags_info.is_3d_mesh
            || poly.flags_info.is_polyface_mesh;

        if !poly.owned_handles.is_empty() {
            for handle in &poly.owned_handles {
                if let Some(vertex) = vertex_map.get(handle) {
                    let z = if use_vertex_z {
                        vertex.position.2
                    } else {
                        poly.elevation
                    };
                    vertices.push((
                        vertex.position.0,
                        vertex.position.1,
                        z,
                        vertex.start_width,
                        vertex.end_width,
                        vertex.bulge,
                        vertex.tangent_dir,
                        vertex.flags,
                    ));
                }
            }
            i += 1;
        } else {
            let mut j = i + 1;
            while j < sorted.len() {
                let next = sorted[j];
                let next_record = decoder
                    .parse_object_record(next.offset)
                    .map_err(to_py_err)?;
                let next_header = parse_object_header_for_version(&next_record, decoder.version())
                    .map_err(to_py_err)?;
                let mut next_reader = next_record.bit_reader();
                if matches_type_name(next_header.type_code, 0x0A, "VERTEX_2D", &dynamic_types) {
                    let _next_type = skip_object_type_prefix(&mut next_reader, decoder.version())
                        .map_err(to_py_err)?;
                    let vertex = entities::decode_vertex_2d(&mut next_reader).map_err(to_py_err)?;
                    let z = if use_vertex_z {
                        vertex.position.2
                    } else {
                        poly.elevation
                    };
                    vertices.push((
                        vertex.position.0,
                        vertex.position.1,
                        z,
                        vertex.start_width,
                        vertex.end_width,
                        vertex.bulge,
                        vertex.tangent_dir,
                        vertex.flags,
                    ));
                    j += 1;
                    continue;
                }
                if matches_type_name(next_header.type_code, 0x06, "SEQEND", &dynamic_types) {
                    let _next_type = skip_object_type_prefix(&mut next_reader, decoder.version())
                        .map_err(to_py_err)?;
                    let _seqend = entities::decode_seqend(&mut next_reader).map_err(to_py_err)?;
                    j += 1;
                }
                break;
            }
            i = j;
        }

        if poly.flags_info.closed && vertices.len() > 1 {
            let first = vertices[0];
            let last = *vertices.last().unwrap();
            if !points_equal_3d_with_data(first, last) {
                vertices.push(first);
            }
        }

        result.push((poly.handle, poly.flags, vertices));
        if let Some(limit) = limit {
            if result.len() >= limit {
                break;
            }
        }
    }

    Ok(result)
}

pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(hello_from_bin, module)?)?;
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
    module.add_function(wrap_pyfunction!(decode_text_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_mtext_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_linear_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_diameter_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_dim_radius_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_insert_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_2d_entities, module)?)?;
    module.add_function(wrap_pyfunction!(
        decode_polyline_2d_entities_interpreted,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(decode_lwpolyline_entities, module)?)?;
    module.add_function(wrap_pyfunction!(decode_polyline_2d_with_vertices, module)?)?;
    module.add_function(wrap_pyfunction!(
        decode_polyline_2d_with_vertices_interpolated,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(decode_vertex_2d_entities, module)?)?;
    module.add_function(wrap_pyfunction!(
        decode_polyline_2d_with_vertex_data,
        module
    )?)?;
    Ok(())
}

fn is_best_effort_compat_version(decoder: &decoder::Decoder<'_>) -> bool {
    matches!(
        decoder.version(),
        version::DwgVersion::R2010 | version::DwgVersion::R2013
    )
}

fn decode_line_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
) -> crate::core::result::Result<entities::LineEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_line_r2010(reader, object_data_end_bit)
        }
        version::DwgVersion::R2013 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_line_r2013(reader, object_data_end_bit)
        }
        version::DwgVersion::R2007 => entities::decode_line_r2007(reader),
        _ => entities::decode_line(reader),
    }
}

fn decode_arc_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
) -> crate::core::result::Result<entities::ArcEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_arc_r2010(reader, object_data_end_bit)
        }
        version::DwgVersion::R2013 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_arc_r2013(reader, object_data_end_bit)
        }
        version::DwgVersion::R2007 => entities::decode_arc_r2007(reader),
        _ => entities::decode_arc(reader),
    }
}

fn decode_lwpolyline_for_version(
    reader: &mut BitReader<'_>,
    version: &version::DwgVersion,
    header: &ApiObjectHeader,
) -> crate::core::result::Result<entities::LwPolylineEntity> {
    match version {
        version::DwgVersion::R2010 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_lwpolyline_r2010(reader, object_data_end_bit)
        }
        version::DwgVersion::R2013 => {
            let object_data_end_bit = resolve_r2010_object_data_end_bit(header)?;
            entities::decode_lwpolyline_r2013(reader, object_data_end_bit)
        }
        version::DwgVersion::R2007 => entities::decode_lwpolyline_r2007(reader),
        _ => entities::decode_lwpolyline(reader),
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

fn decode_layer_color_record(
    reader: &mut BitReader<'_>,
) -> crate::core::result::Result<(u64, u16, Option<u32>)> {
    let _obj_size = reader.read_rl(Endian::Little)?;
    let handle = reader.read_h()?.value;
    skip_eed(reader)?;

    let _num_reactors = reader.read_bl()?;
    let _xdic_missing_flag = reader.read_b()?;
    let _entry_name = reader.read_tv()?;

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
        return Ok((handle, color_index, true_color));
    }

    // Last resort: parse in the simplest form to keep progress.
    reader.set_pos(style_start.0, style_start.1);
    let (color_index, true_color, _) = decode_layer_color_cmc(reader, variants[0])?;
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

fn _unused_true_color_example(color_rgb: u32) -> Option<u32> {
    if color_rgb == 0 {
        None
    } else {
        Some(color_rgb)
    }
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
