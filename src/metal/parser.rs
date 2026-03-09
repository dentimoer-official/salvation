use crate::metal::lexer::{Lexer, Token};

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

impl Span {
    pub fn zero() -> Self { Self { line: 0, col: 0 } }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AddressSpace {
    Device,
    Threadgroup,
    Constant,
    Private,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    F32, F16, I32, U32, Bool, Void,
    Vec2(Box<Type>), Vec3(Box<Type>), Vec4(Box<Type>),
    Mat4,
    Array { space: AddressSpace, elem: Box<Type>, mutable: bool },
    ArrayN   { space: AddressSpace, elem: Box<Type>, mutable: bool, size: u64 },
    Named(String),
    Qualified(String, String),
}

#[derive(Debug, Clone)]
pub enum Expr {
    IntLit   (i64, Span),
    FloatLit (f64, Span),
    Bool     (bool, Span),
    Ident    (String, Span),
    Index    { array: Box<Expr>, index: Box<Expr>, span: Span },
    Field    { object: Box<Expr>, field: String, span: Span },
    BinOp    { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
    UnaryOp  { op: UnaryOp, expr: Box<Expr>, span: Span },
    Call     { name: String, args: Vec<Expr>, span: Span },
}

impl Expr {
    pub fn span(&self) -> &Span {
        match self {
            Expr::IntLit(_, s)         => s,
            Expr::FloatLit(_, s)       => s,
            Expr::Bool(_, s)           => s,
            Expr::Ident(_, s)          => s,
            Expr::Index { span, .. }   => span,
            Expr::Field { span, .. }   => span,
            Expr::BinOp { span, .. }   => span,
            Expr::UnaryOp { span, .. } => span,
            Expr::Call { span, .. }    => span,
        }
    }
}

#[derive(Debug, Clone)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or,
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Neg, Not,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let    { name: String, mutable: bool, ty: Option<Type>, value: Expr, span: Span },
    Assign { target: Expr, value: Expr, span: Span },
    Return (Option<Expr>, Span),
    If     { cond: Expr, then: Vec<Stmt>, else_: Option<Vec<Stmt>>, span: Span },
    For    { var: String, from: Expr, to: Expr, body: Vec<Stmt>, span: Span },
    Expr   (Expr, Span),
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub enum Decl {
    KernelFn {
        name: String,
        params: Vec<Param>,
        body: Vec<Stmt>,
        span: Span,
    },
    Struct {
        name: String,
        fields: Vec<StructField>,
        is_pub: bool,
        span: Span,
    },
    Const {
        name: String,
        ty: Type,
        value: Expr,
        is_pub: bool,
        span: Span,
    },
    Import   {
        path: String, 
        span: Span
    }, 
}

pub struct ItemFlags {
    pub is_pub: bool,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub decls: Vec<Decl>,
}

pub struct Parser {
    tokens: Vec<(Token, std::ops::Range<usize>)>,
    pos: usize,
    src: String,
}

impl Parser {
    pub fn new(src: &str) -> Self {
        let tokens = Lexer::new(src)
            .filter_map(|r| r.ok())
            .collect();
        Self { tokens, pos: 0, src: src.to_string() }
    }

    fn offset_to_span(&self, offset: usize) -> Span {
        let mut line = 1;
        let mut col = 1;
        for (i, ch) in self.src.char_indices() {
            if i >= offset { break; }
            if ch == '\n' { line += 1; col = 1; }
            else { col += 1; }
        }
        Span { line, col }
    }

    fn current_span(&self) -> Span {
        self.tokens.get(self.pos)
            .map(|(_, r)| self.offset_to_span(r.start))
            .unwrap_or(Span::zero())
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(t, _)| t)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos).map(|(t, _)| t);
        self.pos += 1;
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        match self.peek() {
            Some(t) if t == expected => { self.advance(); Ok(()) }
            Some(t) => Err(format!("expected {:?}, got {:?}", expected, t)),
            None => Err(format!("expected {:?}, got EOF", expected)),
        }
    }

