use crate::ast::{Span, Spanned};

// ---------------------------------------------------------------------------
// Token types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    // --- Keywords ---
    KwTile,
    KwPartition,
    KwFn,
    KwPattern,
    KwRule,
    KwState,
    KwColor,
    KwRoot,
    KwEdge,
    KwInterior,
    KwCut,
    KwChild,
    KwIf,
    KwElse,
    KwPerm,
    KwSlotOrder,
    KwAlignment,
    KwNone,
    KwTrue,
    KwFalse,

    // --- Literals ---
    Int(u64),
    Float(f64),
    /// Any identifier that is not a keyword.
    /// Built-in names (`lerp`, `sin`, `pos`, `scale`, `child`, etc.) are
    /// resolved by the parser, not the lexer.
    Ident(String),

    // --- Paired delimiters ---
    LBrace,    // {
    RBrace,    // }
    LParen,    // (
    RParen,    // )
    LBracket,  // [
    RBracket,  // ]

    // --- Operators and punctuation ---
    Equals,    // =
    EqEq,      // ==
    Ne,        // !=
    Arrow,     // ->
    DashDash,  // --
    Dot,       // .
    Comma,     // ,
    Question,  // ?
    Plus,      // +
    Minus,     // -
    Star,      // *
    Slash,     // /
    Lt,        // <
    Gt,        // >
    Le,        // <=
    Ge,        // >=
    AmpAmp,    // &&
    PipePipe,  // ||
    Bang,      // !
    Colon,     // :

    // --- Sentinel / errors ---
    Eof,
    /// An unrecognised character; the lexer emits this and keeps going.
    Error(char),
}

impl Token {
    /// Human-readable description for error messages.
    pub fn describe(&self) -> &'static str {
        match self {
            Token::KwTile       => "`tile`",
            Token::KwPartition  => "`partition`",
            Token::KwFn         => "`fn`",
            Token::KwPattern    => "`pattern`",
            Token::KwRule       => "`rule`",
            Token::KwState      => "`state`",
            Token::KwColor      => "`color`",
            Token::KwRoot       => "`root`",
            Token::KwEdge       => "`edge`",
            Token::KwInterior   => "`interior`",
            Token::KwCut        => "`cut`",
            Token::KwChild      => "`child`",
            Token::KwIf         => "`if`",
            Token::KwElse       => "`else`",
            Token::KwPerm       => "`perm`",
            Token::KwSlotOrder  => "`slot_order`",
            Token::KwAlignment  => "`alignment`",
            Token::KwNone       => "`none`",
            Token::KwTrue       => "`true`",
            Token::KwFalse      => "`false`",
            Token::Int(_)       => "integer",
            Token::Float(_)     => "float",
            Token::Ident(_)     => "identifier",
            Token::LBrace       => "`{`",
            Token::RBrace       => "`}`",
            Token::LParen       => "`(`",
            Token::RParen       => "`)`",
            Token::LBracket     => "`[`",
            Token::RBracket     => "`]`",
            Token::Equals       => "`=`",
            Token::EqEq         => "`==`",
            Token::Ne           => "`!=`",
            Token::Arrow        => "`->`",
            Token::DashDash     => "`--`",
            Token::Dot          => "`.`",
            Token::Comma        => "`,`",
            Token::Question     => "`?`",
            Token::Plus         => "`+`",
            Token::Minus        => "`-`",
            Token::Star         => "`*`",
            Token::Slash        => "`/`",
            Token::Lt           => "`<`",
            Token::Gt           => "`>`",
            Token::Le           => "`<=`",
            Token::Ge           => "`>=`",
            Token::AmpAmp       => "`&&`",
            Token::PipePipe     => "`||`",
            Token::Bang         => "`!`",
            Token::Colon        => "`:`",
            Token::Eof          => "end of file",
            Token::Error(_)     => "unrecognised character",
        }
    }
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

