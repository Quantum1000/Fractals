# Writing `.frac` Files

A practical guide

---

## File Structure

A `.frac` file contains four kinds of declarations, written in any order (the normalizer sorts them canonically on save):

1. **`tile`** — declare a tile shape with its canonical polygon
2. **`partition`** — declare how a tile is cut into children
3. **`fn`** — optional pure helper functions
4. **`pattern`** — the root block: state, color, and rules

---

## Tile Declarations

```frac
tile quad {
    sides 4
    canonical (0,0) (1,0) (1,1) (0,1)
}

tile tri {
    sides 3
    canonical (0,0) (1,0) (1,1)
}
```

- `sides` must match the number of points in `canonical`.
- Vertices are listed in CCW order.
- `symmetry d4` (or `d3`, `cn`, `trivial`) is optional — the normalizer infers it and stores it internally; it is never written back to source.

---

## Partition Declarations

Partitions describe how a tile is cut into child tiles using named vertices and cut lines. The normalizer derives the child polygons from the planar graph.

```frac
partition quad.quad_split {
    edge m_s = (0.5, 0)     # midpoint on the bottom edge
    edge m_e = (1, 0.5)     # midpoint on the right edge
    edge m_n = (0.5, 1)     # midpoint on the top edge
    edge m_w = (0, 0.5)     # midpoint on the left edge

    cut m_s -- m_e
    cut m_e -- m_n
    cut m_n -- m_w
    cut m_w -- m_s
}
```

### Vertex types

| Keyword | When to use |
|---|---|
| `edge` | The point lies on a boundary edge of the parent tile |
| `interior` | The point is strictly inside the parent tile |

> **Important:** a point on a boundary edge is always `edge`, even if the JSON source called it an "interior vertex." For example, in `tri.tri_split` the point `(0.5, 0.5)` lies on the hypotenuse of the canonical tri `(0,0)→(1,0)→(1,1)`, so it must be declared `edge`, not `interior`.

### How cuts become children

The normalizer builds a planar graph from the boundary edges of the tile plus the declared `cut` lines, then enumerates faces. Each face becomes a child. The normalizer:
- Matches each face's shape to a declared tile type via affine invariants (side-length ratios).
- Assigns a canonical CCW winding and anchor vertex.
- Numbers children in a canonical order.

You do not specify the child polygons directly — they are computed.

### Naming children

Inside the partition block you can give children names for use in rules:

```frac
child 0 = inner_quad
child 1 = corner_bl
```

If omitted, children are referenced by number in rules.

---

## Pattern Block

```frac
pattern {
    root quad

    state { ... }

    color (r, g, b, 1)

    rule quad { ... }
    rule tri  { ... }
}
```

- `root` names the tile type of the root tile.
- There is one `rule` per tile type that can appear in the substitution tree.

---

## State

```frac
state {
    r      = 0.0    # has an initial value at the root
    g      = 0.0
    b      = 0.0
    w      = 1.0
    target = 1.0
    factor = 1.0
    decay  = 0.6
    foo             # no value → root tile starts with `none`
}
```

- Every variable is `Option<T>` (float, group element, or perm).
- Variables with `= value` are initialised at the root; others start as `none`.
- **Children inherit all variables automatically.** You only need to write updates for variables you want to change.
- Use `x ? default` (null-coalescing) anywhere a variable might be `none`. The color expression must never produce `none` — the normalizer rejects it.

---

## Color Expression

```frac
color (r, g, b, 1)
```

- Four components: R, G, B, A.
- Can use any expression, including literals and state variables.
- The alpha component can be a literal `1` rather than a state variable.
- If a component is potentially `none`, use null-coalescing: `r ? 0`.

---

## Rules

A rule fires when the renderer visits a tile of the given type:

```frac
rule quad {
    -> quad.quad_split {
        # tile-level updates (inherited by all children)
        factor = factor * 0.9

        # per-child state injection
        child 0 {
            r = lerp(target, r, 0.0)
        }
        child 1 {
            r = lerp(target, r, 0.5)
        }
    }
}
```

### Rule branches

```frac
rule quad {
    if depth < 4 {
        -> quad.quad_split { ... }
    } else {
        -> quad.quad_split { decay = decay * 0.5 }
    }
}
```

### Update ordering

State updates are **sequential within each block**. This matters when a later update references a variable that was already changed earlier in the same block. In particular, if you want `target` to be computed using the *old* `factor`, write `target = ...` before `factor = ...`.

### Child injection is pre-permutation

`child 0 { ... }` assigns to the canonical child 0 *before* any `slot_order` permutation. If you use `slot_order`, the injected state travels with the tile to its new geometric slot.

---

## Expression Language

| Form | Meaning |
|---|---|
| `1`, `0.5` | float literal |
| `none` | null literal |
| `r`, `target` | read a state variable |
| `a + b`, `a - b`, `a * b`, `a / b`, `-a` | arithmetic |
| `lerp(a, b, t)` | `a + (b-a)*t` — linear interpolation |
| `clamp(x, lo, hi)` | clamp |
| `sin(x)`, `cos(x)`, `exp(x)`, `sqrt(x)`, `abs(x)`, `log(x)` | math |
| `x ? default` | null-coalescing |
| `if c { e } else { e }` | conditional |
| `<`, `>`, `<=`, `>=`, `==`, `!=`, `&&`, `\|\|`, `!` | comparison / logic |
| `compose(a, b)`, `inverse(a)` | group / perm operations |

### Geometric read-only variables (available in any expression)

| Name | Type | Description |
|---|---|---|
| `pos.x`, `pos.y` | float | centroid of current tile |
| `scale` | float | similarity scale from root |
| `orientation` | float | degrees, first basis vector direction |
| `depth` | float | recursion depth |
| `random` | float | deterministic per-tile random value |

### Additional read-only variables (inside child blocks only)

| Name | Type | Description |
|---|---|---|
| `child.index` | int | canonical index of this child |
| `child.canonical_orientation` | group element | orientation of the slot this child lands in |

---

## Translating from the JSON spec format

The JSON `.spec.json` format uses expression nodes that map directly to frac expressions:

| JSON | frac |
|---|---|
| `{"State": n}` | the state variable at index n (by name) |
| `{"Lit": x}` | float literal `x` |
| `{"Mix": [a, b, t]}` | `lerp(a, b, t)` |
| `{"Mul": [a, b]}` | `a * b` |

State indices 0–N map to the variables in the order they appear in the `state` block.

An update of `{"State": n}` where n is the variable's own index means "inherited unchanged" — just omit it from the child block.

---
