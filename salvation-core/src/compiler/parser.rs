// 여기서 ast의 내용을 codegen에 가기 전에 최종 가공 할꺼임
// lexer에서 정의도니 문법을 ast에서 조립하는 방법을 알려주면, 얘는 .slvt파일의 코드가 멀쩡한가 확인 역할
// 메모리 문제나 알고리즘 문제는 여기서 안 함. 
// 딱 그냥 코드가 작동이 되나만 판단해서, 완벽하게 변환 가능하면 그거 허가 해주고 제작도 해주는 애

// ============================================================
//  slvt Parser
//  Token 스트림 → AST
//  "문법 모양이 맞는가" 만 검사, 의미/타입은 Checker가 담당
// ============================================================

use crate::compiler::lexer::Spanned;
use crate::compiler::lexer::tokens::Token;
use crate::compiler::ast::{
    Attribute, BinOp, Expr, Field, Param, Program,
    ShaderStage, Spanned as AstSpanned, Span as AstSpan,
    Stmt, TopLevel, UnOp,
};
use crate::compiler::ast::types::Type;
use crate::compiler::ast::spaces::AddressSpace;

// ────────────────────────────────────────────────────────────
//  파서 에러
// ────────────────────────────────────────────────────────────
#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub line:    usize,
    pub col:     usize,
}

impl ParseError {
    fn new(msg: impl Into<String>, line: usize, col: usize) -> Self {
        ParseError { message: msg.into(), line, col }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{} {}", self.line, self.col, self.message)
    }
}

type ParseResult<T> = Result<T, ParseError>;

// ────────────────────────────────────────────────────────────
//  Parser 구조체
// ────────────────────────────────────────────────────────────
pub struct Parser {
    tokens: Vec<Spanned<Token>>,
    pos:    usize,
}

// ────────────────────────────────────────────────────────────
//  헬퍼 함수들
// ────────────────────────────────────────────────────────────
impl Parser {
    pub fn new(tokens: Vec<Spanned<Token>>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos].node
    }

    fn current_span(&self) -> (usize, usize) {
        let s = &self.tokens[self.pos].span;
        (s.line, s.col)
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos].node;
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &Token) -> ParseResult<()> {
        if self.peek() == expected {
            self.advance();
            Ok(())
        } else {
            let (l, c) = self.current_span();
            Err(ParseError::new(
                format!("'{:?}' 예상, '{:?}' 발견", expected, self.peek()),
                l, c,
            ))
        }
    }

    fn expect_ident(&mut self) -> ParseResult<String> {
        let (l, c) = self.current_span();
        match self.peek().clone() {
            Token::Ident(s) => { self.advance(); Ok(s) }
            Token::In       => { self.advance(); Ok("in".into())  }
            Token::Mut      => { self.advance(); Ok("mut".into()) }
            _ => Err(ParseError::new(
                format!("식별자 예상, '{:?}' 발견", self.peek()),
                l, c,
            )),
        }
    }

    fn check(&self, tok: &Token) -> bool {
        self.peek() == tok
    }

    fn eat(&mut self, tok: &Token) -> bool {
        if self.check(tok) { self.advance(); true }
        else { false }
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), Token::Eof)
    }

    fn ast_span(&self) -> AstSpan {
        let (line, col) = self.current_span();
        AstSpan { line, col }
    }
}

