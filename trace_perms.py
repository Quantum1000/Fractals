#!/usr/bin/env python3
"""
Trace the permutation grid for both generate_fractal (new) and
old_generate_fractal (old) algorithms to a given depth.

Usage:
    python trace_perms.py patterns/bricks.json [--depth 3]
    python trace_perms.py patterns/bricks.json --depth 2 --old-only
"""

import json, argparse, sys
from copy import deepcopy

# ---------------------------------------------------------------------------
# Permutation logic (mirrors Rust impl)
# ---------------------------------------------------------------------------

NAMED = {
    ((0,0),(0,1),(1,0),(1,1)): "Id ",   # identity
    ((0,1),(1,1),(0,0),(1,0)): "R90",   # rotate 90°
    ((1,1),(1,0),(0,1),(0,0)): "180",   # rotate 180°
    ((1,0),(0,0),(1,1),(0,1)): "R270",  # rotate 270°
    ((0,1),(0,0),(1,1),(1,0)): "FH ",   # flip horizontal (swap L/R columns)
    ((1,0),(1,1),(0,0),(0,1)): "FV ",   # flip vertical (swap T/B rows)
    ((0,0),(1,0),(0,1),(1,1)): "FD ",   # flip main diagonal (transpose)
    ((1,1),(0,1),(1,0),(0,0)): "FAD",   # flip anti-diagonal
}

def perm_name(mapping):
    return NAMED.get(tuple(map(tuple, mapping)), "?? ")

def compose(p, q):
    """Apply p first, then q. Returns composed mapping."""
    result = [None]*4
    for i in range(4):
        y, x = p[i]
        idx = y*2 + x
        result[i] = q[idx]
    return result

def apply_perm(mapping, grid):
    """grid is list[list[pixel]]. Returns new 2x2 grid."""
    result = [[None,None],[None,None]]
    for i in range(4):
        from_y, from_x = i//2, i%2
        to_y, to_x = mapping[i]
        result[to_y][to_x] = grid[from_y][from_x]
    return result

# ---------------------------------------------------------------------------
# Pattern loading
# ---------------------------------------------------------------------------

PERM_PRESETS = {
    "identity":   [(0,0),(0,1),(1,0),(1,1)],
    "rotate_90":  [(0,1),(1,1),(0,0),(1,0)],
    "rotate_180": [(1,1),(1,0),(0,1),(0,0)],
    "rotate_270": [(1,0),(0,0),(1,1),(0,1)],
    "flip_h":     [(0,1),(0,0),(1,1),(1,0)],
    "flip_v":     [(1,0),(1,1),(0,0),(0,1)],
}

IDENTITY = [(0,0),(0,1),(1,0),(1,1)]

def load_pattern(path):
    with open(path) as f:
        data = json.load(f)
    pixels = data["pixels"]
    base = []
    for row in pixels:
        r = []
        for px in row:
            m = px["perm"]["mapping"]
            r.append({"color": px["color"], "perm": [tuple(p) for p in m]})
        base.append(r)
    return base

# ---------------------------------------------------------------------------
# Simulation
# ---------------------------------------------------------------------------

def make_grid(size):
    return [[None]*size for _ in range(size)]

def simulate(base, depth, algorithm="new"):
    """
    Simulate expand to `depth` iterations (final_size = 2^depth).
    Returns list of grids (one per expansion step), each grid cell = perm mapping.
    """
    final_size = 1 << depth
    result = make_grid(final_size)

    # seed with base pattern
    for y in range(2):
        for x in range(2):
            result[y][x] = deepcopy(base[y][x])

    snapshots = [("depth 0 (base 2×2)", deepcopy(result[:2]))]  # just 2x2
    snapshots[0] = ("depth 0 (base 2×2)", [[result[y][x] for x in range(2)] for y in range(2)])

    current_size = 2
    step = 0
    while current_size < final_size:
        step += 1
        new_size = current_size * 2
        is_last = (new_size == final_size)

        for y in range(current_size-1, -1, -1):
            for x in range(current_size-1, -1, -1):
                pixel = result[y][x]
                perm = pixel["perm"]
                permuted_base = apply_perm(perm, base)

                for dy in range(2):
                    for dx in range(2):
                        bp = permuted_base[dy][dx]
                        if algorithm == "new":
                            new_perm = compose(perm, bp["perm"])
                        else:  # old: identity on last expansion
                            if is_last:
                                new_perm = list(IDENTITY)
                            else:
                                new_perm = compose(perm, bp["perm"])
                        result[y*2+dy][x*2+dx] = {
                            "color": bp["color"],
                            "perm": new_perm,
                        }

        snap_size = new_size
        snapshots.append((
            f"depth {step} ({snap_size}×{snap_size})",
            [[result[r][c] for c in range(snap_size)] for r in range(snap_size)]
        ))
        current_size = new_size

    return snapshots

