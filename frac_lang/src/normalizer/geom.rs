/// 2-D point as a plain array for arithmetic convenience.
pub type Point2 = [f64; 2];

const EPS: f64 = 1e-9;

// ── Basic geometry ────────────────────────────────────────────────────────────

pub fn add(a: Point2, b: Point2) -> Point2 { [a[0]+b[0], a[1]+b[1]] }
pub fn sub(a: Point2, b: Point2) -> Point2 { [a[0]-b[0], a[1]-b[1]] }
pub fn scale(a: Point2, t: f64) -> Point2  { [a[0]*t, a[1]*t] }
pub fn lerp(a: Point2, b: Point2, t: f64) -> Point2 { add(a, scale(sub(b, a), t)) }
pub fn dot(a: Point2, b: Point2) -> f64 { a[0]*b[0] + a[1]*b[1] }
pub fn cross(a: Point2, b: Point2) -> f64 { a[0]*b[1] - a[1]*b[0] }
pub fn dist(a: Point2, b: Point2) -> f64 { let d = sub(b, a); (d[0]*d[0]+d[1]*d[1]).sqrt() }
pub fn dist_sq(a: Point2, b: Point2) -> f64 { let d = sub(b, a); d[0]*d[0]+d[1]*d[1] }
pub fn angle(v: Point2) -> f64 { v[1].atan2(v[0]) }

/// Rotate a point by `theta` radians about the origin.
pub fn rotate(p: Point2, theta: f64) -> Point2 {
    let (c, s) = (theta.cos(), theta.sin());
    [c*p[0] - s*p[1], s*p[0] + c*p[1]]
}

/// Reflect a point about the line through the origin at angle `axis_angle`.
pub fn reflect(p: Point2, axis_angle: f64) -> Point2 {
    let (c, s) = ((2.0*axis_angle).cos(), (2.0*axis_angle).sin());
    [c*p[0] + s*p[1], s*p[0] - c*p[1]]
}

/// Centroid of a point set.
pub fn centroid(pts: &[Point2]) -> Point2 {
    let n = pts.len() as f64;
    let sx: f64 = pts.iter().map(|p| p[0]).sum();
    let sy: f64 = pts.iter().map(|p| p[1]).sum();
    [sx/n, sy/n]
}

/// Translate all points by -centroid(pts).
pub fn center_polygon(pts: &[Point2]) -> Vec<Point2> {
    let c = centroid(pts);
    pts.iter().map(|p| sub(*p, c)).collect()
}

// ── Polygon area / winding ────────────────────────────────────────────────────

/// Signed area (positive = CCW).
pub fn signed_area(poly: &[Point2]) -> f64 {
    let n = poly.len();
    let mut a = 0.0;
    for i in 0..n {
        a += cross(poly[i], poly[(i+1)%n]);
    }
    a * 0.5
}

pub fn area(poly: &[Point2]) -> f64 { signed_area(poly).abs() }
pub fn is_ccw(poly: &[Point2]) -> bool { signed_area(poly) > 0.0 }

/// Reverse vertex order to flip winding.
pub fn ensure_ccw(mut poly: Vec<Point2>) -> Vec<Point2> {
    if !is_ccw(&poly) { poly.reverse(); }
    poly
}

// ── Segment intersection ──────────────────────────────────────────────────────

/// Parametric intersection of open segments AB and CD.
/// Returns (t, u) ∈ (0,1)² for a proper (interior) crossing, else None.
pub fn segment_intersection(a: Point2, b: Point2, c: Point2, d: Point2) -> Option<(f64, f64)> {
    let ab = sub(b, a);
    let cd = sub(d, c);
    let denom = cross(ab, cd);
    if denom.abs() < EPS { return None; }
    let ac = sub(c, a);
    let t = cross(ac, cd) / denom;
    let u = cross(ac, ab) / denom;
    if t > EPS && t < 1.0-EPS && u > EPS && u < 1.0-EPS {
        Some((t, u))
    } else {
        None
    }
}

/// True if point P lies on segment AB (within EPS).
pub fn point_on_segment(p: Point2, a: Point2, b: Point2) -> bool {
    let ab = sub(b, a);
    let ap = sub(p, a);
    let cross_val = cross(ab, ap);
    if cross_val.abs() > EPS * (dist_sq(a, b).sqrt() + 1.0) { return false; }
    let t = if ab[0].abs() > ab[1].abs() { ap[0] / ab[0] } else { ap[1] / ab[1] };
    t >= -EPS && t <= 1.0+EPS
}

// ── Affine invariants (side-length ratios) ────────────────────────────────────

