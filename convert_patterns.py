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

# d4 element names emitted by the normalizer for a canonical (0,0)(1,0)(1,1)(0,1) quad.
# (Verified against frac_lang::normalizer::group_perm.)
PERM_TO_D4 = {
    'identity':   'identity',
    'rotate_90':  'r90',
    'rotate_180': 'r180',
    'rotate_270': 'r270',
    'flip_h':     'fv1',  # mirror across x = 0.5
    'flip_v':     'fv3',  # mirror across y = 0.5
}

# Old grid coordinate (y, x) -> partition child name.
# The partition declares: child 0=tl, child 1=bl, child 2=tr, child 3=br
# (centroid-sort order, x then y, matching the normalizer's child indexing).
GRID_TO_NAME = {
    (0, 0): 'tl',
    (0, 1): 'tr',
    (1, 0): 'bl',
    (1, 1): 'br',
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

    child 0 = tl
    child 1 = bl
    child 2 = tr
    child 3 = br
}

"""


def render_child(name, color, perm_name):
    r, g, b, a = color['r'], color['g'], color['b'], color['a']
    d4 = PERM_TO_D4[perm_name]
    # target * a is the lerp amount: pixel's alpha scales how much it overrides
    # the parent.  When a == 1, this is just `target` (matching wa.frac).
    if a == 1.0:
        mix = "target"
    else:
        mix = f"target * {fmt_f(a)}"
    lines = [
        f"        child {name} {{",
        f"            r = lerp(r, {fmt_f(r)}, {mix})",
        f"            g = lerp(g, {fmt_f(g)}, {mix})",
        f"            b = lerp(b, {fmt_f(b)}, {mix})",
        f"            target = factor * decay",
        f"            factor = factor * decay",
        f"            alignment = d4.{d4}",
        f"        }}",
    ]
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
        r      = 0.0
        g      = 0.0
        b      = 0.0
        target = 1.0
        factor = 1.0
        decay  = 0.5
    }}

    color (r, g, b, 1.0)

    rule quad -> quad.quad_split {{
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
