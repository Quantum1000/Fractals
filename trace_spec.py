#!/usr/bin/env python3
"""
Trace the expansion tree of a spec pattern to a given depth, showing
the affine transform (orientation, scale, flip) at each node — the
spec-world analogue of the permutation trace for classic patterns.

Usage:
    python trace_spec.py patterns/bricks.spec.json [--depth 3]
    python trace_spec.py patterns/quilt.spec.json --depth 2 --state
    python trace_spec.py patterns/chaos_grid.spec.json --depth 3 --flat
"""

import json, math, argparse, sys

# ---------------------------------------------------------------------------
# Affine helpers
# ---------------------------------------------------------------------------

def compose_transforms(parent, child):
    r = [[0.0]*3, [0.0]*3]
    for i in range(2):
        for j in range(3):
            if j < 2:
                r[i][j] = parent[i][0]*child[0][j] + parent[i][1]*child[1][j]
            else:
                r[i][j] = (parent[i][0]*child[0][2] + parent[i][1]*child[1][2]
                           + parent[i][2])
    return r

def build_child_transform(parent_t, child_verts, n):
    p0, p1, p_last = child_verts[0], child_verts[1], child_verts[n-1]
    local = [
        [p1[0]-p0[0], p_last[0]-p0[0], p0[0]],
        [p1[1]-p0[1], p_last[1]-p0[1], p0[1]],
    ]
    return compose_transforms(parent_t, local)

def decompose(t):
    a, b = t[0][0], t[0][1]
    c, d = t[1][0], t[1][1]
    det = a*d - b*c
    scale = math.sqrt(abs(det))
    # orientation: angle of first basis vector
    orient = math.degrees(math.atan2(c, a))
    flipped = det < 0
    pos = (t[0][2], t[1][2])
    return scale, orient, flipped, pos

def fmt_orient(deg):
    # snap to nearest 45°
    snapped = round(deg / 45) * 45
    if abs(deg - snapped) < 0.5:
        return f"{snapped:+.0f}°"
    return f"{deg:+.1f}°"

def fmt_transform(t):
    scale, orient, flipped, pos = decompose(t)
    flip_str = " [flip]" if flipped else ""
    return f"scale={scale:.4f}  orient={fmt_orient(orient)}{flip_str}  pos=({pos[0]:.3f},{pos[1]:.3f})"

# ---------------------------------------------------------------------------
# Expression evaluator (minimal, for state propagation)
# ---------------------------------------------------------------------------

