// word_check.rs
// Metal 예약어/빌트인과 충돌하는 식별자 이름 검사.
//
// MSL 컴파일러는 식별자와 Metal 내부 이름이 겹치면 조용히 잘못된 코드를 만들 수 있음.
// 이 검사기는 함수명/변수명/파라미터명/struct명이 Metal 예약어와 겹치지 않도록 사전 차단한다.

use salvation_core::compiler::ast::types::{Block, Item, Program, Stmt};
use super::CheckError;

// Metal Shading Language 예약어 + 흔히 충돌하는 빌트인 이름
const METAL_RESERVED: &[&str] = &[
    // 주소 공간 한정자 (MSL 키워드)
    "device", "constant", "threadgroup", "thread", "threadgroup_imageblock",
    // MSL 스테이지 한정자 (salvation 토크나이저가 키워드로 처리하지만 Ident로 내려오는 경우 방어)
    "vertex", "fragment", "kernel", "visible",
    // MSL 내장 타입 (salvation 토크나이저가 못 잡는 것들)
    "half", "half2", "half3", "half4",
    "ushort", "short", "char", "uchar",
    "long", "ulong", "size_t",
    "atomic_uint", "atomic_int", "atomic_float",
    // MSL 구조 키워드
    "using", "namespace", "template", "typename",
    // MSL 전용 빌트인 함수명 (사용자가 덮어쓰면 위험)
    "discard_fragment", "threadgroup_barrier", "simdgroup_barrier",
    "mem_flags", "memory_scope",
    // Metal stdlib 타입
    "metal", "texture1d", "texture3d", "texturecube", "depth2d", "depth_cube",
    // codegen이 자동 생성하는 변수명과 충돌 방지
    "in", "out",     // [[stage_in]] 파라미터명으로 자동 생성
    "__out",         // vertex 출력 구조체 변수명으로 자동 생성
];

pub fn check(program: &Program) -> Vec<CheckError> {
    let mut errors = Vec::new();
    for item in program {
        match item {
            Item::FnDecl { name, params, body, .. } => {
                check_name("함수", name, &mut errors);
                for p in params {
                    check_name("파라미터", &p.name, &mut errors);
                }
                check_block(body, &mut errors);
            }
            Item::StructDecl { name, fields } => {
                check_name("struct", name, &mut errors);
                for f in fields {
                    check_name("struct 필드", &f.name, &mut errors);
                }
            }
            Item::Import(_) => {}
        }
    }
    errors
}

fn check_name(kind: &str, name: &str, errors: &mut Vec<CheckError>) {
    if METAL_RESERVED.contains(&name) {
        errors.push(CheckError::new(format!(
            "{} 이름 '{}' 는 Metal 예약어예요. 다른 이름을 사용하세요.",
            kind, name
        )));
    }
    // 밑줄 두 개로 시작하는 이름은 Metal 컴파일러 내부 예약
    if name.starts_with("__") {
        errors.push(CheckError::new(format!(
            "{} 이름 '{}': '__' 로 시작하는 이름은 Metal 내부 예약 공간이에요.",
            kind, name
        )));
    }
}

fn check_block(block: &Block, errors: &mut Vec<CheckError>) {
    for stmt in block {
        match stmt {
            Stmt::VarDecl { name, .. } => check_name("변수", name, errors),
            Stmt::If { then_block, else_block, .. } => {
                check_block(then_block, errors);
                if let Some(eb) = else_block { check_block(eb, errors); }
            }
            Stmt::For { var, body, .. } => {
                check_name("루프 변수", var, errors);
                check_block(body, errors);
            }
            Stmt::While { body, .. } => check_block(body, errors),
            _ => {}
        }
    }
}
