// 여기서는 lexer에서 만든 문법에 사용될 단어들을 어떻게 조립 될 수 있는지 정리할꺼임
// 설명서 같은 역할 

pub mod types;
pub mod spaces;

use types::Type;
use spaces::AddressSpace;

// ────────────────────────────────────────────────────────────
//  위치 정보
//  모든 노드에 달아두면 Checker가 오류 위치를 정확히 보고 가능
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Span {
    pub line: usize,
    pub col:  usize,
}

#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

// ────────────────────────────────────────────────────────────
//  어트리뷰트  @binding(0), @stage_in, @position ...
//  struct 필드와 함수 파라미터에 붙음
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: String,       // "binding", "stage_in", "position" ...
    pub args: Vec<String>,  // @binding(0) → ["0"]
    pub span: Span,
}

// ────────────────────────────────────────────────────────────
//  셰이더 스테이지
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Kernel,  // GPGPU compute
    None,    // 일반 헬퍼 함수
}

// ────────────────────────────────────────────────────────────
//  최상위 선언 (Top-level)
//  .slvt 파일의 큰 덩어리 단위
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub enum TopLevel {

    // import "path/to/file.slvt"
    Import {
        path: String,
        span: Span,
    },

    // type Vec3 = float3
    TypeAlias {
        name:   String,
        target: Type,
        span:   Span,
    },

    // struct VertexIn { ... }
    Struct {
        name:   String,
        fields: Vec<Field>,
        span:   Span,
    },

    // @binding(0) @group(0)
    // uniform SceneUniforms { ... }
    Uniform {
        attrs:  Vec<Attribute>,
        name:   String,
        fields: Vec<Field>,
        span:   Span,
    },

    // @binding(1) buffer / texture2d / sampler
    Resource {
        attrs:         Vec<Attribute>,
        name:          String,
        ty:            Type,
        address_space: AddressSpace,
        span:          Span,
    },

    // vertex fn / fragment fn / kernel fn / fn
    Function {
        stage:  ShaderStage,
        name:   String,
        params: Vec<Param>,
        ret:    Type,
        body:   Vec<Spanned<Stmt>>,
        span:   Span,
    },
}

// ────────────────────────────────────────────────────────────
//  Struct 필드
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Field {
    pub attrs: Vec<Attribute>,
    pub name:  String,
    pub ty:    Type,
    pub span:  Span,
}

// ────────────────────────────────────────────────────────────
//  함수 파라미터
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Param {
    pub attrs:         Vec<Attribute>,
    pub name:          String,
    pub ty:            Type,
    pub address_space: AddressSpace,  // Checker의 borrow 검사에 사용
    pub span:          Span,
}

// ────────────────────────────────────────────────────────────
//  구문 (Statement)
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub enum Stmt {

    // let x: float = expr
    // let mut x: float = expr
    Let {
        mutable: bool,          // true → let mut
        name:    String,
        ty:      Option<Type>,  // 생략 가능 → Checker가 추론
        init:    Spanned<Expr>,
        span:    Span,
    },

    // x = expr
    Assign {
        target: Spanned<Expr>,
        value:  Spanned<Expr>,
        span:   Span,
    },

    // return expr
    Return {
        value: Option<Spanned<Expr>>,
        span:  Span,
    },

    // if cond { } else { }
    If {
        cond:      Spanned<Expr>,
        then_body: Vec<Spanned<Stmt>>,
        else_body: Option<Vec<Spanned<Stmt>>>,
        span:      Span,
    },

    // for i in 0..n { }
    For {
        var:  String,
        from: Spanned<Expr>,
        to:   Spanned<Expr>,
        body: Vec<Spanned<Stmt>>,
        span: Span,
    },

    // 단독 표현식  foo(x);
    Expr {
        expr: Spanned<Expr>,
        span: Span,
    },
}

// ────────────────────────────────────────────────────────────
//  표현식 (Expression)
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub enum Expr {

    // 리터럴
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),

    // 변수 참조
    // Checker: 선언됐는가, 주소 공간 맞는가, move 됐는가
    Ident(String),

    // 이항 연산  a + b
    BinOp {
        op:  BinOp,
        lhs: Box<Spanned<Expr>>,
        rhs: Box<Spanned<Expr>>,
    },

    // 단항 연산  -a  !a
    UnOp {
        op:   UnOp,
        expr: Box<Spanned<Expr>>,
    },

    // 함수 호출  foo(a, b)
    Call {
        func: String,
        args: Vec<Spanned<Expr>>,
    },

    // 필드 접근  v.xyz  uniforms.mvp
    Field {
        base:  Box<Spanned<Expr>>,
        field: String,
    },

    // 인덱스 접근  arr[i]
    Index {
        base:  Box<Spanned<Expr>>,
        index: Box<Spanned<Expr>>,
    },

    // 타입 생성자  float4(1.0, 0.0, 0.0, 1.0)
    Constructor {
        ty:   Type,
        args: Vec<Spanned<Expr>>,
    },

    // 텍스처 샘플링  sample(tex, smp, uv)
    // Codegen → tex.sample(smp, uv)
    Sample {
        texture: Box<Spanned<Expr>>,
        sampler: Box<Spanned<Expr>>,
        coord:   Box<Spanned<Expr>>,
    },
}

// ────────────────────────────────────────────────────────────
//  연산자
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,  // 산술
    Eq, Ne, Lt, Gt, Le, Ge,   // 비교
    And, Or,                   // 논리
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnOp {
    Neg,  // -x
    Not,  // !x
}

// ────────────────────────────────────────────────────────────
//  프로그램 (루트 노드)
//  파싱 결과 이 하나가 나옴
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<TopLevel>,
}