def eval_expr(expr, state, scale, orient, pos, rng, depth):
    if isinstance(expr, (int, float)):
        return float(expr)
    if isinstance(expr, dict):
        k = next(iter(expr))
        v = expr[k]
        if k == "Lit":     return float(v)
        if k == "State":   return state[v] if v < len(state) else 0.0
        if k == "Scale":   return scale
        if k == "Orientation": return orient
        if k == "PosX":    return pos[0]
        if k == "PosY":    return pos[1]
        if k == "Depth":   return float(depth)
        if k == "Random":  return rng
        if k == "Neg":     return -eval_expr(v, state, scale, orient, pos, rng, depth)
        if k == "Sin":     return math.sin(eval_expr(v, state, scale, orient, pos, rng, depth))
        if k == "Cos":     return math.cos(eval_expr(v, state, scale, orient, pos, rng, depth))
        if k == "Exp":     return math.exp(min(eval_expr(v, state, scale, orient, pos, rng, depth), 80))
        if k == "Sqrt":    return math.sqrt(max(0.0, eval_expr(v, state, scale, orient, pos, rng, depth)))
        if k == "Abs":     return abs(eval_expr(v, state, scale, orient, pos, rng, depth))
        if k == "Log":
            x = eval_expr(v, state, scale, orient, pos, rng, depth)
            return math.log(x) if x > 0 else 0.0
        if k == "Add":     return eval_expr(v[0], state, scale, orient, pos, rng, depth) + eval_expr(v[1], state, scale, orient, pos, rng, depth)
        if k == "Sub":     return eval_expr(v[0], state, scale, orient, pos, rng, depth) - eval_expr(v[1], state, scale, orient, pos, rng, depth)
        if k == "Mul":     return eval_expr(v[0], state, scale, orient, pos, rng, depth) * eval_expr(v[1], state, scale, orient, pos, rng, depth)
        if k == "Div":
            denom = eval_expr(v[1], state, scale, orient, pos, rng, depth)
            return eval_expr(v[0], state, scale, orient, pos, rng, depth) / denom if abs(denom) > 1e-30 else 0.0
        if k == "Clamp":   return max(eval_expr(v[1], state, scale, orient, pos, rng, depth), min(eval_expr(v[2], state, scale, orient, pos, rng, depth), eval_expr(v[0], state, scale, orient, pos, rng, depth)))
        if k == "Mix":
            t_val = eval_expr(v[0], state, scale, orient, pos, rng, depth)
            a = eval_expr(v[1], state, scale, orient, pos, rng, depth)
            b = eval_expr(v[2], state, scale, orient, pos, rng, depth)
            return a + (b - a) * t_val
        if k == "Lt":      return 1.0 if eval_expr(v[0], state, scale, orient, pos, rng, depth) < eval_expr(v[1], state, scale, orient, pos, rng, depth) else 0.0
        if k == "Gt":      return 1.0 if eval_expr(v[0], state, scale, orient, pos, rng, depth) > eval_expr(v[1], state, scale, orient, pos, rng, depth) else 0.0
        if k == "And":     return 1.0 if eval_expr(v[0], state, scale, orient, pos, rng, depth) != 0 and eval_expr(v[1], state, scale, orient, pos, rng, depth) != 0 else 0.0
        if k == "Or":      return 1.0 if eval_expr(v[0], state, scale, orient, pos, rng, depth) != 0 or eval_expr(v[1], state, scale, orient, pos, rng, depth) != 0 else 0.0
        if k == "Not":     return 0.0 if eval_expr(v, state, scale, orient, pos, rng, depth) != 0 else 1.0
        if k == "If":      return eval_expr(v[1], state, scale, orient, pos, rng, depth) if eval_expr(v[0], state, scale, orient, pos, rng, depth) != 0 else eval_expr(v[2], state, scale, orient, pos, rng, depth)
    return 0.0

def eval_decision_tree(tree, state, scale, orient, pos, rng, depth):
    if isinstance(tree, dict):
        if "Leaf" in tree:
            return tree["Leaf"]
        if "Branch" in tree:
            b = tree["Branch"]
            cond = eval_expr(b["condition"], state, scale, orient, pos, rng, depth)
            if cond != 0.0:
                return eval_decision_tree(b["if_true"], state, scale, orient, pos, rng, depth)
            else:
                return eval_decision_tree(b["if_false"], state, scale, orient, pos, rng, depth)
    return str(tree)

LCG_MUL = 6364136223846793005
LCG_ADD = 1442695040888963407
MASK64   = (1 << 64) - 1

def lcg_next(seed):
    s = ((seed * LCG_MUL) + LCG_ADD) & MASK64
    rng = (s >> 33) / 0xFFFFFFFF
    return s, rng

# ---------------------------------------------------------------------------
# Tile node
# ---------------------------------------------------------------------------

