use crate::bit::BitReader;
use crate::core::error::{DwgError, ErrorKind};
use crate::core::result::Result;
use crate::entities::common::{
    parse_common_entity_handles, parse_common_entity_header, parse_common_entity_header_r2007,
    parse_common_entity_header_r2010, parse_common_entity_header_r2013,
    parse_common_entity_layer_handle, CommonEntityHeader,
};

type Point3 = (f64, f64, f64);
type Knots = (f64, f64, f64, f64);

#[derive(Debug, Clone)]
pub struct SplineEntity {
    pub handle: u64,
    pub color_index: Option<u16>,
    pub true_color: Option<u32>,
    pub layer_handle: u64,
    pub scenario: u32,
    pub spline_flags1: Option<u32>,
    pub knot_parameter: Option<u32>,
    pub degree: u32,
    pub rational: bool,
    pub closed: bool,
    pub periodic: bool,
    pub fit_tolerance: Option<f64>,
    pub knot_tolerance: Option<f64>,
    pub ctrl_tolerance: Option<f64>,
    pub start_tangent: Option<Point3>,
    pub end_tangent: Option<Point3>,
    pub knots: Vec<f64>,
    pub control_points: Vec<Point3>,
    pub weights: Vec<f64>,
    pub fit_points: Vec<Point3>,
}

#[derive(Debug, Clone)]
struct ParsedSplineData {
    scenario: u32,
    spline_flags1: Option<u32>,
    knot_parameter: Option<u32>,
    degree: u32,
    rational: bool,
    closed: bool,
    periodic: bool,
    fit_tolerance: Option<f64>,
    knot_tolerance: Option<f64>,
    ctrl_tolerance: Option<f64>,
    start_tangent: Option<Point3>,
    end_tangent: Option<Point3>,
    knots: Vec<f64>,
    control_points: Vec<Point3>,
    weights: Vec<f64>,
    fit_points: Vec<Point3>,
}

#[derive(Clone, Copy)]
enum SplineMode {
    Control,
    Fit,
}

pub fn decode_spline(reader: &mut BitReader<'_>) -> Result<SplineEntity> {
    let header = parse_common_entity_header(reader)?;
    decode_spline_with_header(reader, header, false, false, false)
}

pub fn decode_spline_r2007(reader: &mut BitReader<'_>) -> Result<SplineEntity> {
    let header = parse_common_entity_header_r2007(reader)?;
    decode_spline_with_header(reader, header, true, true, false)
}

