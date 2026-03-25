// 언어에서 쓸 수 있는 타입들
#[derive(Debug, Clone)]
pub enum Type {
    Bool,
    Int,
    Uint,
    Float,

    // 벡터
    Float2,
    Float3,
    Float4,

    // 행렬 (Metal 지원 전체)
    Mat2x2, Mat2x3, Mat2x4,
    Mat3x2, Mat3x3, Mat3x4,
    Mat4x2, Mat4x3, Mat4x4,

    // GPU 리소스
    Texture2D,
    Sampler,

    // 배열  [float; 4]
    Array {
        inner: Box<Type>,
        size:  usize,
    },

    // 커스텀 struct 이름
    Named(String),

    // 반환값 없음 — void 대신 () 느낌
    Unit,
}

// 표현식
#[derive(Debug, Clone)]
pub enum Expr {
    // 리터럴
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),

    // 변수 참조
    Ident(String),

    // 이항 연산  a + b
    BinOp {
        op: BinOpKind,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },

    // 단항 연산  !x, -x
    UnaryOp {
        op: UnaryOpKind,
        expr: Box<Expr>,
    },

    // 함수 호출  foo(a, b)
    Call {
        name: String,
        args: Vec<Expr>,
    },

    // 필드 접근  v.x
    Field {
        object: Box<Expr>,
        field: String,
    },

    // 인덱스  arr[i]
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
}

#[derive(Debug, Clone)]
pub enum BinOpKind {
    Add, Sub, Mul, Div, Mod,
    Eq, NotEq,
    Lt, Gt, LtEq, GtEq,
    And, Or,
    Assign,
}

#[derive(Debug, Clone)]
pub enum UnaryOpKind {
    Neg,  // -x
    Not,  // !x
}

// 구문 (Statement)
#[derive(Debug, Clone)]
pub enum Stmt {
    // let [mut] x: Type = expr;
    VarDecl {
        name: String,
        mutable: bool,
        ty: Type,
        value: Option<Expr>,
    },

    // return expr;
    Return(Option<Expr>),

    // if expr { } else { }
    If {
        cond: Expr,
        then_block: Block,
        else_block: Option<Block>,
    },

    // for i in 0..n { }
    For {
        var: String,
        from: Expr,
        to: Expr,
        body: Block,
    },

    // 단독 표현식  foo();
    ExprStmt(Expr),
}

// 블록 { stmt; stmt; }
pub type Block = Vec<Stmt>;

// 함수 인자  name: Type
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

// 어트리뷰트  @vertex / @fragment / @kernel
#[derive(Debug, Clone)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Kernel,
}

// 최상위 선언들
#[derive(Debug, Clone)]
pub enum Item {
    // fn name(args) -> RetType { body }
    FnDecl {
        stage: Option<ShaderStage>,
        name: String,
        params: Vec<Param>,
        ret_ty: Option<Type>,
        body: Block,
    },

    // struct Name { fields }
    StructDecl {
        name: String,
        fields: Vec<Param>,
    },

    // import "path"
    Import(String),
}

// 파일 전체
pub type Program = Vec<Item>;
