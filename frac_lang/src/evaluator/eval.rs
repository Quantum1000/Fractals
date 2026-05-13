/// Tile expansion: recursively expand a tile hierarchy and collect leaf tiles.

use std::collections::HashMap;
use crate::ast::*;
use super::{Value, RenderTile, EvalConfig};
use super::expr::{self, EvalContext};
use crate::normalizer::{NormalizedFile, NormalizedPartition};
use crate::normalizer::geom::{self, Point2};

/// Expand a single tile instance and collect all leaf `RenderTile`s into `out`.
pub fn expand(
    tile_type: &str,
    transform: &[f64; 6],          // maps canonical → world space
    state: HashMap<String, Option<Value>>,
    depth: u32,
    nf: &NormalizedFile,
    cfg: &EvalConfig,
    functions: &HashMap<String, (Vec<String>, Spanned<Expr>)>,
    out: &mut Vec<RenderTile>,
) {
    // Compute the world-space polygon for this tile instance.
    let _tile_data = match nf.tiles.get(tile_type) {
        Some(t) => t,
        None => return,
    };
    let canonical: Vec<Point2> = get_canonical(nf, tile_type);
    let polygon: Vec<Point2> = canonical.iter()
        .map(|&p| geom::apply_transform(transform, p))
        .collect();

    // Decompose the transform into geometric parameters.
    let (pos, scale, orientation, shear, stretch) = decompose_transform(transform, &polygon);
    let random = tile_random(transform, depth);

    let ctx = EvalContext {
        state: &state,
        pos,
        scale,
        orientation,
        shear,
        stretch,
        depth,
        random,
        child_index: None,
        child_orientation: None,
        functions,
    };

    // At max depth or if no pattern rule matches, emit this tile as a leaf.
    let pattern = match nf.file.items.iter().find_map(|i| {
        if let Item::Pattern(p) = &i.node { Some(p) } else { None }
    }) {
        Some(p) => p,
        None => return,
    };

    let should_stop = depth >= cfg.max_depth
        || cfg.min_size.map_or(false, |ms| scale < ms);
    if should_stop {
        if let Some(color) = eval_color(pattern, &ctx) {
            out.push(RenderTile { polygon, color, depth });
        }
        return;
    }

    // Find a matching rule.
    let rule = pattern.rules.iter().find(|r| r.node.tile.0.node == tile_type);
    let rule = match rule {
        Some(r) => r,
        None => {
            // No rule — emit as leaf.
            if let Some(color) = eval_color(pattern, &ctx) {
                out.push(RenderTile { polygon, color, depth });
            }
            return;
        }
    };

    // Walk the decision tree to find the substitution.
    let sub = match find_substitution(&rule.node.body, &ctx) {
        Some(s) => s,
        None => {
            if let Some(color) = eval_color(pattern, &ctx) {
                out.push(RenderTile { polygon, color, depth });
            }
            return;
        }
    };

    // Apply tile-level state updates sequentially.
    let mut updated_state = state.clone();
    for upd in &sub.updates {
        let val = expr::eval(&upd.node.value.node, &ctx);
        updated_state.insert(upd.node.var.0.node.clone(), val);
    }

    // Compute updated context for per-child evaluation.
    let updated_ctx = EvalContext {
        state: &updated_state,
        ..ctx
    };

    // Look up the partition.
    let part_key = (sub.tile.0.node.clone(), sub.partition.0.node.clone());
    let part = match nf.partitions.get(&part_key) {
        Some(p) => p,
        None => return,
    };

    // Resolve slot_order permutation (if any).
    let slot_perm = resolve_slot_order(sub, part, &updated_ctx, nf);

    // Expand each child.
    for (slot_idx, _child) in part.children.iter().enumerate() {
        // Apply slot permutation: slot_perm[slot_idx] is the canonical child index
        // that goes into this slot.
        let canonical_child_idx = if slot_idx < slot_perm.len() {
            slot_perm[slot_idx] as usize
        } else {
            slot_idx
        };
        let child = &part.children[canonical_child_idx.min(part.children.len() - 1)];

        // child.similarity_transform maps child canonical → parent canonical space.
        // We compose with `transform` (parent canonical → world) to get child world transform.
        let child_transform = geom::compose_transforms(transform, &child.similarity_transform);

        // Build child state: inherit from parent, then apply per-child injection.
        let mut child_state = updated_state.clone();

        // Add child context variables.
        let child_orientation_val = Some(Value::Float(0.0)); // placeholder
        let child_ctx_state = child_state.clone();

        // Find injection for this child (by name or index).
        let injection = sub.child_injections.iter().find(|inj| {
            match &inj.node.child.node {
                ChildRef::Index(i) => *i == child.index,
                ChildRef::Name(n) => child.name.as_deref() == Some(&n.0.node),
            }
        });

        let mut effective_child_transform = child_transform;

        if let Some(inj) = injection {
            let inj_ctx = EvalContext {
                state: &child_ctx_state,
                child_index: Some(slot_idx as u32),
                child_orientation: child_orientation_val.clone(),
                ..updated_ctx
            };
            for upd in &inj.node.updates {
                let val = expr::eval(&upd.node.value.node, &inj_ctx);
                child_state.insert(upd.node.var.0.node.clone(), val);
            }
            // Apply alignment: remap the child's canonical vertices by the group
            // element's vertex permutation, then fit an affine from canonical→permuted.
            if let Some(align_expr) = &inj.node.alignment {
                if let Some(Value::GroupElem { group: _, element }) = expr::eval(&align_expr.node, &inj_ctx) {
                    let child_canonical = get_canonical(nf, &child.tile_type);
                    if let Some(rot) = alignment_transform(&child.tile_type, &element, &child_canonical, nf) {
                        effective_child_transform = geom::compose_transforms(&child_transform, &rot);
                    }
                }
            }
        }

        expand(
            &child.tile_type,
            &effective_child_transform,
            child_state,
            depth + 1,
            nf,
            cfg,
            functions,
            out,
        );
    }
}

