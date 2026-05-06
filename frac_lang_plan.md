# Plan: Complete the frac_lang Language

## Context

The `frac_lang` crate already has a lexer, parser, AST, and printer. The spec describes an 8-step normalization pipeline plus a rendering evaluator. `main.rs` has a working egui GUI with a JSON-based spec renderer. The goal is to add:
1. A normalizer (steps 2–8 from the spec)
2. An evaluator/interpreter (produces a renderable tile tree)
3. Integration into `main.rs` as a new "Frac Mode"

---

## Phase 1: Normalizer (`frac_lang/src/normalizer/`)

Create `frac_lang/src/normalizer/mod.rs` exposing:
```rust
pub fn normalize(file: &mut File) -> Result<NormalizedFile, Vec<NormalizeError>>;
```

`NormalizedFile` augments the AST with computed data (child polygons, types, group mappings).

### Step 2: Symmetry validation (`symmetry.rs`)

- For each `TileDecl`, verify declared symmetry is consistent with canonical vertex geometry.
- Infer maximal symmetry when `symmetry` is absent: check if rotating/reflecting canonical vertices maps the polygon to itself.
- Emit error if declared symmetry is inconsistent.

### Step 3: Partition topology + affine invariants (`topology.rs`) — most complex step

Input: named vertices (with 2D coords), cuts (pairs of `VertexRef`), parent canonical polygon.

Algorithm:
1. Build a planar graph: nodes = named vertices + canonical boundary vertices. Edges = cuts + boundary edges of parent polygon.
2. Handle edge intersections: compute intersection points, subdivide edges.
3. Enumerate faces using a half-edge traversal (for each directed edge, rotate CCW around origin vertex to find next edge in face).
4. Discard the outer (unbounded) face.
5. For each inner face (child polygon):
   - Compute vertices in CCW order.
   - Compute **affine invariants** (side-length ratios of consecutive edges) — shape descriptors preserved under similarity transforms.
   - Match to a declared `TileDecl` by comparing invariants against each tile type's precomputed invariants (computed once from `canonical` vertices).
   - Record the similarity transform (position, scale, rotation, reflection) mapping the child's actual polygon to the tile type's canonical polygon.
   - Record the anchor vertex (canonical vertex 0 → which boundary vertex).
6. Assign child indices (order of faces, e.g., by centroid position or declaration order).
7. Validate `child_names` map against computed child count.

Output: `Vec<ChildInfo>` per partition, where `ChildInfo` = `{ tile_type, invariants, polygon, similarity_transform, anchor_vertex, index, name? }`.

Geometric utilities needed (add to `normalizer/geom.rs`):
- Segment intersection
- CCW polygon winding test
- Polygon area (shoelace)
- Side-length ratio sequence computation (affine invariants)
- Similarity transform fitting (find scale, rotation, reflection mapping one CCW polygon to another of the same type)

### Step 4: Topology correctness tests (not a runtime validation step)

The planar face enumeration algorithm is correct by construction — a valid input (non-degenerate vertices, non-crossing cuts except at endpoints) produces a valid partition. This step is replaced by:
- Unit tests on the topology algorithm using hand-crafted vertex/cut sets.
- A lightweight sanity check in debug builds: assert that child polygon areas sum to parent area (within float tolerance), to catch bugs in the topology code during development.

### Step 5: Type-checking + nullability inference (`typecheck.rs`)

Types: `Float | Bool | GroupElem(SymmetryGroup) | Perm(n)`.
Each expression has `(Type, Nullable)`.

