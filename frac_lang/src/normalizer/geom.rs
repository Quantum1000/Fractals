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
pub fn fit_affine_3(src: [Point2; 3], dst: [Point2; 3]) -> Option<[f64; 6]> {
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

/// Find an affine transform that maps `canonical` polygon (by some cyclic rotation
/// and optional reflection) to `actual` polygon.  Both must have the same number of
/// vertices.  Returns the transform and the rotation offset (which canonical vertex
/// maps to `actual[0]`).
pub fn fit_affine_to_polygon(
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

// ── Affine invariants (§3.3) ──────────────────────────────────────────────────

/// Compute the canonical affine-invariant tuple for a polygon.
///
/// The result is a `(2n − 6)`-tuple (empty for n ≤ 3).  Two polygons belong
/// to the same affine equivalence class iff their invariant tuples are equal
/// (within floating-point tolerance).
///
/// Construction: enumerate all 2n labellings (n cyclic rotations × 2
/// orientations), compute affine coordinates of remaining vertices in the
/// basis formed by the first three, and return the lex-min tuple.
pub fn compute_affine_invariants(poly: &[Point2]) -> Vec<f64> {
    let n = poly.len();
    if n < 3 { return vec![]; }
    // 2n - 6 coordinates; for n == 3 that's 0.
    let coord_count = 2 * n - 6;
    if coord_count == 0 { return vec![]; }

    let mut min_tuple: Option<Vec<f64>> = None;

    for start in 0..n {
        for &reversed in &[false, true] {
            let labelling: Vec<Point2> = (0..n)
                .map(|i| if reversed {
                    poly[(start + n - i) % n]
                } else {
                    poly[(start + i) % n]
                })
                .collect();

            // Affine map sending labelling[0]→(0,0), labelling[1]→(1,0), labelling[2]→(0,1).
            let src = [labelling[0], labelling[1], labelling[2]];
            let dst = [[0.0_f64, 0.0], [1.0, 0.0], [0.0, 1.0]];
            let Some(t) = fit_affine_3(src, dst) else { continue };

            let mut coords: Vec<f64> = Vec::with_capacity(coord_count);
            for i in 3..n {
                let p = apply_transform(&t, labelling[i]);
                coords.push(p[0]);
                coords.push(p[1]);
            }

            if min_tuple.is_none() || coords < *min_tuple.as_ref().unwrap() {
                min_tuple = Some(coords);
            }
        }
    }

    min_tuple.unwrap_or_default()
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
    fn test_fit_affine_identity() {
        let sq: Vec<Point2> = vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]];
        let (t, offset) = fit_affine_to_polygon(&sq, &sq).unwrap();
        assert_eq!(offset, 0);
        for p in &sq {
            let tp = apply_transform(&t, *p);
            assert!(dist(tp, *p) < 1e-9);
        }
    }

    #[test]
    fn test_fit_affine_scaled() {
        let can: Vec<Point2> = vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]];
        let actual: Vec<Point2> = vec![[1.0,2.0],[1.5,2.0],[1.5,2.5],[1.0,2.5]];
        let (t, _) = fit_affine_to_polygon(&can, &actual).unwrap();
        for (i, p) in can.iter().enumerate() {
            assert!(dist(apply_transform(&t, *p), actual[i]) < 1e-6);
        }
    }

    #[test]
    fn test_fit_affine_matches_sheared_quad() {
        // Affine matching: a unit square should map to an arbitrary
        // parallelogram (they are affine-equivalent).
        let can: Vec<Point2> = vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]];
        let actual: Vec<Point2> = vec![[0.0,0.0],[2.0,0.0],[3.0,1.0],[1.0,1.0]];
        let fit = fit_affine_to_polygon(&can, &actual);
        assert!(fit.is_some(), "parallelogram should affine-match square");
    }

    #[test]
    fn test_affine_invariants_triangle() {
        // Any non-degenerate triangle → empty tuple
        let tri = [[0.0,0.0],[1.0,0.0],[0.5,1.0]];
        assert!(compute_affine_invariants(&tri).is_empty());
    }

    #[test]
    fn test_affine_invariants_unit_square() {
        // Unit square → (-1, 1) per spec §3.3 examples
        let sq: Vec<Point2> = vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]];
        let inv = compute_affine_invariants(&sq);
        assert_eq!(inv.len(), 2);
        assert!((inv[0] - (-1.0)).abs() < 1e-9, "inv[0] = {}", inv[0]);
        assert!((inv[1] - 1.0).abs() < 1e-9,    "inv[1] = {}", inv[1]);
    }

    #[test]
    fn test_affine_invariants_sheared_parallelogram_matches_square() {
        // Any parallelogram is affinely equivalent to the unit square.
        let sq:   Vec<Point2> = vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]];
        let para: Vec<Point2> = vec![[0.0,0.0],[2.0,0.0],[3.0,1.0],[1.0,1.0]];
        let inv_sq   = compute_affine_invariants(&sq);
        let inv_para = compute_affine_invariants(&para);
        assert_eq!(inv_sq.len(), inv_para.len());
        for (a, b) in inv_sq.iter().zip(inv_para.iter()) {
            assert!((a - b).abs() < 1e-9, "invariants differ: {:?} vs {:?}", inv_sq, inv_para);
        }
    }

    #[test]
    fn test_affine_invariants_non_parallelogram_quad() {
        // (0,0)(1,0)(1,1)(0,2) → (-2, 1) per spec §3.3 examples
        let quad: Vec<Point2> = vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,2.0]];
        let inv = compute_affine_invariants(&quad);
        assert_eq!(inv.len(), 2);
        assert!((inv[0] - (-2.0)).abs() < 1e-9, "inv[0] = {}", inv[0]);
        assert!((inv[1] - 1.0).abs() < 1e-9,    "inv[1] = {}", inv[1]);
    }

    #[test]
    fn test_affine_invariants_distinguishes_non_equivalent_quads() {
        let sq:   Vec<Point2> = vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]];
        let quad: Vec<Point2> = vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,2.0]];
        let inv_sq   = compute_affine_invariants(&sq);
        let inv_quad = compute_affine_invariants(&quad);
        assert_ne!(inv_sq, inv_quad);
    }

    #[test]
    fn test_fit_affine_rejects_non_affine_equivalent() {
        // A regular pentagon cannot affine-match a square (different vertex count).
        let can: Vec<Point2> = vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]];
        let actual: Vec<Point2> = vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.5,1.5],[0.0,1.0]];
        assert!(fit_affine_to_polygon(&can, &actual).is_none());
    }
}
