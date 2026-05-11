/// Partition topology: planar graph construction + face enumeration.
///
/// Given a partition declaration (named vertices + cuts) and the parent tile's
/// canonical polygon, this module:
///   1. Builds a planar graph (with implicit intersection vertices).
///   2. Enumerates faces using half-edge traversal.
///   3. Discards the outer face.
///   4. Matches each inner face to a tile type by direct affine fitting.
///   5. Records the affine transform and anchor vertex for each child.
///   6. Assigns child indices and names.

use std::collections::HashMap;
use crate::ast::{File, Item, PartitionDecl, VertexKind};
use super::{NormalizeError, ChildInfo, NormalizedPartition, geom};
use super::geom::Point2;

const EPS: f64 = 1e-9;
const MATCH_TOL: f64 = 1e-5;

/// An ordered list of (tile_name, canonical_polygon), in source declaration
/// order.  Iteration order is deterministic; first affine match wins.
type TileCanonicals = Vec<(String, Vec<Point2>)>;

// ── Public entry point ────────────────────────────────────────────────────────

pub fn compute_partitions(
    file: &File,
    tile_canonicals: &TileCanonicals,
    errors: &mut Vec<NormalizeError>,
) -> HashMap<(String, String), NormalizedPartition> {
    let mut result = HashMap::new();

    let tile_polygons: HashMap<&str, &[Point2]> = tile_canonicals
        .iter()
        .map(|(n, p)| (n.as_str(), p.as_slice()))
        .collect();

    for item in &file.items {
        let part = match &item.node {
            Item::Partition(p) => p,
            _ => continue,
        };

        let tile_name = part.tile.0.node.clone();
        let part_name = part.name.0.node.clone();
        let key = (tile_name.clone(), part_name.clone());

        let parent_poly = match tile_polygons.get(tile_name.as_str()) {
            Some(p) => p.to_vec(),
            None => {
                errors.push(NormalizeError::UnknownTile {
                    name: tile_name.clone(),
                    span: part.tile.0.span,
                });
                continue;
            }
        };

        match compute_one_partition(
            part,
            &parent_poly,
            tile_canonicals,
            &key,
            errors,
        ) {
            Some(np) => { result.insert(key, np); }
            None => {}
        }
    }

    result
}

// ── Per-partition computation ─────────────────────────────────────────────────

