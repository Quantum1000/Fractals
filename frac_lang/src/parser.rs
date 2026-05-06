use crate::ast::*;
use crate::lexer::{Lexer, Token};

// ---------------------------------------------------------------------------
// Parser state
// ---------------------------------------------------------------------------

pub struct Parser {
    tokens: Vec<Spanned<Token>>,
    pos: usize,
    errors: Vec<ParseError>,
}

impl Parser {
    pub fn new(tokens: Vec<Spanned<Token>>) -> Self {
        Self { tokens, pos: 0, errors: Vec::new() }
    }

    // -----------------------------------------------------------------------
    // Token-level primitives
    // -----------------------------------------------------------------------

    fn peek(&self) -> &Spanned<Token> {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn peek2(&self) -> &Spanned<Token> {
        let idx = (self.pos + 1).min(self.tokens.len() - 1);
        &self.tokens[idx]
    }

    fn advance(&mut self) -> &Spanned<Token> {
        let tok = &self.tokens[self.pos.min(self.tokens.len() - 1)];
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    fn at(&self, tok: &Token) -> bool {
        std::mem::discriminant(&self.peek().node) == std::mem::discriminant(tok)
    }

    /// Consume a token if it matches, returning its span; otherwise record an
    /// error and return the current span without advancing.
    fn expect(&mut self, expected: &Token) -> Span {
        if self.at(expected) {
            self.advance().span
        } else {
            let span = self.peek().span;
            self.error(
                format!("expected {}, found {}", expected.describe(), self.peek().node.describe()),
                span,
            );
            span
        }
    }

    fn eat(&mut self, tok: &Token) -> bool {
        if self.at(tok) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.errors.push(ParseError { message: message.into(), span });
    }

    fn current_span(&self) -> Span {
        self.peek().span
    }

    // -----------------------------------------------------------------------
    // Identifier helpers
    // -----------------------------------------------------------------------

    fn expect_ident(&mut self) -> Spanned<String> {
        let span = self.peek().span;
        match self.peek().node.clone() {
            Token::Ident(s) => { self.advance(); Spanned::new(s, span) }
            _ => {
                self.error(format!("expected identifier, found {}", self.peek().node.describe()), span);
                Spanned::new(String::new(), span)
            }
        }
    }

    fn expect_tile_ref(&mut self)      -> TileRef      { TileRef(self.expect_ident()) }
    fn expect_partition_ref(&mut self) -> PartitionRef { PartitionRef(self.expect_ident()) }
    fn expect_vertex_ref(&mut self)    -> VertexRef    { VertexRef(self.expect_ident()) }
    fn expect_var_ref(&mut self)       -> VarRef       { VarRef(self.expect_ident()) }
    fn expect_func_ref(&mut self)      -> FuncRef      { FuncRef(self.expect_ident()) }
    fn expect_child_name_ref(&mut self) -> ChildNameRef { ChildNameRef(self.expect_ident()) }

    // -----------------------------------------------------------------------
    // Top-level parse
    // -----------------------------------------------------------------------

    pub fn parse_file(&mut self) -> File {
        let mut items = Vec::new();
        while !self.at(&Token::Eof) {
            let start = self.current_span().start;
            let item = match &self.peek().node.clone() {
                Token::KwTile      => Item::Tile(self.parse_tile_decl()),
                Token::KwPartition => Item::Partition(self.parse_partition_decl()),
                Token::KwFn        => Item::Function(self.parse_function_decl()),
                Token::KwPattern   => Item::Pattern(self.parse_pattern_decl()),
                _ => {
                    let span = self.current_span();
                    self.error(
                        format!("expected `tile`, `partition`, `fn`, or `pattern`, found {}", self.peek().node.describe()),
                        span,
                    );
                    // Recover: skip tokens until we reach something that looks like a new item.
                    self.recover_to_item_start();
                    let end = self.current_span().start;
                    items.push(Spanned::new(Item::Error(ParseError {
                        message: "unexpected token at top level".into(),
                        span,
                    }), Span { start, end }));
                    continue;
                }
            };
            let end = self.current_span().start;
            items.push(Spanned::new(item, Span { start, end }));
        }
        File { items }
    }

    /// Skip tokens until we're at the start of a new top-level item or EOF.
    fn recover_to_item_start(&mut self) {
        loop {
            match &self.peek().node {
                Token::KwTile | Token::KwPartition | Token::KwFn | Token::KwPattern | Token::Eof => break,
                _ => { self.advance(); }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Tile declaration:
    //   tile <name> {
    //     sides <n>
    //     canonical <point> ...
    //     [symmetry <group>]
    //   }
    // -----------------------------------------------------------------------

    fn parse_tile_decl(&mut self) -> TileDecl {
        self.expect(&Token::KwTile);
        let name = self.expect_tile_ref();
        self.expect(&Token::LBrace);

        let mut sides: Option<Spanned<u32>> = None;
        let mut canonical: Vec<Spanned<Point>> = Vec::new();
        let mut symmetry: Option<Spanned<SymmetryGroup>> = None;

        while !self.at(&Token::RBrace) && !self.at(&Token::Eof) {
            match self.peek().node.clone() {
                Token::Ident(ref s) if s == "sides" => {
                    let start = self.current_span().start;
                    self.advance();
                    let n = self.expect_u32();
                    sides = Some(Spanned::new(n, Span { start, end: self.current_span().start }));
                }
                Token::Ident(ref s) if s == "canonical" => {
                    self.advance();
                    while self.at(&Token::LParen) {
                        canonical.push(self.parse_point());
                    }
                }
                Token::Ident(ref s) if s == "symmetry" => {
                    let start = self.current_span().start;
                    self.advance();
                    let g = self.parse_symmetry_group();
                    symmetry = Some(Spanned::new(g, Span { start, end: self.current_span().start }));
                }
                _ => {
                    let span = self.current_span();
                    self.error(format!("unexpected token in tile block: {}", self.peek().node.describe()), span);
                    self.advance();
                }
            }
        }
        self.expect(&Token::RBrace);

        let sides = sides.unwrap_or_else(|| {
            let span = self.current_span();
            self.error("tile block missing `sides`", span);
            Spanned::new(0, span)
        });

        TileDecl { name, sides, canonical, symmetry }
    }

    fn expect_u32(&mut self) -> u32 {
        match self.peek().node.clone() {
            Token::Int(n) => { self.advance(); n as u32 }
            _ => {
                let span = self.current_span();
                self.error(format!("expected integer, found {}", self.peek().node.describe()), span);
                0
            }
        }
    }

    fn parse_point(&mut self) -> Spanned<Point> {
        let start = self.current_span().start;
        self.expect(&Token::LParen);
        let x = self.parse_number_f64();
        self.expect(&Token::Comma);
        let y = self.parse_number_f64();
        self.expect(&Token::RParen);
        Spanned::new(Point { x, y }, Span { start, end: self.current_span().start })
    }

    fn parse_number_f64(&mut self) -> f64 {
        match self.peek().node.clone() {
            Token::Float(f) => { self.advance(); f }
            Token::Int(n)   => { self.advance(); n as f64 }
            Token::Minus    => {
                self.advance();
                -self.parse_number_f64()
            }
            _ => {
                let span = self.current_span();
                self.error(format!("expected number, found {}", self.peek().node.describe()), span);
                0.0
            }
        }
    }

    fn parse_symmetry_group(&mut self) -> SymmetryGroup {
        match self.peek().node.clone() {
            Token::Ident(s) => {
                self.advance();
                parse_symmetry_group_str(&s).unwrap_or_else(|| {
                    let span = self.current_span();
                    self.error(format!("unknown symmetry group `{s}`; expected `trivial`, `cn`, or `dn`"), span);
                    SymmetryGroup::Trivial
                })
            }
            _ => {
                let span = self.current_span();
                self.error(format!("expected symmetry group, found {}", self.peek().node.describe()), span);
                SymmetryGroup::Trivial
            }
        }
    }

    // -----------------------------------------------------------------------
    // Partition declaration:
    //   partition <tile>.<name> {
    //     edge <name> = <point>
    //     interior <name> = <point>
    //     cut <vertex> -- <vertex>
    //     child <n> = <name>
    //   }
    // -----------------------------------------------------------------------

    fn parse_partition_decl(&mut self) -> PartitionDecl {
        self.expect(&Token::KwPartition);
        let tile = self.expect_tile_ref();
        self.expect(&Token::Dot);
        let name = self.expect_partition_ref();
        self.expect(&Token::LBrace);

        let mut vertices = Vec::new();
        let mut cuts = Vec::new();
        let mut child_names = Vec::new();

        while !self.at(&Token::RBrace) && !self.at(&Token::Eof) {
            match self.peek().node.clone() {
                Token::KwEdge | Token::KwInterior => {
                    let start = self.current_span().start;
                    let kind = if self.peek().node == Token::KwEdge {
                        self.advance(); VertexKind::Edge
                    } else {
                        self.advance(); VertexKind::Interior
                    };
                    let vname = self.expect_vertex_ref();
                    self.expect(&Token::Equals);
                    let pos = self.parse_point();
                    let end = self.current_span().start;
                    vertices.push(Spanned::new(PartitionVertex { kind, name: vname, pos }, Span { start, end }));
                }
                Token::KwCut => {
                    let start = self.current_span().start;
                    self.advance();
                    let from = self.expect_vertex_ref();
                    self.expect(&Token::DashDash);
                    let to = self.expect_vertex_ref();
                    let end = self.current_span().start;
                    cuts.push(Spanned::new(Cut { from, to }, Span { start, end }));
                }
                Token::KwChild => {
                    let start = self.current_span().start;
                    self.advance();
                    let index = self.expect_u32();
                    let index_span = Span { start, end: self.current_span().start };
                    self.expect(&Token::Equals);
                    let cname = self.expect_child_name_ref();
                    let end = self.current_span().start;
                    child_names.push(Spanned::new(
                        ChildName { index: Spanned::new(index, index_span), name: cname },
                        Span { start, end },
                    ));
                }
                _ => {
                    let span = self.current_span();
                    self.error(format!("unexpected token in partition block: {}", self.peek().node.describe()), span);
                    self.advance();
                }
            }
        }
        self.expect(&Token::RBrace);
        PartitionDecl { tile, name, vertices, cuts, child_names }
    }

    // -----------------------------------------------------------------------
    // Function declaration:
    //   fn <name>(<param>, ...) { <expr> }
    // -----------------------------------------------------------------------

    fn parse_function_decl(&mut self) -> FunctionDecl {
        self.expect(&Token::KwFn);
        let name = self.expect_func_ref();
        self.expect(&Token::LParen);
        let mut params = Vec::new();
        while !self.at(&Token::RParen) && !self.at(&Token::Eof) {
            params.push(self.expect_var_ref());
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.expect(&Token::RParen);
        self.expect(&Token::LBrace);
        let body = self.parse_expr();
        self.expect(&Token::RBrace);
        FunctionDecl { name, params, body }
    }

    // -----------------------------------------------------------------------
    // Pattern declaration:
    //   pattern {
    //     root <tile>
    //     state { ... }
    //     color ( <expr>, <expr>, <expr>, <expr> )
    //     rule <tile> { ... }  |  rule <tile> -> <tile>.<part> { ... }
    //   }
    // -----------------------------------------------------------------------

    fn parse_pattern_decl(&mut self) -> PatternDecl {
        self.expect(&Token::KwPattern);
        self.expect(&Token::LBrace);

        let mut root: Option<TileRef> = None;
        let mut state: Option<Spanned<StateBlock>> = None;
        let mut color: Option<Spanned<ColorExpr>> = None;
        let mut rules = Vec::new();

        while !self.at(&Token::RBrace) && !self.at(&Token::Eof) {
            match self.peek().node.clone() {
                Token::KwRoot => {
                    self.advance();
                    root = Some(self.expect_tile_ref());
                }
                Token::KwState => {
                    let start = self.current_span().start;
                    self.advance();
                    let block = self.parse_state_block();
                    let end = self.current_span().start;
                    state = Some(Spanned::new(block, Span { start, end }));
                }
                Token::KwColor => {
                    let start = self.current_span().start;
                    self.advance();
                    let ce = self.parse_color_expr();
                    let end = self.current_span().start;
                    color = Some(Spanned::new(ce, Span { start, end }));
                }
                Token::KwRule => {
                    let start = self.current_span().start;
                    let rule = self.parse_rule_decl();
                    let end = self.current_span().start;
                    rules.push(Spanned::new(rule, Span { start, end }));
                }
                _ => {
                    let span = self.current_span();
                    self.error(format!("unexpected token in pattern block: {}", self.peek().node.describe()), span);
                    self.advance();
                }
            }
        }
        self.expect(&Token::RBrace);

        let dummy_span = self.current_span();

        let root = root.unwrap_or_else(|| {
            self.error("pattern block missing `root`", dummy_span);
            TileRef(Spanned::new(String::new(), dummy_span))
        });
        let state = state.unwrap_or_else(|| {
            self.error("pattern block missing `state`", dummy_span);
            Spanned::new(StateBlock { vars: Vec::new() }, dummy_span)
        });
        let color = color.unwrap_or_else(|| {
            self.error("pattern block missing `color`", dummy_span);
            let err_expr = Spanned::new(Expr::Error(ParseError {
                message: "missing color expression".into(),
                span: dummy_span,
            }), dummy_span);
            Spanned::new(ColorExpr {
                r: err_expr.clone(), g: err_expr.clone(),
                b: err_expr.clone(), a: err_expr,
            }, dummy_span)
        });

        PatternDecl { root, state, color, rules }
    }

    // state { <var> [= <expr>] ... }
    fn parse_state_block(&mut self) -> StateBlock {
        self.expect(&Token::LBrace);
        let mut vars = Vec::new();
        while !self.at(&Token::RBrace) && !self.at(&Token::Eof) {
            let start = self.current_span().start;
            let name = self.expect_var_ref();
            let initial = if self.eat(&Token::Equals) {
                Some(self.parse_expr())
            } else {
                None
            };
            let end = self.current_span().start;
            vars.push(Spanned::new(StateVar { name, initial }, Span { start, end }));
        }
        self.expect(&Token::RBrace);
        StateBlock { vars }
    }

    // color ( <r>, <g>, <b>, <a> )
    fn parse_color_expr(&mut self) -> ColorExpr {
        self.expect(&Token::LParen);
        let r = self.parse_expr();
        self.expect(&Token::Comma);
        let g = self.parse_expr();
        self.expect(&Token::Comma);
        let b = self.parse_expr();
        self.expect(&Token::Comma);
        let a = self.parse_expr();
        self.expect(&Token::RParen);
        ColorExpr { r, g, b, a }
    }

    // rule <tile> { <rule_body> }
    // rule <tile> -> <tile>.<part> { <substitution> }   (shorthand)
    fn parse_rule_decl(&mut self) -> RuleDecl {
        self.expect(&Token::KwRule);
        let tile = self.expect_tile_ref();

        let start = self.current_span().start;
        let body = if self.at(&Token::Arrow) {
            // Shorthand: rule quad -> quad.split { ... }
            let sub = self.parse_substitution();
            let end = self.current_span().start;
            Spanned::new(RuleBody::Substitute(sub), Span { start, end })
        } else {
            self.expect(&Token::LBrace);
            let body = self.parse_rule_body();
            self.expect(&Token::RBrace);
            body
        };

        RuleDecl { tile, body }
    }

    // rule_body = if_branch | substitution
    fn parse_rule_body(&mut self) -> Spanned<RuleBody> {
        let start = self.current_span().start;
        if self.at(&Token::KwIf) {
            self.advance();
            let condition = self.parse_expr();
            self.expect(&Token::LBrace);
            let then_branch = self.parse_rule_body();
            self.expect(&Token::RBrace);
            self.expect(&Token::KwElse);
            self.expect(&Token::LBrace);
            let else_branch = self.parse_rule_body();
            self.expect(&Token::RBrace);
            let end = self.current_span().start;
            Spanned::new(RuleBody::If {
                condition,
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            }, Span { start, end })
        } else if self.at(&Token::Arrow) {
            let sub = self.parse_substitution();
            let end = self.current_span().start;
            Spanned::new(RuleBody::Substitute(sub), Span { start, end })
        } else {
            let span = self.current_span();
            self.error(
                format!("expected `if` or `->` in rule body, found {}", self.peek().node.describe()),
                span,
            );
            let end = self.current_span().start;
            Spanned::new(RuleBody::Substitute(Spanned::new(
                Substitution {
                    tile: TileRef(Spanned::new(String::new(), span)),
                    partition: PartitionRef(Spanned::new(String::new(), span)),
                    updates: Vec::new(),
                    child_injections: Vec::new(),
                    slot_orders: Vec::new(),
                },
                span,
            )), Span { start, end })
        }
    }

    // -> <tile>.<partition> { <updates> <child_injections> <slot_orders> }
    fn parse_substitution(&mut self) -> Spanned<Substitution> {
        let start = self.current_span().start;
        self.expect(&Token::Arrow);
        let tile = self.expect_tile_ref();
        self.expect(&Token::Dot);
        let partition = self.expect_partition_ref();
        self.expect(&Token::LBrace);

        let mut updates = Vec::new();
        let mut child_injections = Vec::new();
        let mut slot_orders = Vec::new();

        while !self.at(&Token::RBrace) && !self.at(&Token::Eof) {
            match self.peek().node.clone() {
                Token::KwChild => {
                    let ci = self.parse_child_injection();
                    child_injections.push(ci);
                }
                Token::KwSlotOrder => {
                    let so = self.parse_slot_order();
                    slot_orders.push(so);
                }
                // state update: <var> = <expr>
                Token::Ident(_) => {
                    let upd = self.parse_state_update();
                    updates.push(upd);
                }
                _ => {
                    let span = self.current_span();
                    self.error(
                        format!("unexpected token in substitution block: {}", self.peek().node.describe()),
                        span,
                    );
                    self.advance();
                }
            }
        }
        self.expect(&Token::RBrace);
        let end = self.current_span().start;
        Spanned::new(Substitution { tile, partition, updates, child_injections, slot_orders },
            Span { start, end })
    }

    // <var> = <expr>
    fn parse_state_update(&mut self) -> Spanned<StateUpdate> {
        let start = self.current_span().start;
        let var = self.expect_var_ref();
        self.expect(&Token::Equals);
        let value = self.parse_expr();
        let end = self.current_span().start;
        Spanned::new(StateUpdate { var, value }, Span { start, end })
    }

    // child (<name> | <index>) { [<update> ...] [alignment = <expr>] }
    fn parse_child_injection(&mut self) -> Spanned<ChildInjection> {
        let start = self.current_span().start;
        self.expect(&Token::KwChild);

        let child = match self.peek().node.clone() {
            Token::Int(n) => {
                let span = self.current_span();
                self.advance();
                Spanned::new(ChildRef::Index(n as u32), span)
            }
            Token::Ident(_) => {
                let s = self.expect_ident();
                let span = s.span;
                Spanned::new(ChildRef::Name(ChildNameRef(s)), span)
            }
            _ => {
                let span = self.current_span();
                self.error(format!("expected child name or index, found {}", self.peek().node.describe()), span);
                Spanned::new(ChildRef::Index(0), span)
            }
        };

        self.expect(&Token::LBrace);
        let mut updates = Vec::new();
        let mut alignment = None;

        while !self.at(&Token::RBrace) && !self.at(&Token::Eof) {
            match self.peek().node.clone() {
                Token::KwAlignment => {
                    let start = self.current_span().start;
                    self.advance();
                    self.expect(&Token::Equals);
                    let expr = self.parse_expr();
                    let end = self.current_span().start;
                    alignment = Some(Spanned::new(expr.node, Span { start, end }));
                }
                Token::Ident(_) => {
                    updates.push(self.parse_state_update());
                }
                _ => {
                    let span = self.current_span();
                    self.error(format!("unexpected token in child block: {}", self.peek().node.describe()), span);
                    self.advance();
                }
            }
        }
        self.expect(&Token::RBrace);
        let end = self.current_span().start;
        Spanned::new(ChildInjection { child, updates, alignment }, Span { start, end })
    }

    // slot_order [<tile> | <tile>.class] = <expr>
    fn parse_slot_order(&mut self) -> Spanned<SlotOrder> {
        let start = self.current_span().start;
        self.expect(&Token::KwSlotOrder);

        // Determine the target: just `= expr` means All; otherwise an ident
        // optionally followed by `.class`.
        let target = if self.at(&Token::Equals) {
            TileClassRef::All
        } else {
            let tile = self.expect_tile_ref();
            if self.at(&Token::Dot) && matches!(self.peek2().node, Token::Ident(ref s) if s == "class") {
                self.advance(); // consume '.'
                self.advance(); // consume 'class'
                TileClassRef::Class(tile)
            } else {
                TileClassRef::Tile(tile)
            }
        };

        self.expect(&Token::Equals);
        let value = self.parse_expr();
        let end = self.current_span().start;
        Spanned::new(SlotOrder { target, value }, Span { start, end })
    }

    // -----------------------------------------------------------------------
    // Expression parsing (Pratt / precedence climbing)
    //
    // Precedence levels (low → high):
    //   1. null-coalescing  ?
    //   2. logical or       ||
    //   3. logical and      &&
    //   4. equality         == !=
    //   5. comparison       < > <= >=
    //   6. additive         + -
    //   7. multiplicative   * /
    //   8. unary            - !
    //   9. primary          literals, calls, parens, if, keywords
    // -----------------------------------------------------------------------

    fn parse_expr(&mut self) -> Spanned<Expr> {
        self.parse_null_coalesce()
    }

    fn parse_null_coalesce(&mut self) -> Spanned<Expr> {
        let mut lhs = self.parse_or();
        while self.at(&Token::Question) {
            let start = lhs.span.start;
            self.advance();
            let rhs = self.parse_or();
            let end = rhs.span.end;
            lhs = Spanned::new(
                Expr::NullCoalesce(Box::new(lhs), Box::new(rhs)),
                Span { start, end },
            );
        }
        lhs
    }

    fn parse_or(&mut self) -> Spanned<Expr> {
        let mut lhs = self.parse_and();
        while self.at(&Token::PipePipe) {
            let start = lhs.span.start;
            self.advance();
            let rhs = self.parse_and();
            let end = rhs.span.end;
            lhs = Spanned::new(Expr::Or(Box::new(lhs), Box::new(rhs)), Span { start, end });
        }
        lhs
    }

    fn parse_and(&mut self) -> Spanned<Expr> {
        let mut lhs = self.parse_equality();
        while self.at(&Token::AmpAmp) {
            let start = lhs.span.start;
            self.advance();
            let rhs = self.parse_equality();
            let end = rhs.span.end;
            lhs = Spanned::new(Expr::And(Box::new(lhs), Box::new(rhs)), Span { start, end });
        }
        lhs
    }

    fn parse_equality(&mut self) -> Spanned<Expr> {
        let mut lhs = self.parse_comparison();
        loop {
            let start = lhs.span.start;
            let expr = match self.peek().node {
                Token::EqEq => { self.advance(); let r = self.parse_comparison(); Expr::Eq(Box::new(lhs.clone()), Box::new(r.clone())) }
                Token::Ne   => { self.advance(); let r = self.parse_comparison(); Expr::Ne(Box::new(lhs.clone()), Box::new(r.clone())) }
                _ => break,
            };
            let end = self.current_span().start;
            lhs = Spanned::new(expr, Span { start, end });
        }
        lhs
    }

    fn parse_comparison(&mut self) -> Spanned<Expr> {
        let mut lhs = self.parse_additive();
        loop {
            let start = lhs.span.start;
            let expr = match self.peek().node {
                Token::Lt => { self.advance(); let r = self.parse_additive(); Expr::Lt(Box::new(lhs.clone()), Box::new(r)) }
                Token::Gt => { self.advance(); let r = self.parse_additive(); Expr::Gt(Box::new(lhs.clone()), Box::new(r)) }
                Token::Le => { self.advance(); let r = self.parse_additive(); Expr::Le(Box::new(lhs.clone()), Box::new(r)) }
                Token::Ge => { self.advance(); let r = self.parse_additive(); Expr::Ge(Box::new(lhs.clone()), Box::new(r)) }
                _ => break,
            };
            let end = self.current_span().start;
            lhs = Spanned::new(expr, Span { start, end });
        }
        lhs
    }

    fn parse_additive(&mut self) -> Spanned<Expr> {
        let mut lhs = self.parse_multiplicative();
        loop {
            let start = lhs.span.start;
            let expr = match self.peek().node {
                Token::Plus  => { self.advance(); let r = self.parse_multiplicative(); Expr::Add(Box::new(lhs.clone()), Box::new(r)) }
                Token::Minus => { self.advance(); let r = self.parse_multiplicative(); Expr::Sub(Box::new(lhs.clone()), Box::new(r)) }
                _ => break,
            };
            let end = self.current_span().start;
            lhs = Spanned::new(expr, Span { start, end });
        }
        lhs
    }

    fn parse_multiplicative(&mut self) -> Spanned<Expr> {
        let mut lhs = self.parse_unary();
        loop {
            let start = lhs.span.start;
            let expr = match self.peek().node {
                Token::Star  => { self.advance(); let r = self.parse_unary(); Expr::Mul(Box::new(lhs.clone()), Box::new(r)) }
                Token::Slash => { self.advance(); let r = self.parse_unary(); Expr::Div(Box::new(lhs.clone()), Box::new(r)) }
                _ => break,
            };
            let end = self.current_span().start;
            lhs = Spanned::new(expr, Span { start, end });
        }
        lhs
    }

    fn parse_unary(&mut self) -> Spanned<Expr> {
        let start = self.current_span().start;
        match self.peek().node.clone() {
            Token::Minus => {
                self.advance();
                let operand = self.parse_unary();
                let end = operand.span.end;
                Spanned::new(Expr::Neg(Box::new(operand)), Span { start, end })
            }
            Token::Bang => {
                self.advance();
                let operand = self.parse_unary();
                let end = operand.span.end;
                Spanned::new(Expr::Not(Box::new(operand)), Span { start, end })
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Spanned<Expr> {
        let start = self.current_span().start;

        match self.peek().node.clone() {
            Token::Float(f) => {
                self.advance();
                Spanned::new(Expr::Float(f), Span { start, end: self.current_span().start })
            }
            Token::Int(n) => {
                self.advance();
                Spanned::new(Expr::Float(n as f64), Span { start, end: self.current_span().start })
            }
            Token::KwNone => {
                self.advance();
                Spanned::new(Expr::None, Span { start, end: self.current_span().start })
            }
            Token::KwTrue => {
                self.advance();
                Spanned::new(Expr::Bool(true), Span { start, end: self.current_span().start })
            }
            Token::KwFalse => {
                self.advance();
                Spanned::new(Expr::Bool(false), Span { start, end: self.current_span().start })
            }
            // `child.canonical_orientation` and `child.index` used as expressions
            Token::KwChild => {
                self.advance();
                self.expect(&Token::Dot);
                let field = self.expect_ident();
                let end = self.current_span().start;
                let span = Span { start, end };
                match field.node.as_str() {
                    "canonical_orientation" => Spanned::new(Expr::ChildCanonicalOrientation, span),
                    "index"                 => Spanned::new(Expr::ChildIndex, span),
                    other => {
                        self.error(format!("unknown child field `{other}`; expected `canonical_orientation` or `index`"), span);
                        Spanned::new(Expr::Error(ParseError { message: format!("unknown child field `{other}`"), span }), span)
                    }
                }
            }
            Token::KwPerm => {
                self.advance();
                self.expect(&Token::LBracket);
                let mut indices = Vec::new();
                while !self.at(&Token::RBracket) && !self.at(&Token::Eof) {
                    let s = self.current_span();
                    let n = self.expect_u32();
                    indices.push(Spanned::new(n, s));
                    if !self.eat(&Token::Comma) { break; }
                }
                self.expect(&Token::RBracket);
                Spanned::new(Expr::PermLiteral(indices), Span { start, end: self.current_span().start })
            }
            Token::KwIf => {
                self.advance();
                let condition = self.parse_expr();
                self.expect(&Token::LBrace);
                let then_expr = self.parse_expr();
                self.expect(&Token::RBrace);
                self.expect(&Token::KwElse);
                self.expect(&Token::LBrace);
                let else_expr = self.parse_expr();
                self.expect(&Token::RBrace);
                let end = self.current_span().start;
                Spanned::new(Expr::If {
                    condition: Box::new(condition),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                }, Span { start, end })
            }
            Token::LParen => {
                self.advance();
                let inner = self.parse_expr();
                self.expect(&Token::RParen);
                inner
            }
            Token::Ident(name) => {
                self.advance();
                let after_ident = self.current_span().start;

                // `ident.something` — could be:
                //   pos.x, pos.y
                //   child.canonical_orientation, child.index
                //   d4.r90  (group element literal)
                if self.at(&Token::Dot) {
                    self.advance(); // consume '.'
                    let field = self.expect_ident();
                    let end = self.current_span().start;
                    let span = Span { start, end };

                    match (name.as_str(), field.node.as_str()) {
                        ("pos", "x") => return Spanned::new(Expr::PosX, span),
                        ("pos", "y") => return Spanned::new(Expr::PosY, span),
                        ("child", "canonical_orientation") => return Spanned::new(Expr::ChildCanonicalOrientation, span),
                        ("child", "index") => return Spanned::new(Expr::ChildIndex, span),
                        _ => {
                            // Treat as group element literal: `group.element`
                            return Spanned::new(Expr::GroupElem {
                                group: Spanned::new(name, Span { start, end: after_ident }),
                                element: field,
                            }, span);
                        }
                    }
                }

                // Function call or built-in
                if self.at(&Token::LParen) {
                    return self.parse_call(name, start);
                }

                // Built-in scalar inputs (no dot, no call)
                let expr = match name.as_str() {
                    "scale"       => Expr::Scale,
                    "orientation" => Expr::Orientation,
                    "shear"       => Expr::Shear,
                    "stretch"     => Expr::Stretch,
                    "depth"       => Expr::Depth,
                    "random"      => Expr::Random,
                    _ => Expr::Var(VarRef(Spanned::new(name, Span { start, end: after_ident }))),
                };
                Spanned::new(expr, Span { start, end: after_ident })
            }
            _ => {
                let span = self.current_span();
                self.error(format!("expected expression, found {}", self.peek().node.describe()), span);
                self.advance();
                Spanned::new(Expr::Error(ParseError {
                    message: "unexpected token in expression".into(),
                    span,
                }), span)
            }
        }
    }

    // Built-in and user-defined function calls: name(arg, ...)
    fn parse_call(&mut self, name: String, start: usize) -> Spanned<Expr> {
        self.expect(&Token::LParen);
        let mut args = Vec::new();
        while !self.at(&Token::RParen) && !self.at(&Token::Eof) {
            args.push(self.parse_expr());
            if !self.eat(&Token::Comma) { break; }
        }
        self.expect(&Token::RParen);
        let end = self.current_span().start;
        let span = Span { start, end };

        // Resolve built-in functions by name; everything else is a user call.
        let expr = match (name.as_str(), args.len()) {
            ("sin",     1) => Expr::Sin(    Box::new(args.remove(0))),
            ("cos",     1) => Expr::Cos(    Box::new(args.remove(0))),
            ("exp",     1) => Expr::Exp(    Box::new(args.remove(0))),
            ("sqrt",    1) => Expr::Sqrt(   Box::new(args.remove(0))),
            ("abs",     1) => Expr::Abs(    Box::new(args.remove(0))),
            ("log",     1) => Expr::Log(    Box::new(args.remove(0))),
            ("inverse", 1) => Expr::Inverse(Box::new(args.remove(0))),
            ("lerp",    3) => {
                let a = args.remove(0); let b = args.remove(0); let t = args.remove(0);
                Expr::Lerp(Box::new(a), Box::new(b), Box::new(t))
            }
            ("clamp",   3) => {
                let x = args.remove(0); let lo = args.remove(0); let hi = args.remove(0);
                Expr::Clamp(Box::new(x), Box::new(lo), Box::new(hi))
            }
            ("compose", 2) => {
                let a = args.remove(0); let b = args.remove(0);
                Expr::Compose(Box::new(a), Box::new(b))
            }
            _ => {
                // Wrong arity for a known built-in is also caught here as a user call;
                // the type checker will report the error later.
                Expr::Call {
                    func: FuncRef(Spanned::new(name, span)),
                    args,
                }
            }
        };
        Spanned::new(expr, span)
    }
}

// ---------------------------------------------------------------------------
// Symmetry group string parsing
// ---------------------------------------------------------------------------

fn parse_symmetry_group_str(s: &str) -> Option<SymmetryGroup> {
    match s {
        "trivial" => Some(SymmetryGroup::Trivial),
        _ if s.starts_with("d") || s.starts_with("D") => {
            s[1..].parse::<u32>().ok().map(SymmetryGroup::Dihedral)
        }
        _ if s.starts_with("c") || s.starts_with("C") => {
            s[1..].parse::<u32>().ok().map(SymmetryGroup::Cyclic)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn parse(src: &str) -> (File, Vec<ParseError>) {
    let tokens = Lexer::tokenize(src);
    let mut parser = Parser::new(tokens);
    let file = parser.parse_file();
    (file, parser.errors)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(src: &str) -> File {
        let (file, errors) = parse(src);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        file
    }

    #[test]
    fn tile_decl_minimal() {
        let file = parse_ok("tile quad { sides 4 canonical (0,0) (1,0) (1,1) (0,1) }");
        assert_eq!(file.items.len(), 1);
        let Item::Tile(ref t) = file.items[0].node else { panic!("expected Tile") };
        assert_eq!(t.name.0.node, "quad");
        assert_eq!(t.sides.node, 4);
        assert_eq!(t.canonical.len(), 4);
        assert!(t.symmetry.is_none());
    }

    #[test]
    fn tile_decl_with_symmetry() {
        let file = parse_ok("tile quad { sides 4 canonical (0,0) (1,0) (1,1) (0,1) symmetry d4 }");
        let Item::Tile(ref t) = file.items[0].node else { panic!() };
        assert_eq!(t.symmetry.as_ref().unwrap().node, SymmetryGroup::Dihedral(4));
    }

    #[test]
    fn partition_decl() {
        let src = r#"
            partition quad.split {
                edge m_top = (0.5, 0)
                interior center = (0.5, 0.5)
                cut m_top -- center
                child 0 = top_left
            }
        "#;
        let file = parse_ok(src);
        let Item::Partition(ref p) = file.items[0].node else { panic!() };
        assert_eq!(p.tile.0.node, "quad");
        assert_eq!(p.name.0.node, "split");
        assert_eq!(p.vertices.len(), 2);
        assert_eq!(p.cuts.len(), 1);
        assert_eq!(p.child_names.len(), 1);
    }

    #[test]
    fn function_decl() {
        let file = parse_ok("fn blend(val, target, factor) { lerp(val, target, factor) }");
        let Item::Function(ref f) = file.items[0].node else { panic!() };
        assert_eq!(f.name.0.node, "blend");
        assert_eq!(f.params.len(), 3);
        assert!(matches!(f.body.node, Expr::Lerp(_, _, _)));
    }

    #[test]
    fn pattern_minimal() {
        let src = r#"
            pattern {
                root quad
                state { r = 0 }
                color (r, r, r, 1)
                rule quad -> quad.split { r = r * 0.5 }
            }
        "#;
        let file = parse_ok(src);
        assert_eq!(file.items.len(), 1);
        let Item::Pattern(ref p) = file.items[0].node else { panic!() };
        assert_eq!(p.root.0.node, "quad");
        assert_eq!(p.state.node.vars.len(), 1);
        assert_eq!(p.rules.len(), 1);
    }

    #[test]
    fn rule_if_else() {
        let src = r#"
            pattern {
                root quad
                state { d = 0 }
                color (d, d, d, 1)
                rule quad {
                    if depth < 3 {
                        -> quad.split { d = depth }
                    } else {
                        -> quad.split { d = 1 }
                    }
                }
            }
        "#;
        let file = parse_ok(src);
        let Item::Pattern(ref p) = file.items[0].node else { panic!() };
        assert!(matches!(p.rules[0].node.body.node, RuleBody::If { .. }));
    }

    #[test]
    fn expr_group_elem() {
        let src = "pattern { root q state {} color (0,0,0,1) rule q -> q.s { p = d4.r90 } }";
        let (file, _) = parse(src);
        let Item::Pattern(ref p) = file.items[0].node else { panic!() };
        let RuleBody::Substitute(ref sub) = p.rules[0].node.body.node else { panic!() };
        let upd = &sub.node.updates[0];
        assert!(matches!(upd.node.value.node, Expr::GroupElem { .. }));
    }

    #[test]
    fn expr_null_coalesce() {
        let src = "pattern { root q state {} color (r ? 0, 0, 0, 1) rule q -> q.s {} }";
        let (file, _) = parse(src);
        let Item::Pattern(ref p) = file.items[0].node else { panic!() };
        assert!(matches!(p.color.node.r.node, Expr::NullCoalesce(_, _)));
    }

    #[test]
    fn slot_order_all() {
        let src = "pattern { root q state {} color (0,0,0,1) rule q -> q.s { slot_order = p } }";
        let (file, _) = parse(src);
        let Item::Pattern(ref p) = file.items[0].node else { panic!() };
        let RuleBody::Substitute(ref sub) = p.rules[0].node.body.node else { panic!() };
        assert!(matches!(sub.node.slot_orders[0].node.target, TileClassRef::All));
    }

    #[test]
    fn slot_order_class() {
        let src = "pattern { root q state {} color (0,0,0,1) rule q -> q.s { slot_order quad.class = p } }";
        let (file, _) = parse(src);
        let Item::Pattern(ref p) = file.items[0].node else { panic!() };
        let RuleBody::Substitute(ref sub) = p.rules[0].node.body.node else { panic!() };
        assert!(matches!(sub.node.slot_orders[0].node.target, TileClassRef::Class(_)));
    }

    #[test]
    fn error_recovery_at_top_level() {
        // Junk before a valid declaration — parser should recover and still see the tile.
        let (file, errors) = parse("??? tile quad { sides 4 canonical (0,0) (1,0) (1,1) (0,1) }");
        assert!(!errors.is_empty());
        assert!(file.items.iter().any(|i| matches!(i.node, Item::Tile(_))));
    }
}
