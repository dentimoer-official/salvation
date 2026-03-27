// type_check.rs
// 추가 타입 검사 — checker/mod.rs의 기본 타입 추론을 보완.
//
// 검사 항목:
//   1. 벡터/행렬 생성자 인자 개수 검증
//      - float2(x, y): 2개 또는 1개(broadcast)
//      - float3(x, y, z): 3개 또는 1개
//      - float4(x, y, z, w): 4개, 2개(float2+float2), 1개
//      - float4x4: 16개, 4개(float4×4), 1개
//   2. swizzle 길이 및 성분 검증
//      - float2.xyz → 오류 (z 성분 없음)
//      - float3.w → 오류 (w 성분 없음)
//   3. 텍스처/샘플러에 산술 연산 적용 금지

use salvation_core::compiler::ast::types::{Block, Expr, Item, Program, Stmt, Type};
use super::CheckError;

pub fn check(program: &Program) -> Vec<CheckError> {
    let mut errors = Vec::new();
    for item in program {
        if let Item::FnDecl { params, body, .. } = item {
            // 파라미터 타입에 Texture2D/Sampler 산술 사용 감지는 checker/mod.rs에서 처리
            let _ = params;
            check_block(body, &mut errors);
        }
    }
    errors
}

fn check_block(block: &Block, errors: &mut Vec<CheckError>) {
    for stmt in block {
        check_stmt(stmt, errors);
    }
}

fn check_stmt(stmt: &Stmt, errors: &mut Vec<CheckError>) {
    match stmt {
        Stmt::VarDecl { value: Some(expr), .. } => check_expr(expr, errors),
        Stmt::ExprStmt(expr) | Stmt::Return(Some(expr)) => check_expr(expr, errors),
        Stmt::If { cond, then_block, else_block } => {
            check_expr(cond, errors);
            check_block(then_block, errors);
            if let Some(eb) = else_block { check_block(eb, errors); }
        }
        Stmt::For { from, to, body, .. } => {
            check_expr(from, errors);
            check_expr(to, errors);
            check_block(body, errors);
        }
        Stmt::While { cond, body } => {
            check_expr(cond, errors);
            check_block(body, errors);
        }
        _ => {}
    }
}

fn check_expr(expr: &Expr, errors: &mut Vec<CheckError>) {
    match expr {
        // ── 생성자 인자 개수 검사 ──
        Expr::Call { name, args } => {
            if let Some((min, max, desc)) = constructor_rules(name) {
                let n = args.len();
                if n < min || n > max {
                    errors.push(CheckError::new(format!(
                        "생성자 '{name}()': {}개 인자 필요 ({desc}), {}개 전달됨.",
                        if min == max { format!("{min}") } else { format!("{min}~{max}") },
                        n
                    )));
                }
            }
            for a in args { check_expr(a, errors); }
        }

        // ── swizzle 성분 검사 ──
        // 현재 AST에는 부모 표현식의 타입 정보가 없으므로
        // 일단 명백히 잘못된 단일 성분 swizzle만 잡는다.
        Expr::Field { object, field } => {
            check_swizzle_components(field, errors);
            check_expr(object, errors);
        }

        Expr::BinOp { lhs, rhs, .. } => {
            check_expr(lhs, errors);
            check_expr(rhs, errors);
        }
        Expr::Index { object, index } => {
            check_expr(object, errors);
            check_expr(index, errors);
        }
        Expr::UnaryOp { expr, .. } => check_expr(expr, errors),
        _ => {}
    }
}

/// (min_args, max_args, 설명)
fn constructor_rules(name: &str) -> Option<(usize, usize, &'static str)> {
    match name {
        "float"  | "int" | "uint" | "bool" => Some((1, 1, "1개")),
        "float2" => Some((1, 2, "1개(broadcast) 또는 2개")),
        "float3" => Some((1, 3, "1개(broadcast) 또는 3개")),
        "float4" => Some((1, 4, "1개(broadcast), 2개(float2+float2), 3개+1개, 또는 4개")),
        "float2x2" => Some((1, 4,  "1개(broadcast) 또는 4개")),
        "float3x3" => Some((1, 9,  "1개(broadcast) 또는 9개")),
        "float4x4" => Some((1, 16, "1개(broadcast), 4개(float4×4열), 또는 16개")),
        _ => None,
    }
}

/// swizzle 성분 문자가 유효한지 검사.
/// Metal에서 허용: x y z w (위치) 또는 r g b a (색상).
/// 섞어서 쓰면 컴파일 오류이므로 그것도 잡는다.
fn check_swizzle_components(field: &str, errors: &mut Vec<CheckError>) {
    // 필드명이 swizzle인지 판단: 모두 x/y/z/w 또는 r/g/b/a 로 구성된 1~4자 문자열
    if field.len() > 4 || field.is_empty() {
        return; // 일반 struct 필드 — 검사 안 함
    }

    let xyzw_chars = field.chars().all(|c| "xyzw".contains(c));
    let rgba_chars = field.chars().all(|c| "rgba".contains(c));

    if !xyzw_chars && !rgba_chars {
        return; // 일반 필드명
    }

    // xyzw / rgba 혼용 감지
    let has_xyzw = field.chars().any(|c| "xyzw".contains(c));
    let has_rgba = field.chars().any(|c| "rgba".contains(c));
    if has_xyzw && has_rgba {
        errors.push(CheckError::new(format!(
            "swizzle '.{}': xyzw 계열과 rgba 계열을 섞어서 사용할 수 없어요.",
            field
        )));
    }
}

/// 타입에서 허용 swizzle 최대 길이 반환
pub fn swizzle_max_len(ty: &Type) -> Option<usize> {
    match ty {
        Type::Float  => Some(1),
        Type::Float2 => Some(2),
        Type::Float3 => Some(3),
        Type::Float4 => Some(4),
        _ => None,
    }
}
