# Spec Pattern JSON Format

A spec pattern file is a JSON object that fully defines a hierarchical substitution tiling system. Load it in the GUI via **Spec → Load Spec Pattern**, or pass the path to `load_spec_pattern()`.

---

## Top-Level Fields

| Field | Type | Description |
|---|---|---|
| `tile_types` | object | Named tile type definitions |
| `canonical_vertices` | object | Named vertex lists for each tile type |
| `partitions` | object | Named partition sets per tile type |
| `rules` | object | Substitution rule (decision tree) per tile type |
| `initial_state` | `f32[]` | Starting state vector for the root tile |
| `root_tile_type` | string | Name of the tile type used as the root |
| `color_expr` | `[Expr, Expr, Expr, Expr]` | RGBA color expressions evaluated at terminal tiles |

---

## `tile_types`

Maps a tile name to its combinatorial description.

```json
"tile_types": {
  "quad": { "n": 4, "invariants": [] }
}
```

- **`n`** — number of sides (3 = triangle, 4 = quadrilateral, etc.)
- **`invariants`** — list of `f32` affine invariants that distinguish the shape class within its n-gon family. Triangles require none (all triangles are affinely equivalent). Quadrilaterals and above may carry ratios or other invariants.

---

## `canonical_vertices`

Maps each tile name to its reference polygon as a list of 2D points `[x, y]`. These are the local coordinates the partition system and transform composition work in.

```json
"canonical_vertices": {
  "quad": [[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0]]
}
```

Vertices are listed in order (typically counter-clockwise). The first vertex is the origin anchor, the second defines the first axis, and the last vertex defines the second axis — these three points determine the affine frame used to build child transforms.

---

## `partitions`

A two-level map: tile type → partition name → partition definition.

```json
"partitions": {
  "quad": {
    "my_partition": {
      "interior_vertices": [[0.5,0.0],[0.5,0.5]],
      "children": [ ... ]
    }
  }
}
```

### Partition definition

- **`interior_vertices`** — additional `[x, y]` points (in canonical coordinates) introduced by this partition. They are appended after the canonical vertices when resolving vertex indices.
- **`children`** — list of child tile specs (see below).

### Vertex index numbering

When resolving child vertex indices, the full vertex array is:

```
[ canonical_vertices[tile_type]..., interior_vertices... ]
```

So for a quad (4 canonical vertices) with 5 interior vertices, indices 0–3 are the canonical corners and indices 4–8 are the interior points.

### Child tile spec

```json
{
  "tile_type": "quad",
  "vertices": [8, 7, 0, 4],
  "state_updates": [ <Expr>, <Expr>, ... ]
}
```

- **`tile_type`** — which tile type this child is.
- **`vertices`** — vertex indices (from the combined array above) that define the child's polygon, in the same winding order as the child tile type's canonical vertices. The first vertex, second vertex, and last vertex are used to build the affine transform.
- **`state_updates`** — one expression per state slot; the full list becomes the child's state vector. Evaluated in the parent's `EvalContext` before recursion.

---

## `rules`

Maps each tile name to a **decision tree** that selects a partition name at substitution time.

### Leaf

```json
"rules": {
  "quad": { "Leaf": "my_partition" }
}
```

Always selects the named partition.

### Branch

```json
{ "Branch": {
    "condition": <Expr>,
    "if_true":   <DecisionTree>,
    "if_false":  <DecisionTree>
} }
```

Evaluates `condition` in the current tile's `EvalContext`. A non-zero result takes `if_true`; zero takes `if_false`. Trees can be nested arbitrarily.

---

## `color_expr`

Four expressions `[R, G, B, A]` evaluated when a tile becomes terminal (its scale drops below the render threshold). Each expression has access to the full `EvalContext` at that tile.

```json
"color_expr": [
  {"State": 0},
  {"State": 1},
  {"State": 2},
  {"Lit": 1.0}
]
```

Output values are clamped to `[0.0, 1.0]` and converted to `u8` for rasterization.

---

## `initial_state`

A flat `f32` array. The root tile starts with this state vector; each child's state is computed via `state_updates` in the partition.

```json
"initial_state": [0.0, 0.0, 0.0, 1.0, 1.0, 0.5]
```

The length determines the number of state slots. Expressions that access `State(i)` for `i >= len` return `0.0`.

---

## Expression Language

Expressions are JSON objects with a single key naming the operation.

### Terminals

| JSON | Description |
|---|---|
| `{"Lit": 3.14}` | Literal float |
| `"PosX"` | X coordinate of tile center (world space) |
| `"PosY"` | Y coordinate of tile center |
| `"Scale"` | Approximate tile size (square root of area) |
| `"Orientation"` | Rotation angle in radians |
| `"Shear"` | Shear component of the affine transform |
| `"Stretch"` | Anisotropic stretch component |
| `{"State": 2}` | State slot at index 2 |
| `"Random"` | Per-tile pseudo-random float in `[0, 1)` |
| `"Depth"` | Recursion depth (root = 0) |

### Arithmetic

| JSON | Result |
|---|---|
| `{"Add": [a, b]}` | `a + b` |
| `{"Sub": [a, b]}` | `a - b` |
| `{"Mul": [a, b]}` | `a * b` |
| `{"Div": [a, b]}` | `a / b` (returns 0 if `b ≈ 0`) |
| `{"Neg": a}` | `-a` |

### Math functions

| JSON | Result |
|---|---|
| `{"Sin": a}` | `sin(a)` |
| `{"Cos": a}` | `cos(a)` |
| `{"Exp": a}` | `eᵃ` |
| `{"Sqrt": a}` | `√max(a, 0)` |
| `{"Abs": a}` | `|a|` |
| `{"Log": a}` | `ln(a)` (returns 0 if `a ≤ 0`) |
| `{"Clamp": [val, lo, hi]}` | `clamp(val, lo, hi)` |
| `{"Mix": [t, a, b]}` | `a + (b - a) * t` (linear interpolation) |

### Logic / comparison

Boolean values are `1.0` (true) and `0.0` (false).

| JSON | Result |
|---|---|
| `{"Lt": [a, b]}` | `1.0` if `a < b`, else `0.0` |
| `{"Gt": [a, b]}` | `1.0` if `a > b`, else `0.0` |
| `{"And": [a, b]}` | `1.0` if both non-zero |
| `{"Or": [a, b]}` | `1.0` if either non-zero |
| `{"Not": a}` | `1.0` if `a == 0.0`, else `0.0` |
| `{"If": [cond, then, else]}` | `then` if `cond != 0.0`, else `else` |

---

## Render Parameters

These are not stored in the spec file — they are set in the GUI or passed at render time:

- **Threshold** — recursion stops when a tile's scale falls below this value. Lower = finer detail, higher = coarser/faster. GUI default: `2.0`.
- **Output size** — pixel dimensions of the square output image (128, 256, 512, 1024).

---

## Example: `quilt_new.spec.json`

A single `quad` tile type that always applies `quilt_split`, dividing each square into four rotated children via a center point and four edge midpoints. Each child carries a target grayscale value in `state[0..2]` that is mixed toward a fixed color with each generation. The blend rate is controlled by `state[3]` and decays by a factor stored in `state[4]` and `state[5]`.

The color expression reads `state[0..2]` directly as RGB, producing a self-similar grayscale quilt pattern.