// ────────────────────────────────────────────────────────────
//  타입 파싱
// ────────────────────────────────────────────────────────────
impl Parser {
    fn parse_type(&mut self) -> ParseResult<Type> {
        let (l, c) = self.current_span();
        match self.peek().clone() {
            Token::Bool      => { self.advance(); Ok(Type::Bool)      }
            Token::Int       => { self.advance(); Ok(Type::Int)       }
            Token::Uint      => { self.advance(); Ok(Type::Uint)      }
            Token::Float     => { self.advance(); Ok(Type::Float)     }
            Token::Float2    => { self.advance(); Ok(Type::Float2)    }
            Token::Float3    => { self.advance(); Ok(Type::Float3)    }
            Token::Float4    => { self.advance(); Ok(Type::Float4)    }
            Token::Mat2x2    => { self.advance(); Ok(Type::Mat2x2)    }
            Token::Mat2x3    => { self.advance(); Ok(Type::Mat2x3)    }
            Token::Mat2x4    => { self.advance(); Ok(Type::Mat2x4)    }
            Token::Mat3x2    => { self.advance(); Ok(Type::Mat3x2)    }
            Token::Mat3x3    => { self.advance(); Ok(Type::Mat3x3)    }
            Token::Mat3x4    => { self.advance(); Ok(Type::Mat3x4)    }
            Token::Mat4x2    => { self.advance(); Ok(Type::Mat4x2)    }
            Token::Mat4x3    => { self.advance(); Ok(Type::Mat4x3)    }
            Token::Mat4x4    => { self.advance(); Ok(Type::Mat4x4)    }
            Token::Texture2D => { self.advance(); Ok(Type::Texture2D) }
            Token::Sampler   => { self.advance(); Ok(Type::Sampler)   }
            Token::Ident(s)  => { self.advance(); Ok(Type::Named(s))  }
            Token::LBracket  => {
                self.advance();
                let inner = self.parse_type()?;
                self.expect(&Token::Semicolon)?;
                let size = match self.peek().clone() {
                    Token::IntLit(n) => { self.advance(); n as usize }
                    _ => return Err(ParseError::new("배열 크기는 정수여야 합니다", l, c)),
                };
                self.expect(&Token::RBracket)?;
                Ok(Type::Array { inner: Box::new(inner), size })
            }
            _ => Err(ParseError::new(
                format!("타입 예상, '{:?}' 발견", self.peek()),
                l, c,
            )),
        }
    }
}

// ────────────────────────────────────────────────────────────
//  어트리뷰트 파싱  @name(arg1, arg2)
// ────────────────────────────────────────────────────────────
impl Parser {
    fn parse_attributes(&mut self) -> ParseResult<Vec<Attribute>> {
        let mut attrs = Vec::new();
        while self.check(&Token::At) {
            let span = self.ast_span();
            self.advance();
            let name = self.expect_ident()?;
            let mut args = Vec::new();
            if self.eat(&Token::LParen) {
                while !self.check(&Token::RParen) && !self.at_eof() {
                    match self.peek().clone() {
                        Token::Ident(s)  => { self.advance(); args.push(s); }
                        Token::IntLit(n) => { self.advance(); args.push(n.to_string()); }
                        _ => {
                            let (l, c) = self.current_span();
                            return Err(ParseError::new(
                                format!("어트리뷰트 인자 오류: '{:?}'", self.peek()),
                                l, c,
                            ));
                        }
                    }
                    self.eat(&Token::Comma);
                }
                self.expect(&Token::RParen)?;
            }
            attrs.push(Attribute { name, args, span });
        }
        Ok(attrs)
    }
}

// ────────────────────────────────────────────────────────────
//  최상위 파싱
// ────────────────────────────────────────────────────────────
impl Parser {
    pub fn parse_program(&mut self) -> ParseResult<Program> {
        let mut items = Vec::new();
        while !self.at_eof() {
            items.push(self.parse_top_level()?);
        }
        Ok(Program { items })
    }

