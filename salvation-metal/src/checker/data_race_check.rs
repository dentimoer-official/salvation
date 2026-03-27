// data_race_check.rs
// GPU 데이터 레이스 감지.
//
// GPU 셰이더에서 데이터 레이스의 주요 원인:
//   1. threadgroup 공유 메모리에 쓴 후 배리어 없이 읽기
//   2. 동일 device 버퍼를 여러 스레드가 동기화 없이 쓰기
//   3. atomic 연산 없이 공유 카운터 수정
//
// 정적 분석이므로 휴리스틱 기반 — 완벽하지 않지만 명백한 패턴은 잡아냄.

use salvation_core::compiler::ast::types::{BinOpKind, Block, Expr, Item, Program, ShaderStage, Stmt};
use super::CheckError;

pub fn check(program: &Program) -> Vec<CheckError> {
    let mut errors = Vec::new();
    for item in program {
        // 커널 함수만 데이터 레이스 대상
        if let Item::FnDecl { stage: Some(ShaderStage::Kernel), name, body, .. } = item {
            check_kernel(name, body, &mut errors);
        }
    }
    errors
}

fn check_kernel(fn_name: &str, body: &Block, errors: &mut Vec<CheckError>) {
    // 배열 쓰기 후 배리어 없이 배열 읽기 패턴 감지
    let mut wrote_shared = false; // 배열/버퍼에 쓴 적 있나
    let mut last_write_line = 0usize;

    for (i, stmt) in body.iter().enumerate() {
        match stmt {
            // threadgroup_barrier() / simdgroup_barrier() 호출 → 리셋
            Stmt::ExprStmt(Expr::Call { name, .. })
                if name == "threadgroup_barrier" || name == "simdgroup_barrier" =>
            {
                wrote_shared = false;
            }

            // 배열 인덱스 쓰기: data[i] = ...; 형태
            Stmt::ExprStmt(Expr::BinOp {
                op: BinOpKind::Assign | BinOpKind::AddAssign | BinOpKind::SubAssign
                    | BinOpKind::MulAssign | BinOpKind::DivAssign | BinOpKind::ModAssign,
                lhs,
                ..
            }) if is_index_expr(lhs) => {
                wrote_shared = true;
                last_write_line = i;
            }

            // 배리어 없이 배열 인덱스 읽기가 쓰기 이후에 나타나면 경고
            Stmt::VarDecl { value: Some(val), .. } if wrote_shared && has_index_read(val) => {
                errors.push(CheckError::new(format!(
                    "커널 '{}': 공유 버퍼 쓰기({}번째 구문) 후 threadgroup_barrier() 없이 \
                     읽고 있어요. 데이터 레이스가 발생할 수 있어요.",
                    fn_name, last_write_line + 1
                )));
                wrote_shared = false; // 한 번만 보고
            }

            // 중첩 블록 재귀 검사
            Stmt::If { then_block, else_block, .. } => {
                check_kernel(fn_name, then_block, errors);
                if let Some(eb) = else_block {
                    check_kernel(fn_name, eb, errors);
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } => {
                check_kernel(fn_name, body, errors);
            }

            _ => {}
        }
    }

    // atomic 없는 카운터 누적 패턴 경고
    // 예: count = count + 1; 을 여러 스레드가 동시에 하면 레이스
    check_shared_counter_race(fn_name, body, errors);
}

// arr[i] 형태인지 확인
fn is_index_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::Index { .. })
}

// 표현식에 배열 인덱스 읽기가 있는지 확인
fn has_index_read(expr: &Expr) -> bool {
    match expr {
        Expr::Index { .. } => true,
        Expr::BinOp { lhs, rhs, .. } => has_index_read(lhs) || has_index_read(rhs),
        Expr::Call { args, .. } => args.iter().any(has_index_read),
        Expr::Field { object, .. } => has_index_read(object),
        Expr::UnaryOp { expr, .. } => has_index_read(expr),
        _ => false,
    }
}

// 공유 카운터를 atomic 없이 수정하는 패턴 탐지
// (단순 휴리스틱: 동일 변수에 += 1 / -= 1 이 루프 안에 있으면 경고)
fn check_shared_counter_race(fn_name: &str, body: &Block, errors: &mut Vec<CheckError>) {
    for stmt in body {
        // for/while 루프 내부의 단순 카운터 += 패턴
        if let Stmt::For { body, .. } | Stmt::While { body, .. } = stmt {
            for inner in body {
                if let Stmt::ExprStmt(Expr::BinOp {
                    op: BinOpKind::AddAssign | BinOpKind::SubAssign,
                    lhs: lhs_expr,
                    rhs,
                    ..
                }) = inner
                {
                    // 인덱스 대상(배열 원소)에 += 하는 경우만 레이스 위험
                    if is_index_expr(lhs_expr) {
                        if matches!(rhs.as_ref(), Expr::IntLit(1) | Expr::FloatLit(_)) {
                            errors.push(CheckError::new(format!(
                                "커널 '{}': 루프 내 배열 원소 누적 연산 — \
                                 여러 스레드가 동시에 실행되면 데이터 레이스가 발생해요. \
                                 `atomic_fetch_add` 사용을 고려하세요.",
                                fn_name
                            )));
                        }
                    }
                }
            }
        }
    }
}