fn compute_one_partition(
    part: &PartitionDecl,
    parent_poly: &[Point2],
    tile_canonicals: &TileCanonicals,
    key: &(String, String),
    errors: &mut Vec<NormalizeError>,
) -> Option<NormalizedPartition> {
    // ── Step 1: collect all vertices ─────────────────────────────────────────
    // Start with canonical boundary vertices.
    let mut verts: Vec<Point2> = parent_poly.to_vec();
    let boundary_count = verts.len();

    // Build vertex name → index map.
    let mut name_to_idx: HashMap<String, usize> = HashMap::new();

    // Named partition vertices
    for pv in &part.vertices {
        let pos: Point2 = [pv.node.pos.node.x, pv.node.pos.node.y];
        let vname = pv.node.name.0.node.clone();

        if pv.node.kind == VertexKind::Edge {
            // Find which boundary edge this vertex lies on and insert it there.
            // We store it as a regular vertex; the boundary edge will be split
            // when we build the planar graph edges.
            match find_boundary_edge(pos, parent_poly) {
                Some(_) => {}
                None => {
                    errors.push(NormalizeError::InvalidPartitionVertex {
                        partition: key.clone(),
                        name: vname.clone(),
                        reason: "edge vertex does not lie on any boundary edge".into(),
                        span: pv.node.pos.span,
                    });
                    return None;
                }
            }
        }

        let idx = verts.len();
        verts.push(pos);
        name_to_idx.insert(vname, idx);
    }

    // ── Step 2: build initial edges ──────────────────────────────────────────
    // Boundary edges (will be split where edge vertices lie on them).
    let mut edges: Vec<(usize, usize)> = Vec::new();
    build_boundary_edges(parent_poly, &verts[boundary_count..], &name_to_idx, part, &mut edges);

    // Cut edges
    for cut in &part.cuts {
        let from = cut.node.from.0.node.as_str();
        let to   = cut.node.to.0.node.as_str();
        let fi = match name_to_idx.get(from) {
            Some(&i) => i,
            None => {
                errors.push(NormalizeError::InvalidPartitionVertex {
                    partition: key.clone(),
                    name: from.to_string(),
                    reason: "unknown vertex name in cut".into(),
                    span: cut.node.from.0.span,
                });
                return None;
            }
        };
        let ti = match name_to_idx.get(to) {
            Some(&i) => i,
            None => {
                errors.push(NormalizeError::InvalidPartitionVertex {
                    partition: key.clone(),
                    name: to.to_string(),
                    reason: "unknown vertex name in cut".into(),
                    span: cut.node.to.0.span,
                });
                return None;
            }
        };
        edges.push((fi, ti));
    }

    // ── Step 3: subdivide intersecting cut edges ──────────────────────────────
    let (verts, edges) = subdivide_intersections(verts, edges);

    // ── Step 4: half-edge DCEL + face enumeration ────────────────────────────
    let faces = enumerate_faces(&verts, &edges);

    // Filter out the outer face (largest by area, or negative signed area).
    let inner_faces: Vec<Vec<usize>> = faces.into_iter()
        .filter(|f| {
            let poly: Vec<Point2> = f.iter().map(|&i| verts[i]).collect();
            geom::signed_area(&poly) > EPS
        })
        .collect();

    if inner_faces.is_empty() {
        errors.push(NormalizeError::NoTileMatch {
            partition: key.clone(),
            child_index: 0,
        });
        return None;
    }

    // Debug assertion: child areas sum to parent area.
    #[cfg(debug_assertions)]
    {
        let parent_area = geom::area(parent_poly);
        let child_sum: f64 = inner_faces.iter()
            .map(|f| geom::area(&f.iter().map(|&i| verts[i]).collect::<Vec<_>>()))
            .sum();
        debug_assert!(
            (child_sum - parent_area).abs() < parent_area * 1e-6,
            "child areas {child_sum} do not sum to parent area {parent_area}"
        );
    }

    // ── Step 5: match each face to a tile type ────────────────────────────────
    // Build child name lookup from author-assigned names.
    let mut index_to_name: HashMap<u32, String> = HashMap::new();
    for cn in &part.child_names {
        if let Some(ref nm) = cn.node.name {
            index_to_name.insert(cn.node.index.node, nm.0.node.clone());
        }
    }

    // Sort faces for stable canonical ordering: by centroid (x first, then y).
    let mut sorted_faces: Vec<Vec<usize>> = inner_faces;
    sorted_faces.sort_by(|a, b| {
        let ca = geom::centroid(&a.iter().map(|&i| verts[i]).collect::<Vec<_>>());
        let cb = geom::centroid(&b.iter().map(|&i| verts[i]).collect::<Vec<_>>());
        ca[0].partial_cmp(&cb[0]).unwrap().then(ca[1].partial_cmp(&cb[1]).unwrap())
    });

    // Validate child_names indices.
    for cn in &part.child_names {
        let idx = cn.node.index.node as usize;
        if idx >= sorted_faces.len() {
            errors.push(NormalizeError::ChildNameOutOfRange {
                partition: key.clone(),
                declared_index: cn.node.index.node,
                child_count: sorted_faces.len(),
                span: cn.node.index.span,
            });
        }
    }

    let mut children = Vec::new();
    for (i, face_indices) in sorted_faces.iter().enumerate() {
        let polygon: Vec<Point2> = face_indices.iter().map(|&j| verts[j]).collect();

        // Find the first declared tile (in source order) whose canonical polygon
        // can be affine-mapped onto this face.  Under affine matching, any
        // n-gon matches any other n-gon of the same affine class.
        let mut matched: Option<(String, [f64; 6], usize)> = None;
        for (tname, canonical) in tile_canonicals {
            if canonical.len() != polygon.len() { continue; }
            if let Some((transform, anchor)) =
                geom::fit_affine_to_polygon(canonical, &polygon)
            {
                matched = Some((tname.clone(), transform, anchor));
                break;
            }
        }

        match matched {
            None => {
                errors.push(NormalizeError::NoTileMatch {
                    partition: key.clone(),
                    child_index: i,
                });
                return None;
            }
            Some((tile_type, similarity_transform, anchor_vertex)) => {
                children.push(ChildInfo {
                    tile_type,
                    polygon,
                    similarity_transform,
                    anchor_vertex,
                    index: i as u32,
                    name: index_to_name.get(&(i as u32)).cloned(),
                });
            }
        }
    }

    Some(NormalizedPartition { children })
}

// ── Boundary edge construction ────────────────────────────────────────────────

/// Find which boundary segment (a, a+1) the point `pos` lies on.
fn find_boundary_edge(pos: Point2, poly: &[Point2]) -> Option<usize> {
    let n = poly.len();
    for i in 0..n {
        if geom::point_on_segment(pos, poly[i], poly[(i+1)%n]) {
            return Some(i);
        }
    }
    None
}

