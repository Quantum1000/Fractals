/// Expression evaluator.
///
/// Evaluates an `Expr` AST node against an `EvalContext` and returns a
/// `Value`.  All values are `Option<_>` — `None` represents the `.frac`
/// `none` literal and propagates through arithmetic.

use std::collections::HashMap;
use crate::ast::*;
use super::Value;

/// Everything the expression evaluator can read.
pub struct EvalContext<'a> {
    /// Current tile's state (variable name → optional value).
    pub state: &'a HashMap<String, Option<Value>>,
    /// Current tile's centroid in world space.
    pub pos: [f64; 2],
    /// Geometric parameters from the similarity transform.
    pub scale: f64,
    pub orientation: f64,  // degrees
    pub shear: f64,
    pub stretch: f64,
    /// Recursion depth (root = 0).
    pub depth: u32,
    /// Deterministic random value for this tile instance (0.0 … 1.0).
    pub random: f64,
    /// `child.index` — only valid inside child injection blocks.
    pub child_index: Option<u32>,
    /// `child.canonical_orientation` — only valid inside child injection blocks.
    pub child_orientation: Option<Value>,
    /// User-defined functions: name → (params, body).
    pub functions: &'a HashMap<String, (Vec<String>, Spanned<Expr>)>,
}

pub fn eval(expr: &Expr, ctx: &EvalContext) -> Option<Value> {
    use Expr::*;
    match expr {
        // Literals
        Float(f)  => Some(Value::Float(*f)),
        Bool(b)   => Some(Value::Bool(*b)),
        None      => Option::None,
        GroupElem { group, element } => {
            Some(Value::GroupElem { group: group.node.clone(), element: element.node.clone() })
        }
        PermLiteral(elems) => {
            let perm: Vec<u32> = elems.iter().map(|e| e.node).collect();
            Some(Value::Perm(perm))
        }

        // Variable access
        Var(vref) => {
            ctx.state.get(&vref.0.node).cloned().flatten()
        }

        // Geometric inputs
        PosX        => Some(Value::Float(ctx.pos[0])),
        PosY        => Some(Value::Float(ctx.pos[1])),
        Scale       => Some(Value::Float(ctx.scale)),
        Orientation => Some(Value::Float(ctx.orientation)),
        Shear       => Some(Value::Float(ctx.shear)),
        Stretch     => Some(Value::Float(ctx.stretch)),
        Depth       => Some(Value::Float(ctx.depth as f64)),
        Random      => Some(Value::Float(ctx.random)),
        ChildIndex  => ctx.child_index.map(|i| Value::Float(i as f64)),
        ChildCanonicalOrientation => ctx.child_orientation.clone(),

        // Arithmetic — propagate None
        Neg(e) => {
            let v = eval_float(&e.node, ctx)?;
            Some(Value::Float(-v))
        }
        Add(a, b) => bin_float(a, b, ctx, |x,y| x+y),
        Sub(a, b) => bin_float(a, b, ctx, |x,y| x-y),
        Mul(a, b) => {
            // Allow group_elem * float or perm * float in future; for now Float * Float.
            let va = eval(&a.node, ctx)?;
            let vb = eval(&b.node, ctx)?;
            match (va, vb) {
                (Value::Float(x), Value::Float(y)) => Some(Value::Float(x * y)),
                _ => Option::None,
            }
        }
        Div(a, b) => bin_float(a, b, ctx, |x,y| if y.abs() < 1e-15 { 0.0 } else { x/y }),

        // Math functions
        Sin(e)  => Some(Value::Float(eval_float(&e.node, ctx)?.to_radians().sin())),
        Cos(e)  => Some(Value::Float(eval_float(&e.node, ctx)?.to_radians().cos())),
        Exp(e)  => Some(Value::Float(eval_float(&e.node, ctx)?.exp())),
        Sqrt(e) => Some(Value::Float(eval_float(&e.node, ctx)?.max(0.0).sqrt())),
        Abs(e)  => Some(Value::Float(eval_float(&e.node, ctx)?.abs())),
        Log(e)  => Some(Value::Float(eval_float(&e.node, ctx)?.max(f64::EPSILON).ln())),

        Lerp(a, b, t) => {
            let va = eval_float(&a.node, ctx)?;
            let vb = eval_float(&b.node, ctx)?;
            let vt = eval_float(&t.node, ctx)?;
            Some(Value::Float(va + (vb - va) * vt))
        }
        Clamp(x, lo, hi) => {
            let vx  = eval_float(&x.node, ctx)?;
            let vlo = eval_float(&lo.node, ctx)?;
            let vhi = eval_float(&hi.node, ctx)?;
            Some(Value::Float(vx.clamp(vlo, vhi)))
        }

        // Null coalescing
        NullCoalesce(x, default) => {
            eval(&x.node, ctx).or_else(|| eval(&default.node, ctx))
        }

        // Comparisons
        Lt(a,b) => cmp_float(a, b, ctx, |x,y| x < y),
        Gt(a,b) => cmp_float(a, b, ctx, |x,y| x > y),
        Le(a,b) => cmp_float(a, b, ctx, |x,y| x <= y),
        Ge(a,b) => cmp_float(a, b, ctx, |x,y| x >= y),
        Eq(a,b) => {
            let va = eval(&a.node, ctx)?;
            let vb = eval(&b.node, ctx)?;
            Some(Value::Bool(va == vb))
        }
        Ne(a,b) => {
            let va = eval(&a.node, ctx)?;
            let vb = eval(&b.node, ctx)?;
            Some(Value::Bool(va != vb))
        }

        // Logic
        And(a,b) => {
            let va = eval_bool(&a.node, ctx)?;
            // Short-circuit: if false, don't eval b.
            if !va { return Some(Value::Bool(false)); }
            let vb = eval_bool(&b.node, ctx)?;
            Some(Value::Bool(vb))
        }
        Or(a,b) => {
            let va = eval_bool(&a.node, ctx)?;
            if va { return Some(Value::Bool(true)); }
            let vb = eval_bool(&b.node, ctx)?;
            Some(Value::Bool(vb))
        }
        Not(e) => Some(Value::Bool(!eval_bool(&e.node, ctx)?)),

        // Conditional
        If { condition, then_expr, else_expr } => {
            let cond = eval_bool(&condition.node, ctx)?;
            if cond { eval(&then_expr.node, ctx) } else { eval(&else_expr.node, ctx) }
        }

        // Group / perm operations
        Compose(a, b) => {
            let va = eval(&a.node, ctx)?;
            let vb = eval(&b.node, ctx)?;
            compose_values(va, vb)
        }
        Inverse(e) => {
            let v = eval(&e.node, ctx)?;
            inverse_value(v)
        }

        // User-defined function call
        Call { func, args } => {
            let fname = &func.0.node;
            let (params, body) = ctx.functions.get(fname.as_str())?;
            if args.len() != params.len() { return Option::None; }

            let mut call_state: HashMap<String, Option<Value>> = HashMap::new();
            for (param, arg_expr) in params.iter().zip(args.iter()) {
                call_state.insert(param.clone(), eval(&arg_expr.node, ctx));
            }
            let call_ctx = EvalContext {
                state: &call_state,
                pos: ctx.pos,
                scale: ctx.scale,
                orientation: ctx.orientation,
                shear: ctx.shear,
                stretch: ctx.stretch,
                depth: ctx.depth,
                random: ctx.random,
                child_index: Option::None,
                child_orientation: Option::None,
                functions: ctx.functions,
            };
            eval(&body.node, &call_ctx)
        }

        Error(_) => Option::None,
    }
}

