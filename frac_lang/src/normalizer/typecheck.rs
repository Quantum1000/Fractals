/// Type-checking + nullability inference (step 5).
///
/// Every expression has a `Type` (Float | Bool | GroupElem | Perm) and a
/// `Nullable` flag (does it evaluate to `none` for some inputs?).
///
/// Rules:
///  - Literals are non-nullable with their concrete type.
///  - State variables are always nullable (they are `Option<T>`).
///  - Arithmetic/group ops propagate nullability from their arguments.
///  - `x ? default` yields the type of x with non-nullable result if `default`
///    is non-nullable.
///  - Color components must be Float and non-nullable (hard error).
///  - `slot_order` / `alignment` expressions must be `Perm` or `GroupElem`.
///  - State updates and child injections must match the variable's type.
///
/// This pass does NOT attempt to be a full Hindley-Milner inference; it
/// propagates concrete types bottom-up and reports mismatches.

use std::collections::HashMap;
use crate::ast::*;
use super::{NormalizeError, NormalizedPartition};

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Float,
    Bool,
    GroupElem(SymmetryGroup),
    Perm(u32), // Perm(n) — a permutation of n items
    TileRef,   // §10.1 — a tile-type identifier
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Float => write!(f, "float"),
            Type::Bool => write!(f, "bool"),
            Type::GroupElem(g) => write!(f, "group_elem({g:?})"),
            Type::Perm(n) => write!(f, "perm({n})"),
            Type::TileRef => write!(f, "tile_ref"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Nullable { Yes, No }

impl Nullable {
    fn or(self, other: Nullable) -> Nullable {
        if self == Nullable::Yes || other == Nullable::Yes { Nullable::Yes } else { Nullable::No }
    }
}

type Env = HashMap<String, (Type, Nullable)>;

pub fn check(
    file: &File,
    partitions: &HashMap<(String, String), NormalizedPartition>,
    errors: &mut Vec<NormalizeError>,
) {
    // Collect tile names for §10.2 tile-ref literal resolution (§5.4 step 3).
    let tile_names: std::collections::HashSet<String> = file.items.iter()
        .filter_map(|item| match &item.node {
            Item::Tile(t) => Some(t.name.0.node.clone()),
            _ => None,
        })
        .collect();

    for item in &file.items {
        if let Item::Pattern(pat) = &item.node {
            check_pattern(pat, partitions, &tile_names, errors);
        }
    }
}

fn check_pattern(
    pat: &PatternDecl,
    partitions: &HashMap<(String, String), NormalizedPartition>,
    tile_names: &std::collections::HashSet<String>,
    errors: &mut Vec<NormalizeError>,
) {
    // Build environment from state block.
    // Variables default to (Float, Nullable::Yes); initial exprs refine the type.
    let mut env: Env = HashMap::new();
    for sv in &pat.state.node.vars {
        let name = sv.node.name.0.node.clone();
        let (ty, nullable) = match &sv.node.initial {
            None => (Type::Float, Nullable::Yes),
            Some(init) => {
                let (ty, n) = infer_expr(&init.node, &env, tile_names, errors);
                (ty, n)
            }
        };
        env.insert(name, (ty, nullable));
    }

    // Check color expression — all 4 components must be (Float, No).
    for (comp, expr) in [
        ("r", &pat.color.node.r),
        ("g", &pat.color.node.g),
        ("b", &pat.color.node.b),
        ("a", &pat.color.node.a),
    ] {
        let (ty, nullable) = infer_expr(&expr.node, &env, tile_names, errors);
        if ty != Type::Float {
            errors.push(NormalizeError::TypeMismatch {
                expected: "float".into(),
                found: ty.to_string(),
                span: expr.span,
            });
        }
        if nullable == Nullable::Yes {
            errors.push(NormalizeError::NullableColor { component: comp, span: expr.span });
        }
    }

    // Check rules.
    for rule in &pat.rules {
        let tile_name = rule.node.tile.0.node.clone();
        if !tile_names.contains(&tile_name) {
            errors.push(NormalizeError::UnknownTile {
                name: tile_name.clone(),
                span: rule.node.tile.0.span,
            });
        }
        check_rule_body(&rule.node.body, &env, partitions, tile_names, &tile_name, errors);
    }
}

fn check_rule_body(
    body: &Spanned<RuleBody>,
    env: &Env,
    partitions: &HashMap<(String, String), NormalizedPartition>,
    tile_names: &std::collections::HashSet<String>,
    tile_name: &str,
    errors: &mut Vec<NormalizeError>,
) {
    match &body.node {
        RuleBody::If { condition, then_branch, else_branch } => {
            let (ty, _) = infer_expr(&condition.node, env, tile_names, errors);
            if ty != Type::Bool {
                errors.push(NormalizeError::TypeMismatch {
                    expected: "bool".into(),
                    found: ty.to_string(),
                    span: condition.span,
                });
            }
            check_rule_body(then_branch, env, partitions, tile_names, tile_name, errors);
            check_rule_body(else_branch, env, partitions, tile_names, tile_name, errors);
        }
        RuleBody::Substitute(sub) => {
            check_substitution(&sub.node, env, partitions, tile_names, errors);
        }
    }
}

fn check_substitution(
    sub: &Substitution,
    env: &Env,
    partitions: &HashMap<(String, String), NormalizedPartition>,
    tile_names: &std::collections::HashSet<String>,
    errors: &mut Vec<NormalizeError>,
) {
    // Validate the substitution targets a real tile and partition.
    let sub_tile = &sub.tile.0.node;
    let sub_part = &sub.partition.0.node;
    if !tile_names.contains(sub_tile) {
        errors.push(NormalizeError::UnknownTile {
            name: sub_tile.clone(),
            span: sub.tile.0.span,
        });
    } else if !partitions.contains_key(&(sub_tile.clone(), sub_part.clone())) {
        errors.push(NormalizeError::UnknownPartition {
            tile: sub_tile.clone(),
            name: sub_part.clone(),
            span: sub.partition.0.span,
        });
    }

    // Apply tile-level updates sequentially (each update visible to subsequent ones).
    let mut env = env.clone();

    for upd in &sub.updates {
        let var_name = &upd.node.var.0.node;
        let (ty, nullable) = infer_expr(&upd.node.value.node, &env, tile_names, errors);
        if let Some((expected_ty, _)) = env.get(var_name.as_str()) {
            if *expected_ty != ty {
                errors.push(NormalizeError::TypeMismatch {
                    expected: expected_ty.to_string(),
                    found: ty.to_string(),
                    span: upd.node.value.span,
                });
            }
        }
        env.insert(var_name.clone(), (ty, nullable));
    }

    // Build child context env: add child.index and child.canonical_orientation.
    let mut child_env = env.clone();
    child_env.insert("_child_index".into(), (Type::Float, Nullable::No));
    // canonical_orientation type is inferred as GroupElem — we use Float as a
    // stand-in since we don't know the tile type's symmetry group here.
    child_env.insert("_child_orientation".into(), (Type::Float, Nullable::No));

    // Check child injections.
    for inj in &sub.child_injections {
        for upd in &inj.node.updates {
            let var_name = &upd.node.var.0.node;
            let (ty, _) = infer_expr(&upd.node.value.node, &child_env, tile_names, errors);
            if let Some((expected_ty, _)) = env.get(var_name.as_str()) {
                if *expected_ty != ty {
                    errors.push(NormalizeError::TypeMismatch {
                        expected: expected_ty.to_string(),
                        found: ty.to_string(),
                        span: upd.node.value.span,
                    });
                }
            }
        }
        // Alignment expression, if present, must be Perm or GroupElem.
        if let Some(align) = &inj.node.alignment {
            let (ty, _) = infer_expr(&align.node, &child_env, tile_names, errors);
            if !matches!(ty, Type::Perm(_) | Type::GroupElem(_)) {
                errors.push(NormalizeError::TypeMismatch {
                    expected: "perm or group_elem".into(),
                    found: ty.to_string(),
                    span: align.span,
                });
            }
        }
        // tile_override expression, if present, must be TileRef.
        if let Some(tile_ov) = &inj.node.tile_override {
            let (ty, _) = infer_expr(&tile_ov.node, &child_env, tile_names, errors);
            if ty != Type::TileRef {
                errors.push(NormalizeError::TypeMismatch {
                    expected: "tile_ref".into(),
                    found: ty.to_string(),
                    span: tile_ov.span,
                });
            }
        }
    }

    // Check slot_order expressions — must be Perm or GroupElem.
    for so in &sub.slot_orders {
        let (ty, _) = infer_expr(&so.node.value.node, &env, tile_names, errors);
        if !matches!(ty, Type::Perm(_) | Type::GroupElem(_)) {
            errors.push(NormalizeError::InvalidSlotOrder {
                span: so.node.value.span,
                reason: format!("expected perm or group_elem, found {ty}"),
            });
        }
    }
}

/// Infer the type and nullability of an expression, reporting errors for
/// obvious type mismatches.  Returns (Type::Float, Nullable::Yes) as the
/// "error recovery" type so downstream checks can continue.
pub fn infer_expr(
    expr: &Expr,
    env: &Env,
    tile_names: &std::collections::HashSet<String>,
    errors: &mut Vec<NormalizeError>,
) -> (Type, Nullable) {
    use Expr::*;
    match expr {
        // Literals
        Float(_)   => (Type::Float, Nullable::No),
        Bool(_)    => (Type::Bool,  Nullable::No),
        None       => (Type::Float, Nullable::Yes), // `none` is polymorphic; use Float as default
        GroupElem { group, .. } => {
            let g = parse_group_name(&group.node);
            (Type::GroupElem(g), Nullable::No)
        }
        PermLiteral(elems) => (Type::Perm(elems.len() as u32), Nullable::No),

        // Variable access — resolves per §5.4: params/state first, then tile names.
        Var(vref) => {
            let name = &vref.0.node;
            match env.get(name.as_str()) {
                Some((ty, n)) => (ty.clone(), *n),
                Option::None => {
                    // §5.4 step 3: bare tile name → tile_ref (non-nullable)
                    if tile_names.contains(name.as_str()) {
                        (Type::TileRef, Nullable::No)
                    } else {
                        (Type::Float, Nullable::Yes) // unknown — acyclicity pass will catch
                    }
                }
            }
        }

        // Built-in geometric inputs — Float, non-nullable
        PosX | PosY | Scale | Orientation | Shear | Stretch | Depth | Random => {
            (Type::Float, Nullable::No)
        }
        ChildIndex => (Type::Float, Nullable::No),
        // canonical_orientation is a group element — we use Float as a stand-in
        ChildCanonicalOrientation => (Type::Float, Nullable::No),

        // Arithmetic — Float in, Float out; propagate nullability
        Neg(e)  => { let (_, n) = infer_expr(&e.node, env, tile_names, errors); (Type::Float, n) }
        Add(a,b)|Sub(a,b)|Mul(a,b)|Div(a,b) => {
            let (ta, na) = infer_expr(&a.node, env, tile_names, errors);
            let (tb, nb) = infer_expr(&b.node, env, tile_names, errors);
            expect_float_or_group(a.span, &ta, errors);
            expect_float_or_group(b.span, &tb, errors);
            (ta, na.or(nb))
        }

        // Math functions — Float → Float
        Sin(e)|Cos(e)|Exp(e)|Sqrt(e)|Abs(e)|Log(e) => {
            let (ty, n) = infer_expr(&e.node, env, tile_names, errors);
            if ty != Type::Float { errors.push(NormalizeError::TypeMismatch {
                expected: "float".into(), found: ty.to_string(), span: e.span });
            }
            (Type::Float, n)
        }

        Lerp(a, b, t) => {
            let (_, na) = infer_expr(&a.node, env, tile_names, errors);
            let (_, nb) = infer_expr(&b.node, env, tile_names, errors);
            let (_, nt) = infer_expr(&t.node, env, tile_names, errors);
            (Type::Float, na.or(nb).or(nt))
        }
        Clamp(x, lo, hi) => {
            let (_, nx) = infer_expr(&x.node, env, tile_names, errors);
            let (_, nl) = infer_expr(&lo.node, env, tile_names, errors);
            let (_, nh) = infer_expr(&hi.node, env, tile_names, errors);
            (Type::Float, nx.or(nl).or(nh))
        }

        // Null coalescing: `x ? default`
        NullCoalesce(x, default) => {
            let (tx, _) = infer_expr(&x.node, env, tile_names, errors);
            let (_, nd) = infer_expr(&default.node, env, tile_names, errors);
            (tx, nd)
        }

        // Comparisons → Bool
        Lt(a,b)|Gt(a,b)|Le(a,b)|Ge(a,b) => {
            let (_, na) = infer_expr(&a.node, env, tile_names, errors);
            let (_, nb) = infer_expr(&b.node, env, tile_names, errors);
            (Type::Bool, na.or(nb))
        }
        Eq(a,b)|Ne(a,b) => {
            let (_, na) = infer_expr(&a.node, env, tile_names, errors);
            let (_, nb) = infer_expr(&b.node, env, tile_names, errors);
            (Type::Bool, na.or(nb))
        }

        // Logic → Bool
        And(a,b)|Or(a,b) => {
            let (_, na) = infer_expr(&a.node, env, tile_names, errors);
            let (_, nb) = infer_expr(&b.node, env, tile_names, errors);
            (Type::Bool, na.or(nb))
        }
        Not(e) => { let (_, n) = infer_expr(&e.node, env, tile_names, errors); (Type::Bool, n) }

        // Conditional — branches must agree on type
        If { condition, then_expr, else_expr } => {
            let (_, nc) = infer_expr(&condition.node, env, tile_names, errors);
            let (tt, nt) = infer_expr(&then_expr.node, env, tile_names, errors);
            let (_, ne) = infer_expr(&else_expr.node, env, tile_names, errors);
            (tt, nc.or(nt).or(ne))
        }

        // Group / perm operations
        Compose(a, b) => {
            let (ta, na) = infer_expr(&a.node, env, tile_names, errors);
            let (_, nb) = infer_expr(&b.node, env, tile_names, errors);
            (ta, na.or(nb))
        }
        Inverse(e) => {
            let (ty, n) = infer_expr(&e.node, env, tile_names, errors);
            (ty, n)
        }

        // User-defined function call — return Float (conservative)
        Call { args, .. } => {
            let mut n = Nullable::No;
            for arg in args {
                let (_, na) = infer_expr(&arg.node, env, tile_names, errors);
                n = n.or(na);
            }
            (Type::Float, n)
        }

        Error(_) => (Type::Float, Nullable::Yes),
    }
}

fn expect_float_or_group(span: Span, ty: &Type, errors: &mut Vec<NormalizeError>) {
    match ty {
        Type::Float | Type::GroupElem(_) | Type::Perm(_) => {}
        Type::Bool | Type::TileRef => errors.push(NormalizeError::TypeMismatch {
            expected: "float or group_elem".into(),
            found: ty.to_string(),
            span,
        }),
    }
}

fn parse_group_name(s: &str) -> SymmetryGroup {
    if s == "trivial" { return SymmetryGroup::Trivial; }
    if let Some(n) = s.strip_prefix('d').and_then(|s| s.parse::<u32>().ok()) {
        return SymmetryGroup::Dihedral(n);
    }
    if let Some(n) = s.strip_prefix('c').and_then(|s| s.parse::<u32>().ok()) {
        return SymmetryGroup::Cyclic(n);
    }
    SymmetryGroup::Trivial
}
