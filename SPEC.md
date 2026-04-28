# Thread 1: Hierarchical Substitution Tiling System — Design Specification

## Tile Types

A tile type is defined by:
- A combinatorial type: an n-gon (number of sides)
- For n ≥ 4: a minimal set of ratio-based affine invariants that distinguish its shape class (e.g. for a quadrilateral, the ratios in which the diagonals divide each other)
- Triangles require no invariants — all triangles are affinely equivalent

## Partitions

A partition of a tile type is defined as a planar graph embedded within the canonical parent shape:
- The parent's n boundary vertices are fixed nodes
- Additional interior vertices may be introduced
- Edges connect vertices (boundary or interior), subdividing the parent into child polygons
- Any valid planar subdivision is a valid partition — validity is structural, not checked separately
- Each resulting child polygon must match an existing tile type (same n-gon and compatible invariants)

Multiple partitions may be defined per tile type.

## Substitution Rules

When a tile is selected for substitution, a binary decision tree is traversed to select a partition. The inputs available at each decision node are:

- Position, scale, orientation, shear, stretch (decomposed from the tile's affine transformation)
- A carried state vector (shared floats, threaded from parent to child)
- Randomness

Decision nodes are binary expressions over these inputs. Leaves specify a partition choice. The rule is stochastic if any node uses randomness; otherwise deterministic.

After a partition is selected, each child tile receives:
- Its affine transformation (derived from its placement in the partition)
- An updated state vector, computed by the expression language

## Termination

Recursion terminates when a tile's size falls below a threshold. The threshold is a parameter of the render call, not the pattern definition, allowing the same pattern to be rendered at different resolutions or explored as an infinite fractal.

## Expression Language

Used for: substitution decision nodes, state vector updates, and terminal color computation. Unified across all three.

- Inputs: position, scale, orientation, shear, stretch, carried state vector components, randomness, depth
- Outputs: floats or booleans
- Supported operations: arithmetic, comparison, common math functions (trig, exp, clamp, mix, etc.)
- No control flow
- No loops or recursion

Terminal color is computed by the expression language from the same inputs available at termination.

## Pattern File

A complete pattern definition contains:
- A set of named tile types (combinatorial type + invariants)
- A set of named partitions per tile type (planar graph definitions)
- A substitution rule per tile type (binary decision tree with expression nodes)
- Expression definitions for state vector updates and terminal color

Render parameters (size threshold, output resolution) are supplied separately at render time.
