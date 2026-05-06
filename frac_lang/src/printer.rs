use crate::ast::*;

// ---------------------------------------------------------------------------
// Indentation helper
// ---------------------------------------------------------------------------

struct Printer {
    buf: String,
    indent: usize,
}

impl Printer {
    fn new() -> Self {
        Self { buf: String::new(), indent: 0 }
    }

    fn indent(&mut self)   { self.indent += 1; }
    fn dedent(&mut self)   { self.indent = self.indent.saturating_sub(1); }

    fn line(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.buf.push_str("    ");
        }
        self.buf.push_str(s);
        self.buf.push('\n');
    }

}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn print_file(file: &File) -> String {
    let mut p = Printer::new();
    let mut first = true;
    for item in &file.items {
        if !first { p.buf.push('\n'); }
        first = false;
        print_item(&mut p, &item.node);
    }
    p.buf
}

// ---------------------------------------------------------------------------
// Items
// ---------------------------------------------------------------------------

fn print_item(p: &mut Printer, item: &Item) {
    match item {
        Item::Tile(t)      => print_tile_decl(p, t),
        Item::Partition(d) => print_partition_decl(p, d),
        Item::Function(f)  => print_function_decl(p, f),
        Item::Pattern(pat) => print_pattern_decl(p, pat),
        Item::Error(e)     => p.line(&format!("# parse error: {}", e.message)),
    }
}

// ---------------------------------------------------------------------------
// Tile
// ---------------------------------------------------------------------------

fn print_tile_decl(p: &mut Printer, t: &TileDecl) {
    p.line(&format!("tile {} {{", t.name.0.node));
    p.indent();
    p.line(&format!("sides {}", t.sides.node));

    let pts: Vec<String> = t.canonical.iter().map(|pt| fmt_point(&pt.node)).collect();
    p.line(&format!("canonical {}", pts.join(" ")));

    if let Some(sym) = &t.symmetry {
        p.line(&format!("symmetry {}", fmt_symmetry(&sym.node)));
    }
    p.dedent();
    p.line("}");
}

fn fmt_point(pt: &Point) -> String {
    format!("({}, {})", fmt_f64(pt.x), fmt_f64(pt.y))
}

fn fmt_symmetry(g: &SymmetryGroup) -> String {
    match g {
        SymmetryGroup::Trivial    => "trivial".into(),
        SymmetryGroup::Cyclic(n)  => format!("c{n}"),
        SymmetryGroup::Dihedral(n)=> format!("d{n}"),
    }
}

// ---------------------------------------------------------------------------
// Partition
// ---------------------------------------------------------------------------

fn print_partition_decl(p: &mut Printer, d: &PartitionDecl) {
    p.line(&format!("partition {}.{} {{", d.tile.0.node, d.name.0.node));
    p.indent();

    for v in &d.vertices {
        let kind = match v.node.kind { VertexKind::Edge => "edge", VertexKind::Interior => "interior" };
        p.line(&format!("{} {} = {}", kind, v.node.name.0.node, fmt_point(&v.node.pos.node)));
    }

    for c in &d.cuts {
        p.line(&format!("cut {} -- {}", c.node.from.0.node, c.node.to.0.node));
    }

    for cn in &d.child_names {
        p.line(&format!("child {} = {}", cn.node.index.node, cn.node.name.0.node));
    }

    p.dedent();
    p.line("}");
}

// ---------------------------------------------------------------------------
// Function
// ---------------------------------------------------------------------------

fn print_function_decl(p: &mut Printer, f: &FunctionDecl) {
    let params: Vec<&str> = f.params.iter().map(|v| v.0.node.as_str()).collect();
    p.line(&format!("fn {}({}) {{", f.name.0.node, params.join(", ")));
    p.indent();
    let body = fmt_expr(&f.body.node);
    p.line(&body);
    p.dedent();
    p.line("}");
}

// ---------------------------------------------------------------------------
// Pattern
// ---------------------------------------------------------------------------

