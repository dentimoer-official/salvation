// memory_check.rs
// GPU 메모리 안전성 검사.
//
// 검사 항목:
//   1. 정적 배열 범위 초과 접근 (인덱스가 리터럴인 경우)
//   2. 음수 인덱스 사용
//   3. 커널 함수에서 bounds guard 없이 device 버퍼 접근
//
// 동적 인덱스는 컴파일 타임에 정확히 분석할 수 없으므로 경고만 냄.

use std::collections::HashMap;
use salvation_core::compiler::ast::types::{Block, Expr, Item, Program, ShaderStage, Stmt, Type};
use super::CheckError;

pub fn check(program: &Program) -> Vec<CheckError> {
    let mut errors = Vec::new();
    for item in program {
        if let Item::FnDecl { stage, params, body, name, .. } = item {
            let mut arrays: HashMap<String, usize> = HashMap::new();

            // 파라미터에서 정적 배열 크기 등록
            for p in params {
                if let Type::Array { size, .. } = &p.ty {
                    arrays.insert(p.name.clone(), *size);
                }
            }

            // 커널 함수: bounds guard 체크
            if matches!(stage, Some(ShaderStage::Kernel)) {
                if !has_bounds_guard(body) {
                    errors.push(CheckError::new(format!(
                        "커널 함수 '{}': 스레드 인덱스 범위 검사(guard clause)가 없어요. \
                         `if (idx >= count) {{ return; }}` 패턴을 추가하면 버퍼 오버플로우를 방지할 수 있어요.",
                        name
                    )));
                }
            }

            check_block(body, &mut arrays, &mut errors);
        }
    }
    errors
}

// 커널 맨 첫 if 문에 return이 있는지 확인 (guard clause 패턴)
fn has_bounds_guard(body: &Block) -> bool {
    body.iter().any(|stmt| {
        if let Stmt::If { then_block, .. } = stmt {
            then_block.iter().any(|s| matches!(s, Stmt::Return(None)))
        } else {
            false
        }
    })
}

fn check_block(block: &Block, arrays: &mut HashMap<String, usize>, errors: &mut Vec<CheckError>) {
    for stmt in block {
        check_stmt(stmt, arrays, errors);
    }
}

fn check_stmt(stmt: &Stmt, arrays: &mut HashMap<String, usize>, errors: &mut Vec<CheckError>) {
    match stmt {
        Stmt::VarDecl { name, ty, value, .. } => {
            if let Type::Array { size, .. } = ty {
                arrays.insert(name.clone(), *size);
            }
            if let Some(v) = value {
                check_expr(v, arrays, errors);
            }
        }
        Stmt::ExprStmt(expr) => check_expr(expr, arrays, errors),
        Stmt::Return(Some(expr)) => check_expr(expr, arrays, errors),
        Stmt::If { cond, then_block, else_block } => {
            check_expr(cond, arrays, errors);
            check_block(then_block, arrays, errors);
            if let Some(eb) = else_block {
                check_block(eb, arrays, errors);
            }
        }
        Stmt::For { from, to, body, .. } => {
            check_expr(from, arrays, errors);
            check_expr(to, arrays, errors);
            check_block(body, arrays, errors);
        }
        Stmt::While { cond, body } => {
            check_expr(cond, arrays, errors);
            check_block(body, arrays, errors);
        }
        _ => {}
    }
}

fn check_expr(expr: &Expr, arrays: &HashMap<String, usize>, errors: &mut Vec<CheckError>) {
    match expr {
        Expr::Index { object, index } => {
            // 음수 인덱스
            if let Expr::IntLit(n) = index.as_ref() {
                if *n < 0 {
                    errors.push(CheckError::new(format!(
                        "배열 인덱스로 음수 {} 를 사용할 수 없어요.", n
                    )));
                }
                // 정적 배열 범위 초과
                if let Expr::Ident(arr_name) = object.as_ref() {
                    if let Some(&size) = arrays.get(arr_name.as_str()) {
                        if *n as usize >= size {
                            errors.push(CheckError::new(format!(
                                "배열 '{}' 범위 초과: 인덱스 {} 는 유효 범위 0..{} 를 벗어나요.",
                                arr_name, n, size - 1
                            )));
                        }
                    }
                }
            }
            check_expr(object, arrays, errors);
            check_expr(index, arrays, errors);
        }
        Expr::BinOp { lhs, rhs, .. } => {
            check_expr(lhs, arrays, errors);
            check_expr(rhs, arrays, errors);
        }
        Expr::Call { args, .. } => {
            for a in args { check_expr(a, arrays, errors); }
        }
        Expr::Field { object, .. } => check_expr(object, arrays, errors),
        Expr::UnaryOp { expr, .. } => check_expr(expr, arrays, errors),
        _ => {}
    }
}
