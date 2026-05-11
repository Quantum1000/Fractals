use std::collections::HashMap;

use crate::ast::{File, Item, SymmetryGroup};
use super::{NormalizeError, geom};

/// Run the symmetry pass.  Returns a map from tile name → inferred/validated
/// SymmetryGroup.  Pushes errors for inconsistent declarations.
///
/// Symmetry is inferred under affine transformations: a permutation π of the
/// vertices is a symmetry iff there exists an affine map sending vertex i to
/// vertex π(i) for every i.  Under this definition every non-degenerate
/// triangle has D3, every non-degenerate parallelogram has at least C2, every
/// rhombus/rectangle/square has D4 (they are all affinely equivalent), etc.
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
        (_, Trivial) => true,
        (Cyclic(inf_n), Cyclic(dec_n)) => inf_n % dec_n == 0,
        (Dihedral(inf_n), Cyclic(dec_n)) => inf_n % dec_n == 0,
        (Dihedral(inf_n), Dihedral(dec_n)) => inf_n % dec_n == 0,
        _ => false,
    }
}

/// Infer the maximal affine symmetry group of a polygon.
pub fn infer_symmetry(pts: &[geom::Point2]) -> SymmetryGroup {
    let n = pts.len();
    if n < 3 { return SymmetryGroup::Trivial; }

    // Count rotational symmetries (cyclic shifts of vertex indices).
    let mut cyclic_order: u32 = 0;
    for s in 0..n {
        let perm: Vec<usize> = (0..n).map(|i| (i + s) % n).collect();
        if is_affine_symmetry(pts, &perm) {
            cyclic_order += 1;
        }
    }

    // Check whether any reflection (reversal of vertex order with an offset)
    // is also an affine symmetry.  If so, the full group is Dihedral.
    let mut has_refl = false;
    for j in 0..n {
        let perm = reversal_perm(j, n);
        if is_affine_symmetry(pts, &perm) {
            has_refl = true;
            break;
        }
    }

    match (cyclic_order, has_refl) {
        (0, _) | (1, false) => SymmetryGroup::Trivial,
        (1, true) => SymmetryGroup::Dihedral(1),
        (k, false) => SymmetryGroup::Cyclic(k),
        (k, true) => SymmetryGroup::Dihedral(k),
    }
}

/// True iff some affine map sends pts[i] → pts[perm[i]] for every i.
pub(super) fn is_affine_symmetry(pts: &[geom::Point2], perm: &[usize]) -> bool {
    let n = pts.len();
    if n == 0 || perm.len() != n { return n == perm.len(); }
    if n < 3 {
        return (0..n).all(|i| geom::dist(pts[i], pts[perm[i]]) < 1e-6);
    }
    let src = [pts[0], pts[1], pts[2]];
    let dst = [pts[perm[0]], pts[perm[1]], pts[perm[2]]];
    let Some(t) = geom::fit_affine_3(src, dst) else { return false; };
    for i in 3..n {
        let mapped = geom::apply_transform(&t, pts[i]);
        if geom::dist(mapped, pts[perm[i]]) > 1e-6 { return false; }
    }
    true
}

/// Cyclic shift permutation: i ↦ (i + s) mod n.
pub(super) fn cyclic_shift_perm(shift: usize, n: usize) -> Vec<usize> {
    (0..n).map(|i| (i + shift) % n).collect()
}

/// Reversal permutation parameterised by axis offset j: i ↦ (j − i) mod n.
pub(super) fn reversal_perm(j: usize, n: usize) -> Vec<usize> {
    (0..n)
        .map(|i| ((j as isize - i as isize).rem_euclid(n as isize)) as usize)
        .collect()
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

    fn generic_parallelogram() -> Vec<geom::Point2> {
        vec![[0.0,0.0],[1.0,0.0],[1.3,1.0],[0.3,1.0]]
    }

    fn generic_quad() -> Vec<geom::Point2> {
        vec![[0.0,0.0],[1.0,0.0],[1.2,1.1],[0.1,0.9]]
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
    fn test_right_triangle_is_d3_under_affine() {
        // All triangles are affinely equivalent to the equilateral, so they
        // all have D3 affine symmetry.
        assert_eq!(infer_symmetry(&right_triangle()), SymmetryGroup::Dihedral(3));
    }

    #[test]
    fn test_parallelogram_is_d4_under_affine() {
        // Every non-degenerate parallelogram is affinely equivalent to the
        // unit square, so under affine maps it has the same symmetry group: D4.
        assert_eq!(infer_symmetry(&generic_parallelogram()), SymmetryGroup::Dihedral(4));
    }

    #[test]
    fn test_isoceles_trapezoid_is_d1() {
        // Isoceles trapezoid: a single reflection axis, no rotational symmetry.
        let poly = vec![[0.0,0.0],[1.0,0.0],[0.7,1.0],[0.3,1.0]];
        assert_eq!(infer_symmetry(&poly), SymmetryGroup::Dihedral(1));
    }

    #[test]
    fn test_generic_quad_is_trivial() {
        assert_eq!(infer_symmetry(&generic_quad()), SymmetryGroup::Trivial);
    }

    #[test]
    fn test_compatible_subgroup() {
        assert!(symmetry_compatible(&SymmetryGroup::Dihedral(4), &SymmetryGroup::Cyclic(2)));
        assert!(!symmetry_compatible(&SymmetryGroup::Trivial, &SymmetryGroup::Dihedral(4)));
    }
}
