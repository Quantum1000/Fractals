/// Group-element ↔ permutation mappings (step 7).
///
/// For each tile type, enumerate all group elements of its symmetry group,
/// apply each as a geometric transform to the canonical polygon, and record
/// which permutation of vertex indices results.

use std::collections::HashMap;
use crate::ast::{File, Item, SymmetryGroup};
use super::{NormalizeError, GroupPermEntry, geom};
use super::symmetry::{group_element_names, apply_group_element};

pub fn compute(
    file: &File,
    tile_symmetries: &HashMap<String, SymmetryGroup>,
    errors: &mut Vec<NormalizeError>,
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
        let centered = geom::center_polygon(&canonical);

        let elem_names = group_element_names(sym);
        let mut entries: Vec<GroupPermEntry> = Vec::new();

        for ename in &elem_names {
            let transformed = match apply_group_element(&centered, sym, ename) {
                Some(t) => t,
                None => continue,
            };

            // Find the permutation: for each transformed vertex, which original
            // centered vertex is it closest to?
            let n = centered.len();
            let mut perm: Vec<usize> = vec![usize::MAX; n];
            let mut used = vec![false; n];

            let mut ok = true;
            for (i, tp) in transformed.iter().enumerate() {
                let closest = centered.iter().enumerate()
                    .filter(|(j, _)| !used[*j])
                    .min_by(|(_, a), (_, b)| {
                        geom::dist_sq(**a, *tp).partial_cmp(&geom::dist_sq(**b, *tp)).unwrap()
                    });
                match closest {
                    Some((j, _)) if geom::dist(centered[j], *tp) < 1e-6 => {
                        perm[i] = j;
                        used[j] = true;
                    }
                    _ => { ok = false; break; }
                }
            }

            if ok && !perm.contains(&usize::MAX) {
                entries.push(GroupPermEntry { name: ename.clone(), perm });
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
}
