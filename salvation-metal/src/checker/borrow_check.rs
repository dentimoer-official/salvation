// borrow_check.rs
// 소유권 스타일 가변성 검사 (Ownership-style mutability check).
//
// GPU 언어에서의 핵심 불변식:
//   1. let 선언 변수는 재대입 불가
//   2. let mut 선언 변수만 재대입/복합 대입 가능
//   3. 함수 파라미터는 기본 불변 (Metal에서 값 복사로 전달)
//   4. for 루프 변수는 불변
//
// 실제 Rust의 borrow checker만큼 복잡하진 않지만,
// GPU 셰이더에서 흔히 발생하는 "const 버퍼에 실수로 쓰기" 류의 버그를 사전 차단한다.

use std::collections::HashMap;
use salvation_core::compiler::ast::types::{BinOpKind, Block, Expr, Item, Program, Stmt};
use super::CheckError;

pub fn check(program: &Program) -> Vec<CheckError> {
    let mut errors = Vec::new();
    for item in program {
        if let Item::FnDecl { params, body, .. } = item {
            let mut ctx = MutCtx::new();
            // 파라미터는 불변 (Metal에서 값 복사)
            for p in params {
                ctx.declare(&p.name, false);
            }
            check_block(body, &mut ctx, &mut errors);
        }
    }
    errors
}

// 변수 가변성 추적 컨텍스트
struct MutCtx {
    // 스코프 스택. 각 스코프: 변수명 → 가변 여부
    scopes: Vec<HashMap<String, bool>>,
}

impl MutCtx {
    fn new() -> Self {
        MutCtx { scopes: vec![HashMap::new()] }
    }
    fn push(&mut self) { self.scopes.push(HashMap::new()); }
    fn pop(&mut self)  { self.scopes.pop(); }

    fn declare(&mut self, name: &str, mutable: bool) {
        if let Some(s) = self.scopes.last_mut() {
            s.insert(name.to_string(), mutable);
        }
    }

    // None = 알 수 없음 (외부 / 내장 — 타입 체커가 처리)
    fn is_mutable(&self, name: &str) -> Option<bool> {
        for scope in self.scopes.iter().rev() {
            if let Some(&m) = scope.get(name) {
                return Some(m);
            }
        }
        None
    }
}

fn check_block(block: &Block, ctx: &mut MutCtx, errors: &mut Vec<CheckError>) {
    ctx.push();
    for stmt in block {
        check_stmt(stmt, ctx, errors);
    }
    ctx.pop();
}

fn check_stmt(stmt: &Stmt, ctx: &mut MutCtx, errors: &mut Vec<CheckError>) {
    match stmt {
        Stmt::VarDecl { name, mutable, value, .. } => {
            if let Some(val) = value {
                check_expr_mut(val, ctx, errors);
            }
            ctx.declare(name, *mutable);
        }

        Stmt::ExprStmt(expr) => {
            // 대입 / 복합 대입 → lhs 가변성 검사
            check_expr_mut(expr, ctx, errors);
        }

        Stmt::Return(Some(expr)) => {
            check_expr_mut(expr, ctx, errors);
        }
        Stmt::Return(None) => {}

        Stmt::If { cond, then_block, else_block } => {
            check_expr_mut(cond, ctx, errors);
            check_block(then_block, ctx, errors);
            if let Some(eb) = else_block {
                check_block(eb, ctx, errors);
            }
        }

        Stmt::For { var, from, to, body } => {
            check_expr_mut(from, ctx, errors);
            check_expr_mut(to, ctx, errors);
            ctx.push();
            ctx.declare(var, false); // 루프 변수 불변
            check_block(body, ctx, errors);
            ctx.pop();
        }

        Stmt::While { cond, body } => {
            check_expr_mut(cond, ctx, errors);
            check_block(body, ctx, errors);
        }

        Stmt::Break | Stmt::Continue => {}
    }
}

fn check_expr_mut(expr: &Expr, ctx: &MutCtx, errors: &mut Vec<CheckError>) {
    match expr {
        Expr::BinOp { op, lhs, rhs } => {
            if is_assign_op(op) {
                // lhs 가 단순 Ident 인 경우 가변성 검사
                check_assign_target(lhs, op, ctx, errors);
            }
            check_expr_mut(lhs, ctx, errors);
            check_expr_mut(rhs, ctx, errors);
        }
        Expr::Call { args, .. } => {
            for a in args { check_expr_mut(a, ctx, errors); }
        }
        Expr::Field { object, .. } => check_expr_mut(object, ctx, errors),
        Expr::Index { object, index } => {
            check_expr_mut(object, ctx, errors);
            check_expr_mut(index, ctx, errors);
        }
        Expr::UnaryOp { expr, .. } => check_expr_mut(expr, ctx, errors),
        _ => {}
    }
}

fn check_assign_target(lhs: &Expr, op: &BinOpKind, ctx: &MutCtx, errors: &mut Vec<CheckError>) {
    // lhs가 단순 변수 참조인 경우만 검사 (배열 인덱스/필드는 통과)
    if let Expr::Ident(name) = lhs {
        match ctx.is_mutable(name) {
            Some(false) => {
                let op_str = assign_op_str(op);
                errors.push(CheckError::new(format!(
                    "불변 변수 '{}' 에 '{}' 연산을 할 수 없어요. \
                     수정하려면 `let mut {}` 로 선언하세요.",
                    name, op_str, name
                )));
            }
            Some(true) | None => {}
        }
    }
}

fn is_assign_op(op: &BinOpKind) -> bool {
    matches!(
        op,
        BinOpKind::Assign
            | BinOpKind::AddAssign
            | BinOpKind::SubAssign
            | BinOpKind::MulAssign
            | BinOpKind::DivAssign
            | BinOpKind::ModAssign
    )
}

fn assign_op_str(op: &BinOpKind) -> &'static str {
    match op {
        BinOpKind::Assign    => "=",
        BinOpKind::AddAssign => "+=",
        BinOpKind::SubAssign => "-=",
        BinOpKind::MulAssign => "*=",
        BinOpKind::DivAssign => "/=",
        BinOpKind::ModAssign => "%=",
        _ => "?=",
    }
}
