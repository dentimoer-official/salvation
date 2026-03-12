// 여기서는 lexer에서 만든 문법에 사용될 단어들을 어떻게 조립 될 수 있는지 정리할꺼임
// 설명서 같은 역할 

pub mod types;

#[derive(Debug, Clone)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

// 노드랑 위치를 묶는 애
// 사용: Spanned<Expr>, Spanned<Stmt>
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

// 메모리 공간 문법