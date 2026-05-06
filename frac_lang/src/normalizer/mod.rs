/// Normalizer: steps 2–8 of the `.frac` normalization pipeline.
///
/// Entry point: `normalize(file) -> Result<NormalizedFile, Vec<NormalizeError>>`

use std::collections::HashMap;
use crate::ast::{File, Item, ParseError, Span, SymmetryGroup};

pub mod geom;
mod symmetry;
mod topology;
mod typecheck;
mod acyclicity;
mod group_perm;
mod canonicalize;

pub use geom::Point2;

// ── Output types ──────────────────────────────────────────────────────────────

/// All derived data for a single tile type.
pub struct NormalizedTile {
    /// Inferred or declared symmetry group.
    pub symmetry: SymmetryGroup,
    /// Side-length ratio invariants for the canonical polygon.
    pub invariants: Vec<f64>,
    /// Group-element ↔ vertex-permutation map (step 7).
    pub group_perm: Vec<GroupPermEntry>,
}

pub struct GroupPermEntry {
    /// Human-readable element name, e.g. `"r90"`, `"fh"`.
    pub name: String,
    /// `perm[i]` = the index of original canonical vertex i after applying
    /// this group element.
    pub perm: Vec<usize>,
}

/// All derived data for a single partition.
pub struct NormalizedPartition {
    pub children: Vec<ChildInfo>,
}

/// Derived data for one child slot in a partition.
pub struct ChildInfo {
    /// Name of the tile type this child matches.
    pub tile_type: String,
    /// Side-length ratio invariants of the child polygon.
    pub invariants: Vec<f64>,
    /// Polygon vertices in CCW order, in the parent tile's coordinate space.
    pub polygon: Vec<Point2>,
    /// Affine transform [a,b,c,d,e,f] mapping the child tile's canonical polygon
    /// to its actual position inside the parent.
    pub similarity_transform: [f64; 6],
    /// Index of the polygon vertex that corresponds to canonical vertex 0 of the
    /// child tile type (the "anchor vertex").
    pub anchor_vertex: usize,
    /// Normalizer-assigned canonical index (stable across re-parses).
    pub index: u32,
    /// Author-assigned name, if provided via `child N = name`.
    pub name: Option<String>,
}