/// Build boundary edges, inserting named edge vertices as subdivision points.
fn build_boundary_edges(
    parent_poly: &[Point2],
    named_verts_slice: &[Point2],
    name_to_idx: &HashMap<String, usize>,
    part: &PartitionDecl,
    edges: &mut Vec<(usize, usize)>,
) {
    let n = parent_poly.len();

    // For each boundary edge, collect any edge-kind vertices that lie on it,
    // sorted by their parameter t along the edge.
    let mut subdivisions: Vec<Vec<(f64, usize)>> = vec![Vec::new(); n];

    for pv in &part.vertices {
        if pv.node.kind != VertexKind::Edge { continue; }
        let pos: Point2 = [pv.node.pos.node.x, pv.node.pos.node.y];
        let vname = &pv.node.name.0.node;
        let vidx = match name_to_idx.get(vname.as_str()) {
            Some(&i) => i,
            None => continue,
        };

        if let Some(edge_i) = find_boundary_edge(pos, parent_poly) {
            let a = parent_poly[edge_i];
            let b = parent_poly[(edge_i+1)%n];
            let len = geom::dist(a, b);
            let t = if len > EPS { geom::dist(a, pos) / len } else { 0.0 };
            subdivisions[edge_i].push((t, vidx));
        }
    }

    // Build edges for each boundary segment with its subdivision points.
    for i in 0..n {
        let mut pts = subdivisions[i].clone();
        pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let mut prev_idx = i; // index into verts = the i-th canonical vertex
        for (_, vidx) in pts {
            edges.push((prev_idx, vidx));
            prev_idx = vidx;
        }
        edges.push((prev_idx, (i+1)%n));
    }
}

// ── Edge intersection subdivision ─────────────────────────────────────────────

/// Find all intersection points among edges, insert them as new vertices,
/// and subdivide the intersecting edges.
fn subdivide_intersections(
    mut verts: Vec<Point2>,
    edges: Vec<(usize, usize)>,
) -> (Vec<Point2>, Vec<(usize, usize)>) {
    let mut result_edges: Vec<(usize, usize)> = Vec::new();

    // For each edge, collect the subdivision events (t, new_vertex_idx).
    let mut subdivs: Vec<Vec<(f64, usize)>> = vec![Vec::new(); edges.len()];

    for i in 0..edges.len() {
        for j in (i+1)..edges.len() {
            let a = verts[edges[i].0];
            let b = verts[edges[i].1];
            let c = verts[edges[j].0];
            let d = verts[edges[j].1];

            if let Some((t, u)) = geom::segment_intersection(a, b, c, d) {
                let p = geom::lerp(a, b, t);
                // Check if this point is already a vertex (within tolerance).
                let existing = verts.iter().position(|&v| geom::dist(v, p) < MATCH_TOL);
                let vidx = existing.unwrap_or_else(|| {
                    let idx = verts.len();
                    verts.push(p);
                    idx
                });
                subdivs[i].push((t, vidx));
                subdivs[j].push((u, vidx));
            }
        }
    }

    // Reconstruct edge lists with subdivision points inserted.
    for (i, (from, to)) in edges.iter().enumerate() {
        let mut pts = subdivs[i].clone();
        pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        pts.dedup_by(|a, b| (a.0 - b.0).abs() < EPS);

        let mut prev = *from;
        for (_, vidx) in &pts {
            result_edges.push((prev, *vidx));
            prev = *vidx;
        }
        result_edges.push((prev, *to));
    }

    (verts, result_edges)
}

// ── Half-edge face enumeration (DCEL) ─────────────────────────────────────────

