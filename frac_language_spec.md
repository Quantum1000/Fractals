# Draft Language Specification: `.frac` Format

## Overview

A text-first language for hierarchical substitution tilings. Files are normalized in-place before rendering; the normalizer is a hard gate.

**File extension**: `.frac`

---

## Lexical Conventions

```
# Line comments
Identifiers:    [a-zA-Z_][a-zA-Z0-9_]*
Namespaced:     tile_name.partition_name
Numbers:        standard float/int literals
```

---

## Tile Type Declarations

```frac
tile quad {
    sides 4
    canonical (0,0) (1,0) (1,1) (0,1)
    symmetry d4         # optional — normalizer infers if omitted; shown via LSP hover
}
```

**`symmetry`**: the tile's isometry group. Accepted: `trivial`, `cn` or `dn` for any positive integer n. Inferred by normalizer if omitted; stored in `NormalizedFile`, not written back to source. LSP hover on the tile name shows the inferred symmetry.

---

## Partition Declarations

Purely geometric. Children derived from the planar graph of cuts.

```frac
partition quad.split {
    edge m_top    = (0.5, 0)
    edge m_right  = (1, 0.5)
    edge m_bottom = (0.5, 1)
    edge m_left   = (0, 0.5)
    interior center = (0.5, 0.5)

    cut m_top    -- center
    cut m_right  -- center
    cut m_bottom -- center
    cut m_left   -- center

    # Optional: author-assigned names for normalizer-assigned indices
    child 0 = top_left
    child 1 = top_right
    child 2 = bottom_left
    child 3 = bottom_right
}
```

The normalizer: computes child polygons from cuts (intersecting cuts → implicit vertices), matches each child to a tile type, assigns CCW winding and **anchor vertex** (topmost-leftmost vertex), numbers children in canonical order, and records the similarity transform (position, scale, rotation, reflection) mapping each child's canonical polygon to its actual position in the partition. All derived data is stored in `NormalizedFile` — nothing is written back to source. LSP exposes child geometry, tile type match, and similarity transform on hover.

**Tile type matching** uses **affine invariants** — shape descriptors that are preserved under similarity transforms, such as side-length ratios of consecutive edges. Each tile type has a canonical set of invariants computed from its `canonical` vertices; a child polygon is matched to a tile type by comparing its invariants. Ratio-based invariants are used as they are numerically stable and sufficient to distinguish tile shapes in practice.

**`slot_order` validity**: a permutation is legal only if it maps each child slot to another slot of the same tile type (same affine invariants). Within a uniform-type partition whose tile has a non-trivial symmetry group, the valid permutations are exactly those in the image of the group action on children. The normalizer computes this valid set and rejects any `slot_order` outside it.

---

## User-Defined Functions

Pure expression functions, no control flow, no recursion. Normalizer validates acyclicity.

```frac
fn blend(val, target, factor) {
    lerp(val, target, factor)
}
```

---

## Patterns

```frac
pattern {
    root quad

    state {
        r     = 0
        g     = 0
        b     = 0
        blend = 1
        decay = 0.5
        orient = d4.identity    # group-element state
        target_r                # no value → root tile gets `none`
    }

    color (r ? 0, g ? 0, b ? 0, 1)

    rule quad { ... }
}
```

### State Semantics

Every state variable is `Option<T>` where T is `float`, `group_element`, or `perm`.

- Variables with `= value` at root; others start as `none`.
- Children inherit all variables automatically.
- `x ? default` (binary null-coalescing, any type).
- `none` propagates through arithmetic and group operations.
- LSP flags nullable expressions. Color expression producing `none` is a hard normalization error.

---

## Rules and Decision Trees

```frac
rule quad {
    if depth < 3 {
        -> quad.split {
            # Tile-level update: once, inherited by all children
            blend = blend * decay

            # Per-child injection: PRE-PERMUTATION (by canonical child name/index)
            # Roles travel with tiles when slot_order rearranges
            child top_left   { target_r = 1.0 }
            child top_right  { target_r = 0.5 }
            child bottom_left  { target_r = 0.0 }
            child bottom_right { target_r = 0.0 }
        }
    } else {
        -> quad.split { blend = blend * decay }
    }
}
```

**Pre-permutation semantics**: `child top_left { ... }` assigns to the canonical top_left tile. `slot_order` moves that tile — and its injected state — to whatever geometric slot the permutation specifies.

**State update order**: sequential within each block. Per-child blocks see already-updated tile-level variables.

---

## Two Distinct Uses of Permutations

### Case 1: Slot permutation — which tile goes where

Expressed in the rule body (not bracket syntax):

```frac
rule quad -> quad.split {
    # Single tile class (all children same type)
    slot_order = orient

    orient = compose(orient, child.canonical_orientation)
}
```

For **mixed-type partitions**, one `slot_order` per tile class. The tile class is referenced by any tile type name belonging to that class (same `sides` + `invariants`). The normalizer resolves to the equivalence class:

```frac
rule mixed_tile -> mixed_partition {
    slot_order quad     = quad_perm
    slot_order triangle = tri_perm

    quad_perm = compose(quad_perm, child.canonical_orientation)
}
```