    fn expect_ident(&mut self) -> Result<String, String> {
        match self.peek().cloned() {
            Some(Token::Ident(name)) => { self.advance(); Ok(name) }
            Some(t) => Err(format!("expected identifier, got {:?}", t)),
            None => Err("expected identifier, got EOF".to_string()),
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut decls = vec![];
        while self.peek().is_some() {
            decls.push(self.parse_decl()?);
        }
        Ok(Program { decls })
    }

    fn parse_decl(&mut self) -> Result<Decl, String> {
        match self.peek() {
            Some(Token::Import) => self.parse_import(),
            Some(Token::Pub)    => self.parse_pub_decl(),
            Some(Token::Kernel) => self.parse_kernel_fn(),
            Some(Token::Struct) => self.parse_struct(false),
            Some(Token::Const)  => self.parse_const(false),
            Some(t) => Err(format!("unexpected token: {:?}", t)),
            None => Err("unexpected EOF".to_string()),
        }
    }

    fn parse_kernel_fn(&mut self) -> Result<Decl, String> {
        let span = self.current_span();
        self.expect(&Token::Kernel)?;
        self.expect(&Token::Fn)?;
        let name = self.expect_ident()?;
        self.expect(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(&Token::RParen)?;
        self.expect(&Token::LBrace)?;
        let body = self.parse_block()?;
        self.expect(&Token::RBrace)?;
        Ok(Decl::KernelFn { name, params, body, span })
    }

    fn parse_struct(&mut self, is_pub: bool) -> Result<Decl, String> {
        let span = self.current_span();
        self.expect(&Token::Struct)?;
        let name = self.expect_ident()?;
        self.expect(&Token::LBrace)?;
        let mut fields = vec![];
        while self.peek() != Some(&Token::RBrace) {
            let field_name = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let ty = self.parse_type()?;
            fields.push(StructField { name: field_name, ty });
            if self.peek() == Some(&Token::Comma) { self.advance(); }
        }
        self.expect(&Token::RBrace)?;
        Ok(Decl::Struct { name, fields, is_pub, span })
    }
    
    fn parse_import(&mut self) -> Result<Decl, String> {
        let span = self.current_span();
        self.expect(&Token::Import)?;
        let path = self.expect_ident()?;
        self.expect(&Token::Semi)?;
        Ok(Decl::Import { path, span })
    }
    
    fn parse_pub_decl(&mut self) -> Result<Decl, String> {
        self.expect(&Token::Pub)?;
        match self.peek() {
            Some(Token::Struct) => self.parse_struct(true),
            Some(Token::Const)  => self.parse_const(true),
            Some(t) => Err(format!("pub is not allowed here: {:?}", t)),
            None => Err("unexpected EOF after pub".to_string()),
        }
    }

    fn parse_const(&mut self, is_pub: bool) -> Result<Decl, String> {
        let span = self.current_span();
        self.expect(&Token::Const)?;
        let name = self.expect_ident()?;
        self.expect(&Token::Colon)?;
        let ty = self.parse_type()?;
        self.expect(&Token::Eq)?;
        let value = self.parse_expr()?;
        self.expect(&Token::Semi)?;
        Ok(Decl::Const { name, ty, value, is_pub, span })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, String> {
        let mut params = vec![];
        while self.peek() != Some(&Token::RParen) {
            let name = self.expect_ident()?;
            self.expect(&Token::Colon)?;
            let ty = self.parse_type()?;
            params.push(Param { name, ty });
            if self.peek() == Some(&Token::Comma) { self.advance(); }
        }
        Ok(params)
    }
    
    fn parse_type(&mut self) -> Result<Type, String> {
        let space = match self.peek() {
            Some(Token::Device)      => { self.advance(); Some(AddressSpace::Device) }
            Some(Token::Threadgroup) => { self.advance(); Some(AddressSpace::Threadgroup) }
            Some(Token::Private)     => { self.advance(); Some(AddressSpace::Private) }
            _ => None,
        };
        let mutable = if self.peek() == Some(&Token::Mut) {
            self.advance(); true
        } else { false };
    
        if self.peek() == Some(&Token::LBracket) {
            self.advance();
            let elem = self.parse_base_type()?;
    
            // [f32; 32]  크기 있는 경우
            if self.peek() == Some(&Token::Semi) {
                self.advance();
                let size = match self.peek().cloned() {
                    Some(Token::IntLit(n)) => { self.advance(); n as u64 }
                    _ => return Err("expected array size after ';'".to_string()),
                };
                self.expect(&Token::RBracket)?;
                let space = space.ok_or("array type needs address space")?;
                return Ok(Type::ArrayN { space, elem: Box::new(elem), mutable, size });
            }
    
            self.expect(&Token::RBracket)?;
            let space = space.ok_or("array type needs address space (device/threadgroup/private)")?;
            return Ok(Type::Array { space, elem: Box::new(elem), mutable });
        }
        self.parse_base_type()
    }

    fn parse_base_type(&mut self) -> Result<Type, String> {
        match self.peek().cloned() {
            Some(Token::F32)         => { self.advance(); Ok(Type::F32) }
            Some(Token::F16)         => { self.advance(); Ok(Type::F16) }
            Some(Token::I32)         => { self.advance(); Ok(Type::I32) }
            Some(Token::U32)         => { self.advance(); Ok(Type::U32) }
            Some(Token::Bool)        => { self.advance(); Ok(Type::Bool) }
            Some(Token::Void)        => { self.advance(); Ok(Type::Void) }
            Some(Token::Vec2)        => { self.advance(); Ok(Type::Vec2(Box::new(self.parse_vec_elem()?))) }
            Some(Token::Vec3)        => { self.advance(); Ok(Type::Vec3(Box::new(self.parse_vec_elem()?))) }
            Some(Token::Vec4)        => { self.advance(); Ok(Type::Vec4(Box::new(self.parse_vec_elem()?))) }
            Some(Token::Mat4)        => { self.advance(); Ok(Type::Mat4) }
            Some(Token::Ident(name)) => {
                self.advance();
                // math::Matrix4 같은 qualified 타입
                if self.peek() == Some(&Token::ColonColon) {
                    self.advance();
                    let item = self.expect_ident()?;
                    Ok(Type::Qualified(name, item))
                } else {
                    Ok(Type::Named(name))
                }
            }
            Some(t) => Err(format!("expected type, got {:?}", t)),
            None    => Err("expected type, got EOF".to_string()),
        }
    }

    fn parse_vec_elem(&mut self) -> Result<Type, String> {
        self.expect(&Token::Lt)?;
        let ty = self.parse_base_type()?;
        self.expect(&Token::Gt)?;
        Ok(ty)
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        let mut stmts = vec![];
        while self.peek() != Some(&Token::RBrace) && self.peek().is_some() {
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        match self.peek() {
            Some(Token::Let)    => self.parse_let(),
            Some(Token::Return) => self.parse_return(),
            Some(Token::If)     => self.parse_if(),
            Some(Token::For)    => self.parse_for(),
            _ => {
                let span = self.current_span();
                let expr = self.parse_expr()?;
                if self.peek() == Some(&Token::Eq) {
                    self.advance();
                    let value = self.parse_expr()?;
                    self.expect(&Token::Semi)?;
                    Ok(Stmt::Assign { target: expr, value, span })
                } else {
                    self.expect(&Token::Semi)?;
                    Ok(Stmt::Expr(expr, span))
                }
            }
        }
    }

    fn parse_let(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        self.expect(&Token::Let)?;
        let mutable = if self.peek() == Some(&Token::Mut) {
            self.advance(); true
        } else { false };
        let name = self.expect_ident()?;
        let ty = if self.peek() == Some(&Token::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else { None };
    
        // threadgroup 지역 배열은 초기화 없이 선언 가능
        let is_threadgroup_local = matches!(&ty,
            Some(Type::ArrayN { space: AddressSpace::Threadgroup, .. })
        );
    
        if self.peek() == Some(&Token::Semi) && is_threadgroup_local {
            self.advance();
            return Ok(Stmt::Let {
                name, mutable, ty,
                value: Expr::IntLit(0, span.clone()),
                span,
            });
        }
    
        self.expect(&Token::Eq)?;
        let value = self.parse_expr()?;
        self.expect(&Token::Semi)?;
        Ok(Stmt::Let { name, mutable, ty, value, span })
    }

    fn parse_return(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        self.expect(&Token::Return)?;
        if self.peek() == Some(&Token::Semi) {
            self.advance();
            Ok(Stmt::Return(None, span))
        } else {
            let expr = self.parse_expr()?;
            self.expect(&Token::Semi)?;
            Ok(Stmt::Return(Some(expr), span))
        }
    }

    fn parse_if(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        self.expect(&Token::If)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::LBrace)?;
        let then = self.parse_block()?;
        self.expect(&Token::RBrace)?;
        let else_ = if self.peek() == Some(&Token::Else) {
            self.advance();
            self.expect(&Token::LBrace)?;
            let b = self.parse_block()?;
            self.expect(&Token::RBrace)?;
            Some(b)
        } else { None };
        Ok(Stmt::If { cond, then, else_, span })
    }

    fn parse_for(&mut self) -> Result<Stmt, String> {
        let span = self.current_span();
        self.expect(&Token::For)?;
        let var = self.expect_ident()?;
        self.expect(&Token::In)?;
        let from = self.parse_expr()?;
        self.expect(&Token::DotDot)?;
        let to = self.parse_expr()?;
        self.expect(&Token::LBrace)?;
        let body = self.parse_block()?;
        self.expect(&Token::RBrace)?;
        Ok(Stmt::For { var, from, to, body, span })
    }

    fn parse_expr(&mut self) -> Result<Expr, String> { self.parse_or() }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_and()?;
        while self.peek() == Some(&Token::Or) {
            let span = self.current_span();
            self.advance();
            let rhs = self.parse_and()?;
            lhs = Expr::BinOp { op: BinOp::Or, lhs: Box::new(lhs), rhs: Box::new(rhs), span };
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_cmp()?;
        while self.peek() == Some(&Token::And) {
            let span = self.current_span();
            self.advance();
            let rhs = self.parse_cmp()?;
            lhs = Expr::BinOp { op: BinOp::And, lhs: Box::new(lhs), rhs: Box::new(rhs), span };
        }
        Ok(lhs)
    }

    fn parse_cmp(&mut self) -> Result<Expr, String> {
        let lhs = self.parse_add()?;
        let span = self.current_span();
        let op = match self.peek() {
            Some(Token::EqEq) => BinOp::Eq,
            Some(Token::Ne)   => BinOp::Ne,
            Some(Token::Lt)   => BinOp::Lt,
            Some(Token::Gt)   => BinOp::Gt,
            Some(Token::Le)   => BinOp::Le,
            Some(Token::Ge)   => BinOp::Ge,
            _ => return Ok(lhs),
        };
        self.advance();
        let rhs = self.parse_add()?;
        Ok(Expr::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span })
    }

    fn parse_add(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_mul()?;
        loop {
            let span = self.current_span();
            let op = match self.peek() {
                Some(Token::Plus)  => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_mul()?;
            lhs = Expr::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span };
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> Result<Expr, String> {
        let mut lhs = self.parse_unary()?;
        loop {
            let span = self.current_span();
            let op = match self.peek() {
                Some(Token::Star)    => BinOp::Mul,
                Some(Token::Slash)   => BinOp::Div,
                Some(Token::Percent) => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_unary()?;
            lhs = Expr::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        let span = self.current_span();
        match self.peek() {
            Some(Token::Minus) => {
                self.advance();
                Ok(Expr::UnaryOp { op: UnaryOp::Neg, expr: Box::new(self.parse_postfix()?), span })
            }
            Some(Token::Bang) => {
                self.advance();
                Ok(Expr::UnaryOp { op: UnaryOp::Not, expr: Box::new(self.parse_postfix()?), span })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;
        loop {
            let span = self.current_span();
            match self.peek() {
                Some(Token::LBracket) => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(&Token::RBracket)?;
                    expr = Expr::Index { array: Box::new(expr), index: Box::new(index), span };
                }
                Some(Token::Dot) => {
                    self.advance();
                    let field = self.expect_ident()?;
                    expr = Expr::Field { object: Box::new(expr), field, span };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let span = self.current_span();
        match self.peek().cloned() {
            Some(Token::IntLit(n))   => { self.advance(); Ok(Expr::IntLit(n, span)) }
            Some(Token::FloatLit(f)) => { self.advance(); Ok(Expr::FloatLit(f, span)) }
            Some(Token::True)        => { self.advance(); Ok(Expr::Bool(true, span)) }
            Some(Token::False)       => { self.advance(); Ok(Expr::Bool(false, span)) }
            Some(Token::Ident(name)) => {
                self.advance();
                // math::PI 같은 qualified 접근
                if self.peek() == Some(&Token::ColonColon) {
                    self.advance();
                    let item = self.expect_ident()?;
                    // math::foo() 함수 호출
                    if self.peek() == Some(&Token::LParen) {
                        self.advance();
                        let mut args = vec![];
                        while self.peek() != Some(&Token::RParen) {
                            args.push(self.parse_expr()?);
                            if self.peek() == Some(&Token::Comma) { self.advance(); }
                        }
                        self.expect(&Token::RParen)?;
                        // qualified 함수 호출은 name::item 형태로 합쳐서 Call로
                        Ok(Expr::Call { name: format!("{}::{}", name, item), args, span })
                    } else {
                        // math::PI 같은 상수 접근은 Ident로
                        Ok(Expr::Ident(format!("{}::{}", name, item), span))
                    }
                } else if self.peek() == Some(&Token::LParen) {
                    self.advance();
                    let mut args = vec![];
                    while self.peek() != Some(&Token::RParen) {
                        args.push(self.parse_expr()?);
                        if self.peek() == Some(&Token::Comma) { self.advance(); }
                    }
                    self.expect(&Token::RParen)?;
                    Ok(Expr::Call { name, args, span })
                } else {
                    Ok(Expr::Ident(name, span))
                }
            }
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            Some(t) => Err(format!("unexpected token in expression: {:?}", t)),
            None    => Err("unexpected EOF in expression".to_string()),
        }
        
    }
}