// ── Decision tree traversal ───────────────────────────────────────────────────

fn find_substitution<'a>(
    body: &'a Spanned<RuleBody>,
    ctx: &EvalContext,
) -> Option<&'a Substitution> {
    match &body.node {
        RuleBody::Substitute(sub) => Some(&sub.node),
        RuleBody::If { condition, then_branch, else_branch } => {
            let cond = expr::eval(&condition.node, ctx)?;
            match cond {
                Value::Bool(true)  => find_substitution(then_branch, ctx),
                Value::Bool(false) => find_substitution(else_branch, ctx),
                Value::Float(f) if f != 0.0 => find_substitution(then_branch, ctx),
                _ => find_substitution(else_branch, ctx),
            }
        }
    }
}

// ── Color evaluation ──────────────────────────────────────────────────────────

fn eval_color(pattern: &PatternDecl, ctx: &EvalContext) -> Option<[f64; 4]> {
    let r = expr::eval_as_float(&pattern.color.node.r.node, ctx)?;
    let g = expr::eval_as_float(&pattern.color.node.g.node, ctx)?;
    let b = expr::eval_as_float(&pattern.color.node.b.node, ctx)?;
    let a = expr::eval_as_float(&pattern.color.node.a.node, ctx)?;
    Some([r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0), a.clamp(0.0, 1.0)])
}

// ── Slot permutation ──────────────────────────────────────────────────────────

fn resolve_slot_order(
    sub: &Substitution,
    part: &NormalizedPartition,
    ctx: &EvalContext,
    nf: &NormalizedFile,
) -> Vec<u32> {
    let n = part.children.len();
    // Default: identity permutation.
    let mut perm: Vec<u32> = (0..n as u32).collect();

    for so in &sub.slot_orders {
        if let Some(v) = expr::eval(&so.node.value.node, ctx) {
            match v {
                Value::Perm(p) => {
                    if p.len() == n { perm = p; }
                }
                Value::GroupElem { group: _, ref element } => {
                    // Resolve group element to a permutation via the tile's group_perm map.
                    if let Some(tile_data) = nf.tiles.values().find(|_| true) {
                        for entry in &tile_data.group_perm {
                            if &entry.name == element {
                                perm = entry.perm.iter().map(|&i| i as u32).collect();
                                break;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    perm
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn get_canonical(nf: &NormalizedFile, tile_type: &str) -> Vec<Point2> {
    for item in &nf.file.items {
        if let Item::Tile(t) = &item.node {
            if t.name.0.node == tile_type {
                return t.canonical.iter().map(|p| [p.node.x, p.node.y]).collect();
            }
        }
    }
    vec![]
}

/// Decompose an affine transform into human-readable parameters.
/// Returns (centroid_world, scale, orientation_degrees, shear, stretch).
fn decompose_transform(t: &[f64; 6], polygon: &[Point2]) -> ([f64; 2], f64, f64, f64, f64) {
    let pos = geom::centroid(polygon);
    // Extract scale from the transform columns.
    let col0_len = (t[0]*t[0] + t[3]*t[3]).sqrt();
    let col1_len = (t[1]*t[1] + t[4]*t[4]).sqrt();
    let scale = (col0_len * col1_len).sqrt();
    let orientation = t[3].atan2(t[0]).to_degrees();
    let shear = 0.0;   // simplified
    let stretch = if col0_len > 1e-15 { col1_len / col0_len } else { 1.0 };
    (pos, scale, orientation, shear, stretch)
}

/// Build the affine transform for an alignment group element by looking up its
/// vertex permutation and fitting affine canonical → permuted(canonical).
fn alignment_transform(
    tile_type: &str,
    element: &str,
    canonical: &[Point2],
    nf: &NormalizedFile,
) -> Option<[f64; 6]> {
    let tile_data = nf.tiles.get(tile_type)?;
    let entry = tile_data.group_perm.iter().find(|e| e.name == element)?;
    let perm = &entry.perm;
    let n = canonical.len();
    if n < 3 || perm.len() != n { return None; }
    // src[i] = canonical[i], dst[i] = canonical[perm[i]]
    let src = [canonical[0], canonical[1], canonical[2]];
    let dst = [canonical[perm[0]], canonical[perm[1]], canonical[perm[2]]];
    geom::fit_affine_3(src, dst)
}

/// Deterministic pseudo-random value for a tile, based on its transform + depth.
fn tile_random(t: &[f64; 6], depth: u32) -> f64 {
    // Simple hash of transform translation and depth.
    let bits = (t[2].to_bits() ^ t[5].to_bits()).wrapping_mul(6364136223846793005)
        .wrapping_add((depth as u64).wrapping_mul(1442695040888963407));
    (bits >> 11) as f64 / (1u64 << 53) as f64
}