The valid permutation group for each class is `S_k` where k is the number of children of that class. A group element of the tile type's 2D symmetry group (e.g. `d4.r90`) and an explicit `perm [...]` literal are interchangeable representations of the same permutation, for uniform-type partitions. The normalizer establishes the mapping.

### Case 2: Alignment — how each tile is oriented in its slot

Each child slot has a **canonical orientation** derived from its geometry (anchor vertex + winding). When a tile lands in a slot (after slot permutation), its `child.canonical_orientation` is the canonical orientation of that slot (post-permutation: the slot the tile ended up in, not the slot it came from).

The rule can override this with `alignment`:

```frac
rule quad -> quad.split {
    child top_left { alignment = d4.r90 }
    child top_right { alignment = d4.fh }
    # Children without an override use child.canonical_orientation
}
```

`alignment` is a group element in the child tile type's symmetry group. It is available as a state variable or literal; children inherit the chosen alignment into their geometric transform.

For the perm-system case, both slot permutation and alignment are driven by the same accumulated group element:

```frac
rule quad -> quad.split {
    slot_order = orient
    orient = compose(orient, child.canonical_orientation)
    # child.canonical_orientation is post-permutation (the slot's canonical orientation)
    # so composed orient reflects the full accumulated symmetry
}
```

⚠️ **Prototype required**: the precise interaction between slot_order (case 1) and child.canonical_orientation (case 2) in the same rule is subtle and needs prototyping to verify the semantics are correct and intuitive before finalizing.

---

## Group-Valued and Permutation-Valued State

**Group element literals**:
- `d4`: `d4.identity`, `d4.r90`, `d4.r180`, `d4.r270`, `d4.fh`, `d4.fv`, `d4.fd`, `d4.fad`
- `d3`: `d3.identity`, `d3.r120`, `d3.r240`, `d3.fv0`, `d3.fv1`, `d3.fv2`
- `cn`: `cn.identity`, `cn.r1`, ..., `cn.r(n-1)`

**Explicit permutation literals**: `perm [0, 1, 2, 3]`

For uniform-type partitions, named group element literals and explicit perm literals represent the same thing and are interchangeable once the normalizer establishes the slot-permutation mapping.

**Operations**: `compose(a, b)`, `inverse(a)`

---

## Expression Language

**Literals**: `1`, `0.5`, `none`, `true`, `false`, group/perm literals

**State access**: variable names directly

**Geometric inputs** (read-only):
```
pos.x   pos.y
scale
orientation      # degrees, continuous, first basis vector
shear   stretch  # critical for partitions producing stretched/sheared tiles
depth
random
```

**Inside child blocks and tile-level update** (additional read-only):
```
child.canonical_orientation    # group element: canonical orientation of the slot
                               # this child lands in (post-slot-permutation)
child.index                    # integer: canonical index of this child
```

**Arithmetic**: `+  -  *  /  -x`

**Math**: `sin(x)  cos(x)  exp(x)  sqrt(x)  abs(x)  log(x)`

**Interpolation**: `lerp(a, b, t)` = `a + (b-a)*t`

**Clamp**: `clamp(x, lo, hi)`

**Null coalescing**: `x ? default`

**Conditionals**: `if cond { expr } else { expr }`

**Comparisons / Logic**: `<  >  <=  >=  ==  !=  &&  ||  !`

**Group/permutation ops**: `compose(a, b)`, `inverse(a)`

**User functions**: called by name

---

## Normalization (In-Place)

1. Parse with full error recovery
2. Infer/validate symmetry for all tile types (stored in `NormalizedFile`; not written back)
3. Compute partition topology from cuts; match each child polygon to a tile type via affine invariants (side-length ratios); assign child anchors, canonical orientations, and similarity transforms (stored in `NormalizedFile`; not written back). LSP surfaces all derived data on hover.
4. *(Not a runtime step)* Topology correctness is guaranteed by construction for valid inputs; correctness is verified by unit tests on the topology algorithm.
5. Type-check expressions; nullable inference; hard error if color may produce `none`; validate `slot_order` legality using affine invariants from step 3
6. Validate function acyclicity; validate `slot_order` tile class references
7. Establish group-element ↔ perm-literal equivalence mappings for uniform-type partitions
8. Canonicalize and reformat source (sort items, normalize float precision, assign explicit child indices if missing); no semantic data added to source

---

## Resolved Points

- **`alignment` is geometric**: setting `alignment` in a per-child block overrides the child's actual affine transform orientation, choosing among the discrete valid orientations for that child slot. It is not merely state.

- **Tile class explicit syntax**: `slot_order` references a tile class (not a tile type name) using `.class` syntax. Example:

```frac
rule mixed_tile -> mixed_partition {
    slot_order quad.class     = quad_perm
    slot_order triangle.class = tri_perm
}
```

`quad.class` refers to the equivalence class of all tile types with the same `sides` and `invariants` as `quad`. This is explicit and avoids ambiguity from name-based lookup.

## Open Questions

1. **Case 1/2 interaction**: deferred to prototyping. Current proposal: `child.canonical_orientation` is always post-slot-permutation (canonical orientation of the slot the tile ends up in), so composing it accumulates the full geometric symmetry chain.