/// Side-length ratio invariants for a polygon: edge_i / perimeter for each i.
/// Invariant under similarity transforms (scale, rotation, reflection).
pub fn side_length_ratios(poly: &[Point2]) -> Vec<f64> {
    let n = poly.len();
    let lengths: Vec<f64> = (0..n).map(|i| dist(poly[i], poly[(i+1)%n])).collect();
    let perimeter: f64 = lengths.iter().sum();
    if perimeter < EPS { return vec![1.0 / n as f64; n]; }
    lengths.iter().map(|l| l / perimeter).collect()
}

/// Check if two ratio sequences are equal up to cyclic rotation and/or reflection.
pub fn invariants_match(a: &[f64], b: &[f64]) -> bool {
    let n = a.len();
    if n != b.len() { return false; }
    const TOL: f64 = 1e-5;
    // Try all cyclic rotations
    'rot: for r in 0..n {
        for i in 0..n {
            if (a[i] - b[(i+r)%n]).abs() > TOL { continue 'rot; }
        }
        return true;
    }
    // Try rotations of reversed b (handles reflections)
    let b_rev: Vec<f64> = b.iter().copied().rev().collect();
    'rev: for r in 0..n {
        for i in 0..n {
            if (a[i] - b_rev[(i+r)%n]).abs() > TOL { continue 'rev; }
        }
        return true;
    }
    false
}

// ── Similarity transform fitting ──────────────────────────────────────────────

/// Apply affine transform [a,b,c,d,e,f] to a point:
///   x' = a*x + b*y + c
///   y' = d*x + e*y + f
pub fn apply_transform(t: &[f64; 6], p: Point2) -> Point2 {
    [t[0]*p[0] + t[1]*p[1] + t[2],
     t[3]*p[0] + t[4]*p[1] + t[5]]
}

/// Compose two affine transforms: apply `inner` then `outer`.
pub fn compose_transforms(outer: &[f64; 6], inner: &[f64; 6]) -> [f64; 6] {
    [
        outer[0]*inner[0] + outer[1]*inner[3],
        outer[0]*inner[1] + outer[1]*inner[4],
        outer[0]*inner[2] + outer[1]*inner[5] + outer[2],
        outer[3]*inner[0] + outer[4]*inner[3],
        outer[3]*inner[1] + outer[4]*inner[4],
        outer[3]*inner[2] + outer[4]*inner[5] + outer[5],
    ]
}

pub const IDENTITY_TRANSFORM: [f64; 6] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

/// Fit an affine transform mapping `src[0..3]` exactly to `dst[0..3]`.
/// Returns None if the source triangle is degenerate.
fn fit_affine_3(src: [Point2; 3], dst: [Point2; 3]) -> Option<[f64; 6]> {
    // Solve 2 independent 3×3 systems (one for x', one for y'):
    //   [sx0 sy0 1] [a b c]^T = dx0, etc.
    let m = [
        [src[0][0], src[0][1], 1.0],
        [src[1][0], src[1][1], 1.0],
        [src[2][0], src[2][1], 1.0],
    ];
    let det = m[0][0]*(m[1][1]*m[2][2] - m[1][2]*m[2][1])
            - m[0][1]*(m[1][0]*m[2][2] - m[1][2]*m[2][0])
            + m[0][2]*(m[1][0]*m[2][1] - m[1][1]*m[2][0]);
    if det.abs() < EPS { return None; }
    let inv = 1.0 / det;

    let solve = |rhs: [f64; 3]| -> [f64; 3] {
        let r0 = rhs[0]; let r1 = rhs[1]; let r2 = rhs[2];
        let c0 = inv * (r0*(m[1][1]*m[2][2]-m[1][2]*m[2][1])
                      - m[0][1]*(r1*m[2][2]-m[1][2]*r2)
                      + m[0][2]*(r1*m[2][1]-m[1][1]*r2));
        let c1 = inv * (m[0][0]*(r1*m[2][2]-m[1][2]*r2)
                      - r0*(m[1][0]*m[2][2]-m[1][2]*m[2][0])
                      + m[0][2]*(m[1][0]*r2-r1*m[2][0]));
        let c2 = inv * (m[0][0]*(m[1][1]*r2-r1*m[2][1])
                      - m[0][1]*(m[1][0]*r2-r1*m[2][0])
                      + r0*(m[1][0]*m[2][1]-m[1][1]*m[2][0]));
        [c0, c1, c2]
    };

    let [a, b, c] = solve([dst[0][0], dst[1][0], dst[2][0]]);
    let [d, e, f] = solve([dst[0][1], dst[1][1], dst[2][1]]);
    Some([a, b, c, d, e, f])
}

