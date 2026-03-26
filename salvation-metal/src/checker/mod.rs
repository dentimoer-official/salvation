// 여기서 parser에서 받은 코드가 생성 되기 전에 안전한지 볼꺼임
// 코드가 변환과 생성 직전에 문법적, 알고리즘 문제가 있나 없나 확인하는 검수 역할
// parser에서 안 하는 논리적인 문제들 다 얘가 처리함. 씹검수관

pub mod borrow_check;
pub mod data_race_check;
pub mod memory_check;
pub mod type_check;
pub mod word_check;

// checker.rs
// AST를 받아서 의미 검사 (타입 체크, 미선언 변수, 반환 타입 등)
// codegen 이전에 한 번만 돌리면 모든 백엔드가 검증된 AST를 받음

use std::collections::HashMap;
use salvation_core::compiler::ast::types::{
    Block, BinOpKind, Expr, Item, Program, Stmt, Type, UnaryOpKind,
};

// ── 에러 ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CheckError {
    pub message: String,
}

impl CheckError {
    fn new(msg: impl Into<String>) -> Self {
        CheckError { message: msg.into() }
    }
}

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[checker] {}", self.message)
    }
}

// ── 스코프 ─────────────────────────────────────────────────
// 변수 이름 → 타입 매핑
// 함수 호출마다 새 스코프, 블록마다 새 스코프

#[derive(Debug, Clone)]
struct Scope {
    vars: HashMap<String, Type>,
}

impl Scope {
    fn new() -> Self {
        Scope { vars: HashMap::new() }
    }

    fn insert(&mut self, name: String, ty: Type) {
        self.vars.insert(name, ty);
    }

    fn get(&self, name: &str) -> Option<&Type> {
        self.vars.get(name)
    }
}

// ── Checker ────────────────────────────────────────────────

pub struct Checker {
    // 스코프 스택 (마지막이 현재 스코프)
    scopes: Vec<Scope>,
    // 선언된 함수  이름 → (파라미터 타입들, 반환 타입)
    functions: HashMap<String, (Vec<Type>, Option<Type>)>,
    // 선언된 struct  이름 → 필드 목록
    structs: HashMap<String, Vec<(String, Type)>>,
    // 현재 함수의 반환 타입 (return 검사용)
    current_ret_ty: Option<Type>,
    // 수집된 에러들
    errors: Vec<CheckError>,
}

impl Checker {
    pub fn new() -> Self {
        Checker {
            scopes: vec![Scope::new()], // 글로벌 스코프
            functions: HashMap::new(),
            structs: HashMap::new(),
            current_ret_ty: None,
            errors: Vec::new(),
        }
    }

    // ── 스코프 관리 ──────────────────────────────────────

    fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare_var(&mut self, name: String, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn lookup_var(&self, name: &str) -> Option<&Type> {
        // 현재 스코프부터 바깥으로 탐색
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }

    // ── 에러 수집 ────────────────────────────────────────

    fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(CheckError::new(msg));
    }

    // ── 타입 호환성 ──────────────────────────────────────

    fn types_match(a: &Type, b: &Type) -> bool {
        match (a, b) {
            (Type::Named(x), Type::Named(y)) => x == y,
            (a, b) => std::mem::discriminant(a) == std::mem::discriminant(b),
        }
    }

    // 숫자 타입인지
    fn is_numeric(ty: &Type) -> bool {
        matches!(ty,
            Type::Int | Type::Uint | Type::Float |
            Type::Float2 | Type::Float3 | Type::Float4 |
            Type::Mat2x2 | Type::Mat2x3 | Type::Mat2x4 |
            Type::Mat3x2 | Type::Mat3x3 | Type::Mat3x4 |
            Type::Mat4x2 | Type::Mat4x3 | Type::Mat4x4
        )
    }

    // 비교 연산 결과는 bool
    fn is_comparison(op: &BinOpKind) -> bool {
        matches!(op,
            BinOpKind::Eq | BinOpKind::NotEq |
            BinOpKind::Lt | BinOpKind::Gt |
            BinOpKind::LtEq | BinOpKind::GtEq
        )
    }

