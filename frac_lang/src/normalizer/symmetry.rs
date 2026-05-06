use std::collections::HashMap;
use std::f64::consts::PI;

use crate::ast::{File, Item, SymmetryGroup, Span};
use super::{NormalizeError, geom};

/// Run the symmetry pass.  Returns a map from tile name → inferred/validated
/// SymmetryGroup.  Pushes errors for inconsistent declarations.
pub fn infer_symmetries(
    file: &File,
    errors: &mut Vec<NormalizeError>,
) -> HashMap<String, SymmetryGroup> {
    let mut result = HashMap::new();

    for item in &file.items {
        let tile = match &item.node {
            Item::Tile(t) => t,
            _ => continue,
        };

        let name = tile.name.0.node.clone();
        let pts: Vec<geom::Point2> = tile.canonical.iter()
            .map(|p| [p.node.x, p.node.y])
            .collect();

        let inferred = infer_symmetry(&pts);

        match &tile.symmetry {
            None => {
                result.insert(name, inferred);
            }
            Some(decl) => {
                if !symmetry_compatible(&inferred, &decl.node) {
                    errors.push(NormalizeError::InconsistentSymmetry {
                        tile: name.clone(),
                        declared: decl.node.clone(),
                        inferred: inferred.clone(),
                        span: decl.span,
                    });
                }
                // Use the declared value even when inconsistent — later passes
                // will see the error and stop before using bad data.
                result.insert(name, decl.node.clone());
            }
        }
    }

    result
}

/// Return true if `declared` is a subgroup of `inferred` (i.e. the tile really
/// does have at least the declared symmetry).
fn symmetry_compatible(inferred: &SymmetryGroup, declared: &SymmetryGroup) -> bool {
    use SymmetryGroup::*;
    match (inferred, declared) {
        // Trivial is a subgroup of everything
        (_, Trivial) => true,
        // Declared Cn: inferred must be Cn or Dn with the same or larger n that is a multiple
        (Cyclic(inf_n), Cyclic(dec_n)) => inf_n % dec_n == 0,
        (Dihedral(inf_n), Cyclic(dec_n)) => inf_n % dec_n == 0,
        // Declared Dn: inferred must be Dn with the same or larger n that is a multiple
        (Dihedral(inf_n), Dihedral(dec_n)) => inf_n % dec_n == 0,
        // Everything else is incompatible
        _ => false,
    }
}

/// Infer the maximal symmetry group of a polygon.
///
/// Algorithm:
/// 1. Center the polygon.
/// 2. For k = sides down to 1, check Dk and Ck.
/// 3. Return the largest group found.
///
/// For practical tile shapes (triangles, quads, hexagons …) this is O(n²).
pub fn infer_symmetry(pts: &[geom::Point2]) -> SymmetryGroup {
    let n = pts.len();
    if n == 0 { return SymmetryGroup::Trivial; }

    let centered = geom::center_polygon(pts);

    // Try Dn then Cn in descending order of k (k=1 is trivial, skip it).
    for k in (2..=n).rev() {
        if n % k != 0 { continue; }
        if has_dihedral(&centered, k as u32) {
            return SymmetryGroup::Dihedral(k as u32);
        }
    }
    for k in (2..=n).rev() {
        if n % k != 0 { continue; }
        if has_cyclic(&centered, k as u32) {
            return SymmetryGroup::Cyclic(k as u32);
        }
    }
    SymmetryGroup::Trivial
}

/// True if the polygon has cyclic symmetry of order k (rotation by 2π/k maps it to itself).
fn has_cyclic(centered: &[geom::Point2], k: u32) -> bool {
    let theta = 2.0 * PI / k as f64;
    let rotated: Vec<geom::Point2> = centered.iter()
        .map(|&p| geom::rotate(p, theta))
        .collect();
    polygon_equal(centered, &rotated)
}

/// True if the polygon has dihedral symmetry of order k (rotation + reflection).
fn has_dihedral(centered: &[geom::Point2], k: u32) -> bool {
    if !has_cyclic(centered, k) { return false; }
    // One reflection suffices (along the axis through vertex 0 and the centroid).
    // Reflection reverses winding, so compare the reflected polygon in REVERSE
    // vertex order against the original CCW polygon.
    let axis = geom::angle(centered[0]);
    let reflected: Vec<geom::Point2> = centered.iter()
        .map(|&p| geom::reflect(p, axis))
        .collect();
    let reflected_rev: Vec<geom::Point2> = reflected.into_iter().rev().collect();
    polygon_equal(centered, &reflected_rev)
}

