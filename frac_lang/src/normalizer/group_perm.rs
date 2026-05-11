/// Group-element ↔ permutation mappings.
///
/// For each tile type, enumerate the group elements of its (declared or
/// inferred) symmetry group, derive the corresponding vertex permutation
/// directly from the group structure, and verify each permutation really is an
/// affine symmetry of the canonical polygon.  Invalid candidates (e.g. a
/// declared symmetry the tile does not actually possess) are silently dropped;
/// the symmetry pass produces a `InconsistentSymmetry` diagnostic in that case.

use std::collections::HashMap;
use crate::ast::{File, Item, SymmetryGroup};
use super::{NormalizeError, GroupPermEntry, geom};
use super::symmetry::{is_affine_symmetry, cyclic_shift_perm, reversal_perm};

pub fn compute(
    file: &File,
    tile_symmetries: &HashMap<String, SymmetryGroup>,
    _errors: &mut Vec<NormalizeError>,
) -> HashMap<String, Vec<GroupPermEntry>> {
    let mut result = HashMap::new();

    for item in &file.items {
        let tile = match &item.node {
            Item::Tile(t) => t,
            _ => continue,
        };

        let name = tile.name.0.node.clone();
        let sym = match tile_symmetries.get(&name) {
            Some(s) => s,
            None => continue,
        };

        let canonical: Vec<geom::Point2> = tile.canonical.iter()
            .map(|p| [p.node.x, p.node.y])
            .collect();
        let n = canonical.len();

        let mut entries: Vec<GroupPermEntry> = Vec::new();

        // Identity is always the first element.
        entries.push(GroupPermEntry {
            name: "identity".to_string(),
            perm: (0..n).collect(),
        });

        // Rotational part.
        let ord = match sym {
            SymmetryGroup::Trivial => 1,
            SymmetryGroup::Cyclic(k) | SymmetryGroup::Dihedral(k) => *k as usize,
        };

        if ord > 1 && n > 0 && n % ord == 0 {
            let step = n / ord;
            for i in 1..ord {
                let perm = cyclic_shift_perm(i * step, n);
                if is_affine_symmetry(&canonical, &perm) {
                    entries.push(GroupPermEntry {
                        name: format!("r{}", i * 360 / ord),
                        perm,
                    });
                }
            }
        }

        // Reflections (only present for Dihedral groups).
        if let SymmetryGroup::Dihedral(k) = sym {
            let k = *k as usize;
            let mut found = 0;
            for j in 0..n {
                if found >= k { break; }
                let perm = reversal_perm(j, n);
                if is_affine_symmetry(&canonical, &perm) {
                    entries.push(GroupPermEntry {
                        name: format!("fv{}", found),
                        perm,
                    });
                    found += 1;
                }
            }
        }

        result.insert(name, entries);
    }

    result
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn test_square_identity_perm() {
        let src = "tile quad { sides 4 canonical (0,0) (1,0) (1,1) (0,1) symmetry d4 }";
        let (file, _) = parse(src);
        let syms: HashMap<String, SymmetryGroup> =
            [("quad".to_string(), SymmetryGroup::Dihedral(4))].into();
        let mut errors = Vec::new();
        let gp = compute(&file, &syms, &mut errors);
        assert!(errors.is_empty());
        let entries = &gp["quad"];
        let identity = entries.iter().find(|e| e.name == "identity").unwrap();
        assert_eq!(identity.perm, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_square_d4_has_8_elements() {
        let src = "tile quad { sides 4 canonical (0,0) (1,0) (1,1) (0,1) symmetry d4 }";
        let (file, _) = parse(src);
        let syms: HashMap<String, SymmetryGroup> =
            [("quad".to_string(), SymmetryGroup::Dihedral(4))].into();
        let mut errors = Vec::new();
        let gp = compute(&file, &syms, &mut errors);
        assert_eq!(gp["quad"].len(), 8);
    }

    #[test]
    fn test_right_triangle_d3_has_6_elements() {
        let src = "tile tri { sides 3 canonical (0,0) (1,0) (0,1) }";
        let (file, _) = parse(src);
        let syms: HashMap<String, SymmetryGroup> =
            [("tri".to_string(), SymmetryGroup::Dihedral(3))].into();
        let mut errors = Vec::new();
        let gp = compute(&file, &syms, &mut errors);
        let entries = &gp["tri"];
        assert_eq!(entries.len(), 6);
        // r120 should map vertex 0 → 1, 1 → 2, 2 → 0
        let r120 = entries.iter().find(|e| e.name == "r120").unwrap();
        assert_eq!(r120.perm, vec![1, 2, 0]);
    }
}
