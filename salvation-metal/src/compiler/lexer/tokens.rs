/*
"let x: float = 1.0;"
→ [Let] [Ident("x")] [Colon] [Float] [Eq] [FloatLit(1.0)] [Semicolon]

이런 느낌으로 쪼갤꺼임
*/

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Import,
    Type,
    Struct,
    Fn,
    Vertex,
    Fragment,
    Kernel,
    Uniform,
    Buffer,
    Texture2D,
    Sampler,

    // 구문으로 쓸 애들
    Let,
    Mut,
    Return,
    If,
    Else,
    For,
    In,

    // 타입으로 쓸 애들
    Bool,
    Int,
    Uint,   // Unit → Uint 수정
    Float,
    Float2,
    Float3,
    Float4,

    // 행렬 타입 (Metal 지원 전체)
    Mat2x2, Mat2x3, Mat2x4,
    Mat3x2, Mat3x3, Mat3x4,
    Mat4x2, Mat4x3, Mat4x4,

    // 주소 공간 키워드
    Device,       // GPU VRAM (read/write)
    Constant,     // 읽기 전용 상수 버퍼
    Threadgroup,  // 스레드 그룹 공유 메모리
    Thread,       // 스레드 로컬

    // literal 문법
    IntLit(i64),
    FloatLit(f64), // Flaot 오타 수정
    BoolLit(bool),
    StrLit(String),

    // 식별자
    Ident(String),

    // 산술 연산자
    Plus,     // +
    Minus,    // -
    Star,     // *
    Slash,    // /
    Percent,  // %

    // 비교 & 논리 연산자
    Eq,      // =
    EqEq,    // ==
    BangEq,  // !=
    Lt,      // <
    Gt,      // >
    LtEq,    // <=
    GtEq,    // >=
    And,     // &&
    Or,      // ||
    Bang,    // !

    // 구조 연산자
    Dot,         // .
    DotDot,      // ..
    Arrow,       // ->
    ColonColon,  // ::
    At,          // @

    // 구분자
    LParen,    // (
    RParen,    // )
    LBrace,    // {
    RBrace,    // }
    LBracket,  // [
    RBracket,  // ]
    Semicolon, // ;
    Colon,     // :
    Comma,     // ,

    // 특수
    Eof,
}
