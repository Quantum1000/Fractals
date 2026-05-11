/// Source span: byte offsets into the source string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// A value together with the source span it came from.
#[derive(Clone, Debug)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

// ---------------------------------------------------------------------------
// Typed identifier references
//
// Each reference type is structurally identical (a spanned string) but
// distinct in the type system, so later passes can accept exactly the right
// kind of name without ambiguity.
// ---------------------------------------------------------------------------

/// A tile type name — `quad` in `tile quad { ... }` or `rule quad { ... }`
#[derive(Clone, Debug)] pub struct TileRef(pub Spanned<String>);
/// A partition name — `split` in `partition quad.split { ... }` or `-> quad.split`
#[derive(Clone, Debug)] pub struct PartitionRef(pub Spanned<String>);
/// A named vertex inside a partition — `center` in `cut m_top -- center`
#[derive(Clone, Debug)] pub struct VertexRef(pub Spanned<String>);
/// A state variable or function parameter
#[derive(Clone, Debug)] pub struct VarRef(pub Spanned<String>);
/// A user-defined function name
#[derive(Clone, Debug)] pub struct FuncRef(pub Spanned<String>);
/// An author-assigned child name — `top_left` in `child 0 = top_left`
#[derive(Clone, Debug)] pub struct ChildNameRef(pub Spanned<String>);

// ---------------------------------------------------------------------------
// Top-level file
// ---------------------------------------------------------------------------

/// The root of a parsed `.frac` file.
#[derive(Clone, Debug)]
pub struct File {
    pub items: Vec<Spanned<Item>>,
}

#[derive(Clone, Debug)]
pub enum Item {
    Tile(TileDecl),
    Partition(PartitionDecl),
    Function(FunctionDecl),
    Pattern(PatternDecl),
    /// A parse error that was recovered from; the parser kept going.
    Error(ParseError),
}

// ---------------------------------------------------------------------------
// Tile declarations
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct TileDecl {
    pub name: TileRef,
    pub sides: Spanned<u32>,
    pub canonical: Vec<Spanned<Point>>,
    /// If absent at parse time, the normalizer fills it in.
    pub symmetry: Option<Spanned<SymmetryGroup>>,
}

/// A symmetry group: trivial, cyclic (Cn), or dihedral (Dn).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymmetryGroup {
    Trivial,
    Cyclic(u32),    // Cn
    Dihedral(u32),  // Dn
}

// ---------------------------------------------------------------------------
// Partition declarations
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct PartitionDecl {
    /// The tile (or representative tile of a class) this partition belongs to.
    pub tile: TileRef,
    /// True when the head was `tile.class.name` instead of `tile.name`.
    pub tile_is_class: bool,
    pub name: PartitionRef,
    pub vertices: Vec<Spanned<PartitionVertex>>,
    pub cuts: Vec<Spanned<Cut>>,
    /// Optional author-assigned names for normalizer-assigned child indices.
    pub child_names: Vec<Spanned<ChildName>>,
}