class TileNode:
    def __init__(self, tile_type, transform, state, depth, rng_seed, path="root"):
        self.tile_type  = tile_type
        self.transform  = transform
        self.state      = state
        self.depth      = depth
        self.rng_seed   = rng_seed
        self.path       = path          # breadcrumb like "root.c0.c2"
        self.partition  = None          # which partition was chosen
        self.children   = []

    def expand_one(self, pattern):
        t = self.transform
        s, orient_rad, flipped, pos = decompose(t)
        _, rng = lcg_next(self.rng_seed)
        orient_rad_val = math.atan2(t[1][0], t[0][0])

        part_name = eval_decision_tree(
            pattern["rules"][self.tile_type],
            self.state, s, math.degrees(orient_rad_val), pos, rng, self.depth
        )
        self.partition = part_name
        partition = pattern["partitions"][self.tile_type][part_name]

        canon = pattern["canonical_vertices"][self.tile_type]
        interior = partition["interior_vertices"]
        all_verts = canon + interior

        for i, child_spec in enumerate(partition["children"]):
            vidxs = child_spec["vertices"]
            child_verts = [all_verts[vi] for vi in vidxs]
            n = pattern["tile_types"][child_spec["tile_type"]]["n"]
            child_t = build_child_transform(t, child_verts, n)

            child_scale, _, _, child_pos = decompose(child_t)
            child_rng_seed, child_rng = lcg_next(self.rng_seed ^ (i * 0x9e3779b97f4a7c15 & MASK64))

            new_state = [
                eval_expr(e, self.state, child_scale, 0.0, child_pos, child_rng, self.depth + 1)
                for e in child_spec["state_updates"]
            ]

            child = TileNode(
                tile_type=child_spec["tile_type"],
                transform=child_t,
                state=new_state,
                depth=self.depth + 1,
                rng_seed=child_rng_seed,
                path=f"{self.path}.c{i}",
            )
            self.children.append(child)

    def expand_to_depth(self, pattern, max_depth):
        if self.depth >= max_depth:
            return
        self.expand_one(pattern)
        for child in self.children:
            child.expand_to_depth(pattern, max_depth)

# ---------------------------------------------------------------------------
# Display
# ---------------------------------------------------------------------------

ANGLE_NAMES = {0: "0°", 90: "90°", 180: "180°", -180: "180°", -90: "-90°",
               45: "45°", -45: "-45°", 135: "135°", -135: "-135°"}

def snap_angle(deg):
    snapped = round(deg / 45) * 45
    if abs(deg - snapped) < 0.5:
        name = ANGLE_NAMES.get(snapped, f"{snapped}°")
        return name
    return f"{deg:.1f}°"

def print_tree(node, show_state=False, prefix="", is_last=True):
    connector = "└─ " if is_last else "├─ "
    scale, orient, flipped, pos = decompose(node.transform)
    flip_tag = " FLIP" if flipped else ""
    part_tag = f" [{node.partition}]" if node.partition else ""
    state_str = ""
    if show_state and node.state:
        vals = " ".join(f"{v:.3f}" for v in node.state)
        state_str = f"  state=[{vals}]"
    print(f"{prefix}{connector}{node.tile_type}  orient={snap_angle(orient)}{flip_tag}  "
          f"scale={scale:.4f}  pos=({pos[0]:.3f},{pos[1]:.3f}){part_tag}{state_str}")
    child_prefix = prefix + ("   " if is_last else "│  ")
    for i, child in enumerate(node.children):
        print_tree(child, show_state, child_prefix, is_last=(i == len(node.children)-1))

def print_flat_by_depth(root, max_depth, show_state=False):
    """Group nodes by depth and print as tables."""
    levels = [[] for _ in range(max_depth + 1)]
    def collect(node):
        levels[node.depth].append(node)
        for c in node.children:
            collect(c)
    collect(root)

    for d, nodes in enumerate(levels):
        if not nodes:
            continue
        print(f"\n  depth {d}  ({len(nodes)} tile{'s' if len(nodes)!=1 else ''}):")
        for node in nodes:
            scale, orient, flipped, pos = decompose(node.transform)
            flip_tag = " FLIP" if flipped else ""
            part_tag = f"  [{node.partition}]" if node.partition else ""
            state_str = ""
            if show_state and node.state:
                vals = " ".join(f"{v:.3f}" for v in node.state)
                state_str = f"  state=[{vals}]"
            print(f"    {node.path:<30}  {node.tile_type:<8}  "
                  f"orient={snap_angle(orient):<6}{flip_tag:<5}  "
                  f"scale={scale:.4f}  pos=({pos[0]:.3f},{pos[1]:.3f})"
                  f"{part_tag}{state_str}")

def cell_label(orient, flipped):
    """Short fixed-width label matching trace_perms.py style."""
    snapped = round(orient / 45) * 45
    if abs(orient - snapped) > 0.5:
        snapped = orient
    name = ANGLE_NAMES.get(snapped, f"{snapped:.0f}°")
    prefix = "F" if flipped else " "
    return f"{prefix}{name}"