    // 논리 연산자
    fn is_logical(op: &BinOpKind) -> bool {
        matches!(op, BinOpKind::And | BinOpKind::Or)
    }

    // ── 표현식 타입 추론 ─────────────────────────────────

    fn check_expr(&mut self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::IntLit(_)   => Some(Type::Int),
            Expr::FloatLit(_) => Some(Type::Float),
            Expr::BoolLit(_)  => Some(Type::Bool),

            Expr::Ident(name) => {
                match self.lookup_var(name) {
                    Some(ty) => Some(ty.clone()),
                    None => {
                        self.error(format!("'{}' 는 선언되지 않은 변수예요", name));
                        None
                    }
                }
            }

            Expr::BinOp { op, lhs, rhs } => {
                let lhs_ty = self.check_expr(lhs);
                let rhs_ty = self.check_expr(rhs);

                match (lhs_ty, rhs_ty) {
                    (Some(l), Some(r)) => {
                        if Self::is_logical(op) {
                            // && || 는 양쪽 bool 이어야 함
                            if !matches!(l, Type::Bool) {
                                self.error(format!("논리 연산자 왼쪽이 bool 이어야 해요, got {:?}", l));
                            }
                            if !matches!(r, Type::Bool) {
                                self.error(format!("논리 연산자 오른쪽이 bool 이어야 해요, got {:?}", r));
                            }
                            Some(Type::Bool)
                        } else if Self::is_comparison(op) {
                            // 비교 연산 결과는 bool
                            if !Self::types_match(&l, &r) {
                                self.error(format!(
                                    "비교 연산 타입 불일치: {:?} vs {:?}", l, r
                                ));
                            }
                            Some(Type::Bool)
                        } else if matches!(op,
                            BinOpKind::Assign |
                            BinOpKind::AddAssign | BinOpKind::SubAssign |
                            BinOpKind::MulAssign | BinOpKind::DivAssign |
                            BinOpKind::ModAssign
                        ) {
                            // 대입 / 복합 대입은 양쪽 타입 같아야 함
                            if !Self::types_match(&l, &r) {
                                self.error(format!(
                                    "대입 타입 불일치: {:?} op= {:?}", l, r
                                ));
                            }
                            Some(l)
                        } else {
                            // 산술 연산은 숫자 타입이어야 함
                            if !Self::is_numeric(&l) {
                                self.error(format!("산술 연산에 숫자 타입이 필요해요, got {:?}", l));
                            }
                            if !Self::types_match(&l, &r) {
                                self.error(format!(
                                    "산술 연산 타입 불일치: {:?} vs {:?}", l, r
                                ));
                            }
                            Some(l)
                        }
                    }
                    _ => None,
                }
            }

            Expr::UnaryOp { op, expr } => {
                let ty = self.check_expr(expr)?;
                match op {
                    UnaryOpKind::Neg => {
                        if !Self::is_numeric(&ty) {
                            self.error(format!("'-' 연산자는 숫자 타입에만 써요, got {:?}", ty));
                        }
                        Some(ty)
                    }
                    UnaryOpKind::Not => {
                        if !matches!(ty, Type::Bool) {
                            self.error(format!("'!' 연산자는 bool 타입에만 써요, got {:?}", ty));
                        }
                        Some(Type::Bool)
                    }
                }
            }

            Expr::Call { name, args } => {
                // 내장 함수 처리
                if let Some(ret) = self.check_builtin(name, args) {
                    return Some(ret);
                }

                // 유저 정의 함수
                match self.functions.get(name).cloned() {
                    None => {
                        self.error(format!("'{}' 는 선언되지 않은 함수예요", name));
                        None
                    }
                    Some((param_tys, ret_ty)) => {
                        if args.len() != param_tys.len() {
                            self.error(format!(
                                "'{}' 인자 개수 불일치: 기대 {}, 실제 {}",
                                name, param_tys.len(), args.len()
                            ));
                        }
                        for (i, (arg, expected)) in args.iter().zip(param_tys.iter()).enumerate() {
                            if let Some(got) = self.check_expr(arg) {
                                if !Self::types_match(&got, expected) {
                                    self.error(format!(
                                        "'{}' {}번째 인자 타입 불일치: 기대 {:?}, 실제 {:?}",
                                        name, i + 1, expected, got
                                    ));
                                }
                            }
                        }
                        ret_ty
                    }
                }
            }

            Expr::Field { object, field } => {
                let obj_ty = self.check_expr(object)?;
                match &obj_ty {
                    Type::Named(struct_name) => {
                        let struct_name = struct_name.clone();
                        match self.structs.get(&struct_name).cloned() {
                            None => {
                                self.error(format!("'{}' 는 알 수 없는 타입이에요", struct_name));
                                None
                            }
                            Some(fields) => {
                                match fields.iter().find(|(n, _)| n == field) {
                                    Some((_, ty)) => Some(ty.clone()),
                                    None => {
                                        self.error(format!(
                                            "'{}' 에 '{}' 필드가 없어요",
                                            struct_name, field
                                        ));
                                        None
                                    }
                                }
                            }
                        }
                    }
                    // float2/3/4 swizzle은 float 계열로 반환
                    Type::Float2 | Type::Float3 | Type::Float4 => {
                        Some(Type::Float) // 간단히 float 반환 (swizzle 길이 검사는 생략)
                    }
                    _ => {
                        self.error(format!("'{:?}' 타입에는 필드 접근이 안 돼요", obj_ty));
                        None
                    }
                }
            }

            Expr::Index { object, index } => {
                self.check_expr(object);
                if let Some(idx_ty) = self.check_expr(index) {
                    if !matches!(idx_ty, Type::Int | Type::Uint) {
                        self.error(format!("인덱스는 int/uint 이어야 해요, got {:?}", idx_ty));
                    }
                }
                Some(Type::Float) // 배열 원소 타입 추론은 단순화
            }
        }
    }

    // Metal 내장 함수 허용 목록
    fn check_builtin(&mut self, name: &str, args: &[Expr]) -> Option<Type> {
        match name {
            "normalize" | "reflect" | "refract" => {
                for a in args { self.check_expr(a); }
                Some(Type::Float3)
            }
            "dot" | "length" | "distance" => {
                for a in args { self.check_expr(a); }
                Some(Type::Float)
            }
            "cross" => {
                for a in args { self.check_expr(a); }
                Some(Type::Float3)
            }
            "abs" | "floor" | "ceil" | "fract" | "sqrt" | "saturate" | "clamp" | "mix" | "step" => {
                for a in args { self.check_expr(a); }
                Some(Type::Float)
            }
            "float2" => { for a in args { self.check_expr(a); } Some(Type::Float2) }
            "float3" => { for a in args { self.check_expr(a); } Some(Type::Float3) }
            "float4" => { for a in args { self.check_expr(a); } Some(Type::Float4) }
            _ => None,
        }
    }

    // ── 구문 검사 ────────────────────────────────────────

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { name, ty, value, .. } => {
                if let Some(val_expr) = value {
                    if let Some(val_ty) = self.check_expr(val_expr) {
                        if !Self::types_match(ty, &val_ty) {
                            self.error(format!(
                                "'{}' 선언 타입 불일치: 선언은 {:?}, 값은 {:?}",
                                name, ty, val_ty
                            ));
                        }
                    }
                }
                self.declare_var(name.clone(), ty.clone());
            }

            Stmt::Return(expr) => {
                let got = expr.as_ref().and_then(|e| self.check_expr(e));
                match &self.current_ret_ty.clone() {
                    None => {
                        // void 함수인데 값을 반환하면 에러
                        if got.is_some() {
                            self.error("void 함수에서 값을 반환할 수 없어요");
                        }
                    }
                    Some(expected) => {
                        match got {
                            None => self.error(format!(
                                "반환값이 필요해요: {:?}", expected
                            )),
                            Some(ref got_ty) => {
                                if !Self::types_match(expected, got_ty) {
                                    self.error(format!(
                                        "반환 타입 불일치: 기대 {:?}, 실제 {:?}",
                                        expected, got_ty
                                    ));
                                }
                            }
                        }
                    }
                }
            }

            Stmt::If { cond, then_block, else_block } => {
                if let Some(cond_ty) = self.check_expr(cond) {
                    if !matches!(cond_ty, Type::Bool) {
                        self.error(format!("if 조건은 bool 이어야 해요, got {:?}", cond_ty));
                    }
                }
                self.push_scope();
                self.check_block(then_block);
                self.pop_scope();
                if let Some(else_b) = else_block {
                    self.push_scope();
                    self.check_block(else_b);
                    self.pop_scope();
                }
            }

            Stmt::For { var, from, to, body } => {
                if let Some(from_ty) = self.check_expr(from) {
                    if !matches!(from_ty, Type::Int | Type::Uint) {
                        self.error(format!("for 시작값은 int/uint 이어야 해요, got {:?}", from_ty));
                    }
                }
                if let Some(to_ty) = self.check_expr(to) {
                    if !matches!(to_ty, Type::Int | Type::Uint) {
                        self.error(format!("for 끝값은 int/uint 이어야 해요, got {:?}", to_ty));
                    }
                }
                self.push_scope();
                self.declare_var(var.clone(), Type::Int); // 루프 변수는 int
                self.check_block(body);
                self.pop_scope();
            }

            Stmt::While { cond, body } => {
                if let Some(cond_ty) = self.check_expr(cond) {
                    if !matches!(cond_ty, Type::Bool) {
                        self.error(format!("while 조건은 bool 이어야 해요, got {:?}", cond_ty));
                    }
                }
                self.push_scope();
                self.check_block(body);
                self.pop_scope();
            }

            // break / continue — 루프 밖에서의 오남용은 codegen 단계에서 잡음
            Stmt::Break | Stmt::Continue => {}

            Stmt::ExprStmt(expr) => {
                self.check_expr(expr);
            }
        }
    }

    fn check_block(&mut self, block: &Block) {
        for stmt in block {
            self.check_stmt(stmt);
        }
    }

    // ── 최상위 선언 검사 ─────────────────────────────────

    fn collect_declarations(&mut self, program: &Program) {
        // 1패스: 모든 함수/struct 이름을 먼저 등록
        // (순서 관계없이 서로 호출 가능하게)
        for item in program {
            match item {
                Item::FnDecl { name, params, ret_ty, .. } => {
                    let param_tys: Vec<Type> = params.iter().map(|p| p.ty.clone()).collect();
                    self.functions.insert(name.clone(), (param_tys, ret_ty.clone()));
                }
                Item::StructDecl { name, fields } => {
                    let field_list: Vec<(String, Type)> = fields
                        .iter()
                        .map(|f| (f.name.clone(), f.ty.clone()))
                        .collect();
                    self.structs.insert(name.clone(), field_list);
                }
                Item::Import(_) => {}
            }
        }
    }

    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Import(_) => {} // lexer/loader가 처리

            Item::StructDecl { fields, .. } => {
                // 필드 타입이 알려진 타입인지 검사
                for f in fields {
                    if let Type::Named(ref n) = f.ty {
                        if !self.structs.contains_key(n.as_str()) {
                            self.error(format!("struct 필드 타입 '{}' 을 찾을 수 없어요", n));
                        }
                    }
                }
            }

            Item::FnDecl { name, params, ret_ty, body, .. } => {
                self.push_scope();

                // 파라미터를 스코프에 등록
                for p in params {
                    self.declare_var(p.name.clone(), p.ty.clone());
                }

                // 반환 타입 설정
                self.current_ret_ty = ret_ty.clone();

                // 바디 검사
                self.check_block(body);

                self.current_ret_ty = None;
                self.pop_scope();
            }
        }
    }

    // ── 진입점 ───────────────────────────────────────────

    pub fn check(mut self, program: &Program) -> Result<(), Vec<CheckError>> {
        self.collect_declarations(program);
        for item in program {
            self.check_item(item);
        }
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors)
        }
    }
}