pub fn decode_spline_r2010(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<SplineEntity> {
    let mut header = parse_common_entity_header_r2010(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_spline_with_header(reader, header, true, true, false)
}

pub fn decode_spline_r2013(
    reader: &mut BitReader<'_>,
    object_data_end_bit: u32,
    object_handle: u64,
) -> Result<SplineEntity> {
    let mut header = parse_common_entity_header_r2013(reader, object_data_end_bit)?;
    header.handle = object_handle;
    decode_spline_with_header(reader, header, true, true, true)
}

fn decode_spline_with_header(
    reader: &mut BitReader<'_>,
    header: CommonEntityHeader,
    allow_handle_decode_failure: bool,
    r2007_layer_only: bool,
    is_r2013_plus: bool,
) -> Result<SplineEntity> {
    let data_start = reader.get_pos();
    let scenario = reader.read_bl()?;
    let spline_flags1 = if is_r2013_plus {
        Some(reader.read_bl()?)
    } else {
        None
    };
    let knot_parameter = if is_r2013_plus {
        Some(reader.read_bl()?)
    } else {
        None
    };
    let degree = reader.read_bl()?;

    let prefer_mode = if scenario == 2 {
        SplineMode::Fit
    } else {
        SplineMode::Control
    };
    let modes = match prefer_mode {
        SplineMode::Control => [SplineMode::Control, SplineMode::Fit],
        SplineMode::Fit => [SplineMode::Fit, SplineMode::Control],
    };
    let parse_start = reader.get_pos();
    let mut parsed: Option<ParsedSplineData> = None;
    let mut last_error: Option<DwgError> = None;
    for mode in modes {
        reader.set_pos(parse_start.0, parse_start.1);
        match parse_spline_data(
            reader,
            mode,
            scenario,
            spline_flags1,
            knot_parameter,
            degree,
        ) {
            Ok(data) => {
                parsed = Some(data);
                break;
            }
            Err(err)
                if matches!(
                    err.kind,
                    ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                ) =>
            {
                last_error = Some(err);
            }
            Err(err) => return Err(err),
        }
    }

    let Some(data) = parsed else {
        reader.set_pos(data_start.0, data_start.1);
        return Err(last_error
            .unwrap_or_else(|| DwgError::new(ErrorKind::Decode, "failed to decode SPLINE")));
    };

    // Handles are stored in the handle stream at obj_size bit offset.
    reader.set_bit_pos(header.obj_size);
    let layer_handle = match if r2007_layer_only {
        parse_common_entity_layer_handle(reader, &header)
    } else {
        parse_common_entity_handles(reader, &header).map(|common_handles| common_handles.layer)
    } {
        Ok(layer_handle) => layer_handle,
        Err(err)
            if allow_handle_decode_failure
                && matches!(
                    err.kind,
                    ErrorKind::Format | ErrorKind::Decode | ErrorKind::Io
                ) =>
        {
            0
        }
        Err(err) => return Err(err),
    };

    Ok(SplineEntity {
        handle: header.handle,
        color_index: header.color.index,
        true_color: header.color.true_color,
        layer_handle,
        scenario: data.scenario,
        spline_flags1: data.spline_flags1,
        knot_parameter: data.knot_parameter,
        degree: data.degree,
        rational: data.rational,
        closed: data.closed,
        periodic: data.periodic,
        fit_tolerance: data.fit_tolerance,
        knot_tolerance: data.knot_tolerance,
        ctrl_tolerance: data.ctrl_tolerance,
        start_tangent: data.start_tangent,
        end_tangent: data.end_tangent,
        knots: data.knots,
        control_points: data.control_points,
        weights: data.weights,
        fit_points: data.fit_points,
    })
}

fn parse_spline_data(
    reader: &mut BitReader<'_>,
    mode: SplineMode,
    scenario: u32,
    spline_flags1: Option<u32>,
    knot_parameter: Option<u32>,
    degree: u32,
) -> Result<ParsedSplineData> {
    match mode {
        SplineMode::Control => {
            parse_spline_control_data(reader, scenario, spline_flags1, knot_parameter, degree)
        }
        SplineMode::Fit => {
            parse_spline_fit_data(reader, scenario, spline_flags1, knot_parameter, degree)
        }
    }
}

fn parse_spline_control_data(
    reader: &mut BitReader<'_>,
    scenario: u32,
    spline_flags1: Option<u32>,
    knot_parameter: Option<u32>,
    degree: u32,
) -> Result<ParsedSplineData> {
    let rational = reader.read_b()? != 0;
    let closed = reader.read_b()? != 0;
    let periodic = reader.read_b()? != 0;
    let knot_tolerance = Some(reader.read_bd()?);
    let ctrl_tolerance = Some(reader.read_bd()?);
    let num_knots = bounded_count(reader.read_bl()?, "spline knots")?;
    let num_ctrl = bounded_count(reader.read_bl()?, "spline control points")?;
    let _weight_echo = reader.read_b()?;

    let mut knots = Vec::with_capacity(num_knots);
    for _ in 0..num_knots {
        knots.push(reader.read_bd()?);
    }

    let mut control_points = Vec::with_capacity(num_ctrl);
    let mut weights = if rational {
        Vec::with_capacity(num_ctrl)
    } else {
        Vec::new()
    };
    for _ in 0..num_ctrl {
        control_points.push(reader.read_3bd()?);
        if rational {
            weights.push(reader.read_bd()?);
        }
    }

    Ok(ParsedSplineData {
        scenario,
        spline_flags1,
        knot_parameter,
        degree,
        rational,
        closed,
        periodic,
        fit_tolerance: None,
        knot_tolerance,
        ctrl_tolerance,
        start_tangent: None,
        end_tangent: None,
        knots,
        control_points,
        weights,
        fit_points: Vec::new(),
    })
}

fn parse_spline_fit_data(
    reader: &mut BitReader<'_>,
    scenario: u32,
    spline_flags1: Option<u32>,
    knot_parameter: Option<u32>,
    degree: u32,
) -> Result<ParsedSplineData> {
    let fit_tolerance = Some(reader.read_bd()?);
    let start_tangent = Some(reader.read_3bd()?);
    let end_tangent = Some(reader.read_3bd()?);
    let num_fit = bounded_count(reader.read_bl()?, "spline fit points")?;

    let mut fit_points = Vec::with_capacity(num_fit);
    for _ in 0..num_fit {
        fit_points.push(reader.read_3bd()?);
    }

    Ok(ParsedSplineData {
        scenario,
        spline_flags1,
        knot_parameter,
        degree,
        rational: false,
        closed: false,
        periodic: false,
        fit_tolerance,
        knot_tolerance: None,
        ctrl_tolerance: None,
        start_tangent,
        end_tangent,
        knots: Vec::new(),
        control_points: Vec::new(),
        weights: Vec::new(),
        fit_points,
    })
}

fn bounded_count(raw: u32, label: &str) -> Result<usize> {
    let count = raw as usize;
    if count > 1_000_000 {
        return Err(DwgError::new(
            ErrorKind::Format,
            format!("{} count is too large: {}", label, count),
        ));
    }
    Ok(count)
}

pub fn catmull_rom_spline(
    points: &[Point3],
    closed: bool,
    segments_per_span: usize,
) -> Result<Vec<Point3>> {
    if points.len() < 2 {
        return Ok(points.to_vec());
    }

    let segments = segments_per_span.max(1);
    let alpha = 0.5_f64; // centripetal

    let mut out = Vec::new();
    let n = points.len();
    let segment_count = if closed { n } else { n - 1 };

    for i in 0..segment_count {
        let p0 = if closed {
            points[(i + n - 1) % n]
        } else if i == 0 {
            points[0]
        } else {
            points[i - 1]
        };
        let p1 = points[i % n];
        let p2 = points[(i + 1) % n];
        let p3 = if closed {
            points[(i + 2) % n]
        } else if i + 2 < n {
            points[i + 2]
        } else {
            points[n - 1]
        };

        let t0 = 0.0;
        let t1 = tj(t0, p0, p1, alpha);
        let t2 = tj(t1, p1, p2, alpha);
        let t3 = tj(t2, p2, p3, alpha);

        let steps = segments;
        for s in 0..=steps {
            if i > 0 && s == 0 {
                continue; // avoid duplicate points at segment boundaries
            }
            let u = s as f64 / steps as f64;
            let t = t1 + (t2 - t1) * u;
            let pt = catmull_rom_point([p0, p1, p2, p3], (t0, t1, t2, t3), t);
            out.push(pt);
        }
    }

    if closed && !out.is_empty() {
        let first = out[0];
        let last = *out.last().unwrap();
        if !points_equal(first, last) {
            out.push(first);
        }
    }

    Ok(out)
}

fn tj(ti: f64, p0: Point3, p1: Point3, alpha: f64) -> f64 {
    let dist = distance(p0, p1);
    ti + dist.powf(alpha)
}

fn distance(a: Point3, b: Point3) -> f64 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    let dz = a.2 - b.2;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn catmull_rom_point(points: [Point3; 4], knots: Knots, t: f64) -> Point3 {
    let [p0, p1, p2, p3] = points;
    let (t0, t1, t2, t3) = knots;
    let a1 = lerp(p0, p1, t0, t1, t);
    let a2 = lerp(p1, p2, t1, t2, t);
    let a3 = lerp(p2, p3, t2, t3, t);

    let b1 = lerp(a1, a2, t0, t2, t);
    let b2 = lerp(a2, a3, t1, t3, t);

    lerp(b1, b2, t1, t2, t)
}

fn lerp(p0: Point3, p1: Point3, t0: f64, t1: f64, t: f64) -> Point3 {
    if (t1 - t0).abs() < 1e-12 {
        return p0;
    }
    let w0 = (t1 - t) / (t1 - t0);
    let w1 = (t - t0) / (t1 - t0);
    (
        w0 * p0.0 + w1 * p1.0,
        w0 * p0.1 + w1 * p1.1,
        w0 * p0.2 + w1 * p1.2,
    )
}

fn points_equal(a: Point3, b: Point3) -> bool {
    const EPS: f64 = 1e-9;
    (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() < EPS && (a.2 - b.2).abs() < EPS
}