    fn parse_top_level(&mut self) -> ParseResult<TopLevel> {
        let span  = self.ast_span();
        let attrs = self.parse_attributes()?;

        match self.peek().clone() {

            // import "path.slvt"
            Token::Import => {
                self.advance();
                let path = match self.peek().clone() {
                    Token::StrLit(s) => { self.advance(); s }
                    _ => {
                        let (l, c) = self.current_span();
                        return Err(ParseError::new("import 뒤에 문자열 경로 필요", l, c));
                    }
                };
                Ok(TopLevel::Import { path, span })
            }

            // type Vec3 = float3
            Token::Type => {
                self.advance();
                let name   = self.expect_ident()?;
                self.expect(&Token::Eq)?;
                let target = self.parse_type()?;
                Ok(TopLevel::TypeAlias { name, target, span })
            }

            // struct VertexIn { ... }
            Token::Struct => {
                self.advance();
                let name = self.expect_ident()?;
                self.expect(&Token::LBrace)?;
                let fields = self.parse_fields()?;
                self.expect(&Token::RBrace)?;
                Ok(TopLevel::Struct { name, fields, span })
            }

            // uniform SceneUniforms { ... }
            Token::Uniform => {
                self.advance();
                let name = self.expect_ident()?;
                self.expect(&Token::LBrace)?;
                let fields = self.parse_fields()?;
                self.expect(&Token::RBrace)?;
                Ok(TopLevel::Uniform { attrs, name, fields, span })
            }

            // buffer / texture2d / sampler
            Token::Buffer | Token::Texture2D | Token::Sampler => {
                let address_space = match self.peek() {
                    Token::Buffer    => AddressSpace::Device,
                    Token::Texture2D => AddressSpace::Device,
                    Token::Sampler   => AddressSpace::Constant,
                    _ => unreachable!(),
                };
                self.advance();
                let name = self.expect_ident()?;
                self.expect(&Token::Colon)?;
                let ty   = self.parse_type()?;
                self.eat(&Token::Semicolon);
                Ok(TopLevel::Resource { attrs, name, ty, address_space, span })
            }

            // vertex fn / fragment fn / kernel fn / fn
            Token::Vertex | Token::Fragment | Token::Kernel | Token::Fn => {
                self.parse_function(span)
            }

            _ => {
                let (l, c) = self.current_span();
                Err(ParseError::new(
                    format!("최상위 선언 예상, '{:?}' 발견", self.peek()),
                    l, c,
                ))
            }
        }
    }

    fn parse_fields(&mut self) -> ParseResult<Vec<Field>> {
        let mut fields = Vec::new();
        while !self.check(&Token::RBrace) && !self.at_eof() {
            let span  = self.ast_span();
            let attrs = self.parse_attributes()?;
            let name  = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let ty    = self.parse_type()?;
            self.eat(&Token::Comma);
            fields.push(Field { attrs, name, ty, span });
        }
        Ok(fields)
    }

    fn parse_function(&mut self, span: AstSpan) -> ParseResult<TopLevel> {
        let stage = match self.peek().clone() {
            Token::Vertex   => { self.advance(); ShaderStage::Vertex   }
            Token::Fragment => { self.advance(); ShaderStage::Fragment }
            Token::Kernel   => { self.advance(); ShaderStage::Kernel   }
            Token::Fn       => ShaderStage::None,
            _ => unreachable!(),
        };
        self.eat(&Token::Fn);

        let name = self.expect_ident()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;

        let ret = if self.eat(&Token::Arrow) {
            self.parse_type()?
        } else {
            Type::Unit
        };

        self.expect(&Token::LBrace)?;
        let body = self.parse_block()?;
        self.expect(&Token::RBrace)?;

        Ok(TopLevel::Function { stage, name, params, ret, body, span })
    }

    fn parse_params(&mut self) -> ParseResult<Vec<Param>> {
        let mut params = Vec::new();
        while !self.check(&Token::RParen) && !self.at_eof() {
            let span          = self.ast_span();
            let attrs         = self.parse_attributes()?;
            let name          = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let ty            = self.parse_type()?;
            let address_space = Self::infer_address_space(&attrs);
            self.eat(&Token::Comma);
            params.push(Param { attrs, name, ty, address_space, span });
        }
        Ok(params)
    }

    fn infer_address_space(attrs: &[Attribute]) -> AddressSpace {
        for a in attrs {
            match a.name.as_str() {
                "buffer"    => return AddressSpace::Device,
                "stage_in"  => return AddressSpace::Constant,
                "texture"   => return AddressSpace::Constant,
                "sampler"   => return AddressSpace::Constant,
                "thread_id" => return AddressSpace::Thread,
                _ => {}
            }
        }
        AddressSpace::Thread
    }
}

