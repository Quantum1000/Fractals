/// Function call-graph acyclicity check (step 6).
///
/// Builds a directed graph FuncRef → Set<FuncRef> by walking each function
/// body for Call nodes, then performs Kahn's topological sort.  Reports any
/// cycle as a NormalizeError::RecursiveFunction.  Returns the function names
/// in topological order (callees before callers) on success.

use std::collections::{HashMap, HashSet, VecDeque};
use crate::ast::{File, Item, Expr, Spanned};
use super::NormalizeError;

pub fn check_acyclicity(
    file: &File,
    errors: &mut Vec<NormalizeError>,
) -> Vec<String> {
    // Collect all declared function names (in source order for stable output).
    let mut funcs: Vec<String> = Vec::new();
    for item in &file.items {
        if let Item::Function(f) = &item.node {
            funcs.push(f.name.0.node.clone());
        }
    }

    // Build adjacency: caller → set of callees.
    let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
    for name in &funcs {
        adj.insert(name.clone(), HashSet::new());
    }

    for item in &file.items {
        if let Item::Function(f) = &item.node {
            let caller = f.name.0.node.clone();
            collect_calls(&f.body, &funcs, adj.get_mut(&caller).unwrap());
        }
    }

    // Kahn's algorithm.
    // We want callees to appear before callers in the output order.
    // in_degree[v] = number of functions v calls (its callees must come first).
    // adj_rev[callee] = set of callers (so we can propagate when callee is processed).
    let mut in_degree: HashMap<String, usize> = funcs.iter()
        .map(|n| (n.clone(), adj[n].len()))
        .collect();

    let mut adj_rev: HashMap<String, Vec<String>> = funcs.iter()
        .map(|n| (n.clone(), Vec::new()))
        .collect();
    for (caller, callees) in &adj {
        for callee in callees {
            if let Some(v) = adj_rev.get_mut(callee) {
                v.push(caller.clone());
            }
        }
    }

    // Start with functions that call nothing.
    let mut queue: VecDeque<String> = funcs.iter()
        .filter(|n| in_degree[*n] == 0)
        .cloned()
        .collect();
    let mut order: Vec<String> = Vec::new();

    while let Some(callee) = queue.pop_front() {
        order.push(callee.clone());
        // Decrease in-degree of all callers that depended on this callee.
        for caller in adj_rev[&callee].clone() {
            let d = in_degree.get_mut(&caller).unwrap();
            *d -= 1;
            if *d == 0 {
                queue.push_back(caller.clone());
            }
        }
    }

    if order.len() != funcs.len() {
        // There's a cycle — find and report it.
        let remaining: Vec<String> = funcs.iter()
            .filter(|n| !order.contains(n))
            .cloned()
            .collect();
        errors.push(NormalizeError::RecursiveFunction { cycle: remaining });
        return order; // partial order, still useful
    }

    // Topological order is callees-before-callers; reverse to get callers-last.
    order
}

/// Walk an expression tree and add any direct function calls to `out`.
fn collect_calls(expr: &Spanned<Expr>, known: &[String], out: &mut HashSet<String>) {
    match &expr.node {
        Expr::Call { func, args } => {
            let name = &func.0.node;
            if known.contains(name) {
                out.insert(name.clone());
            }
            for arg in args { collect_calls(arg, known, out); }
        }
        // Recurse into all sub-expressions.
        Expr::Neg(e) | Expr::Sin(e) | Expr::Cos(e) | Expr::Exp(e)
        | Expr::Sqrt(e) | Expr::Abs(e) | Expr::Log(e) | Expr::Not(e)
        | Expr::Inverse(e) => collect_calls(e, known, out),

        Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) | Expr::Div(a,b)
        | Expr::Lt(a,b) | Expr::Gt(a,b) | Expr::Le(a,b) | Expr::Ge(a,b)
        | Expr::Eq(a,b) | Expr::Ne(a,b) | Expr::And(a,b) | Expr::Or(a,b)
        | Expr::Compose(a,b) | Expr::NullCoalesce(a,b) => {
            collect_calls(a, known, out);
            collect_calls(b, known, out);
        }

        Expr::Lerp(a,b,c) | Expr::Clamp(a,b,c) => {
            collect_calls(a, known, out);
            collect_calls(b, known, out);
            collect_calls(c, known, out);
        }

        Expr::If { condition, then_expr, else_expr } => {
            collect_calls(condition, known, out);
            collect_calls(then_expr, known, out);
            collect_calls(else_expr, known, out);
        }

        // Leaves — nothing to recurse
        _ => {}
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn test_no_functions() {
        let (file, _) = parse("");
        let mut errors = Vec::new();
        let order = check_acyclicity(&file, &mut errors);
        assert!(errors.is_empty());
        assert!(order.is_empty());
    }

    #[test]
    fn test_two_functions_no_cycle() {
        let src = "fn double(x) { x * 2 }\nfn quad(x) { double(double(x)) }";
        let (file, _) = parse(src);
        let mut errors = Vec::new();
        let order = check_acyclicity(&file, &mut errors);
        assert!(errors.is_empty(), "{:?}", errors);
        // double must appear before quad
        let di = order.iter().position(|s| s == "double").unwrap();
        let qi = order.iter().position(|s| s == "quad").unwrap();
        assert!(di < qi);
    }
}
