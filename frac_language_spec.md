# Language Specification: `.frac` Format

A text-first language for hierarchical substitution tilings. Source files
are *normalized in-place* before any rendering or evaluation: the
normalizer is a hard gate, and downstream tools consume a `NormalizedFile`
rather than the raw AST.

**File extension**: `.frac`

---

## 1. Lexical Conventions

```
# Line comments run to end-of-line
Identifiers:        [a-zA-Z_][a-zA-Z0-9_]*
Tile.partition:     tile_name.partition_name        # partition declarations
Tile.class:         tile_name.class                 # affine-class reference
Group.element:      group_name.element_name         # group-element literals
Child.field:        child.field                     # geometric child access
Numbers:            standard float / integer literals
```

Whitespace is non-significant except as a token separator.

---

## 2. Top-Level Items

A file contains zero or more items in any order:

1. `tile` — declare a tile shape
2. `partition` — declare a subdivision of a tile (or tile class)
3. `fn` — declare a pure helper function
4. `pattern` — declare the root state machine (at most one per file)

The normalizer sorts items into a canonical order on save (tile, partition,
function, pattern). No semantic data is written back into the source.

---

## 3. Tiles

```frac
tile quad {
    sides 4
    canonical (0,0) (1,0) (1,1) (0,1)
    symmetry d4         # optional
}
```

### 3.1 Fields

- **`sides N`** — must equal the number of vertices in `canonical`.
- **`canonical (x,y) (x,y) …`** — vertices in CCW order. The polygon must
  be non-degenerate (the first three vertices must be non-collinear).
- **`symmetry`** *(optional)* — declared symmetry group. Accepted values:
  `trivial`, `c1`…`cn`, `d1`…`dn`. If omitted the normalizer infers it
  under affine equivalence (§3.3). If declared, the normalizer verifies
  the declaration is a subgroup of the inferred group and emits
  `InconsistentSymmetry` otherwise.

Every declared `tile` must have at least one `partition` whose head names
that tile. A tile with no partition has no defined substitution behavior
and is rejected with `TileWithoutPartition`. Termination of recursion
must be expressed explicitly (e.g. via state-guarded rules in the
pattern), never implicitly by omitting a partition.

### 3.2 Symmetry — Affine Semantics

Symmetry is defined under **affine equivalence**. A vertex permutation π
is a symmetry of a polygon iff there exists an affine map (2D linear +
translation, `det ≠ 0`) sending vertex `i` to vertex `π(i)` for every i.

Consequences:

- Every non-degenerate triangle has **D3** symmetry.
- Every non-degenerate parallelogram (rhombus, rectangle, square) has
  **D4** symmetry.
- An isoceles trapezoid has **D1**; a generic quadrilateral has
  **trivial**.

### 3.3 Affine Invariants

For each tile the normalizer computes a deterministic
`(2n − 6)`-tuple of real numbers — the **affine invariants** — that
uniquely identifies the polygon's affine equivalence class.

#### Construction

Given canonical vertices `v_0, …, v_{n-1}` (CCW):

1. Pick three reference vertices `(v_a, v_b, v_c)` along the cycle.
2. Compute the unique affine transform `T` sending
   `v_a → (0, 0)`, `v_b → (1, 0)`, `v_c → (0, 1)`.
3. Apply `T` to the remaining `n − 3` vertices, producing pairs
   `(α_i, β_i)` of affine coordinates in the basis
   `(v_a, v_b − v_a, v_c − v_a)`.
4. Concatenate the pairs into a `(2n − 6)`-tuple.

#### Canonicalization

Enumerate all `2n` valid starting labellings (n cyclic rotations × 2
orientations: forward and reversed). For each, perform steps 1–4 using
the first three vertices of that labelling as the basis. **Store the
lexicographically smallest resulting tuple.** This makes the invariants
independent of the polygon's starting vertex and CCW-vs-CW listing.

#### Contract

The numeric representation is fixed by the construction above. Consumers
rely on:

- **Affine equivalence ⇔ equal invariants**, within numeric tolerance.
- **O(n) comparison** between any two invariant tuples after their
  canonical forms are computed.

#### Examples

| Polygon                              | n | Invariants     |
|--------------------------------------|---|----------------|
| Any non-degenerate triangle          | 3 | `()`           |
| Unit square                          | 4 | `(-1, 1)`      |
| Any sheared parallelogram            | 4 | `(-1, 1)`      |
| Non-parallelogram quad `(0,0)(1,0)(1,1)(0,2)` | 4 | `(-2, 1)` |

### 3.4 Affine Classes

Tiles with equal affine invariants belong to the same **affine class**.
A tile name followed by `.class` refers to its class:

```frac
quad.class       # the affine class containing tile `quad`
tri.class        # the affine class containing tile `tri`
```

Two references to the same class (via different tile-name prefixes)
denote the same class. The class is the equivalence "everything in this
shape, up to affine transformation".

Affine classes have the following roles:

- **Partition matching** (§4.3): a face polygon matches the first
  declared tile of the same class.
- **Class-level rules** (§7.6): a `rule tile.class { … }` applies to
  every tile in the class.
- **Class-level slot_order** (§8): permutes children of the class.
- **Hover display**: the LSP shows each tile's invariant tuple, so the
  author can see at a glance which tiles share a class.

The invariants are also a cache: matching, rule selection, and
slot-order class lookup all key off the same tuple.

### 3.5 Tile Storage Layout

Each `NormalizedTile` stores:

```rust
NormalizedTile {
    symmetry:   SymmetryGroup,         // affine, §3.2
    invariants: Vec<f64>,              // §3.3; length = max(0, 2n − 6)
    group_perm: Vec<GroupPermEntry>,   // §9.4
}
```

---

## 4. Partitions

```frac
partition quad.quad_split {
    edge m_s = (0.5, 0)
    edge m_e = (1, 0.5)
    edge m_n = (0.5, 1)
    edge m_w = (0, 0.5)
    interior center = (0.5, 0.5)

    cut m_s -- center
    cut m_e -- center
    cut m_n -- center
    cut m_w -- center

    child 0 = top_left           # bind a human name to the canonical index
    child 1 = top_right
    # …
}
```

Partition heads may name a specific tile (`partition quad.split { … }`)
or an affine class (`partition quad.class.split { … }`). A class-headed
partition applies to any tile in that class; its cuts are interpreted
relative to the actual tile's canonical polygon via affine pull-back.

### 4.1 Vertices

Three vertex kinds:

- **`edge name = (x, y)`** — a point on the parent's boundary.
- **`interior name = (x, y)`** — a point strictly inside the parent.
- Parent's canonical vertices are implicit and referenced as
  `v0, v1, …`.

### 4.2 Cuts

`cut a -- b` declares a straight segment between two named vertices.
Cuts may cross; the normalizer adds implicit vertices at every
intersection and re-embeds the resulting planar graph.

### 4.3 Tile-Type Matching for Faces

Before matching, **consecutive colinear vertices on a face boundary
are collapsed**. A vertex introduced by a cut that meets a face along
a straight edge (the incoming and outgoing edges at that vertex are
colinear) is not counted toward the face's arity. A corner cut across
a quad therefore yields a 3-vertex triangle and a 5-vertex pentagon,
not two quads. The collapse is per-face: the underlying planar graph
is unchanged, so neighboring faces that need the vertex still see it.

Each interior face of the cut graph is then assigned a tile type:

1. Compute the face polygon's affine invariants (§3.3).
2. Walk declared tiles in **source declaration order**.
3. The first tile whose invariants match wins. The face polygon's
   anchor vertex is determined by the affine fit
   (§4.4).
4. If no tile matches, emit `NoTileMatch`.

This means two tiles sharing an affine class are equally good matches
for any face of that class; the first declared tile wins. To override,
use slot-level tile assignment (§4.6).

### 4.4 Anchor Vertex and Affine Transform

For each matched face the normalizer records:

- The **anchor vertex** — which face vertex corresponds to canonical
  vertex 0 of the matched tile.
- The **affine transform** mapping the tile's canonical polygon to the
  face polygon.

Combined, these determine how state and alignment propagate from the
rule into the child.

### 4.5 Child Indexing

Faces are numbered `0..k` in canonical order (centroid x then y).
Authors may bind names to indices with `child N = name`. Indices are
the canonical *pre-permutation* indexing — they remain stable when
`slot_order` rearranges geometric placement.

### 4.6 Explicit Slot Tile Assignment *(optional)*

By default, each slot's tile type is the result of §4.3 — first
declared tile in the affine class. Authors may override:

```frac
partition quad.quad_split {
    # … vertices and cuts …

    child 0 = top_left : tri        # type override, optional name binding
    child 1 = top_right : tri_alt   # tri_alt must be in the same affine class
}
```

The override must be a tile in the same affine class as the slot's
class (otherwise: `SlotTileClassMismatch` error). If only a class match
is needed without naming, the comma form `child 0 : tri` is permitted.

The slot's default tile type can also be overridden dynamically in a
rule's child-injection block (§7.3.4).

---

## 5. Scopes and Identifier Rules

Identifiers live in named scopes. Within a single scope, no two
identifiers may share a name. Cross-scope shadowing is generally
disallowed unless explicitly noted.

### 5.1 Scopes

| Scope                | Identifiers introduced                  | Parent                   |
|----------------------|-----------------------------------------|--------------------------|
| **File**             | tile, partition, function, pattern      | —                        |
| **Tile**             | (none — `canonical`, `sides`, etc. are keywords) | File             |
| **Partition**        | edge / interior vertex names; child names | File                   |
| **Pattern**          | state variable names                    | File                     |
| **Function**         | parameter names                         | File                     |
| **Rule body**        | (none — assigns to inherited state)     | Pattern                  |
| **Substitution body**| (none — assigns to inherited state)     | Rule body                |
| **Child injection**  | (none — assigns to inherited state)     | Substitution body        |

State variables introduced in `pattern.state { … }` are visible inside
every rule, substitution, and child injection. Function parameters are
visible only inside the function body.

### 5.2 Duplicate-Name Rules

The normalizer rejects:

- Two `tile` declarations with the same name (`DuplicateTile`).
- Two `partition` declarations with the same `tile_name.partition_name`
  pair (`DuplicatePartition`). Class-headed partitions
  (`tile_name.class.partition_name`) live in the same key space.
- Two `fn` declarations with the same name (`DuplicateFunction`).
- More than one `pattern` block (`DuplicatePattern`).
- Two edge/interior/child names within a partition body
  (`DuplicateVertex`).
- Two parameters with the same name within a function
  (`DuplicateParameter`).
- Two state variables with the same name in the pattern's `state` block
  (`DuplicateState`).
- A `tile` declaration with no corresponding `partition`
  (`TileWithoutPartition`). See §3.1.

### 5.3 Cross-Scope Shadowing

Tile names, partition keys, function names, and state-variable names
live in distinct namespaces and never conflict with each other. (E.g.
a tile named `factor` does not collide with a state variable named
`factor`.) Within a function, parameter names shadow nothing because
no other names are in scope inside the function body.

### 5.4 Reference Resolution

Inside an expression, a bare identifier resolves in this order:

1. Local parameters (inside a function).
2. State variables of the enclosing `pattern`.
3. Tile names (yielding a `tile_ref` value, §10).

A reference that does not resolve produces `UnknownIdentifier`.

---

## 6. Functions

```frac
fn blend(val, target, factor) {
    lerp(val, target, factor)
}
```

Pure expression functions: parameters in, value out. No statements, no
control flow outside expression `if/else`, no recursion. The normalizer
verifies the function call graph is acyclic.

---

## 7. Patterns and Rules