pub struct Lexer<'src> {
    src: &'src str,
    /// Current byte position.
    pos: usize,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self {
        Self { src, pos: 0 }
    }

    /// Lex the entire source into a token list ending with `Eof`.
    pub fn tokenize(src: &'src str) -> Vec<Spanned<Token>> {
        let mut lexer = Self::new(src);
        let mut tokens = Vec::new();
        loop {
            let tok = lexer.next_token();
            let is_eof = tok.node == Token::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        tokens
    }

    // -----------------------------------------------------------------------
    // Character-level primitives
    // -----------------------------------------------------------------------

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek2(&self) -> Option<char> {
        let mut chars = self.src[self.pos..].chars();
        chars.next();
        chars.next()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.src[self.pos..].chars().next()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(ch) if ch.is_ascii_whitespace() => { self.advance(); }
                Some('#') => {
                    // Line comment: skip to end of line.
                    while !matches!(self.peek(), Some('\n') | None) {
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    fn span(&self, start: usize) -> Span {
        Span { start, end: self.pos }
    }

    // -----------------------------------------------------------------------
    // Number lexing
    //
    // Called after consuming the first digit. Reads the rest of the integer
    // part, then optionally a `.digits` fractional part.
    // Returns `Int` if there is no `.`, otherwise `Float`.
    // -----------------------------------------------------------------------

    fn lex_number(&mut self, first: char, start: usize) -> Spanned<Token> {
        let mut s = String::new();
        s.push(first);

        while matches!(self.peek(), Some('0'..='9')) {
            s.push(self.advance().unwrap());
        }

        // Fractional part: only consume `.digits` — not `.` alone (which would
        // be a field access) and not `..` (not in this language, but defensive).
        let is_float = matches!(self.peek(), Some('.'))
            && matches!(self.peek2(), Some('0'..='9'));

        if is_float {
            s.push(self.advance().unwrap()); // consume '.'
            while matches!(self.peek(), Some('0'..='9')) {
                s.push(self.advance().unwrap());
            }
            let val: f64 = s.parse().unwrap_or(f64::NAN);
            Spanned::new(Token::Float(val), self.span(start))
        } else {
            let val: u64 = s.parse().unwrap_or(0);
            Spanned::new(Token::Int(val), self.span(start))
        }
    }

    // -----------------------------------------------------------------------
    // Identifier / keyword lexing
    // -----------------------------------------------------------------------

    fn lex_ident_or_keyword(&mut self, first: char, start: usize) -> Spanned<Token> {
        let mut s = String::new();
        s.push(first);
        while matches!(self.peek(), Some(ch) if ch.is_alphanumeric() || ch == '_') {
            s.push(self.advance().unwrap());
        }
        let tok = keyword(&s).unwrap_or(Token::Ident(s));
        Spanned::new(tok, self.span(start))
    }

    // -----------------------------------------------------------------------
    // Main dispatch
    // -----------------------------------------------------------------------

    fn next_token(&mut self) -> Spanned<Token> {
        self.skip_whitespace_and_comments();

        let start = self.pos;

        let Some(ch) = self.advance() else {
            return Spanned::new(Token::Eof, self.span(start));
        };

        match ch {
            '{' => Spanned::new(Token::LBrace,   self.span(start)),
            '}' => Spanned::new(Token::RBrace,   self.span(start)),
            '(' => Spanned::new(Token::LParen,   self.span(start)),
            ')' => Spanned::new(Token::RParen,   self.span(start)),
            '[' => Spanned::new(Token::LBracket, self.span(start)),
            ']' => Spanned::new(Token::RBracket, self.span(start)),
            '.' => Spanned::new(Token::Dot,      self.span(start)),
            ',' => Spanned::new(Token::Comma,    self.span(start)),
            ':' => Spanned::new(Token::Colon,    self.span(start)),
            '?' => Spanned::new(Token::Question, self.span(start)),
            '+' => Spanned::new(Token::Plus,     self.span(start)),
            '*' => Spanned::new(Token::Star,     self.span(start)),
            '/' => Spanned::new(Token::Slash,    self.span(start)),

            '-' => match self.peek() {
                Some('>') => { self.advance(); Spanned::new(Token::Arrow,    self.span(start)) }
                Some('-') => { self.advance(); Spanned::new(Token::DashDash, self.span(start)) }
                _         =>                   Spanned::new(Token::Minus,    self.span(start)),
            },

            '<' => match self.peek() {
                Some('=') => { self.advance(); Spanned::new(Token::Le, self.span(start)) }
                _         =>                   Spanned::new(Token::Lt, self.span(start)),
            },

            '>' => match self.peek() {
                Some('=') => { self.advance(); Spanned::new(Token::Ge, self.span(start)) }
                _         =>                   Spanned::new(Token::Gt, self.span(start)),
            },

            '=' => match self.peek() {
                Some('=') => { self.advance(); Spanned::new(Token::EqEq,   self.span(start)) }
                _         =>                   Spanned::new(Token::Equals, self.span(start)),
            },

            '!' => match self.peek() {
                Some('=') => { self.advance(); Spanned::new(Token::Ne,   self.span(start)) }
                _         =>                   Spanned::new(Token::Bang, self.span(start)),
            },

            '&' => match self.peek() {
                Some('&') => { self.advance(); Spanned::new(Token::AmpAmp, self.span(start)) }
                _         => Spanned::new(Token::Error('&'), self.span(start)),
            },

            '|' => match self.peek() {
                Some('|') => { self.advance(); Spanned::new(Token::PipePipe, self.span(start)) }
                _         => Spanned::new(Token::Error('|'), self.span(start)),
            },

            '0'..='9' => self.lex_number(ch, start),

            ch if ch.is_alphabetic() || ch == '_' => self.lex_ident_or_keyword(ch, start),

            other => Spanned::new(Token::Error(other), self.span(start)),
        }
    }
}

// ---------------------------------------------------------------------------
// Keyword table
// ---------------------------------------------------------------------------

fn keyword(s: &str) -> Option<Token> {
    match s {
        "tile"       => Some(Token::KwTile),
        "partition"  => Some(Token::KwPartition),
        "fn"         => Some(Token::KwFn),
        "pattern"    => Some(Token::KwPattern),
        "rule"       => Some(Token::KwRule),
        "state"      => Some(Token::KwState),
        "color"      => Some(Token::KwColor),
        "root"       => Some(Token::KwRoot),
        "edge"       => Some(Token::KwEdge),
        "interior"   => Some(Token::KwInterior),
        "cut"        => Some(Token::KwCut),
        "child"      => Some(Token::KwChild),
        "if"         => Some(Token::KwIf),
        "else"       => Some(Token::KwElse),
        "perm"       => Some(Token::KwPerm),
        "slot_order" => Some(Token::KwSlotOrder),
        "alignment"  => Some(Token::KwAlignment),
        "none"       => Some(Token::KwNone),
        "true"       => Some(Token::KwTrue),
        "false"      => Some(Token::KwFalse),
        _            => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tok_kinds(src: &str) -> Vec<Token> {
        Lexer::tokenize(src).into_iter().map(|s| s.node).collect()
    }

    #[test]
    fn keywords_and_ident() {
        let toks = tok_kinds("tile quad");
        assert_eq!(toks, vec![Token::KwTile, Token::Ident("quad".into()), Token::Eof]);
    }

    #[test]
    fn numbers() {
        let toks = tok_kinds("4 0.5 1");
        assert_eq!(toks, vec![
            Token::Int(4),
            Token::Float(0.5),
            Token::Int(1),
            Token::Eof,
        ]);
    }

    #[test]
    fn dot_is_not_part_of_int() {
        // `quad.split` must not consume the dot into an identifier
        let toks = tok_kinds("quad.split");
        assert_eq!(toks, vec![
            Token::Ident("quad".into()),
            Token::Dot,
            Token::Ident("split".into()),
            Token::Eof,
        ]);
    }

    #[test]
    fn number_dot_is_not_float() {
        // `4.class` — the `.` is a field access, not a decimal point
        let toks = tok_kinds("4.class");
        assert_eq!(toks, vec![
            Token::Int(4),
            Token::Dot,
            Token::Ident("class".into()),
            Token::Eof,
        ]);
    }

    #[test]
    fn operators() {
        let toks = tok_kinds("-> -- <= >= == != && ||");
        assert_eq!(toks, vec![
            Token::Arrow, Token::DashDash,
            Token::Le, Token::Ge, Token::EqEq, Token::Ne,
            Token::AmpAmp, Token::PipePipe,
            Token::Eof,
        ]);
    }

    #[test]
    fn comments_skipped() {
        let toks = tok_kinds("tile # this is a comment\nquad");
        assert_eq!(toks, vec![Token::KwTile, Token::Ident("quad".into()), Token::Eof]);
    }

    #[test]
    fn slot_order_keyword() {
        // slot_order contains an underscore — must be a single keyword, not two idents
        let toks = tok_kinds("slot_order");
        assert_eq!(toks, vec![Token::KwSlotOrder, Token::Eof]);
    }

    #[test]
    fn spans_are_correct() {
        let toks = Lexer::tokenize("tile quad");
        assert_eq!(toks[0].span, Span { start: 0, end: 4 }); // "tile"
        assert_eq!(toks[1].span, Span { start: 5, end: 9 }); // "quad"
    }

    #[test]
    fn error_token_for_unknown_char() {
        let toks = tok_kinds("@");
        assert_eq!(toks, vec![Token::Error('@'), Token::Eof]);
    }
}
