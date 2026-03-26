// backend_resolver.rs
// 컴파일 타임 호출 그래프 분석으로 @backend 상속을 처리한다.
//
// 규칙:
//   1. @backend(X) 명시된 fn → Backend::X 확정
//   2. @backend 없는 fn → 자신을 호출하는 fn의 백엔드를 상속
//   3. 상속해도 백엔드 미확정 → 컴파일 에러
//   4. 동일 fn이 서로 다른 백엔드에서 호출 → 컴파일 에러 (백엔드 충돌)
//   5. fn main() 존재 → 실행 모드 / 없으면 라이브러리 모드
//   6. fn main()에 @backend 없음 → 컴파일 에러 (파서에서 이미 잡지만 2중 방어)
//   7. main이 없는데 salvation run 시도 → 에러 (호출부인 main.rs에서 판단)

use std::collections::{HashMap, HashSet, VecDeque};
use crate::compiler::ast::types::{Backend, Block, Expr, Item, Program, Stmt};

// ── 에러 ──────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct BackendError {
    pub message: String,
}

impl BackendError {
    fn new(msg: impl Into<String>) -> Self {
        BackendError { message: msg.into() }
    }
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[backend] {}", self.message)
    }
}

// ── 리졸브 결과 ───────────────────────────────────────────────
pub struct ResolveResult {
    pub program: Program,
    /// true  → fn main() 존재, salvation run 가능
    /// false → 라이브러리 모드, salvation run 불가
    pub has_main: bool,
}

// ── 리졸버 ────────────────────────────────────────────────────
pub struct BackendResolver;

impl BackendResolver {
    pub fn new() -> Self {
        BackendResolver
    }

    pub fn resolve(&self, program: Program) -> Result<ResolveResult, Vec<BackendError>> {
        let mut errors: Vec<BackendError> = Vec::new();

        // main 존재 여부 확인
        let has_main = program.iter().any(|item| {
            matches!(item, Item::FnDecl { is_main: true, .. })
        });

        // 1단계: 함수별 초기 백엔드 수집 + 호출 그래프 구성
        let mut fn_backends: HashMap<String, Option<Backend>> = HashMap::new();

        for item in &program {
            if let Item::FnDecl { name, backend, body, .. } = item {
                fn_backends.insert(name.clone(), backend.clone());
                // callee 등록 (없는 키도 미리 or_default)
                for callee in collect_calls(body) {
                    fn_backends.entry(callee).or_insert(None);
                }
            }
        }

        // callees_of[caller] = Vec<callee>
        let mut callees_of: HashMap<String, Vec<String>> = HashMap::new();
        for item in &program {
            if let Item::FnDecl { name, body, .. } = item {
                callees_of.insert(name.clone(), collect_calls(body));
            }
        }

        // 2단계: BFS로 백엔드 전파
        let mut resolved: HashMap<String, Backend> = HashMap::new();
        let mut queue: VecDeque<String> = VecDeque::new();

        for (name, backend) in &fn_backends {
            if let Some(b) = backend {
                resolved.insert(name.clone(), b.clone());
                queue.push_back(name.clone());
            }
        }

        let mut visited: HashSet<String> = HashSet::new();
        while let Some(current) = queue.pop_front() {
            if visited.contains(&current) { continue; }
            visited.insert(current.clone());

            let current_backend = match resolved.get(&current) {
                Some(b) => b.clone(),
                None => continue,
            };

            for callee in callees_of.get(&current).cloned().unwrap_or_default() {
                if !fn_backends.contains_key(&callee) {
                    continue; // 외부/빌트인 함수 — 무시
                }
                match resolved.get(&callee) {
                    None => {
                        resolved.insert(callee.clone(), current_backend.clone());
                        queue.push_back(callee.clone());
                    }
                    Some(existing) if *existing != current_backend => {
                        errors.push(BackendError::new(format!(
                            "함수 '{}': @backend({}) 와 @backend({}) 가 충돌합니다. \
                             서로 다른 백엔드에서 동일 함수를 호출하고 있습니다.",
                            callee,
                            existing.as_str(),
                            current_backend.as_str(),
                        )));
                    }
                    _ => {}
                }
            }
        }

        // 3단계: 백엔드 미확정 fn 에러
        // main은 파서에서 이미 @backend 강제했지만, 일반 fn 중 고립된 것도 체크
        for item in &program {
            if let Item::FnDecl { name, is_main, .. } = item {
                if !resolved.contains_key(name.as_str()) {
                    if *is_main {
                        // 파서가 잡았어야 하지만 2중 방어
                        errors.push(BackendError::new(
                            "fn main()에는 @backend(metal/cuda/rocm/vulkan) 어트리뷰트가 필요합니다."
                        ));
                    } else {
                        errors.push(BackendError::new(format!(
                            "함수 '{}': 백엔드가 지정되지 않았습니다. \
                             @backend(metal/cuda/rocm/vulkan) 어트리뷰트를 추가하거나, \
                             백엔드가 지정된 함수에서 호출되어야 합니다.",
                            name
                        )));
                    }
                }
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        // 4단계: Program 재구성 — backend 필드를 확정값으로 채움
        let new_program = program
            .into_iter()
            .map(|item| match item {
                Item::FnDecl { pub_export, is_main, backend: _, stage, name, params, ret_ty, body } => {
                    let confirmed = resolved.get(&name).cloned();
                    Item::FnDecl { pub_export, is_main, backend: confirmed, stage, name, params, ret_ty, body }
                }
                other => other,
            })
            .collect();

        Ok(ResolveResult { program: new_program, has_main })
    }
}

// ── 헬퍼: 블록 안에서 호출되는 함수 이름 수집 ────────────────
fn collect_calls(block: &Block) -> Vec<String> {
    let mut calls = Vec::new();
    for stmt in block {
        collect_calls_stmt(stmt, &mut calls);
    }
    calls
}

fn collect_calls_stmt(stmt: &Stmt, out: &mut Vec<String>) {
    match stmt {
        Stmt::VarDecl { value: Some(expr), .. } => collect_calls_expr(expr, out),
        Stmt::Return(Some(expr))                => collect_calls_expr(expr, out),
        Stmt::If { cond, then_block, else_block } => {
            collect_calls_expr(cond, out);
            for s in then_block { collect_calls_stmt(s, out); }
            if let Some(eb) = else_block {
                for s in eb { collect_calls_stmt(s, out); }
            }
        }
        Stmt::For { from, to, body, .. } => {
            collect_calls_expr(from, out);
            collect_calls_expr(to, out);
            for s in body { collect_calls_stmt(s, out); }
        }
        Stmt::While { cond, body } => {
            collect_calls_expr(cond, out);
            for s in body { collect_calls_stmt(s, out); }
        }
        Stmt::ExprStmt(expr) => collect_calls_expr(expr, out),
        _ => {}
    }
}

fn collect_calls_expr(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Call { name, args } => {
            out.push(name.clone());
            for a in args { collect_calls_expr(a, out); }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_calls_expr(lhs, out);
            collect_calls_expr(rhs, out);
        }
        Expr::UnaryOp { expr, .. } => collect_calls_expr(expr, out),
        Expr::Field { object, .. }  => collect_calls_expr(object, out),
        Expr::Index { object, index } => {
            collect_calls_expr(object, out);
            collect_calls_expr(index, out);
        }
        _ => {}
    }
}
