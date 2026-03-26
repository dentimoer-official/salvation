// 여기서는 salvation 문법에 사용될 단어들을 만들꺼임
// 코드의 가장 기초적으로 사용될 단어들을 정의하는 역할

/*
"let x: float = 1.0;"
→ [Let] [Ident("x")] [Colon] [Float] [Eq] [FloatLit(1.0)] [Semicolon]

이런 느낌으로 쪼갤꺼임
*/

pub mod tokens;

use tokens::Token;

// span은 공간적, 시각적인 범위를 의
// 여기서는 작업이나 데이터를 코드나 글자의 보니까, 행과 열을 받아서 이것저것 관리 하도록 하는거임
#[derive(Debug, Clone)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(src: &str) -> Self {
        Lexer {
            source: src.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    // 데이터를 딱 읽기만 하는 동작
    fn peek(&self) -> Option<char> {
        self.source.get(self.pos).copied()
    }

    // 데이터를 딱 읽기만 하는 동작 2
    fn peek2(&self) -> Option<char> {
        self.source.get(self.pos + 1).copied()
    }

    // 포인터를 이동시키며 현재 문자를 소비
    fn advance(&mut self) -> Option<char> {
        let ch = self.source.get(self.pos).copied();
        if let Some(c) = ch {
            self.pos += 1;
            if c == '\n' {
                self.line += 1; // += 뒤 값 누락 수정
                self.col = 1; // * 1 → = 1 수정
            } else {
                self.col += 1;
            }
        }
        ch
    }

    // 접근 가능한 데이터의 범위를 관리할 때 쓸 함수
    fn span(&self) -> Span {
        Span {
            line: self.line,
            col: self.col,
        }
    }

    // 띄어쓰기, 줄바꿈, 탭, 주석 등을 건너뛰는 함수
    fn skip_whitespace_and_comments(&mut self) {
        loop {
            while self.peek().map_or(false, |c| c.is_whitespace()) {
                self.advance();
            }
            if self.peek() == Some('/') && self.peek2() == Some('/') {
                while self.peek().map_or(false, |c| c != '\n') {
                    self.advance();
                }
                continue;
            }
            if self.peek() == Some('/') && self.peek2() == Some('*') {
                self.advance();
                self.advance();
                loop {
                    if self.peek() == Some('*') && self.peek2() == Some('/') {
                        self.advance();
                        self.advance();
                        break;
                    }
                    if self.advance().is_none() {
                        break;
                    }
                }
                continue;
            }
            break;
        }
    }

    // 식별자 & 키워드
    //
    // 새 키워드 추가 방법:
    // 1. Token enum에 variant 추가
    // 2. 아래 match에 "문자열" => Token::Variant 추가
    // 알파벳 순으로 정리
    fn read_ident_or_keyword(&mut self) -> Token {
        let mut s = String::new();
        // is_alphabetic → is_alphanumeric 수정 (float2, mat4x4 등 숫자 포함 키워드 처리)
        while self
            .peek()
            .map_or(false, |c| c.is_alphanumeric() || c == '_')
        {
            s.push(self.advance().unwrap());
        }
        match s.as_str() {
            // 최상위 선언 키워드
            "buffer"    => Token::Buffer,
            "fn"        => Token::Fn,
            "fragment"  => Token::Fragment,
            "import"    => Token::Import,
            "kernel"    => Token::Kernel,
            "main"      => Token::Main,
            "pub"       => Token::Pub,
            "sampler"   => Token::Sampler,
            "struct"    => Token::Struct,
            "texture2d" => Token::Texture2D,
            "type"      => Token::Type,
            "uniform"   => Token::Uniform,
            "vertex"    => Token::Vertex,

            // 구문 키워드
            "break"    => Token::Break,
            "continue" => Token::Continue,
            "else"     => Token::Else,
            "for"      => Token::For,
            "if"       => Token::If,
            "in"       => Token::In,
            "let"      => Token::Let,
            "mut"      => Token::Mut,
            "return"   => Token::Return,
            "while"    => Token::While,

            // 타입 키워드
            "bool" => Token::Bool,
            "float" => Token::Float,
            "float2" => Token::Float2,
            "float3" => Token::Float3,
            "float4" => Token::Float4,
            "int" => Token::Int,
            "uint" => Token::Uint,
            "mat2x2" => Token::Mat2x2,
            "mat2x3" => Token::Mat2x3,
            "mat2x4" => Token::Mat2x4,
            "mat3x2" => Token::Mat3x2,
            "mat3x3" => Token::Mat3x3,
            "mat3x4" => Token::Mat3x4,
            "mat4x2" => Token::Mat4x2,
            "mat4x3" => Token::Mat4x3,
            "mat4x4" => Token::Mat4x4,

            // 주소 공간 키워드
            "constant" => Token::Constant,
            "device" => Token::Device,
            "thread" => Token::Thread,
            "threadgroup" => Token::Threadgroup,

            // 리터럴 키워드
            "true" => Token::BoolLit(true),
            "false" => Token::BoolLit(false),

            // 그 외는 전부 식별자
            _ => Token::Ident(s),
        }
    }

    // 숫자 리터럴
    fn read_number(&mut self) -> Token {
        let mut s = String::new();
        let mut is_float = false;

        while self.peek().map_or(false, |c| c.is_ascii_digit()) {
            s.push(self.advance().unwrap());
        }
        if self.peek() == Some('.') && self.peek2().map_or(false, |c| c.is_ascii_digit()) {
            is_float = true;
            s.push(self.advance().unwrap());
            while self.peek().map_or(false, |c| c.is_ascii_digit()) {
                s.push(self.advance().unwrap());
            }
        }

        // f/F 접미사 소비 (1.0f, 0.5F — Metal/GLSL 습관 코드 호환)
        // 값에는 영향을 주지 않고 조용히 무시함
        if matches!(self.peek(), Some('f') | Some('F')) {
            is_float = true;
            self.advance();
        }

        if is_float {
            Token::FloatLit(s.parse().unwrap_or(0.0))
        } else {
            Token::IntLit(s.parse().unwrap())
        }
    }

    // 문자열 리터럴 "..."
    fn read_string(&mut self) -> Result<Token, String> {
        self.advance(); // 여는 '"' 소비
        let mut s = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => {
                    return Err(format!(
                        "{}:{} 문자열이 닫히지 않았습니다",
                        self.line, self.col
                    ));
                }
                Some('"') => {
                    self.advance();
                    break;
                }
                _ => {
                    s.push(self.advance().unwrap());
                }
            }
        }
        Ok(Token::StrLit(s))
    }

    // 메인 토크나이저
    pub fn tokenize(&mut self) -> Result<Vec<Spanned<Token>>, String> {
        let mut tokens = Vec::new();

        loop {
            self.skip_whitespace_and_comments();
            let span = self.span();

            let ch = match self.peek() {
                None => {
                    tokens.push(Spanned {
                        node: Token::Eof,
                        span,
                    });
                    break;
                }
                Some(c) => c,
            };

            let tok = match ch {
                // 식별자 / 키워드
                'a'..='z' | 'A'..='Z' | '_' => self.read_ident_or_keyword(),

                // 숫자
                '0'..='9' => self.read_number(),

                // 문자열
                '"' => self.read_string()?,

                // 두 글자 연산자
                '-' => {
                    self.advance();
                    if self.peek() == Some('>') {
                        self.advance();
                        Token::Arrow
                    } else if self.peek() == Some('=') {
                        self.advance();
                        Token::MinusEq
                    } else {
                        Token::Minus
                    }
                }
                '=' => {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        Token::EqEq
                    } else {
                        Token::Eq
                    }
                }
                '!' => {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        Token::BangEq
                    } else {
                        Token::Bang
                    }
                }
                '<' => {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        Token::LtEq
                    } else {
                        Token::Lt
                    }
                }
                '>' => {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        Token::GtEq
                    } else {
                        Token::Gt
                    }
                }
                '&' => {
                    self.advance();
                    if self.peek() == Some('&') {
                        self.advance();
                        Token::And
                    } else {
                        return Err(format!(
                            "{}:{} 단일 '&' 는 지원하지 않습니다 (논리 AND는 &&)",
                            span.line, span.col
                        ));
                    }
                }
                '|' => {
                    self.advance();
                    if self.peek() == Some('|') {
                        self.advance();
                        Token::Or
                    } else {
                        return Err(format!(
                            "{}:{} 단일 '|' 는 지원하지 않습니다 (논리 OR는 ||)",
                            span.line, span.col
                        ));
                    }
                }
                '.' => {
                    self.advance();
                    if self.peek() == Some('.') {
                        self.advance();
                        Token::DotDot
                    } else {
                        Token::Dot
                    }
                }
                ':' => {
                    self.advance();
                    if self.peek() == Some(':') {
                        self.advance();
                        Token::ColonColon
                    } else {
                        Token::Colon
                    }
                }

                // 단일 문자 연산자 / 구분자
                '+' => {
                    self.advance();
                    if self.peek() == Some('=') { self.advance(); Token::PlusEq }
                    else { Token::Plus }
                }
                '*' => {
                    self.advance();
                    if self.peek() == Some('=') { self.advance(); Token::StarEq }
                    else { Token::Star }
                }
                '/' => {
                    self.advance();
                    if self.peek() == Some('=') { self.advance(); Token::SlashEq }
                    else { Token::Slash }
                }
                '%' => {
                    self.advance();
                    if self.peek() == Some('=') { self.advance(); Token::PercentEq }
                    else { Token::Percent }
                }
                '@' => {
                    self.advance();
                    Token::At
                }
                '(' => {
                    self.advance();
                    Token::LParen
                }
                ')' => {
                    self.advance();
                    Token::RParen
                }
                '{' => {
                    self.advance();
                    Token::LBrace
                }
                '}' => {
                    self.advance();
                    Token::RBrace
                }
                '[' => {
                    self.advance();
                    Token::LBracket
                }
                ']' => {
                    self.advance();
                    Token::RBracket
                }
                ';' => {
                    self.advance();
                    Token::Semicolon
                }
                ',' => {
                    self.advance();
                    Token::Comma
                }

                // 알 수 없는 문자
                c => {
                    return Err(format!(
                        "{}:{} 알 수 없는 문자: '{}'",
                        span.line, span.col, c
                    ));
                }
            };

            tokens.push(Spanned { node: tok, span });
        }

        Ok(tokens)
    }
}

