# Spec Deviations

Where the implementation diverges from
[frac_language_spec.md](frac_language_spec.md). This is a working list,
not a roadmap — entries may be implemented, retracted, or reinterpreted
at any time.

## Unimplemented

### §4.3 Affine-Invariant Matching in Partition Topology

The topology pass (§4.3) should match faces by computing their affine
invariants and comparing against each tile's stored invariants. The
current implementation calls `fit_affine_to_polygon` directly, which is
semantically equivalent (affine-fit succeeds iff invariants match) but
does not use the stored `NormalizedTile.invariants` as the spec describes.

### §3.4 / §7.3.1 Class-Level Semantics (Partition and Rule Bodies)

The parser now accepts `partition tile.class.name { … }` and
`rule tile.class { … }` and stores `tile_is_class: bool` on
`PartitionDecl` and `RuleDecl`. The normalizer does not yet act on this
flag:

- A class-headed partition is still applied only to the named tile, not
  to all tiles in its affine class.
- A class-level rule (`rule tile.class { … }`) is not yet used as a
  fallback for other tiles in the class.

### §4.6 / §7.3.4 Slot Tile Assignment Semantics

The parser now accepts:
- `child N = name : tile` and `child N : tile` in partition headers
  (§4.6), stored as `ChildName.tile_override`.
- `tile = expr` in child injection blocks (§7.3.4), stored as
  `ChildInjection.tile_override`.

These fields are parsed and stored but the normalizer/evaluator does not
yet enforce the `SlotTileClassMismatch` constraint or propagate the
override to child tile-type selection.

### §8 Slot Order — Validation and Execution

`slot_order` is parsed and type-checked (must be `perm` or
`group_elem`). The `§8.4` validity checks (`SlotOrderLengthMismatch`,
`SlotOrderUnusedTile`, `SlotOrderUnusedClass`, `SlotOrderClassMismatch`)
are not yet implemented. The evaluator does not yet apply slot
permutations.

### §5.2 DuplicatePattern Error

More than one `pattern` block in a file is not yet reported as an error.
The second block silently overwrites the first during evaluation.

## Errors

*(none recorded; add as they arise)*
