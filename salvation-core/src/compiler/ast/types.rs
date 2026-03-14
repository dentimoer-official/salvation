// .slvt 에서 쓸 타입 정의

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

    // 커스텀 struct 이름
    Named(String),

    // 반환값 없음 — void 대신 () 느낌
    Unit,
}