// ────────────────────────────────────────────────────────────
// 테스트
// ────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn lex(src: &str) -> Vec<Token> {
        Lexer::new(src)
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|s| s.node)
            .filter(|t| *t != Token::Eof)
            .collect()
    }

    #[test]
    fn test_let_mut() {
        let t = lex("let mut x: float = 1.0;");
        assert_eq!(
            t,
            vec![
                Token::Let,
                Token::Mut,
                Token::Ident("x".into()),
                Token::Colon,
                Token::Float,
                Token::Eq,
                Token::FloatLit(1.0),
                Token::Semicolon,
            ]
        );
    }

    #[test]
    fn test_matrix_types() {
        let t = lex("mat4x4 mat3x3 mat2x4");
        assert_eq!(t, vec![Token::Mat4x4, Token::Mat3x3, Token::Mat2x4]);
    }

    #[test]
    fn test_address_spaces() {
        let t = lex("device constant threadgroup thread");
        assert_eq!(
            t,
            vec![
                Token::Device,
                Token::Constant,
                Token::Threadgroup,
                Token::Thread,
            ]
        );
    }

    #[test]
    fn test_for_range() {
        let t = lex("for i in 0..10");
        assert_eq!(
            t,
            vec![
                Token::For,
                Token::Ident("i".into()),
                Token::In,
                Token::IntLit(0),
                Token::DotDot,
                Token::IntLit(10),
            ]
        );
    }

    #[test]
    fn test_two_char_ops() {
        let t = lex("-> .. == != <= >= && ||");
        assert_eq!(
            t,
            vec![
                Token::Arrow,
                Token::DotDot,
                Token::EqEq,
                Token::BangEq,
                Token::LtEq,
                Token::GtEq,
                Token::And,
                Token::Or,
            ]
        );
    }

    #[test]
    fn test_import() {
        let t = lex("import \"common/math.slvt\"");
        assert_eq!(
            t,
            vec![Token::Import, Token::StrLit("common/math.slvt".into()),]
        );
    }

    #[test]
    fn test_comment_skip() {
        let t = lex("let // ignored\nx: float");
        assert_eq!(
            t,
            vec![
                Token::Let,
                Token::Ident("x".into()),
                Token::Colon,
                Token::Float,
            ]
        );
    }

    #[test]
    fn test_float_suffix() {
        // 1.0f, 0.5F, 2f 모두 FloatLit으로 파싱
        let t = lex("1.0f 0.5F 2f");
        assert_eq!(
            t,
            vec![
                Token::FloatLit(1.0),
                Token::FloatLit(0.5),
                Token::FloatLit(2.0),
            ]
        );
    }

    #[test]
    fn test_compound_assign() {
        let t = lex("+= -= *= /= %=");
        assert_eq!(
            t,
            vec![
                Token::PlusEq,
                Token::MinusEq,
                Token::StarEq,
                Token::SlashEq,
                Token::PercentEq,
            ]
        );
    }

    #[test]
    fn test_while_break_continue() {
        let t = lex("while break continue");
        assert_eq!(
            t,
            vec![Token::While, Token::Break, Token::Continue]
        );
    }

    #[test]
    fn test_arrow_not_minuseq() {
        // -> 는 Arrow, -= 는 MinusEq — 혼동 없어야 함
        let t = lex("-> -=");
        assert_eq!(t, vec![Token::Arrow, Token::MinusEq]);
    }
}
