#!/usr/bin/env python3
"""Convert old-format fractal patterns (grid JSON) to the new .frac format.

Old format: {pixels: [[Pixel; 2]; 2]} where each Pixel has
    color: {r, g, b, a}
    perm:  {mapping: [[y, x]; 4]}  -- one of 6 known rigid motions of the unit square.

The 2x2 grid corresponds to a `quad.quad_split` partition (square cut into
four quadrants by edge-midpoint-to-center cuts).  The pixel's permutation
becomes a `d4` alignment on the corresponding child.
"""

import json
import os
import sys

# Old `perm.mapping` -> name of the rigid motion.
# (Each mapping is the (y, x) of the *source* cell that ends up at child (y, x),
#  enumerated in (0,0), (0,1), (1,0), (1,1) order.)
KNOWN_PERMS = {
    ((0,0),(0,1),(1,0),(1,1)): 'identity',
    ((0,1),(1,1),(0,0),(1,0)): 'rotate_90',
    ((1,1),(1,0),(0,1),(0,0)): 'rotate_180',
    ((1,0),(0,0),(1,1),(0,1)): 'rotate_270',
    ((0,1),(0,0),(1,1),(1,0)): 'flip_h',
    ((1,0),(1,1),(0,0),(0,1)): 'flip_v',
}

# Each grid perm, expressed as a slot_order perm: slot_order[slot] = canonical
# child index that lands in that slot. Canonical child ordering (centroid sort,
# x then y) is: 0=TL, 1=BL, 2=TR, 3=BR. Derived from the grid's pixel.perm
# mapping (which gives target-(y,x) for each source) by inverting through the
# canonical reindexing.
PERM_TO_SLOT_ORDER = {
    'identity':   [0, 1, 2, 3],
    'rotate_90':  [1, 3, 0, 2],
    'rotate_180': [3, 2, 1, 0],
    'rotate_270': [2, 0, 3, 1],
    'flip_h':     [2, 3, 0, 1],
    'flip_v':     [1, 0, 3, 2],
}

# Old grid coordinate (y, x) -> partition child name.
# Names chosen to not collide with state vars (e.g. avoid bare `bl`).
GRID_TO_NAME = {
    (0, 0): 'top_left',
    (0, 1): 'top_right',
    (1, 0): 'bottom_left',
    (1, 1): 'bottom_right',
}


def identify_perm(mapping):
    key = tuple(tuple(x) for x in mapping)
    if key not in KNOWN_PERMS:
        raise ValueError(f"unrecognised permutation mapping: {mapping}")
    return KNOWN_PERMS[key]


def fmt_f(f):
    """Match printer.rs::fmt_f64 — integer floats print as `N.0`."""
    if float(f).is_integer() and abs(f) < 1e15:
        return f"{f:.1f}"
    return repr(float(f))


PREAMBLE = """tile quad {
    sides 4
    canonical (0, 0) (1, 0) (1, 1) (0, 1)
}

partition quad.quad_split {
    edge m_s = (0.5, 0.0)
    edge m_e = (1.0, 0.5)
    edge m_n = (0.5, 1.0)
    edge m_w = (0.0, 0.5)
    interior center = (0.5, 0.5)

    cut center -- m_s
    cut center -- m_e
    cut center -- m_n
    cut center -- m_w

    child 0 = top_left
    child 1 = bottom_left
    child 2 = top_right
    child 3 = bottom_right
}

"""


def render_child(name, color, perm_name):
    """Port of src/main.rs::old_generate_fractal (the preview renderer):

        blend_factor = 1 - (1 - blend) * parent.a
        new.rgb      = lerp(parent.rgb, pixel.rgb, blend_factor)
        new.a        = lerp(1, pixel.a, blend_factor)
                     = 1 - (1 - pixel.a) * blend_factor

    Perm propagates via a state perm `p`, used at tile level as `slot_order`.
    Per-child update: `p = compose(p, <canonical pixel's slot_order perm>)`.
    Frac's `compose(pa, pb)[i] = pa[pb[i]]`, which gives the same direction
    as grid's `parent.perm.compose(base_pixel.perm)` once both perms are in
    slot-to-canonical (i.e. target-to-source) form.
    """
    r, g, b, a = color['r'], color['g'], color['b'], color['a']
    perm = PERM_TO_SLOT_ORDER[perm_name]
    a_lit = fmt_f(a)

    if a == 1.0:
        a_update = "a = 1.0"             # 1 - (1-1)*bf = 1
    elif a == 0.0:
        a_update = "a = 1.0 - bf"        # 1 - (1-0)*bf = 1 - bf
    else:
        a_update = f"a = 1.0 - (1.0 - {a_lit}) * bf"

    lines = [
        f"        child {name} {{",
        f"            r = lerp(r, {fmt_f(r)}, bf)",
        f"            g = lerp(g, {fmt_f(g)}, bf)",
        f"            b = lerp(b, {fmt_f(b)}, bf)",
        f"            {a_update}",
    ]
    if perm != [0, 1, 2, 3]:
        perm_lit = ", ".join(str(i) for i in perm)
        lines.append(f"            p = compose(p, perm [{perm_lit}])")
    lines.append("        }")
    return "\n".join(lines)


def convert_pattern(old_pattern):
    pixels = old_pattern['pixels']

    children = []
    # Emit in TL, TR, BL, BR reading order for readability.
    for (y, x) in [(0,0), (0,1), (1,0), (1,1)]:
        pixel = pixels[y][x]
        perm = identify_perm(pixel['perm']['mapping'])
        name = GRID_TO_NAME[(y, x)]
        children.append(render_child(name, pixel['color'], perm))

    children_block = "\n".join(children)

    pattern = f"""pattern {{
    root quad

    state {{
        r     = 0.0
        g     = 0.0
        b     = 0.0
        a     = 0.0
        blend = 1.0
        bf    = 0.0
        d     = 0.5
        p     = perm [0, 1, 2, 3]
    }}

    color (r, g, b, 1.0)

    rule quad -> quad.quad_split {{
        bf = 1.0 - (1.0 - blend) * a
        blend = blend * d
        slot_order = p
{children_block}
    }}
}}
"""
    return PREAMBLE + pattern


def main():
    args = sys.argv[1:]
    patterns_dir = args[0] if args else os.path.join(os.path.dirname(__file__), 'patterns')

    for filename in sorted(os.listdir(patterns_dir)):
        if not filename.endswith('.json'):
            continue
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
        new_path = os.path.join(patterns_dir, base + '.frac')
        with open(new_path, 'w') as f:
            f.write(new)
        print(f"  OK  {filename}  ->  {base}.frac")


if __name__ == '__main__':
    main()