// ────────────────────────────────────────────────────────────
//  구문 파싱
// ────────────────────────────────────────────────────────────
impl Parser {
    fn parse_block(&mut self) -> ParseResult<Vec<AstSpanned<Stmt>>> {
        let mut stmts = Vec::new();
        while !self.check(&Token::RBrace) && !self.at_eof() {
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> ParseResult<AstSpanned<Stmt>> {
        let span = self.ast_span();
        match self.peek().clone() {

            // let / let mut
            Token::Let => {
                self.advance();
                let mutable = self.eat(&Token::Mut);
                let name    = self.expect_ident()?;
                let ty      = if self.eat(&Token::Colon) { Some(self.parse_type()?) } else { None };
                self.expect(&Token::Eq)?;
                let init    = self.parse_expr()?;
                self.eat(&Token::Semicolon);
                Ok(AstSpanned { node: Stmt::Let { mutable, name, ty, init, span: span.clone() }, span })
            }

            // return
            Token::Return => {
                self.advance();
                let value = if self.check(&Token::Semicolon) || self.check(&Token::RBrace) {
                    None
                } else {
                    Some(self.parse_expr()?)
                };
                self.eat(&Token::Semicolon);
                Ok(AstSpanned { node: Stmt::Return { value, span: span.clone() }, span })
            }

            // if
            Token::If => {
                self.advance();
                let cond = self.parse_expr()?;
                self.expect(&Token::LBrace)?;
                let then_body = self.parse_block()?;
                self.expect(&Token::RBrace)?;
                let else_body = if self.eat(&Token::Else) {
                    self.expect(&Token::LBrace)?;
                    let b = self.parse_block()?;
                    self.expect(&Token::RBrace)?;
                    Some(b)
                } else { None };
                Ok(AstSpanned { node: Stmt::If { cond, then_body, else_body, span: span.clone() }, span })
            }

            // for i in 0..10 { }
            Token::For => {
                self.advance();
                let var  = self.expect_ident()?;
                self.expect(&Token::In)?;
                let from = self.parse_expr()?;
                self.expect(&Token::DotDot)?;
                let to   = self.parse_expr()?;
                self.expect(&Token::LBrace)?;
                let body = self.parse_block()?;
                self.expect(&Token::RBrace)?;
                Ok(AstSpanned { node: Stmt::For { var, from, to, body, span: span.clone() }, span })
            }

            // 대입 or 단독 표현식
            _ => {
                let expr = self.parse_expr()?;
                if self.eat(&Token::Eq) {
                    let value = self.parse_expr()?;
                    self.eat(&Token::Semicolon);
                    Ok(AstSpanned { node: Stmt::Assign { target: expr, value, span: span.clone() }, span })
                } else {
                    self.eat(&Token::Semicolon);
                    Ok(AstSpanned { node: Stmt::Expr { expr, span: span.clone() }, span })
                }
            }
        }
    }
}

// ────────────────────────────────────────────────────────────
//  표현식 파싱
//  우선순위: || → && → 비교 → +- → */ → 단항 → 후위 → 기본
// ────────────────────────────────────────────────────────────
impl Parser {
    fn parse_expr(&mut self) -> ParseResult<AstSpanned<Expr>> { self.parse_or() }

    fn parse_or(&mut self) -> ParseResult<AstSpanned<Expr>> {
        let mut lhs = self.parse_and()?;
        while self.check(&Token::Or) {
            let span = self.ast_span(); self.advance();
            let rhs  = self.parse_and()?;
            lhs = AstSpanned { node: Expr::BinOp { op: BinOp::Or, lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> ParseResult<AstSpanned<Expr>> {
        let mut lhs = self.parse_cmp()?;
        while self.check(&Token::And) {
            let span = self.ast_span(); self.advance();
            let rhs  = self.parse_cmp()?;
            lhs = AstSpanned { node: Expr::BinOp { op: BinOp::And, lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
        }
        Ok(lhs)
    }

    fn parse_cmp(&mut self) -> ParseResult<AstSpanned<Expr>> {
        let mut lhs = self.parse_add()?;
        loop {
            let span = self.ast_span();
            let op = match self.peek() {
                Token::EqEq   => BinOp::Eq, Token::BangEq => BinOp::Ne,
                Token::Lt     => BinOp::Lt, Token::Gt     => BinOp::Gt,
                Token::LtEq   => BinOp::Le, Token::GtEq   => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_add()?;
            lhs = AstSpanned { node: Expr::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> ParseResult<AstSpanned<Expr>> {
        let mut lhs = self.parse_mul()?;
        loop {
            let span = self.ast_span();
            let op = match self.peek() {
                Token::Plus  => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_mul()?;
            lhs = AstSpanned { node: Expr::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> ParseResult<AstSpanned<Expr>> {
        let mut lhs = self.parse_unary()?;
        loop {
            let span = self.ast_span();
            let op = match self.peek() {
                Token::Star    => BinOp::Mul,
                Token::Slash   => BinOp::Div,
                Token::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_unary()?;
            lhs = AstSpanned { node: Expr::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, span };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> ParseResult<AstSpanned<Expr>> {
        let span = self.ast_span();
        match self.peek().clone() {
            Token::Minus => {
                self.advance();
                let expr = self.parse_postfix()?;
                Ok(AstSpanned { node: Expr::UnOp { op: UnOp::Neg, expr: Box::new(expr) }, span })
            }
            Token::Bang  => {
                self.advance();
                let expr = self.parse_postfix()?;
                Ok(AstSpanned { node: Expr::UnOp { op: UnOp::Not, expr: Box::new(expr) }, span })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> ParseResult<AstSpanned<Expr>> {
        let mut base = self.parse_primary()?;
        loop {
            let span = self.ast_span();
            match self.peek().clone() {
                Token::Dot => {
                    self.advance();
                    let field = self.expect_ident()?;
                    base = AstSpanned { node: Expr::Field { base: Box::new(base), field }, span };
                }
                Token::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(&Token::RBracket)?;
                    base = AstSpanned { node: Expr::Index { base: Box::new(base), index: Box::new(index) }, span };
                }
                _ => break,
            }
        }
        Ok(base)
    }

    fn parse_primary(&mut self) -> ParseResult<AstSpanned<Expr>> {
        let span = self.ast_span();
        match self.peek().clone() {
            Token::IntLit(n)   => { self.advance(); Ok(AstSpanned { node: Expr::IntLit(n),   span }) }
            Token::FloatLit(f) => { self.advance(); Ok(AstSpanned { node: Expr::FloatLit(f), span }) }
            Token::BoolLit(b)  => { self.advance(); Ok(AstSpanned { node: Expr::BoolLit(b),  span }) }

            // 타입 생성자  float4(...) mat4x4(...)
            Token::Float  | Token::Float2 | Token::Float3 | Token::Float4 |
            Token::Int    | Token::Uint   | Token::Bool   |
            Token::Mat2x2 | Token::Mat2x3 | Token::Mat2x4 |
            Token::Mat3x2 | Token::Mat3x3 | Token::Mat3x4 |
            Token::Mat4x2 | Token::Mat4x3 | Token::Mat4x4 => {
                let ty = self.parse_type()?;
                self.expect(&Token::LParen)?;
                let args = self.parse_arg_list()?;
                self.expect(&Token::RParen)?;
                Ok(AstSpanned { node: Expr::Constructor { ty, args }, span })
            }

            // 식별자 / 함수 호출 / sample
            Token::Ident(name) => {
                self.advance();
                if name == "sample" {
                    self.expect(&Token::LParen)?;
                    let texture = self.parse_expr()?;
                    self.expect(&Token::Comma)?;
                    let sampler = self.parse_expr()?;
                    self.expect(&Token::Comma)?;
                    let coord   = self.parse_expr()?;
                    self.expect(&Token::RParen)?;
                    return Ok(AstSpanned {
                        node: Expr::Sample {
                            texture: Box::new(texture),
                            sampler: Box::new(sampler),
                            coord:   Box::new(coord),
                        },
                        span,
                    });
                }
                if self.check(&Token::LParen) {
                    self.advance();
                    let args = self.parse_arg_list()?;
                    self.expect(&Token::RParen)?;
                    return Ok(AstSpanned { node: Expr::Call { func: name, args }, span });
                }
                Ok(AstSpanned { node: Expr::Ident(name), span })
            }

            // `in` 은 변수 이름으로도 쓰임
            Token::In => {
                self.advance();
                Ok(AstSpanned { node: Expr::Ident("in".into()), span })
            }

            // 괄호
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }

            _ => {
                let (l, c) = self.current_span();
                Err(ParseError::new(
                    format!("표현식 예상, '{:?}' 발견", self.peek()),
                    l, c,
                ))
            }
        }
    }

    fn parse_arg_list(&mut self) -> ParseResult<Vec<AstSpanned<Expr>>> {
        let mut args = Vec::new();
        while !self.check(&Token::RParen) && !self.at_eof() {
            args.push(self.parse_expr()?);
            self.eat(&Token::Comma);
        }
        Ok(args)
    }
}