Rules:
- Literals: concrete type, not nullable.
- `Var(v)`: type from state decl, always nullable (variables are `Option<T>`).
- Arithmetic on `none` → `none` (propagate nullability).
- `?` (null coalesce): right side must match type; result is `Nullable::No` if right side is non-null.
- Color expression: all 4 components must be `Float`; error if nullable.
- `slot_order` / `alignment` exprs must be `Perm` or `GroupElem`.
- **Slot order validity**: for each `slot_order` in a substitution, verify the referenced permutation maps each child slot to another slot with matching affine invariants (same tile type and same shape up to the tile's symmetry group). For uniform-type partitions, the valid permutations are exactly those in the image of the symmetry group's action on child slots. Uses `ChildInfo.invariants` from topology. Emit `NormalizeError::InvalidSlotOrder` otherwise.
- State update exprs must match variable type.
- Child injection exprs must match target variable type.

Walk each expression bottom-up, threading an environment (var name → `(Type, Nullable)`).

### Step 6: Function acyclicity (`acyclicity.rs`)

- Build a directed call graph: `FuncRef → Set<FuncRef>` by walking each function body for `Call` nodes.
- Topological sort (Kahn's algorithm).
- Emit `NormalizeError::RecursiveFunction` for any cycle, listing the cycle.

### Step 7: Group ↔ perm mappings (`group_perm.rs`)

For each tile type with a symmetry group G of order k:
- Enumerate all group elements (e.g., for D4: identity, r90, r180, r270, s0, s45, s90, s135).
- For each element, compute its action on the canonical vertices → produces a permutation of vertex indices.
- Store bidirectional map: `GroupElem → Perm` and `Perm → GroupElem`.
- Validate that group elements used in `slot_order`/`alignment` exprs are from the correct group.

For cyclic group C_n: rotations only (k = n elements).
For dihedral group D_n: rotations + reflections (k = 2n elements).

### Step 8: Canonicalization (`canonicalize.rs`)

No computed data is written back to the source file. Inferred symmetries and affine invariants live only in `NormalizedFile` and are surfaced via LSP hover (future work). Canonicalization is limited to:
- Sorting items: tiles first, then partitions (by tile name, then partition name), then functions, then patterns.
- Normalizing float precision in vertex coordinates (round to 6 decimal places).
- Assigning child indices explicitly if missing.
- Rewriting source file via the printer (formatting only — no semantic additions).

---

## Phase 2: Evaluator (`frac_lang/src/evaluator/`)

Expose:
```rust
pub fn evaluate(
    nf: &NormalizedFile,
    bounds: Rect,        // bounding box in world space
    max_depth: u32,
    rng_seed: u64,
) -> RenderTree;
```

### `RenderTree`

```rust
pub struct RenderTree {
    pub tiles: Vec<RenderTile>,
}
pub struct RenderTile {
    pub polygon: Vec<[f64; 2]>,   // world-space vertices
    pub color: [f64; 4],           // RGBA 0.0–1.0
    pub depth: u32,
}
```

Leaf tiles (at max_depth or no matching rule) produce entries in `tiles`.

### Evaluation algorithm (`eval.rs`)

```
fn expand(tile_type, transform, state, depth, pattern, nf) -> Vec<RenderTile>:
  if depth == max_depth or no rule for tile_type:
    color = eval_expr(pattern.color, context)
    return [RenderTile { polygon, color, depth }]
  rule = find_rule(pattern, tile_type, state, context)  // walk if/else
  sub = rule.substitution
  apply state updates sequentially
  for each child in sub.partition.children (from NormalizedFile):
    child_transform = transform ∘ child.local_transform
    child_state = inherit(state) + apply injections for this child
    apply slot_order permutation (reorder children)
    apply alignment (adjust child orientation)
    recurse: expand(child.tile_type, child_transform, child_state, depth+1, ...)
```

### Expression evaluator (`expr.rs`)

Reuse logic from `main.rs`'s existing evaluator (lines 963–1010) but adapted for the AST types. Context includes:
- `pos`: centroid of current tile polygon
- `scale`, `orientation`, `shear`, `stretch`: from transform
- `depth`: current recursion depth
- `random`: seeded per tile (hash of transform + depth)
- `child.canonical_orientation`, `child.index`: when evaluating child expressions

State: `HashMap<VarRef, Option<Value>>` where `Value = Float(f64) | GroupElem(...) | Perm(Vec<u32>)`.

---

## Phase 3: main.rs Integration

### Cargo.toml changes

Add to `[dependencies]`:
```toml
frac_lang = { path = "frac_lang" }
```

### New "Frac Mode"

Add a third mode alongside Classic and Spec. App state additions:
```rust
frac_source: Option<String>,          // raw .frac source text
frac_normalized: Option<NormalizedFile>,
frac_errors: Vec<NormalizeError>,
frac_texture: Option<egui::TextureHandle>,
frac_max_depth: u32,                  // slider, default 6
frac_rng_seed: u64,
```

UI:
- "Load .frac file" button → `rfd::FileDialog`, reads file, runs `frac_lang::parse` then `frac_lang::normalize`, stores result.
- Display parse/normalize errors in a scrollable panel.
- Depth slider (1–12).
- On parameter change: run `evaluate()`, rasterize `RenderTree` into pixel buffer (same rasterizer as existing spec mode), upload as egui texture.
- Pan/zoom reuses existing logic.

### Rasterization

The existing spec mode rasterizer in `main.rs` renders polygons by scanline fill. Reuse it: `RenderTile` has a polygon and RGBA color, which maps directly to the existing `render_polygon()` / `fill_pixels()` pattern.

---

## File Structure

```
frac_lang/src/
  lib.rs                    (add pub mod normalizer, pub mod evaluator)
  ast.rs                    (unchanged)
  lexer.rs                  (unchanged)
  parser.rs                 (unchanged)
  printer.rs                (unchanged)
  normalizer/
    mod.rs                  (normalize() entry point, NormalizedFile, NormalizeError)
    symmetry.rs             (step 2)
    geom.rs                 (shared geometry utilities)
    topology.rs             (step 3 — planar graph, face enumeration)
    tiling.rs               (step 4 — coverage validation)
    typecheck.rs            (step 5)
    acyclicity.rs           (step 6)
    group_perm.rs           (step 7)
    canonicalize.rs         (step 8)
  evaluator/
    mod.rs                  (evaluate() entry point, RenderTree, RenderTile)
    expr.rs                 (expression evaluator)
    eval.rs                 (tile expansion recursion)
```

---

## Verification

1. **Unit tests per normalizer step**: test each step with small hand-crafted ASTs.
2. **End-to-end normalization test**: parse a simple `.frac` file (e.g., a triangle tiling), normalize it, check no errors and correct child count.
3. **Evaluator test**: evaluate a trivial pattern (single tile, no substitution) and check output polygon and color.
4. **main.rs smoke test**: load a `.frac` file through the GUI, verify it renders without panicking.
5. **Round-trip**: parse → normalize (step 8 rewrites) → parse again → normalize → check idempotent.

---

## Open Question from Spec

The spec defers the precise interaction between `slot_order` and `child.canonical_orientation` in the same rule. For now: `slot_order` permutes which child goes to which slot (pre-permutation), and `child.canonical_orientation` is evaluated post-permutation (i.e., reflects the child's orientation after slot assignment). This matches the spec's tentative description and can be revised once prototyping clarifies it.