# ---------------------------------------------------------------------------
# Display
# ---------------------------------------------------------------------------

def fmt_color(c):
    return f"({c['r']:.2f},{c['g']:.2f},{c['b']:.2f},a={c['a']:.2f})"

def print_snapshot(label, grid, show_color=False):
    print(f"\n  {label}:")
    for row in grid:
        if row is None:
            continue
        parts = []
        for cell in row:
            if cell is None:
                parts.append("    ")
            else:
                name = perm_name(cell["perm"])
                if show_color:
                    parts.append(f"{name}{fmt_color(cell['color'])}")
                else:
                    parts.append(name)
        print("    " + "  ".join(parts))

def print_diff(snap_new, snap_old):
    """Highlight cells where new and old differ."""
    label, grid_new = snap_new
    _, grid_old = snap_old
    diffs = 0
    size = len(grid_new)
    print(f"\n  DIFF at {label}:")
    for r in range(size):
        row_parts = []
        for c in range(len(grid_new[r])):
            n = grid_new[r][c]
            o = grid_old[r][c]
            n_name = perm_name(n["perm"]) if n else "   "
            o_name = perm_name(o["perm"]) if o else "   "
            if n_name != o_name:
                row_parts.append(f"[{n_name}≠{o_name}]")
                diffs += 1
            else:
                row_parts.append(f" {n_name} ")
        print("    " + "  ".join(row_parts))
    if diffs == 0:
        print("    (no differences)")
    return diffs

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Trace permutation grids for both fractal algorithms")
    parser.add_argument("pattern", help="Path to classic .json pattern file")
    parser.add_argument("--depth", type=int, default=3, help="Expansion depth (default 3)")
    parser.add_argument("--color", action="store_true", help="Also show colors at each cell")
    parser.add_argument("--new-only", action="store_true")
    parser.add_argument("--old-only", action="store_true")
    args = parser.parse_args()

    try:
        base = load_pattern(args.pattern)
    except Exception as e:
        print(f"Error loading pattern: {e}", file=sys.stderr)
        sys.exit(1)

    show_new = not args.old_only
    show_old = not args.new_only

    print(f"\nPattern: {args.pattern}")
    print(f"Depth:   {args.depth}  (final grid: {1<<args.depth}×{1<<args.depth})")
    print(f"Perm key: Id=Identity  R90=Rot90  180=Rot180  R270=Rot270  FH=FlipH  FV=FlipV  FD=FlipDiag  FAD=FlipAntiDiag")

    snaps_new = simulate(base, args.depth, "new") if show_new else None
    snaps_old = simulate(base, args.depth, "old") if show_old else None

    if show_new and show_old:
        print("\n" + "="*60)
        print("NEW algorithm (generate_fractal / a_lerp)")
        print("="*60)
        for snap in snaps_new:
            print_snapshot(*snap, show_color=args.color)

        print("\n" + "="*60)
        print("OLD algorithm (old_generate_fractal / lerp)")
        print("="*60)
        for snap in snaps_old:
            print_snapshot(*snap, show_color=args.color)

        print("\n" + "="*60)
        print("DIFFERENCES (new perm ≠ old perm)")
        print("="*60)
        total_diffs = 0
        for sn, so in zip(snaps_new, snaps_old):
            total_diffs += print_diff(sn, so)
        if total_diffs == 0:
            print("\n  Permutation grids are identical for new and old algorithms.")
            print("  (The algorithms differ only in how they blend colors, not which perms propagate.)")

    elif show_new:
        print("\n" + "="*60)
        print("NEW algorithm")
        print("="*60)
        for snap in snaps_new:
            print_snapshot(*snap, show_color=args.color)

    else:
        print("\n" + "="*60)
        print("OLD algorithm")
        print("="*60)
        for snap in snaps_old:
            print_snapshot(*snap, show_color=args.color)

if __name__ == "__main__":
    main()