def print_grid(root, max_depth):
    """
    Lay out each depth as a 2D grid, assuming the pattern is a 2×2
    subdivision.  At depth d the grid is 2^d × 2^d tiles; tiles are
    sorted by their world position (y first, then x).
    """
    levels = [[] for _ in range(max_depth + 1)]
    def collect(node):
        levels[node.depth].append(node)
        for c in node.children:
            collect(c)
    collect(root)

    for d, nodes in enumerate(levels):
        if not nodes:
            continue
        side = 1 << d          # 2^d cells per row/column
        cell_size = 1.0 / side  # world size of each cell

        # sort by grid row then col using the world-space centroid of the tile.
        # Using pos (canonical origin) breaks for rotated/flipped tiles whose
        # (0,0) corner lands outside the expected cell; the centroid is stable.
        def grid_key(n):
            t = n.transform
            cx = t[0][0]*0.5 + t[0][1]*0.5 + t[0][2]
            cy = t[1][0]*0.5 + t[1][1]*0.5 + t[1][2]
            col = int(round(cx / cell_size - 0.5))
            row = int(round(cy / cell_size - 0.5))
            return (row, col)

        try:
            sorted_nodes = sorted(nodes, key=grid_key)
        except Exception:
            sorted_nodes = nodes  # fallback if positions are unexpected

        # find max label width for alignment
        labels = []
        for n in sorted_nodes:
            _, orient, flipped, _ = decompose(n.transform)
            labels.append(cell_label(orient, flipped))
        col_w = max(len(l) for l in labels) + 2

        print(f"\n  depth {d}  ({side}×{side}):")
        for row_idx in range(side):
            row_labels = labels[row_idx * side : (row_idx + 1) * side]
            print("    " + "".join(l.ljust(col_w) for l in row_labels))

def summarize_orientations(root, max_depth):
    """Print a compact grid of just orientations at each depth."""
    levels = [[] for _ in range(max_depth + 1)]
    def collect(node):
        levels[node.depth].append(node)
        for c in node.children:
            collect(c)
    collect(root)

    print("\n  Orientation summary (F=flipped):")
    for d, nodes in enumerate(levels):
        if not nodes:
            continue
        parts = []
        for n in nodes:
            _, orient, flipped, _ = decompose(n.transform)
            tag = f"{'F' if flipped else ' '}{snap_angle(orient):<5}"
            parts.append(tag)
        print(f"    d{d}: " + "  ".join(parts))

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Trace expansion tree of a spec pattern")
    parser.add_argument("pattern", help="Path to .spec.json pattern file")
    parser.add_argument("--depth", type=int, default=3, help="Expansion depth (default 3)")
    parser.add_argument("--state", action="store_true", help="Show state vector at each tile")
    parser.add_argument("--flat", action="store_true", help="Show flat per-depth tables instead of tree")
    parser.add_argument("--orient-only", action="store_true", help="Show compact orientation-only summary")
    parser.add_argument("--grid", action="store_true", help="Lay out each depth as a 2D grid (assumes 2×2 subdivision)")
    args = parser.parse_args()

    try:
        with open(args.pattern) as f:
            pattern = json.load(f)
    except Exception as e:
        print(f"Error loading pattern: {e}", file=sys.stderr)
        sys.exit(1)

    root_type = pattern["root_tile_type"]
    initial_state = pattern["initial_state"]
    root_t = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]

    root = TileNode(root_type, root_t, list(initial_state), 0, 0, "root")
    root.expand_to_depth(pattern, args.depth)

    print(f"\nPattern: {args.pattern}")
    print(f"Root:    {root_type}  depth={args.depth}")
    print(f"Notes:   orient is the rotation of the first basis vector")
    print(f"         FLIP = det < 0 (reflection component present)")

    if args.grid:
        print(f"  F prefix = reflection (det < 0),  angle = orientation of first basis vector\n")
        print_grid(root, args.depth)
    elif args.orient_only:
        summarize_orientations(root, args.depth)
    elif args.flat:
        print_flat_by_depth(root, args.depth, show_state=args.state)
    else:
        print()
        print_tree(root, show_state=args.state)
        print()
        summarize_orientations(root, args.depth)

if __name__ == "__main__":
    main()