#[derive(Clone, Debug)]
pub struct PartitionVertex {
    pub kind: VertexKind,
    pub name: VertexRef,
    pub pos: Spanned<Point>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VertexKind {
    /// On a boundary edge of the parent polygon.
    Edge,
    /// Strictly interior.
    Interior,
}

/// A cut connecting two named vertices.
#[derive(Clone, Debug)]
pub struct Cut {
    pub from: VertexRef,
    pub to: VertexRef,
}

#[derive(Clone, Debug)]
pub struct ChildName {
    pub index: Spanned<u32>,
    /// `child N = name` — author-assigned human name (optional).
    pub name: Option<ChildNameRef>,
    /// `child N = name : tile` or `child N : tile` — explicit tile-type override (§4.6).
    pub tile_override: Option<TileRef>,
}

// ---------------------------------------------------------------------------
// User-defined functions
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct FunctionDecl {
    pub name: FuncRef,
    pub params: Vec<VarRef>,
    pub body: Spanned<Expr>,
}

// ---------------------------------------------------------------------------
// Pattern declaration
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct PatternDecl {
    pub root: TileRef,
    pub state: Spanned<StateBlock>,
    pub color: Spanned<ColorExpr>,
    pub rules: Vec<Spanned<RuleDecl>>,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct StateBlock {
    pub vars: Vec<Spanned<StateVar>>,
}

#[derive(Clone, Debug)]
pub struct StateVar {
    pub name: VarRef,
    /// `None` means the variable starts as `none` at the root.
    pub initial: Option<Spanned<Expr>>,
}

// ---------------------------------------------------------------------------
// Color expression (four components: R, G, B, A)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ColorExpr {
    pub r: Spanned<Expr>,
    pub g: Spanned<Expr>,
    pub b: Spanned<Expr>,
    pub a: Spanned<Expr>,
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct RuleDecl {
    /// The tile (or representative tile of a class) this rule targets.
    pub tile: TileRef,
    /// True when the head was `rule tile.class { … }` instead of `rule tile { … }`.
    pub tile_is_class: bool,
    pub body: Spanned<RuleBody>,
}

/// A rule body is a decision tree of branches leading to substitutions.
#[derive(Clone, Debug)]
pub enum RuleBody {
    /// `if <cond> { <body> } else { <body> }`
    If {
        condition: Spanned<Expr>,
        then_branch: Box<Spanned<RuleBody>>,
        else_branch: Box<Spanned<RuleBody>>,
    },
    /// `-> tile.partition { <substitution> }`
    Substitute(Spanned<Substitution>),
}

// ---------------------------------------------------------------------------
// Substitution
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Substitution {
    pub tile: TileRef,
    pub partition: PartitionRef,
    /// Tile-level state updates (sequential, applied before children expand).
    pub updates: Vec<Spanned<StateUpdate>>,
    /// Per-child state injection (pre-permutation: by canonical child name/index).
    pub child_injections: Vec<Spanned<ChildInjection>>,
    /// Slot permutation(s). One entry per tile class (or one with `All` for uniform partitions).
    pub slot_orders: Vec<Spanned<SlotOrder>>,
}

#[derive(Clone, Debug)]
pub struct StateUpdate {
    pub var: VarRef,
    pub value: Spanned<Expr>,
}

#[derive(Clone, Debug)]
pub struct ChildInjection {
    pub child: Spanned<ChildRef>,
    pub updates: Vec<Spanned<StateUpdate>>,
    /// Optional alignment override (discrete orientation from symmetry group).
    /// The specific implementation of this feature is uncertain and subject to change.
    pub alignment: Option<Spanned<Expr>>,
    /// Optional tile-type override for this child slot (§7.3.4).
    pub tile_override: Option<Spanned<Expr>>,
}

/// Reference to a child in a substitution block: by author-assigned name or by index.
#[derive(Clone, Debug)]
pub enum ChildRef {
    Name(ChildNameRef),
    Index(u32),
}

/// Which set of children a `slot_order` applies to.
#[derive(Clone, Debug)]
pub enum TileClassRef {
    /// `slot_order quad = ...` — a specific named tile type
    Tile(TileRef),
    /// `slot_order quad.class = ...` — all tiles in the same equivalence class as `quad`
    Class(TileRef),
    /// No qualifier — uniform partition, applies to all children
    All,
}

#[derive(Clone, Debug)]
pub struct SlotOrder {
    pub target: TileClassRef,
    pub value: Spanned<Expr>,
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum Expr {
    // Literals
    Float(f64),
    Bool(bool),
    None,
    /// Group element literal, e.g. `d4.r90`
    GroupElem {
        group: Spanned<String>,
        element: Spanned<String>,
    },
    /// Explicit permutation literal, e.g. `perm [0, 1, 2, 3]`
    PermLiteral(Vec<Spanned<u32>>),

    // Variable access
    Var(VarRef),

    // Built-in geometric inputs
    PosX,
    PosY,
    Scale,
    Orientation,
    Shear,
    Stretch,
    Depth,
    Random,
    /// `child.canonical_orientation` — valid inside child blocks and tile-level update
    ChildCanonicalOrientation,
    /// `child.index`
    ChildIndex,

    // Arithmetic
    Neg(Box<Spanned<Expr>>),
    Add(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Sub(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Mul(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Div(Box<Spanned<Expr>>, Box<Spanned<Expr>>),

    // Math functions
    Sin(Box<Spanned<Expr>>),
    Cos(Box<Spanned<Expr>>),
    Exp(Box<Spanned<Expr>>),
    Sqrt(Box<Spanned<Expr>>),
    Abs(Box<Spanned<Expr>>),
    Log(Box<Spanned<Expr>>),

    // Interpolation / clamping
    Lerp(Box<Spanned<Expr>>, Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Clamp(Box<Spanned<Expr>>, Box<Spanned<Expr>>, Box<Spanned<Expr>>),

    // Null coalescing: `x ? default`
    NullCoalesce(Box<Spanned<Expr>>, Box<Spanned<Expr>>),

    // Comparisons
    Lt(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Gt(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Le(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Ge(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Eq(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Ne(Box<Spanned<Expr>>, Box<Spanned<Expr>>),

    // Logic
    And(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Or(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Not(Box<Spanned<Expr>>),

    // Conditional expression
    If {
        condition: Box<Spanned<Expr>>,
        then_expr: Box<Spanned<Expr>>,
        else_expr: Box<Spanned<Expr>>,
    },

    // Group / permutation operations
    Compose(Box<Spanned<Expr>>, Box<Spanned<Expr>>),
    Inverse(Box<Spanned<Expr>>),

    // User-defined function call
    Call {
        func: FuncRef,
        args: Vec<Spanned<Expr>>,
    },

    /// An expression that failed to parse (error recovery node).
    Error(ParseError),
}

// ---------------------------------------------------------------------------
// Primitives
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

// ---------------------------------------------------------------------------
// Parse errors (carried inline in the AST for error recovery)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}
