// ============================================================
//  slvt AST
//  Parser가 만들어낼 트리의 설계도
//  Checker가 여기 담긴 정보를 보고 안전성 검사를 수행
// ============================================================

// ────────────────────────────────────────────────────────────
//  위치 정보
//  모든 노드에 달아두면 Checker가 오류 위치를 정확히 보고 가능
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Span {
    pub line: usize,
    pub col:  usize,
}

// 노드 + 위치를 묶는 래퍼
// 사용: Spanned<Expr>, Spanned<Stmt> ...
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

// ────────────────────────────────────────────────────────────
//  타입
//  변수, 파라미터, 반환값 등 모든 곳에서 쓰임
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    // 기본 스칼라
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

    // 커스텀 struct 이름  (예: VertexIn, SceneUniforms)
    Named(String),

    // 반환값 없음  void 대신 → () 느낌
    Unit,
}

// ────────────────────────────────────────────────────────────
//  주소 공간
//  Checker의 Polonius / borrow 검사에서 핵심으로 쓰임
//
//  속도:   thread > threadgroup > constant > device
//  안전:   thread, constant 는 안전
//          device, threadgroup 은 Checker가 엄격하게 검사
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq)]
pub enum AddressSpace {
    Device,       // GPU VRAM — read/write 가능, 가장 느림
    Constant,     // 읽기 전용 상수 버퍼 — uniform이 여기
    Threadgroup,  // 스레드 그룹 공유 — race condition 주의
    Thread,       // 스레드 로컬 — 일반 변수 기본값
}

// ────────────────────────────────────────────────────────────
//  어트리뷰트  @binding(0), @stage_in, @position ...
//  struct 필드와 함수 파라미터에 붙음
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: String,       // "binding", "stage_in", "position" ...
    pub args: Vec<String>,  // 괄호 안 인자  @binding(0) → ["0"]
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
//  .slvt 파일의 "큰 덩어리" 단위
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

    // @binding(1) @group(0) buffer Particles: [float4; 1024]
    // @binding(1) @group(0) texture2d base_tex: texture2d
    // @binding(2) @group(0) sampler   base_smp: sampler
    Resource {
        attrs:         Vec<Attribute>,
        name:          String,
        ty:            Type,
        address_space: AddressSpace,  // Checker가 접근 규칙 검사에 사용
        span:          Span,
    },

    // vertex   fn vs_main(...) -> VertexOut { ... }
    // fragment fn fs_main(...) -> float4   { ... }
    // kernel   fn cs_main(...)             { ... }
    // fn helper(...) -> float              { ... }
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
    pub attrs: Vec<Attribute>,  // @position, @attribute(0) ...
    pub name:  String,
    pub ty:    Type,
    pub span:  Span,
}

// ────────────────────────────────────────────────────────────
//  함수 파라미터
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Param {
    pub attrs:         Vec<Attribute>,  // @stage_in, @buffer(0) ...
    pub name:          String,
    pub ty:            Type,
    pub address_space: AddressSpace,    // Checker의 borrow 검사에 사용
    pub span:          Span,
}

// ────────────────────────────────────────────────────────────
//  구문 (Statement)
//  함수 body 안에 오는 것들
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

    // if cond { ... } else { ... }
    If {
        cond:      Spanned<Expr>,
        then_body: Vec<Spanned<Stmt>>,
        else_body: Option<Vec<Spanned<Stmt>>>,
        span:      Span,
    },

    // for i in 0..n { ... }
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
//  값을 만들어내는 모든 것
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub enum Expr {

    // 리터럴
    IntLit(i64),
    FloatLit(f64),
    BoolLit(bool),

    // 변수 참조
    // Checker: 선언됐는가, 주소 공간이 맞는가, move 됐는가
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
    // Codegen에서 → tex.sample(smp, uv) 로 변환
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
//  파싱이 끝나면 이 하나가 결과로 나옴
// ────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<TopLevel>,
}