```frac
pattern {
    root quad

    state {
        r      = 0.0
        g      = 0.0
        b      = 0.0
        target = 1.0
        decay  = 0.6
        # alignment_seed       # no initializer → none on the root
    }

    color (r, g, b, 1)

    rule quad { … }
    rule tri  { … }
}
```

### 7.1 State

Every state variable is `Option<T>` where T ∈ {`float`, `group_elem`,
`perm`, `tile_ref`}. Declared values seed the root; undeclared
variables start as `none`. Children inherit by default.

### 7.2 Color

A single 4-tuple `(r, g, b, a)` evaluated per tile. If the typechecker
proves nullability `yes` for this expression, the normalizer rejects
the file (`NullableColor`).

### 7.3 Rules

#### 7.3.1 Heading

```frac
rule quad           { … }        # specific tile
rule quad.class     { … }        # entire affine class
```

A class-level rule applies to every tile in the class, used as a
fallback when no tile-specific rule exists. Two rules with overlapping
applicability (e.g. a tile rule plus its class rule) is allowed:
the tile-specific rule wins.

#### 7.3.2 Body

A rule body must terminate in a substitution `-> tile.partition { … }`
or `-> tile.class.partition { … }`. Decision trees with `if/else` are
permitted around substitutions.

#### 7.3.3 Substitution Block

```frac
-> quad.quad_split {
    decay = decay * 0.9             # tile-level update

    child 0 {
        target = factor * decay
        alignment = d3.r120
    }
    child top_right { r = lerp(r, 0.6, target) }
    # …
}
```

State updates run sequentially within each block. Per-child blocks
observe already-updated tile-level variables.

#### 7.3.4 Child Injections

Inside a child block, the author may assign to inherited state
variables and to four special slots:

| Field                 | Type           | Effect                                              |
|-----------------------|----------------|-----------------------------------------------------|
| *(state var)*         | matches state  | Override that variable for this child               |
| `alignment`           | `group_elem`   | Override the child's affine orientation (§9.4)      |
| `tile`                | `tile_ref`     | Override the child's tile type (must be in class)   |

The `tile` override allows static or expression-driven tile selection:

```frac
child 0 { tile = if depth < 3 { quad } else { rhombus } }
```

The chosen tile must be in the slot's affine class (otherwise
`TileClassMismatch`). A `tile_ref` literal is a bare tile name in
expression position.

Indexing inside child blocks (`child N` or `child name`) is the
**canonical pre-permutation** index (§4.5). When `slot_order` rearranges
slots, the injected state travels with its tile.

---

## 8. Slot Order

`slot_order` declarations permute which canonical child lands in which
geometric slot.

### 8.1 Forms

```frac
slot_order = perm_expr                       # uniform-type partition
slot_order tile_name = perm_expr             # tiles of that specific type
slot_order tile_name.class = perm_expr       # tiles of that affine class
```

The first form is shorthand allowed only when the partition has exactly
one tile type. The other two forms compose freely.

### 8.2 Source-Order Semantics

`slot_order` declarations are processed **in source order**. Each
declaration permutes a target set of slots:

- **`slot_order T = p`** targets the slots currently holding tiles of
  type `T`.
- **`slot_order C.class = p`** targets the slots currently holding
  tiles in class `C`, **excluding** any tiles that have already been
  the target of a preceding tile-specific `slot_order` in this rule.

The effect: if a tile-specific declaration appears before its class
declaration, those tiles are "claimed" and the class declaration skips
them. If a class declaration appears first, it permutes all class
members; subsequent tile-specific declarations sub-permute within their
already-rearranged positions.

Example A — tile-first:

```frac
slot_order tri        = p_t        # permutes the triangles
slot_order quad.class = p_q        # permutes the remaining quad-class
                                    # tiles (excluding tri, which has its
                                    # own slot_order — even if tri were in
                                    # the quad class, it is now claimed)
```

Example B — class-first:

```frac
slot_order quad.class = p_q        # permutes ALL quad-class members
slot_order tri        = p_t        # sub-permutes tri within its
                                    # already-rearranged positions
```

