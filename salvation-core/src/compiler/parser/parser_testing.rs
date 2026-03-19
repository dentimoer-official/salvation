use crate::compiler::lexer::tokens::Token;
use crate::compiler::lexer::Spanned;
use crate::compiler::ast::ast_testing::*;
use crate::compiler::ast::ast_testing::{
    Type, BinOpKind, UnaryOpKind
};

pub struct Parser {
    tokens: Vec<Spanned<Token>>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Spanned<Token>>) -> Self {
        Parser { tokens, pos: 0 }
    }

    // ── 내부 헬퍼 ──────────────────────────────────────

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos)
            .map(|s| &s.node)
            .unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos)
            .map(|s| s.node.clone())
            .unwrap_or(Token::Eof);
        if self.pos < self.tokens.len() { self.pos += 1; }
        tok
    }

    fn expect(&mut self, expected: &Token) -> Result<(), String> {
        let tok = self.advance();
        if &tok == expected {
            Ok(())
        } else {
            Err(format!("expected {:?}, got {:?}", expected, tok))
        }
    }

    fn eat(&mut self, tok: &Token) -> bool {
        if self.peek() == tok { self.advance(); true } else { false }
    }

    // ── 타입 파싱 ──────────────────────────────────────

    fn parse_type(&mut self) -> Result<Type, String> {
        let tok = self.advance();
        Ok(match tok {
            Token::Bool    => Type::Bool,
            Token::Int     => Type::Int,
            Token::Uint    => Type::Uint,
            Token::Float   => Type::Float,
            Token::Float2  => Type::Float2,
            Token::Float3  => Type::Float3,
            Token::Float4  => Type::Float4,
            Token::Mat2x2  => Type::Mat2x2,
            Token::Mat2x3  => Type::Mat2x3,
            Token::Mat2x4  => Type::Mat2x4,
            Token::Mat3x2  => Type::Mat3x2,
            Token::Mat3x3  => Type::Mat3x3,
            Token::Mat3x4  => Type::Mat3x4,
            Token::Mat4x2  => Type::Mat4x2,
            Token::Mat4x3  => Type::Mat4x3,
            Token::Mat4x4  => Type::Mat4x4,
            Token::Ident(s) => Type::Named(s),
            t => return Err(format!("expected type, got {:?}", t)),
        })
    }

    // ── 표현식 파싱 ────────────────────────────────────

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_binop(0)
    }

    // 연산자 우선순위 테이블
    fn precedence(tok: &Token) -> Option<(u8, BinOpKind)> {
        Some(match tok {
            Token::Or     => (1, BinOpKind::Or),
            Token::And    => (2, BinOpKind::And),
            Token::EqEq   => (3, BinOpKind::Eq),
            Token::BangEq => (3, BinOpKind::NotEq),
            Token::Lt     => (4, BinOpKind::Lt),
            Token::Gt     => (4, BinOpKind::Gt),
            Token::LtEq   => (4, BinOpKind::LtEq),
            Token::GtEq   => (4, BinOpKind::GtEq),
            Token::Plus   => (5, BinOpKind::Add),
            Token::Minus  => (5, BinOpKind::Sub),
            Token::Star   => (6, BinOpKind::Mul),
            Token::Slash  => (6, BinOpKind::Div),
            Token::Percent=> (6, BinOpKind::Mod),
            Token::Eq     => (0, BinOpKind::Assign),
            _ => return None,
        })
    }

    // Pratt parsing으로 이항 연산 처리
    fn parse_binop(&mut self, min_prec: u8) -> Result<Expr, String> {
        let mut lhs = self.parse_unary()?;

        loop {
            let Some((prec, op)) = Self::precedence(self.peek()) else { break };
            if prec < min_prec { break; }
            self.advance();
            let rhs = self.parse_binop(prec + 1)?;
            lhs = Expr::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }

        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Token::Bang => {
                self.advance();
                Ok(Expr::UnaryOp { op: UnaryOpKind::Not, expr: Box::new(self.parse_unary()?) })
            }
            Token::Minus => {
                self.advance();
                Ok(Expr::UnaryOp { op: UnaryOpKind::Neg, expr: Box::new(self.parse_unary()?) })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;

        loop {
            match self.peek() {
                // foo.bar
                Token::Dot => {
                    self.advance();
                    if let Token::Ident(field) = self.advance() {
                        expr = Expr::Field { object: Box::new(expr), field };
                    }
                }
                // arr[i]
                Token::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(&Token::RBracket)?;
                    expr = Expr::Index { object: Box::new(expr), index: Box::new(index) };
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.peek().clone() {
            Token::IntLit(n)   => { self.advance(); Ok(Expr::IntLit(n)) }
            Token::FloatLit(f) => { self.advance(); Ok(Expr::FloatLit(f)) }
            Token::BoolLit(b)  => { self.advance(); Ok(Expr::BoolLit(b)) }

            // 식별자 or 함수 호출
            Token::Ident(name) => {
                self.advance();
                if matches!(self.peek(), Token::LParen) {
                    // 함수 호출
                    self.advance(); // (
                    let mut args = Vec::new();
                    while !matches!(self.peek(), Token::RParen | Token::Eof) {
                        args.push(self.parse_expr()?);
                        self.eat(&Token::Comma);
                    }
                    self.expect(&Token::RParen)?;
                    Ok(Expr::Call { name, args })
                } else {
                    Ok(Expr::Ident(name))
                }
            }

            // 괄호 표현식  (expr)
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }

            t => Err(format!("unexpected token in expression: {:?}", t)),
        }
    }

    // ── 구문 파싱 ──────────────────────────────────────

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        match self.peek().clone() {
            // let [mut] x: Type = expr;
            Token::Let => {
                self.advance();
                let mutable = self.eat(&Token::Mut);
                let name = if let Token::Ident(s) = self.advance() { s }
                           else { return Err("expected variable name".into()) };
                self.expect(&Token::Colon)?;
                let ty = self.parse_type()?;
                let value = if self.eat(&Token::Eq) {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.expect(&Token::Semicolon)?;
                Ok(Stmt::VarDecl { name, mutable, ty, value })
            }

            // return expr;
            Token::Return => {
                self.advance();
                let expr = if !matches!(self.peek(), Token::Semicolon) {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.expect(&Token::Semicolon)?;
                Ok(Stmt::Return(expr))
            }

            // if cond { } else { }
            Token::If => {
                self.advance();
                let cond = self.parse_expr()?;
                let then_block = self.parse_block()?;
                let else_block = if self.eat(&Token::Else) {
                    Some(self.parse_block()?)
                } else {
                    None
                };
                Ok(Stmt::If { cond, then_block, else_block })
            }

            // for i in 0..n { }
            Token::For => {
                self.advance();
                let var = if let Token::Ident(s) = self.advance() { s }
                          else { return Err("expected loop variable".into()) };
                self.expect(&Token::In)?;
                let from = self.parse_expr()?;
                self.expect(&Token::DotDot)?;
                let to = self.parse_expr()?;
                let body = self.parse_block()?;
                Ok(Stmt::For { var, from, to, body })
            }

            // 표현식 구문
            _ => {
                let expr = self.parse_expr()?;
                self.expect(&Token::Semicolon)?;
                Ok(Stmt::ExprStmt(expr))
            }
        }
    }

    fn parse_block(&mut self) -> Result<Block, String> {
        self.expect(&Token::LBrace)?;
        let mut stmts = Vec::new();
        while !matches!(self.peek(), Token::RBrace | Token::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(&Token::RBrace)?;
        Ok(stmts)
    }

    // ── 최상위 선언 파싱 ───────────────────────────────

    fn parse_params(&mut self) -> Result<Vec<Param>, String> {
        self.expect(&Token::LParen)?;
        let mut params = Vec::new();
        while !matches!(self.peek(), Token::RParen | Token::Eof) {
            let name = if let Token::Ident(s) = self.advance() { s }
                       else { return Err("expected param name".into()) };
            self.expect(&Token::Colon)?;
            let ty = self.parse_type()?;
            params.push(Param { name, ty });
            self.eat(&Token::Comma);
        }
        self.expect(&Token::RParen)?;
        Ok(params)
    }

    fn parse_item(&mut self) -> Result<Item, String> {
        match self.peek().clone() {
            // @vertex / @fragment / @kernel
            Token::At => {
                self.advance();
                let stage = match self.peek() {
                    Token::Vertex   => { self.advance(); Some(ShaderStage::Vertex) }
                    Token::Fragment => { self.advance(); Some(ShaderStage::Fragment) }
                    Token::Kernel   => { self.advance(); Some(ShaderStage::Kernel) }
                    _ => None,
                };
                self.parse_fn(stage)
            }

            Token::Fn     => self.parse_fn(None),
            Token::Struct => self.parse_struct(),
            Token::Import => {
                self.advance();
                if let Token::StrLit(path) = self.advance() {
                    Ok(Item::Import(path))
                } else {
                    Err("expected string path after import".into())
                }
            }

            t => Err(format!("unexpected token at top level: {:?}", t)),
        }
    }

    fn parse_fn(&mut self, stage: Option<ShaderStage>) -> Result<Item, String> {
        self.expect(&Token::Fn)?;
        let name = if let Token::Ident(s) = self.advance() { s }
                   else { return Err("expected function name".into()) };
        let params = self.parse_params()?;
        let ret_ty = if self.eat(&Token::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(Item::FnDecl { stage, name, params, ret_ty, body })
    }

    fn parse_struct(&mut self) -> Result<Item, String> {
        self.expect(&Token::Struct)?;
        let name = if let Token::Ident(s) = self.advance() { s }
                   else { return Err("expected struct name".into()) };
        self.expect(&Token::LBrace)?;
        let mut fields = Vec::new();
        while !matches!(self.peek(), Token::RBrace | Token::Eof) {
            let fname = if let Token::Ident(s) = self.advance() { s }
                        else { return Err("expected field name".into()) };
            self.expect(&Token::Colon)?;
            let ty = self.parse_type()?;
            fields.push(Param { name: fname, ty });
            self.eat(&Token::Comma);
        }
        self.expect(&Token::RBrace)?;
        Ok(Item::StructDecl { name, fields })
    }

    // ── 진입점 ─────────────────────────────────────────

    pub fn parse(&mut self) -> Result<Program, String> {
        let mut items = Vec::new();
        while !matches!(self.peek(), Token::Eof) {
            items.push(self.parse_item()?);
        }
        Ok(items)
    }
}
