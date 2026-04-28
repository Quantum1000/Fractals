#!/usr/bin/env python3
"""Convert old-format fractal patterns to the new spec format.

Old format: {pixels: [[Pixel; 2]; 2]} where each Pixel has {color: {r,g,b,a}, perm: {mapping: [...]}}
New format: SpecPattern JSON with tile_types, canonical_vertices, partitions, rules, etc.

The mapping is derived by comparing quilt.json (old) with quilt_new.spec.json (new).
"""

import json
import os

# Vertex index legend (for a unit quad subdivided into 4 quadrants):
#  0: (0.0, 0.0) = TL corner
#  1: (1.0, 0.0) = TR corner
#  2: (1.0, 1.0) = BR corner
#  3: (0.0, 1.0) = BL corner
#  4: (0.5, 0.0) = top-mid   (interior vertex 0)
#  5: (1.0, 0.5) = right-mid (interior vertex 1)
#  6: (0.5, 1.0) = bottom-mid(interior vertex 2)
#  7: (0.0, 0.5) = left-mid  (interior vertex 3)
#  8: (0.5, 0.5) = center    (interior vertex 4)

# For each quadrant (y, x), the four corners in [TL, TR, BR, BL] canonical order:
QUADRANT_CORNERS = {
    (0, 0): [0, 4, 8, 7],  # TL quad
    (0, 1): [4, 1, 5, 8],  # TR quad
    (1, 0): [7, 8, 6, 3],  # BL quad
    (1, 1): [8, 5, 2, 6],  # BR quad
}

# For each permutation type, the reordering of [C0=TL, C1=TR, C2=BR, C3=BL].
# Derived from: child_verts[0] = canonical(0,0), [1] = canonical(1,0), [3] = canonical(0,1).
# Verified against quilt.json ↔ quilt_new.spec.json correspondence.
PERM_VERTEX_ORDER = {
    'identity':   [0, 1, 2, 3],  # child(0,0)=TL, child(1,0)=TR, child(0,1)=BL
    'rotate_90':  [1, 2, 3, 0],  # child(0,0)=BL  (CW 90)
    'rotate_180': [2, 3, 0, 1],  # child(0,0)=BR
    'rotate_270': [3, 0, 1, 2],  # child(0,0)=TR  (CCW 90)
    'flip_h':     [1, 0, 3, 2],  # child(0,0)=TR  (left-right flip)
    'flip_v':     [3, 2, 1, 0],  # child(0,0)=BL  (top-bottom flip)
}

# Known permutation mappings (from Rust source Permutation impls)
KNOWN_PERMS = {
    ((0,0),(0,1),(1,0),(1,1)): 'identity',
    ((0,1),(1,1),(0,0),(1,0)): 'rotate_90',
    ((1,1),(1,0),(0,1),(0,0)): 'rotate_180',
    ((1,0),(0,0),(1,1),(0,1)): 'rotate_270',
    ((0,1),(0,0),(1,1),(1,0)): 'flip_h',
    ((1,0),(1,1),(0,0),(0,1)): 'flip_v',
}


def identify_perm(mapping):
    key = tuple(tuple(x) for x in mapping)
    if key not in KNOWN_PERMS:
        raise ValueError(f"Unrecognised permutation mapping: {mapping}")
    return KNOWN_PERMS[key]


def get_child_vertices(quad_y, quad_x, perm_type):
    corners = QUADRANT_CORNERS[(quad_y, quad_x)]
    order = PERM_VERTEX_ORDER[perm_type]
    return [corners[i] for i in order]


def make_state_updates(r, g, b, a):
    """State vector: [R, G, B, mix_weight, scale, decay]
    Each child blends its target color with the parent's accumulated color.
    mix_weight decays by 'decay' each level (default 0.5).
    """
    return [
        {"Mix": [{"State": 3}, {"State": 0}, {"Lit": r}]},
        {"Mix": [{"State": 3}, {"State": 1}, {"Lit": g}]},
        {"Mix": [{"State": 3}, {"State": 2}, {"Lit": b}]},
        {"Mix": [{"Lit": a}, {"State": 3},{"Mul": [{"State": 4}, {"State": 5}]}]},
        {"Mul": [{"State": 4}, {"State": 5}]},
        {"State": 5},
    ]


def convert_pattern(old_pattern):
    pixels = old_pattern['pixels']

    children = []
    for y in range(2):
        for x in range(2):
            pixel = pixels[y][x]
            c = pixel['color']
            perm_type = identify_perm(pixel['perm']['mapping'])
            vertices = get_child_vertices(y, x, perm_type)
            state_updates = make_state_updates(c['r'], c['g'], c['b'], c['a'])
            children.append({
                "tile_type": "quad",
                "vertices": vertices,
                "state_updates": state_updates,
            })

    return {
        "tile_types": {
            "quad": {"n": 4, "invariants": []}
        },
        "canonical_vertices": {
            "quad": [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
        },
        "partitions": {
            "quad": {
                "quad_split": {
                    "interior_vertices": [
                        [0.5, 0.0], [1.0, 0.5], [0.5, 1.0], [0.0, 0.5], [0.5, 0.5]
                    ],
                    "children": children
                }
            }
        },
        "rules": {
            "quad": {"Leaf": "quad_split"}
        },
        "initial_state": [0.0, 0.0, 0.0, 1.0, 1.0, 0.5],
        "root_tile_type": "quad",
        "color_expr": [
            {"State": 0},
            {"State": 1},
            {"State": 2},
            {"Lit": 1.0}
        ]
    }


def main():
    patterns_dir = os.path.join(os.path.dirname(__file__), 'patterns')
    skip = {'quilt_new.spec.json'}

    for filename in sorted(os.listdir(patterns_dir)):
        if not filename.endswith('.json') or filename in skip:
            continue
        # Skip files that are already spec format
        if filename.endswith('.spec.json'):
            continue

        old_path = os.path.join(patterns_dir, filename)
        with open(old_path) as f:
            try:
                old = json.load(f)
            except json.JSONDecodeError as e:
                print(f"  SKIP {filename}: JSON error: {e}")
                continue

        if 'pixels' not in old:
            print(f"  SKIP {filename}: no 'pixels' key (not old format)")
            continue

        try:
            new = convert_pattern(old)
        except ValueError as e:
            print(f"  ERROR {filename}: {e}")
            continue

        base = filename[:-5]  # strip .json
        new_filename = base + '.spec.json'
        new_path = os.path.join(patterns_dir, new_filename)
        with open(new_path, 'w') as f:
            json.dump(new, f, indent=2)
        print(f"  OK  {filename}  →  {new_filename}")


if __name__ == '__main__':
    main()