/// Enumerate all faces of the planar graph.  Returns each face as a CCW list
/// of vertex indices.  The outer (unbounded) face is included — callers filter
/// it by sign of area.
fn enumerate_faces(verts: &[Point2], edges: &[(usize, usize)]) -> Vec<Vec<usize>> {
    let nhe = edges.len() * 2;
    // Half-edge i*2   = edges[i].0 → edges[i].1
    // Half-edge i*2+1 = edges[i].1 → edges[i].0  (twin)

    let he_from = |h: usize| -> usize {
        if h % 2 == 0 { edges[h/2].0 } else { edges[h/2].1 }
    };
    let he_to = |h: usize| -> usize {
        if h % 2 == 0 { edges[h/2].1 } else { edges[h/2].0 }
    };
    let twin = |h: usize| -> usize { h ^ 1 };

    // For each vertex, sort outgoing half-edges by angle CCW.
    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); verts.len()];
    for h in 0..nhe {
        outgoing[he_from(h)].push(h);
    }
    for v in 0..verts.len() {
        outgoing[v].sort_by(|&a, &b| {
            let va = verts[he_to(a)];
            let vv = verts[v];
            let vb = verts[he_to(b)];
            let aa = geom::angle(geom::sub(va, vv));
            let ab = geom::angle(geom::sub(vb, vv));
            aa.partial_cmp(&ab).unwrap()
        });
    }

    // Build sorted position lookup: for vertex v and outgoing half-edge h,
    // what position is h in v's sorted list?
    let mut he_pos: Vec<usize> = vec![0; nhe];
    for v in 0..verts.len() {
        for (pos, &h) in outgoing[v].iter().enumerate() {
            he_pos[h] = pos;
        }
    }

    // next(h): go to head of h (= he_to(h) = v), look at twin(h) among v's outgoing.
    // next(h) = the outgoing edge before twin(h) in CCW order at v.
    let next = |h: usize| -> usize {
        let v = he_to(h);
        let tw = twin(h);
        let pos = he_pos[tw];
        let k = outgoing[v].len();
        outgoing[v][(pos + k - 1) % k]
    };

    // Walk all faces.
    let mut visited = vec![false; nhe];
    let mut faces = Vec::new();

    for start in 0..nhe {
        if visited[start] { continue; }
        let mut face = Vec::new();
        let mut h = start;
        loop {
            visited[h] = true;
            face.push(he_from(h));
            h = next(h);
            if h == start { break; }
            if face.len() > nhe + 1 { break; } // safety: shouldn't happen
        }
        if face.len() >= 3 {
            faces.push(face);
        }
    }

    faces
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a simple square divided into 4 equal quads by two cuts through
    /// the edge midpoints and center.
    fn simple_quad_partition() -> (Vec<Point2>, Vec<(usize, usize)>) {
        // Canonical square: 0=(0,0) 1=(1,0) 2=(1,1) 3=(0,1)
        let verts: Vec<Point2> = vec![
            [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], // canonical boundary
            [0.5, 0.0], // edge mid-bottom  (idx 4)
            [1.0, 0.5], // edge mid-right   (idx 5)
            [0.5, 1.0], // edge mid-top     (idx 6)
            [0.0, 0.5], // edge mid-left    (idx 7)
            [0.5, 0.5], // interior center  (idx 8)
        ];
        // Boundary edges (with midpoint subdivisions)
        let edges: Vec<(usize, usize)> = vec![
            (0,4),(4,1), // bottom
            (1,5),(5,2), // right
            (2,6),(6,3), // top
            (3,7),(7,0), // left
            // cuts
            (4,8),(5,8),(6,8),(7,8),
        ];
        (verts, edges)
    }

    #[test]
    fn test_face_count_quad_split() {
        let (verts, edges) = simple_quad_partition();
        let faces = enumerate_faces(&verts, &edges);
        // Should have 4 inner faces + 1 outer face = 5 total
        let inner: Vec<_> = faces.iter()
            .filter(|f| {
                let poly: Vec<Point2> = f.iter().map(|&i| verts[i]).collect();
                geom::signed_area(&poly) > 1e-9
            })
            .collect();
        assert_eq!(inner.len(), 4, "expected 4 inner faces, got {}", inner.len());
    }

    #[test]
    fn test_face_areas_sum_to_parent() {
        let (verts, edges) = simple_quad_partition();
        let faces = enumerate_faces(&verts, &edges);
        let inner_area: f64 = faces.iter()
            .map(|f| {
                let poly: Vec<Point2> = f.iter().map(|&i| verts[i]).collect();
                geom::signed_area(&poly).max(0.0)
            })
            .sum();
        assert!((inner_area - 1.0).abs() < 1e-9, "inner area sum = {inner_area}");
    }

    #[test]
    fn test_subdivide_intersections_diagonal() {
        // Two diagonals of the unit square cross at (0.5, 0.5).
        let verts: Vec<Point2> = vec![[0.0,0.0],[1.0,1.0],[1.0,0.0],[0.0,1.0]];
        let edges: Vec<(usize, usize)> = vec![(0,1),(2,3)];
        let (v2, e2) = subdivide_intersections(verts, edges);
        // Should have a new vertex at (0.5, 0.5) and 4 edges.
        assert_eq!(v2.len(), 5);
        assert_eq!(e2.len(), 4);
    }
}