/// True if two polygons are equal up to a cyclic rotation of vertex indices.
fn polygon_equal(a: &[geom::Point2], b: &[geom::Point2]) -> bool {
    let n = a.len();
    if n != b.len() { return false; }
    const TOL: f64 = 1e-6;
    'rot: for offset in 0..n {
        for i in 0..n {
            if geom::dist(a[i], b[(i+offset)%n]) > TOL { continue 'rot; }
        }
        return true;
    }
    false
}

/// All element names for a symmetry group (used by the group_perm pass).
pub fn group_element_names(g: &SymmetryGroup) -> Vec<String> {
    match g {
        SymmetryGroup::Trivial => vec!["identity".to_string()],
        SymmetryGroup::Cyclic(n) => {
            let mut v = vec!["identity".to_string()];
            for i in 1..*n {
                v.push(format!("r{}", i * 360 / n));
            }
            v
        }
        SymmetryGroup::Dihedral(n) => {
            let mut v = vec!["identity".to_string()];
            for i in 1..*n {
                v.push(format!("r{}", i * 360 / n));
            }
            // Reflections: fh, fv, fd, fad for n=4; fv0, fv1, ... for others
            match n {
                1 => v.push("f".to_string()),
                2 => { v.push("fh".to_string()); v.push("fv".to_string()); }
                4 => {
                    v.push("fh".to_string());
                    v.push("fv".to_string());
                    v.push("fd".to_string());
                    v.push("fad".to_string());
                }
                _ => {
                    for i in 0..*n {
                        v.push(format!("fv{}", i));
                    }
                }
            }
            v
        }
    }
}

/// Apply group element `elem_name` (from group `g`) as a rotation/reflection to
/// a centered polygon, returning the permuted vertex sequence.
pub fn apply_group_element(
    centered: &[geom::Point2],
    g: &SymmetryGroup,
    elem_name: &str,
) -> Option<Vec<geom::Point2>> {
    let n = centered.len();
    let pi = PI;

    let transformed: Vec<geom::Point2> = if elem_name == "identity" {
        centered.to_vec()
    } else if let Some(deg_str) = elem_name.strip_prefix('r') {
        let deg: f64 = deg_str.parse().ok()?;
        centered.iter().map(|&p| geom::rotate(p, deg * pi / 180.0)).collect()
    } else {
        // Reflection
        let axis = reflection_axis(g, elem_name, centered)?;
        centered.iter().map(|&p| geom::reflect(p, axis)).collect()
    };

    Some(transformed)
}

fn reflection_axis(
    g: &SymmetryGroup,
    name: &str,
    centered: &[geom::Point2],
) -> Option<f64> {
    match name {
        "fh"  => Some(0.0),          // x-axis
        "fv"  => Some(PI / 2.0),     // y-axis
        "fd"  => Some(PI / 4.0),     // diagonal y=x
        "fad" => Some(-PI / 4.0),    // anti-diagonal y=-x
        "f"   => Some(geom::angle(centered.first().copied().unwrap_or([1.0, 0.0]))),
        other => {
            // "fvN" — axis through vertex N and origin
            if let Some(idx_str) = other.strip_prefix("fv") {
                let idx: usize = idx_str.parse().ok()?;
                if let SymmetryGroup::Dihedral(n) = g {
                    // Axis at angle k * π/n for the k-th reflection
                    let angle = idx as f64 * PI / *n as f64;
                    return Some(angle);
                }
            }
            None
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<geom::Point2> {
        vec![[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]]
    }

    fn equilateral_triangle() -> Vec<geom::Point2> {
        let h = (3.0_f64).sqrt() / 2.0;
        vec![[0.0, 0.0], [1.0, 0.0], [0.5, h]]
    }

    fn right_triangle() -> Vec<geom::Point2> {
        vec![[0.0,0.0],[1.0,0.0],[0.0,1.0]]
    }

    #[test]
    fn test_square_is_d4() {
        assert_eq!(infer_symmetry(&square()), SymmetryGroup::Dihedral(4));
    }

    #[test]
    fn test_equilateral_triangle_is_d3() {
        assert_eq!(infer_symmetry(&equilateral_triangle()), SymmetryGroup::Dihedral(3));
    }

    #[test]
    fn test_right_triangle_is_trivial() {
        assert_eq!(infer_symmetry(&right_triangle()), SymmetryGroup::Trivial);
    }

    #[test]
    fn test_compatible_subgroup() {
        assert!(symmetry_compatible(&SymmetryGroup::Dihedral(4), &SymmetryGroup::Cyclic(2)));
        assert!(!symmetry_compatible(&SymmetryGroup::Trivial, &SymmetryGroup::Dihedral(4)));
    }

    #[test]
    fn test_group_element_names_d4() {
        let names = group_element_names(&SymmetryGroup::Dihedral(4));
        assert!(names.contains(&"r90".to_string()));
        assert!(names.contains(&"fh".to_string()));
    }
}