/// The complete output of a successful normalization pass.
pub struct NormalizedFile {
    /// Canonicalized (reformatted) source file.
    pub file: File,
    /// Per-tile derived data.
    pub tiles: HashMap<String, NormalizedTile>,
    /// Per-partition derived data.
    pub partitions: HashMap<(String, String), NormalizedPartition>,
    /// Function names in topological order (callees before callers).
    pub function_order: Vec<String>,
}

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum NormalizeError {
    ParseErrors(Vec<ParseError>),
    DuplicateTile { name: String, span: Span },
    DuplicatePartition { tile: String, name: String, span: Span },
    UnknownTile { name: String, span: Span },
    UnknownPartition { tile: String, name: String, span: Span },
    InconsistentSymmetry {
        tile: String,
        declared: SymmetryGroup,
        inferred: SymmetryGroup,
        span: Span,
    },
    NoTileMatch { partition: (String, String), child_index: usize },
    ChildNameOutOfRange {
        partition: (String, String),
        declared_index: u32,
        child_count: usize,
        span: Span,
    },
    TypeMismatch { expected: String, found: String, span: Span },
    NullableColor { component: &'static str, span: Span },
    RecursiveFunction { cycle: Vec<String> },
    UnknownFunction { name: String, span: Span },
    UnknownVariable { name: String, span: Span },
    InvalidSlotOrder { span: Span, reason: String },
    WrongArgCount { func: String, expected: usize, found: usize, span: Span },
    InvalidPartitionVertex {
        partition: (String, String),
        name: String,
        reason: String,
        span: Span,
    },
}

impl std::fmt::Display for NormalizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseErrors(errs) => {
                for e in errs { write!(f, "parse error: {}", e.message)?; }
                Ok(())
            }
            Self::DuplicateTile { name, .. } => write!(f, "duplicate tile '{name}'"),
            Self::DuplicatePartition { tile, name, .. } => write!(f, "duplicate partition '{tile}.{name}'"),
            Self::UnknownTile { name, .. } => write!(f, "unknown tile '{name}'"),
            Self::UnknownPartition { tile, name, .. } => write!(f, "unknown partition '{tile}.{name}'"),
            Self::InconsistentSymmetry { tile, declared, inferred, .. } =>
                write!(f, "tile '{tile}': declared symmetry {declared:?} is inconsistent with inferred {inferred:?}"),
            Self::NoTileMatch { partition, child_index } =>
                write!(f, "partition '{}.{}' child {child_index}: no tile type matches this polygon", partition.0, partition.1),
            Self::ChildNameOutOfRange { partition, declared_index, child_count, .. } =>
                write!(f, "partition '{}.{}': child index {declared_index} out of range (partition has {child_count} children)", partition.0, partition.1),
            Self::TypeMismatch { expected, found, .. } =>
                write!(f, "type mismatch: expected {expected}, found {found}"),
            Self::NullableColor { component, .. } =>
                write!(f, "color component '{component}' may be `none`"),
            Self::RecursiveFunction { cycle } =>
                write!(f, "recursive functions: {:?}", cycle),
            Self::UnknownFunction { name, .. } => write!(f, "unknown function '{name}'"),
            Self::UnknownVariable { name, .. } => write!(f, "unknown variable '{name}'"),
            Self::InvalidSlotOrder { reason, .. } => write!(f, "invalid slot_order: {reason}"),
            Self::WrongArgCount { func, expected, found, .. } =>
                write!(f, "function '{func}': expected {expected} args, found {found}"),
            Self::InvalidPartitionVertex { partition, name, reason, .. } =>
                write!(f, "partition '{}.{}' vertex '{name}': {reason}", partition.0, partition.1),
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Normalize a parsed `.frac` file.
///
/// On success returns a `NormalizedFile` with all derived data.
/// On failure returns all errors encountered (the normalizer continues through
/// errors where possible to report as many issues as it can in one pass).
pub fn normalize(file: File) -> Result<NormalizedFile, Vec<NormalizeError>> {
    let mut errors: Vec<NormalizeError> = Vec::new();

    // Collect parse errors embedded in the AST.
    for item in &file.items {
        if let Item::Error(e) = &item.node {
            errors.push(NormalizeError::ParseErrors(vec![e.clone()]));
        }
    }

    // Check for duplicate tile / partition names.
    check_duplicates(&file, &mut errors);

    // Step 2: infer / validate symmetries.
    let tile_symmetries = symmetry::infer_symmetries(&file, &mut errors);

    // Build per-tile canonical polygons and invariants (used by topology).
    let mut tile_canonicals: HashMap<String, Vec<Point2>> = HashMap::new();
    let mut tile_invariants: HashMap<String, Vec<f64>> = HashMap::new();
    for item in &file.items {
        if let Item::Tile(t) = &item.node {
            let pts: Vec<Point2> = t.canonical.iter()
                .map(|p| [p.node.x, p.node.y])
                .collect();
            let invs = geom::side_length_ratios(&pts);
            tile_canonicals.insert(t.name.0.node.clone(), pts);
            tile_invariants.insert(t.name.0.node.clone(), invs);
        }
    }

    // Step 3: compute partition topology.
    let partitions = topology::compute_partitions(
        &file,
        &tile_invariants,
        &tile_canonicals,
        &mut errors,
    );

    // Step 6: function acyclicity.
    let function_order = acyclicity::check_acyclicity(&file, &mut errors);

    // Step 5: type-check expressions.
    typecheck::check(&file, &partitions, &mut errors);

    // Step 7: group ↔ perm mappings.
    let tile_gp = group_perm::compute(&file, &tile_symmetries, &mut errors);

    // Step 8: canonicalize (sort items, round coords, reformat).
    let canonical_file = canonicalize::canonicalize(file);

    if !errors.is_empty() {
        return Err(errors);
    }

    // Assemble NormalizedFile.
    let mut tiles = HashMap::new();
    for (name, sym) in tile_symmetries {
        let invs = tile_invariants.get(&name).cloned().unwrap_or_default();
        let gp   = tile_gp.get(&name).map(|v| v.iter().map(|e| GroupPermEntry {
            name: e.name.clone(),
            perm: e.perm.clone(),
        }).collect()).unwrap_or_default();
        tiles.insert(name, NormalizedTile { symmetry: sym, invariants: invs, group_perm: gp });
    }

    Ok(NormalizedFile {
        file: canonical_file,
        tiles,
        partitions,
        function_order,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn check_duplicates(file: &File, errors: &mut Vec<NormalizeError>) {
    let mut seen_tiles: HashMap<String, Span> = HashMap::new();
    let mut seen_parts: HashMap<(String, String), Span> = HashMap::new();

    for item in &file.items {
        match &item.node {
            Item::Tile(t) => {
                let name = t.name.0.node.clone();
                let span = t.name.0.span;
                if let Some(prev) = seen_tiles.insert(name.clone(), span) {
                    errors.push(NormalizeError::DuplicateTile { name, span });
                    let _ = prev;
                }
            }
            Item::Partition(p) => {
                let key = (p.tile.0.node.clone(), p.name.0.node.clone());
                let span = p.name.0.span;
                if seen_parts.insert(key.clone(), span).is_some() {
                    errors.push(NormalizeError::DuplicatePartition {
                        tile: key.0,
                        name: key.1,
                        span,
                    });
                }
            }
            _ => {}
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn test_normalize_empty_file() {
        let (file, _) = parse("");
        assert!(normalize(file).is_ok());
    }

    #[test]
    fn test_normalize_tile_only() {
        let src = "tile quad { sides 4 canonical (0,0) (1,0) (1,1) (0,1) symmetry d4 }";
        let (file, _) = parse(src);
        let result = normalize(file);
        assert!(result.is_ok(), "{:?}", result.err());
        let nf = result.unwrap();
        assert!(nf.tiles.contains_key("quad"));
        assert_eq!(nf.tiles["quad"].symmetry, SymmetryGroup::Dihedral(4));
    }

    #[test]
    fn test_duplicate_tile_error() {
        let src = "tile a { sides 3 canonical (0,0) (1,0) (0.5,1) }\ntile a { sides 3 canonical (0,0) (1,0) (0.5,1) }";
        let (file, _) = parse(src);
        let result = normalize(file);
        assert!(result.is_err());
        let errs = result.err().unwrap();
        assert!(errs.iter().any(|e| matches!(e, NormalizeError::DuplicateTile { .. })));
    }
}