fn print_pattern_decl(p: &mut Printer, pat: &PatternDecl) {
    p.line("pattern {");
    p.indent();

    p.line(&format!("root {}", pat.root.0.node));

    p.buf.push('\n');
    print_state_block(p, &pat.state.node);

    p.buf.push('\n');
    let ce = &pat.color.node;
    p.line(&format!("color ({}, {}, {}, {})",
        fmt_expr(&ce.r.node), fmt_expr(&ce.g.node),
        fmt_expr(&ce.b.node), fmt_expr(&ce.a.node)));

    for rule in &pat.rules {
        p.buf.push('\n');
        print_rule_decl(p, &rule.node);
    }

    p.dedent();
    p.line("}");
}

fn print_state_block(p: &mut Printer, s: &StateBlock) {
    p.line("state {");
    p.indent();
    for var in &s.vars {
        match &var.node.initial {
            Some(init) => p.line(&format!("{} = {}", var.node.name.0.node, fmt_expr(&init.node))),
            None       => p.line(&var.node.name.0.node),
        }
    }
    p.dedent();
    p.line("}");
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

fn print_rule_decl(p: &mut Printer, rule: &RuleDecl) {
    // Shorthand: if body is a single substitution with no wrapping braces needed.
    if let RuleBody::Substitute(ref sub) = rule.body.node {
        print_substitution_line(p, &format!("rule {} ", rule.tile.0.node), &sub.node);
    } else {
        p.line(&format!("rule {} {{", rule.tile.0.node));
        p.indent();
        print_rule_body(p, &rule.body.node);
        p.dedent();
        p.line("}");
    }
}

fn print_rule_body(p: &mut Printer, body: &RuleBody) {
    match body {
        RuleBody::If { condition, then_branch, else_branch } => {
            p.line(&format!("if {} {{", fmt_expr(&condition.node)));
            p.indent();
            print_rule_body(p, &then_branch.node);
            p.dedent();
            p.line("} else {");
            p.indent();
            print_rule_body(p, &else_branch.node);
            p.dedent();
            p.line("}");
        }
        RuleBody::Substitute(sub) => {
            print_substitution_line(p, "", &sub.node);
        }
    }
}

// Prints `<prefix>-> tile.partition { ... }`, using the shorthand single-line
// form if the substitution body is empty, otherwise a full block.
fn print_substitution_line(p: &mut Printer, prefix: &str, sub: &Substitution) {
    let arrow = format!("{}-> {}.{}", prefix, sub.tile.0.node, sub.partition.0.node);

    let body_empty = sub.updates.is_empty()
        && sub.child_injections.is_empty()
        && sub.slot_orders.is_empty();

    if body_empty {
        p.line(&format!("{} {{}}", arrow));
        return;
    }

    p.line(&format!("{} {{", arrow));
    p.indent();

    for so in &sub.slot_orders {
        print_slot_order(p, &so.node);
    }
    for upd in &sub.updates {
        p.line(&format!("{} = {}", upd.node.var.0.node, fmt_expr(&upd.node.value.node)));
    }
    for ci in &sub.child_injections {
        print_child_injection(p, &ci.node);
    }

    p.dedent();
    p.line("}");
}

fn print_slot_order(p: &mut Printer, so: &SlotOrder) {
    let target = match &so.target {
        TileClassRef::All        => String::new(),
        TileClassRef::Tile(t)    => format!(" {}", t.0.node),
        TileClassRef::Class(t)   => format!(" {}.class", t.0.node),
    };
    p.line(&format!("slot_order{} = {}", target, fmt_expr(&so.value.node)));
}

fn print_child_injection(p: &mut Printer, ci: &ChildInjection) {
    let child = match &ci.child.node {
        ChildRef::Name(n)  => n.0.node.clone(),
        ChildRef::Index(i) => i.to_string(),
    };

    let body_empty = ci.updates.is_empty() && ci.alignment.is_none();
    if body_empty {
        p.line(&format!("child {} {{}}", child));
        return;
    }

    p.line(&format!("child {} {{", child));
    p.indent();
    if let Some(al) = &ci.alignment {
        p.line(&format!("alignment = {}", fmt_expr(&al.node)));
    }
    for upd in &ci.updates {
        p.line(&format!("{} = {}", upd.node.var.0.node, fmt_expr(&upd.node.value.node)));
    }
    p.dedent();
    p.line("}");
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

pub fn fmt_expr(expr: &Expr) -> String {
    match expr {
        Expr::Float(f)            => fmt_f64(*f),
        Expr::Bool(b)             => b.to_string(),
        Expr::None                => "none".into(),
        Expr::GroupElem { group, element } => format!("{}.{}", group.node, element.node),
        Expr::PermLiteral(idxs)   => {
            let inner: Vec<String> = idxs.iter().map(|i| i.node.to_string()).collect();
            format!("perm [{}]", inner.join(", "))
        }

        Expr::Var(v)              => v.0.node.clone(),
        Expr::PosX                => "pos.x".into(),
        Expr::PosY                => "pos.y".into(),
        Expr::Scale               => "scale".into(),
        Expr::Orientation         => "orientation".into(),
        Expr::Shear               => "shear".into(),
        Expr::Stretch             => "stretch".into(),
        Expr::Depth               => "depth".into(),
        Expr::Random              => "random".into(),
        Expr::ChildCanonicalOrientation => "child.canonical_orientation".into(),
        Expr::ChildIndex          => "child.index".into(),

        Expr::Neg(e)              => format!("-{}", fmt_unary_operand(e)),
        Expr::Not(e)              => format!("!{}", fmt_unary_operand(e)),

        Expr::Add(l, r)           => fmt_binop(l, "+",  r, 6),
        Expr::Sub(l, r)           => fmt_binop(l, "-",  r, 6),
        Expr::Mul(l, r)           => fmt_binop(l, "*",  r, 7),
        Expr::Div(l, r)           => fmt_binop(l, "/",  r, 7),
        Expr::Lt(l, r)            => fmt_binop(l, "<",  r, 5),
        Expr::Gt(l, r)            => fmt_binop(l, ">",  r, 5),
        Expr::Le(l, r)            => fmt_binop(l, "<=", r, 5),
        Expr::Ge(l, r)            => fmt_binop(l, ">=", r, 5),
        Expr::Eq(l, r)            => fmt_binop(l, "==", r, 4),
        Expr::Ne(l, r)            => fmt_binop(l, "!=", r, 4),
        Expr::And(l, r)           => fmt_binop(l, "&&", r, 3),
        Expr::Or(l, r)            => fmt_binop(l, "||", r, 2),
        Expr::NullCoalesce(l, r)  => fmt_binop(l, "?",  r, 1),

        Expr::Sin(e)     => format!("sin({})",     fmt_expr(&e.node)),
        Expr::Cos(e)     => format!("cos({})",     fmt_expr(&e.node)),
        Expr::Exp(e)     => format!("exp({})",     fmt_expr(&e.node)),
        Expr::Sqrt(e)    => format!("sqrt({})",    fmt_expr(&e.node)),
        Expr::Abs(e)     => format!("abs({})",     fmt_expr(&e.node)),
        Expr::Log(e)     => format!("log({})",     fmt_expr(&e.node)),
        Expr::Inverse(e) => format!("inverse({})", fmt_expr(&e.node)),

        Expr::Lerp(a, b, t) => format!("lerp({}, {}, {})",
            fmt_expr(&a.node), fmt_expr(&b.node), fmt_expr(&t.node)),
        Expr::Clamp(x, lo, hi) => format!("clamp({}, {}, {})",
            fmt_expr(&x.node), fmt_expr(&lo.node), fmt_expr(&hi.node)),
        Expr::Compose(a, b) => format!("compose({}, {})",
            fmt_expr(&a.node), fmt_expr(&b.node)),

        Expr::If { condition, then_expr, else_expr } => format!(
            "if {} {{ {} }} else {{ {} }}",
            fmt_expr(&condition.node),
            fmt_expr(&then_expr.node),
            fmt_expr(&else_expr.node),
        ),

        Expr::Call { func, args } => {
            let arg_strs: Vec<String> = args.iter().map(|a| fmt_expr(&a.node)).collect();
            format!("{}({})", func.0.node, arg_strs.join(", "))
        }

        Expr::Error(e) => format!("/* error: {} */", e.message),
    }
}

/// Precedence of an expression node (matches parse_expr precedence levels).
fn precedence(expr: &Expr) -> u8 {
    match expr {
        Expr::NullCoalesce(..) => 1,
        Expr::Or(..)           => 2,
        Expr::And(..)          => 3,
        Expr::Eq(..) | Expr::Ne(..)                        => 4,
        Expr::Lt(..) | Expr::Gt(..) | Expr::Le(..) | Expr::Ge(..) => 5,
        Expr::Add(..) | Expr::Sub(..)                      => 6,
        Expr::Mul(..) | Expr::Div(..)                      => 7,
        Expr::Neg(..) | Expr::Not(..)                      => 8,
        _                                                   => 9,
    }
}

fn fmt_binop(lhs: &Spanned<Expr>, op: &str, rhs: &Spanned<Expr>, op_prec: u8) -> String {
    let l = if precedence(&lhs.node) < op_prec {
        format!("({})", fmt_expr(&lhs.node))
    } else {
        fmt_expr(&lhs.node)
    };
    // Right-hand side uses strict less-than so that equal-precedence chains
    // like `a - b - c` print as `a - b - c` (left-assoc), not `a - (b - c)`.
    let r = if precedence(&rhs.node) < op_prec {
        format!("({})", fmt_expr(&rhs.node))
    } else {
        fmt_expr(&rhs.node)
    };
    format!("{l} {op} {r}")
}

fn fmt_unary_operand(e: &Spanned<Expr>) -> String {
    if precedence(&e.node) < 8 {
        format!("({})", fmt_expr(&e.node))
    } else {
        fmt_expr(&e.node)
    }
}

/// Print a float without unnecessary trailing zeros, but always with a decimal
/// point so it round-trips as a float (not an integer).
fn fmt_f64(f: f64) -> String {
    if f == f.floor() && f.abs() < 1e15 {
        format!("{:.1}", f)
    } else {
        // Use enough precision to round-trip exactly.
        format!("{}", f)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    /// Parse src, print it, parse again — the two ASTs should print identically.
    fn round_trip(src: &str) -> String {
        let (file, errors) = parse(src);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        let printed = print_file(&file);
        let (file2, errors2) = parse(&printed);
        assert!(errors2.is_empty(), "re-parse errors: {errors2:?}");
        let printed2 = print_file(&file2);
        assert_eq!(printed, printed2, "print not stable after two round-trips");
        printed
    }

    #[test]
    fn tile_round_trip() {
        round_trip("tile quad { sides 4 canonical (0, 0) (1, 0) (1, 1) (0, 1) symmetry d4 }");
    }

    #[test]
    fn partition_round_trip() {
        round_trip(r#"partition quad.split {
    edge m_top = (0.5, 0)
    interior center = (0.5, 0.5)
    cut m_top -- center
    child 0 = top_left
}"#);
    }

    #[test]
    fn function_round_trip() {
        round_trip("fn blend(val, target, factor) { lerp(val, target, factor) }");
    }

    #[test]
    fn pattern_round_trip() {
        round_trip(r#"pattern {
    root quad

    state {
        r = 0.0
        blend = 1.0
        target_r
    }

    color (r ? 0.0, r ? 0.0, r ? 0.0, 1.0)

    rule quad -> quad.split {
        blend = blend * 0.5
        child top_left {
            target_r = 1.0
        }
    }
}"#);
    }

    #[test]
    fn expr_precedence_round_trip() {
        // These should not gain extra parens on a second print.
        round_trip("fn f(a, b, c) { a + b * c }");
        round_trip("fn f(a, b, c) { (a + b) * c }");
        round_trip("fn f(a, b, c) { a ? b ? c }");
    }

    #[test]
    fn if_else_rule_round_trip() {
        round_trip(r#"pattern {
    root quad

    state {
        d = 0.0
    }

    color (d, d, d, 1.0)

    rule quad {
        if depth < 3.0 {
            -> quad.split {
                d = depth
            }
        } else {
            -> quad.split {
                d = 1.0
            }
        }
    }
}"#);
    }

    #[test]
    fn slot_order_round_trip() {
        round_trip(r#"pattern {
    root quad

    state {
        p = d4.identity
    }

    color (0.0, 0.0, 0.0, 1.0)

    rule quad -> quad.split {
        slot_order quad.class = p
        p = compose(p, child.canonical_orientation)
    }
}"#);
    }
}
