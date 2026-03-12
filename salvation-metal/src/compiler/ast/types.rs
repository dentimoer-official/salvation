// 여기서 .slvt에 코드가 쓸 타입 정의
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    // 기본
    Bool,
    Int,
    Uint,
    Float,
    
    // 벡터
    Float2,
    Float3,
    Float4,
    
    // 행렬
    Mat2x2, Mat2x3, Mat2x4,
    Mat3x2, Mat3x3, Mat3x4,
    Mat4x2, Mat4x3, Mat4x4,
    
    // GPU 자원
    Texture2D,
    Sampler,
    
    // 배열 [float; 4]
    Array {
        inner: Box<Type>,
        size:  usize,
    },
    
    // 커스텀 Struct 이름
    Named(String),
    
    // Void 반환값 처리용.
    // () 같은 느낌으로 처리될 예정 
    Unit,
}
