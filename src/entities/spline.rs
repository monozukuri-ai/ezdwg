use crate::core::result::Result;

type Point3 = (f64, f64, f64);
type Knots = (f64, f64, f64, f64);

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