The permutation length for each declaration equals the number of slots
in its target set at the time it is processed.

### 8.3 Permutation Source

The right-hand side `perm_expr` may be:

- A `perm [i, j, …]` literal.
- A group element (e.g. `d3.r120`), resolved to a permutation via the
  tile or class's `group_perm` table. Only valid when every targeted
  slot holds tiles of the same affine class (so the table is unique).
- Any expression of type `perm` or `group_elem`.

### 8.4 Validity

A `slot_order` is rejected if:

- The permutation length doesn't match the target set size
  (`SlotOrderLengthMismatch`).
- A tile-specific declaration targets a tile that doesn't appear in
  the partition (`SlotOrderUnusedTile`).
- A class-specific declaration targets a class with no members in the
  partition (`SlotOrderUnusedClass`).
- The permutation produces a slot assignment placing a tile in a slot
  whose class differs from the tile's class (`SlotOrderClassMismatch`).

### 8.5 Interaction with `alignment`

`slot_order` rearranges *which child goes where*; `alignment` chooses
*how each child is oriented in its slot*. They are independent
mechanisms applied in this order at evaluation:

1. Resolve tile type for each canonical child (§7.3.4).
2. Apply `slot_order` permutations to obtain final slot assignments.
3. For each placed child, resolve `alignment` (defaulting to the
   slot's canonical orientation, §8.6) and compose with the slot's
   affine transform.

### 8.6 Canonical Orientation

When no explicit `alignment` is given, the child is placed in its
**canonical orientation** within the slot. A slot polygon may admit
several distinct affine fits of the child's canonical vertex list
(one per cyclic rotation of the labelling, times two for reflection);
the canonical orientation is the one that minimizes affine distortion
from a similarity transform.

The four-degree-of-freedom linear part of an affine map decomposes
into rotation, uniform scale, shear, and stretch (non-uniform scale).
Uniform scale and translation are fixed by the slot's position and
size and are identical across all candidate fits, so the canonical
orientation is selected by lexicographically minimizing the remaining
three components, in order:

1. **Shear** — magnitude of the off-diagonal term after extracting
   rotation (equivalently, `|cos θ|` where θ is the angle between the
   linear part's two column vectors).
2. **Stretch** — deviation of the column-length ratio from 1, i.e.
   `|log(‖col₁‖ / ‖col₀‖)|`.
3. **Rotation** — `|θ|` where θ is the angle of the rotation factor
   in the polar decomposition (smallest rotation away from the
   identity / "most upright").

A final tiebreak prefers orientation-preserving (non-reflected) fits
over reflected ones. Comparisons use a small floating-point tolerance
so that fits which are equal up to numerical noise fall through to
the next criterion.

For similarity tilings (Penrose, pinwheel, etc.) every candidate fit
has shear 0 and stretch 1, so this rule reduces to "smallest rotation
from upright". The shear and stretch criteria affect only genuinely
affine-distorted tilings.

---

## 9. Group Elements and Permutations

### 9.1 Group Element Literals

`<group>.<element>` where `<group>` is `trivial`, `cN`, or `dN`, and
`<element>` is a member name:

- **Identity**: `identity`
- **Rotations**: `r{deg}` where `deg = i · 360 / k`, `i = 1 … k-1`.
  Examples: D3 → `r120`, `r240`. D4 → `r90`, `r180`, `r270`.
- **Reflections** *(Dihedral only)*: `fv0`, `fv1`, …, `fv{k-1}`.

### 9.2 Permutation Literals

`perm [0, 1, 2, 3]` — explicit permutation of slot indices.

### 9.3 Operations

- `compose(a, b)` — group / permutation composition.
- `inverse(a)` — inverse element.

### 9.4 Realization as Affine Maps

For a tile, each group element maps to a vertex permutation π via the
normalizer's `group_perm` table. The corresponding affine map is the
unique transform sending `canonical[i] → canonical[π(i)]` (fit from
the first three vertices). This realization holds under affine maps
even when the tile is not Euclidean-symmetric — `d3.r120` on a right
triangle is a valid affine map even though no rigid rotation realizes
it.

---

## 10. Expression Language

### 10.1 Types

- `float`
- `bool`
- `group_elem(G)` — bound to a specific symmetry group `G`
- `perm(n)` — a permutation of `n` elements
- `tile_ref` — a tile-type identifier

All types are wrapped in `Option<T>` for nullability.

### 10.2 Literals

`0`, `1.5`, `true`, `false`, `none`, group literals (§9.1), permutation
literals (§9.2), tile-name literals (any bare tile identifier in
expression position evaluates to a `tile_ref`).

### 10.3 Reads

| Identifier                     | Type        | Notes                                  |
|--------------------------------|-------------|----------------------------------------|
| *(variable name)*              | depends     | Resolves per §5.4                      |
| `pos_x`, `pos_y`               | float       | Tile centroid in world space           |
| `scale`                        | float       | Tile scale relative to root            |
| `orientation`                  | float       | Degrees; first basis vector            |
| `shear`, `stretch`             | float       | Affine distortion of this tile         |
| `depth`                        | float       | Substitution depth (root = 0)          |
| `random`                       | float       | Deterministic per-tile noise           |
| `child.canonical_orientation`  | `group_elem`| Inside child block / tile-level update |
| `child.index`                  | float       | Inside child block: canonical index    |

### 10.4 Operators and Built-ins

- **Arithmetic**: `+ - * / -x`
- **Comparison**: `< > <= >= == !=`
- **Logic**: `&& || !`
- **Null coalescing**: `x ? default`
- **Conditional**: `if cond { e1 } else { e2 }`
- **Math**: `sin cos exp sqrt abs log`
- **Interpolation**: `lerp(a, b, t)` = `a + (b − a) · t`
- **Clamp**: `clamp(x, lo, hi)`
- **Group / perm**: `compose(a, b)`, `inverse(a)`
- **User functions**: by name

`none` propagates through arithmetic and group operations; the
typechecker computes nullability so the LSP can warn and the color
check can error.

---

## 11. Normalization Pipeline

In order:

1. **Parse** with full error recovery; collect `ParseError`s.
2. **Scope / duplicate-name check** (§5).
3. **Affine-invariant computation** (§3.3) for every declared tile.
4. **Symmetry inference and validation** (§3.2).
5. **Partition topology**: planar graph, face enumeration, affine
   matching (§4.3), record anchor vertex and affine transform.
6. **Acyclicity check** for `fn` definitions.
7. **Type and nullability check** for every expression; reject
   `NullableColor` failures.
8. **Group ↔ permutation tables** (§9.4).
9. **`slot_order` validation** (§8.4).
10. **Canonicalize** source. Tiles are sorted by ascending vertex count
    (`sides`), then grouped by equal affine invariants (tiles in the same
    class become contiguous in the rewritten source). Within a group the
    tie-break is source-declaration order. Partitions, functions, and
    the `pattern` block follow tiles. Coordinates are rounded; whitespace
    is reformatted. No semantic data is added to the source.

The pipeline runs to completion; the normalizer returns the collected
error batch rather than short-circuiting at the first error.

---

## 12. Stored Derived Data

```rust
NormalizedFile {
    file:           File,                              // canonicalized AST
    tiles:          HashMap<String, NormalizedTile>,
    partitions:     HashMap<(String, String), NormalizedPartition>,
    function_order: Vec<String>,                       // callees before callers
}

NormalizedTile {
    symmetry:   SymmetryGroup,
    invariants: Vec<f64>,
    group_perm: Vec<GroupPermEntry>,
}

NormalizedPartition { children: Vec<ChildInfo> }

ChildInfo {
    tile_type:        String,
    polygon:          Vec<Point2>,    // CCW, in parent coords
    affine_transform: [f64; 6],       // canonical → actual position
    anchor_vertex:    usize,
    index:            u32,
    name:             Option<String>,
}
```

All of this is consumed by the evaluator and the LSP; none is written
back to the source file.