/// Find the affine transform that maps `canonical` polygon (by some cyclic rotation
/// and optional reflection) to `actual` polygon.  Both must have the same number of
/// vertices and matching invariants.  Returns the transform and the rotation offset
/// (which canonical vertex maps to actual[0]).
pub fn fit_similarity_to_polygon(
    canonical: &[Point2],
    actual: &[Point2],
) -> Option<([f64; 6], usize)> {
    let n = canonical.len();
    if n != actual.len() || n < 3 { return None; }

    // Try every cyclic alignment (which canonical vertex aligns with actual[0])
    for offset in 0..n {
        // Direct orientation
        let src = [
            canonical[offset % n],
            canonical[(offset+1) % n],
            canonical[(offset+2) % n],
        ];
        let dst = [actual[0], actual[1], actual[2]];
        if let Some(t) = fit_affine_3(src, dst) {
            // Verify all other vertices
            if canonical.iter().enumerate().all(|(i, &p)| {
                dist(apply_transform(&t, p), actual[(n + i - offset) % n]) < 1e-6
            }) {
                return Some((t, offset));
            }
        }

        // Reflected orientation: reverse canonical and try
        let can_rev: Vec<Point2> = canonical.iter().copied().rev().collect();
        let src_r = [
            can_rev[offset % n],
            can_rev[(offset+1) % n],
            can_rev[(offset+2) % n],
        ];
        if let Some(t) = fit_affine_3(src_r, dst) {
            if can_rev.iter().enumerate().all(|(i, &p)| {
                dist(apply_transform(&t, p), actual[(n + i - offset) % n]) < 1e-6
            }) {
                // offset into original canonical: vertex (n-1-offset) maps to actual[0]
                let orig_offset = (n - offset) % n;
                return Some((t, orig_offset));
            }
        }
    }
    None
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signed_area_square() {
        let sq = [[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]];
        assert!((signed_area(&sq) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_signed_area_cw_is_negative() {
        let sq = [[0.0,0.0],[0.0,1.0],[1.0,1.0],[1.0,0.0]];
        assert!(signed_area(&sq) < 0.0);
    }

    #[test]
    fn test_segment_intersection() {
        let a=[0.0,0.0]; let b=[1.0,1.0];
        let c=[0.0,1.0]; let d=[1.0,0.0];
        let r = segment_intersection(a,b,c,d);
        assert!(r.is_some());
        let (t, u) = r.unwrap();
        assert!((t-0.5).abs() < 1e-9);
        assert!((u-0.5).abs() < 1e-9);
    }

    #[test]
    fn test_no_intersection_parallel() {
        let a=[0.0,0.0]; let b=[1.0,0.0];
        let c=[0.0,1.0]; let d=[1.0,1.0];
        assert!(segment_intersection(a,b,c,d).is_none());
    }

    #[test]
    fn test_invariants_square() {
        let sq = [[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]];
        let r = side_length_ratios(&sq);
        assert_eq!(r.len(), 4);
        for v in &r { assert!((v - 0.25).abs() < 1e-9); }
    }

    #[test]
    fn test_invariants_match_rotation() {
        let a = [0.5_f64, 0.5];
        let b = [0.5_f64, 0.5];
        assert!(invariants_match(&a, &b));
        let a2 = [0.3_f64, 0.7];
        let b2 = [0.7_f64, 0.3];
        // b2 is a rotation of a2 (len-2 array, rotation by 1 = same as reflection)
        assert!(invariants_match(&a2, &b2));
    }

    #[test]
    fn test_fit_similarity_identity() {
        let sq: Vec<Point2> = vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]];
        let (t, offset) = fit_similarity_to_polygon(&sq, &sq).unwrap();
        assert_eq!(offset, 0);
        for p in &sq {
            let tp = apply_transform(&t, *p);
            assert!(dist(tp, *p) < 1e-9);
        }
    }

    #[test]
    fn test_fit_similarity_scaled() {
        let can: Vec<Point2> = vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]];
        // Scaled by 0.5 and translated to (1, 2)
        let actual: Vec<Point2> = vec![[1.0,2.0],[1.5,2.0],[1.5,2.5],[1.0,2.5]];
        let (t, _) = fit_similarity_to_polygon(&can, &actual).unwrap();
        for (i, p) in can.iter().enumerate() {
            assert!(dist(apply_transform(&t, *p), actual[i]) < 1e-6);
        }
    }
}