// ── Public helpers ────────────────────────────────────────────────────────────

/// Evaluate an expression and coerce to f64.  Used by eval.rs for color eval.
pub fn eval_as_float(expr: &Expr, ctx: &EvalContext) -> Option<f64> {
    match eval(expr, ctx)? {
        Value::Float(f) => Some(f),
        Value::Bool(b) => Some(if b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn eval_float(expr: &Expr, ctx: &EvalContext) -> Option<f64> {
    match eval(expr, ctx)? {
        Value::Float(f) => Some(f),
        _ => Option::None,
    }
}

fn eval_bool(expr: &Expr, ctx: &EvalContext) -> Option<bool> {
    match eval(expr, ctx)? {
        Value::Bool(b) => Some(b),
        Value::Float(f) => Some(f != 0.0),
        _ => Option::None,
    }
}

fn bin_float(
    a: &Spanned<Expr>,
    b: &Spanned<Expr>,
    ctx: &EvalContext,
    f: impl Fn(f64, f64) -> f64,
) -> Option<Value> {
    let va = eval_float(&a.node, ctx)?;
    let vb = eval_float(&b.node, ctx)?;
    Some(Value::Float(f(va, vb)))
}

fn cmp_float(
    a: &Spanned<Expr>,
    b: &Spanned<Expr>,
    ctx: &EvalContext,
    f: impl Fn(f64, f64) -> bool,
) -> Option<Value> {
    let va = eval_float(&a.node, ctx)?;
    let vb = eval_float(&b.node, ctx)?;
    Some(Value::Bool(f(va, vb)))
}

fn compose_values(a: Value, b: Value) -> Option<Value> {
    match (a, b) {
        (Value::Perm(pa), Value::Perm(pb)) => {
            if pa.len() != pb.len() { return Option::None; }
            // compose: result[i] = pa[pb[i]]
            let c: Vec<u32> = pb.iter().map(|&j| pa[j as usize]).collect();
            Some(Value::Perm(c))
        }
        (Value::GroupElem { group: ga, element: ea }, Value::GroupElem { group: gb, element: eb }) => {
            if ga != gb { return Option::None; }
            // Compose group elements symbolically — for the evaluator we just
            // store the composed name as "compose(ea,eb)" and let the renderer
            // resolve it via the group_perm map.
            Some(Value::GroupElem { group: ga, element: format!("compose({ea},{eb})") })
        }
        _ => Option::None,
    }
}

fn inverse_value(v: Value) -> Option<Value> {
    match v {
        Value::Perm(p) => {
            let n = p.len();
            let mut inv = vec![0u32; n];
            for (i, &j) in p.iter().enumerate() {
                inv[j as usize] = i as u32;
            }
            Some(Value::Perm(inv))
        }
        Value::GroupElem { group, element } => {
            Some(Value::GroupElem { group, element: format!("inverse({element})") })
        }
        _ => Option::None,
    }
}
