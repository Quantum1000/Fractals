/// Evaluator: expand a normalized pattern into a flat list of colored polygons.

use std::collections::HashMap;
use crate::ast::{Expr, Item, Spanned};
use crate::normalizer::{NormalizedFile, geom::Point2, geom};

pub mod expr;
mod eval;

// ── Public types ──────────────────────────────────────────────────────────────

/// A single leaf tile ready for rasterization.
#[derive(Debug, Clone)]
pub struct RenderTile {
    /// World-space vertices in CCW order.
    pub polygon: Vec<Point2>,
    /// RGBA color, each component in \[0.0, 1.0\].
    pub color: [f64; 4],
    /// Recursion depth at which this tile was produced (root = 0).
    pub depth: u32,
}

/// The flat collection of leaf tiles produced by `evaluate`.
pub struct RenderTree {
    pub tiles: Vec<RenderTile>,
}

/// Configuration for the evaluator.
pub struct EvalConfig {
    /// Stop expanding at this depth and emit the tile as a leaf.
    pub max_depth: u32,
    /// Seed for the per-tile deterministic RNG.
    pub rng_seed: u64,
    /// Stop expanding when the tile's world-space scale falls below this value.
    /// `None` means no size cutoff (depth-only stopping).
    pub min_size: Option<f64>,
}

/// A runtime value in the evaluator.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Float(f64),
    Bool(bool),
    Perm(Vec<u32>),
    GroupElem { group: String, element: String },
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Evaluate a normalized pattern and return all leaf tiles.
///
/// The root tile is placed with identity transform (its canonical polygon is
/// in world space as-is).  Callers that want a different world-space placement
/// should transform the output polygons afterward.
pub fn evaluate(nf: &NormalizedFile, cfg: &EvalConfig) -> RenderTree {
    // Find the pattern declaration.
    let pattern = match nf.file.items.iter().find_map(|i| {
        if let Item::Pattern(p) = &i.node { Some(p) } else { None }
    }) {
        Some(p) => p,
        None => return RenderTree { tiles: vec![] },
    };

    let root_tile = pattern.root.0.node.clone();

    // Build the initial state: variables with initial values set, others None.
    let mut initial_state: HashMap<String, Option<Value>> = HashMap::new();
    for sv in &pattern.state.node.vars {
        let val = sv.node.initial.as_ref().and_then(|init| {
            let ctx = expr::EvalContext {
                state: &HashMap::new(),
                pos: [0.0, 0.0],
                scale: 1.0,
                orientation: 0.0,
                shear: 0.0,
                stretch: 1.0,
                depth: 0,
                random: 0.0,
                child_index: None,
                child_orientation: None,
                functions: &HashMap::new(),
            };
            expr::eval(&init.node, &ctx)
        });
        initial_state.insert(sv.node.name.0.node.clone(), val);
    }

    // Build function table.
    let functions = build_function_table(nf);

    // Start with identity transform.
    let identity = crate::normalizer::geom::IDENTITY_TRANSFORM;

    let mut tiles = Vec::new();
    eval::expand(
        &root_tile,
        &identity,
        initial_state,
        0,
        nf,
        cfg,
        &functions,
        &mut tiles,
    );

    RenderTree { tiles }
}

fn build_function_table(
    nf: &NormalizedFile,
) -> HashMap<String, (Vec<String>, Spanned<Expr>)> {
    let mut map = HashMap::new();
    for item in &nf.file.items {
        if let Item::Function(f) = &item.node {
            let params: Vec<String> = f.params.iter()
                .map(|p| p.0.node.clone())
                .collect();
            map.insert(f.name.0.node.clone(), (params, f.body.clone()));
        }
    }
    map
}
