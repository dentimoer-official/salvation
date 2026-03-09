use std::collections::HashMap;
use crate::metal::parser::{Program, Decl, Stmt, Expr, Type, AddressSpace, BinOp, Span};
use crate::metal::module::ExportKind;

#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}:{} — {}", self.span.line, self.span.col, self.message)
    }
}

#[derive(Debug, Clone)]
struct VarInfo {
    ty: Type,
    mutable: bool,
}

struct Env {
    scopes: Vec<HashMap<String, VarInfo>>,
}

impl Env {
    fn new() -> Self {
        Self { scopes: vec![HashMap::new()] }
    }
    fn push(&mut self) { self.scopes.push(HashMap::new()); }
    fn pop(&mut self)  { self.scopes.pop(); }
    fn define(&mut self, name: &str, ty: Type, mutable: bool) {
        self.scopes.last_mut().unwrap()
            .insert(name.to_string(), VarInfo { ty, mutable });
    }
    fn lookup(&self, name: &str) -> Option<&VarInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) { return Some(v); }
        }
        None
    }
}

pub struct TypeChecker {
    errors: Vec<TypeError>,
    threadgroup_written: bool,
    threadgroup_barrier_seen: bool,
    structs: HashMap<String, Vec<(String, Type)>>,
    device_reads: Vec<(String, Span)>,
    out_of_bounds_risk: Vec<String>,
    modules: HashMap<String, HashMap<String, ExportKind>>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            errors: vec![],
            threadgroup_written: false,
            threadgroup_barrier_seen: false,
            structs: HashMap::new(),
            device_reads: vec![],
            out_of_bounds_risk: vec![],
            modules: HashMap::new(),
        }
    }

    pub fn with_modules(mut self, modules: HashMap<String, HashMap<String, ExportKind>>) -> Self {
        self.modules = modules;
        self
    }

    pub fn check(mut self, program: &Program) -> Result<(), Vec<TypeError>> {
        // 로컬 struct 등록
        for decl in &program.decls {
            if let Decl::Struct { name, fields, .. } = decl {
                let fs = fields.iter().map(|f| (f.name.clone(), f.ty.clone())).collect();
                self.structs.insert(name.clone(), fs);
            }
        }
        // 모듈에서 import된 struct도 등록
        for (mod_name, exports) in &self.modules {
            for (item_name, kind) in exports {
                if let ExportKind::Struct(fields) = kind {
                    let qualified = format!("{}::{}", mod_name, item_name);
                    let fs = fields.iter().map(|f| (f.name.clone(), f.ty.clone())).collect();
                    self.structs.insert(qualified, fs);
                }
            }
        }
        for decl in &program.decls {
            self.check_decl(decl);
        }
        if self.errors.is_empty() { Ok(()) } else { Err(self.errors) }
    }

    fn error(&mut self, msg: impl Into<String>, span: &Span) {
        self.errors.push(TypeError { message: msg.into(), span: span.clone() });
    }

    fn check_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::KernelFn { name, params, body, .. } => {
                let mut env = Env::new();
                for param in params {
                    if let Type::Array { .. } = &param.ty {
                        // OK
                    } else if let Type::ArrayN { .. } = &param.ty {
                        // OK
                    } else if matches!(&param.ty,
                        Type::F32 | Type::F16 | Type::I32 | Type::U32 | Type::Bool
                    ) || matches!(&param.ty, Type::Named(_))
                      || matches!(&param.ty, Type::Qualified(_, _)) {
                        // OK
                    } else {
                        self.error(
                            format!("kernel '{}': param '{}' has invalid type", name, param.name),
                            &Span::zero(),
                        );
                    }
                    let mutable = match &param.ty {
                        Type::Array  { mutable, .. } => *mutable,
                        Type::ArrayN { mutable, .. } => *mutable,
                        _ => false,
                    };
                    env.define(&param.name, param.ty.clone(), mutable);
                }
                env.define("thread", Type::Vec3(Box::new(Type::U32)), false);
                env.define("threadgroup_id", Type::Vec3(Box::new(Type::U32)), false);
                env.define("threads_per_threadgroup", Type::Vec3(Box::new(Type::U32)), false);
                self.check_block(body, &mut env);
            }
            Decl::Struct { .. } => {}
            Decl::Import { .. } => {}
            Decl::Const { name, ty, value, span, .. } => {
                let val_ty = self.infer_expr(value, &Env::new());
                if let Some(vt) = val_ty {
                    if !self.types_compatible(ty, &vt) {
                        self.error(
                            format!("const '{}': declared {:?} but got {:?}", name, ty, vt),
                            span,
                        );
                    }
                }
            }
        }
    }

    fn check_block(&mut self, stmts: &[Stmt], env: &mut Env) {
        env.push();
        for stmt in stmts { self.check_stmt(stmt, env); }
        env.pop();
    }

    fn check_stmt(&mut self, stmt: &Stmt, env: &mut Env) {
        match stmt {
            Stmt::Let { name, mutable, ty, value, span } => {
                // threadgroup 지역 배열은 초기화 없이 선언되므로 타입 체크 스킵
                if matches!(ty, Some(Type::ArrayN { space: AddressSpace::Threadgroup, .. })) {
                    env.define(name, ty.clone().unwrap(), *mutable);
                    return;
                }

                let val_ty = self.infer_expr(value, env);
                if let Some(declared_ty) = ty {
                    if let Some(ref vt) = val_ty {
                        if !self.types_compatible(declared_ty, vt) {
                            self.error(
                                format!("let '{}': declared {:?} but got {:?}", name, declared_ty, vt),
                                span,
                            );
                        }
                    }
                    env.define(name, declared_ty.clone(), *mutable);
                } else {
                    env.define(name, val_ty.unwrap_or(Type::Void), *mutable);
                }
            }

            Stmt::Assign { target, value, span } => {
                self.check_assignable(target, env, span);

                // 같은 버퍼 읽기/쓰기 aliasing 감지
                if let Some(write_buf) = self.buffer_name(target) {
                    if let Some(read_buf) = self.buffer_name(value) {
                        if write_buf == read_buf {
                            self.error(
                                format!("reading and writing same buffer '{}' in one statement — possible aliasing", write_buf),
                                span,
                            );
                        }
                    }
                }

                let tt = self.infer_expr(target, env);
                let vt = self.infer_expr(value, env);
                if let (Some(tt), Some(vt)) = (tt, vt) {
                    if !self.types_compatible(&tt, &vt) {
                        self.error(format!("assignment mismatch: {:?} = {:?}", tt, vt), span);
                    }
                }
                if self.is_threadgroup_write(target, env) {
                    self.threadgroup_written = true;
                    self.threadgroup_barrier_seen = false;
                }
            }

            Stmt::Return(expr, _) => {
                if let Some(e) = expr { self.infer_expr(e, env); }
            }

            Stmt::If { cond, then, else_, span } => {
                let cond_ty = self.infer_expr(cond, env);
                if let Some(ty) = cond_ty {
                    if ty != Type::Bool {
                        self.error(format!("if condition must be bool, got {:?}", ty), span);
                    }
                }
                self.check_block(then, env);
                if let Some(eb) = else_ { self.check_block(eb, env); }
            }

            Stmt::For { var, from, to, body, .. } => {
                self.infer_expr(from, env);
                self.infer_expr(to, env);
                env.push();
                env.define(var, Type::U32, false);
                for s in body { self.check_stmt(s, env); }
                env.pop();
            }

            Stmt::Expr(e, _) => {
                if let Expr::Call { name, .. } = e {
                    if name == "barrier" {
                        self.threadgroup_barrier_seen = true;
                        self.threadgroup_written = false;
                    }
                }
                self.infer_expr(e, env);
            }
        }
    }

    fn check_assignable(&mut self, expr: &Expr, env: &Env, span: &Span) {
        match expr {
            Expr::Ident(name, _) => {
                match env.lookup(name) {
                    Some(v) if !v.mutable =>
                        self.error(format!("cannot assign to immutable variable '{}'", name), span),
                    None =>
                        self.error(format!("undefined variable '{}'", name), span),
                    _ => {}
                }
            }
            Expr::Index { array, .. } => {
                if let Expr::Ident(name, _) = array.as_ref() {
                    match env.lookup(name) {
                        Some(VarInfo { ty: Type::Array  { mutable, .. }, .. }) |
                        Some(VarInfo { ty: Type::ArrayN { mutable, .. }, .. }) if !mutable =>
                            self.error(format!("cannot write to immutable buffer '{}'", name), span),
                        None =>
                            self.error(format!("undefined variable '{}'", name), span),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn is_threadgroup_write(&self, expr: &Expr, env: &Env) -> bool {
        if let Expr::Index { array, .. } = expr {
            if let Expr::Ident(name, _) = array.as_ref() {
                match env.lookup(name) {
                    Some(VarInfo { ty: Type::Array  { space, .. }, .. }) |
                    Some(VarInfo { ty: Type::ArrayN { space, .. }, .. }) =>
                        return *space == AddressSpace::Threadgroup,
                    _ => {}
                }
            }
        }
        false
    }

    fn infer_expr(&mut self, expr: &Expr, env: &Env) -> Option<Type> {
        match expr {
            Expr::IntLit(_, _)   => Some(Type::I32),
            Expr::FloatLit(_, _) => Some(Type::F32),
            Expr::Bool(_, _)     => Some(Type::Bool),

            Expr::Ident(name, span) => {
                // math::PI 같은 qualified 상수
                if name.contains("::") {
                    let parts: Vec<&str> = name.splitn(2, "::").collect();
                    let (mod_name, item_name) = (parts[0], parts[1]);
                    match self.modules.get(mod_name) {
                        Some(exports) => {
                            match exports.get(item_name) {
                                Some(crate::metal::module::ExportKind::Const(ty)) => return Some(ty.clone()),
                                Some(crate::metal::module::ExportKind::Struct(_)) => {
                                    self.error(format!("'{}::{}' is a struct, not a value", mod_name, item_name), span);
                                    return None;
                                }
                                None => {
                                    self.error(format!("'{}' not found in module '{}'", item_name, mod_name), span);
                                    return None;
                                }
                            }
                        }
                        None => {
                            self.error(format!("unknown module '{}'", mod_name), span);
                            return None;
                        }
                    }
                }
                match env.lookup(name) {
                    Some(v) => Some(v.ty.clone()),
                    None => {
                        self.error(format!("undefined variable '{}'", name), span);
                        None
                    }
                }
            }

            Expr::Field { object, field, span } => {
                let obj_ty = self.infer_expr(object, env)?;
                match &obj_ty {
                    Type::Vec2(_) | Type::Vec3(_) | Type::Vec4(_) => {
                        match field.as_str() {
                            "x" | "y" | "z" | "w" => Some(Type::U32),
                            _ => {
                                self.error(format!("unknown field '.{}'", field), span);
                                None
                            }
                        }
                    }
                    Type::Named(struct_name) => {
                        let struct_name = struct_name.clone();
                        match self.structs.get(&struct_name) {
                            Some(fields) => {
                                match fields.iter().find(|(n, _)| n == field) {
                                    Some((_, ty)) => Some(ty.clone()),
                                    None => {
                                        self.error(
                                            format!("struct '{}' has no field '{}'", struct_name, field),
                                            span,
                                        );
                                        None
                                    }
                                }
                            }
                            None => {
                                self.error(format!("unknown struct '{}'", struct_name), span);
                                None
                            }
                        }
                    }
                    // math::Matrix4 같은 qualified 타입 필드 접근
                    Type::Qualified(mod_name, item_name) => {
                        let key = format!("{}::{}", mod_name, item_name);
                        match self.structs.get(&key) {
                            Some(fields) => {
                                match fields.iter().find(|(n, _)| n == field) {
                                    Some((_, ty)) => Some(ty.clone()),
                                    None => {
                                        self.error(
                                            format!("{}::{} has no field '{}'", mod_name, item_name, field),
                                            span,
                                        );
                                        None
                                    }
                                }
                            }
                            None => {
                                self.error(format!("unknown type '{}::{}'", mod_name, item_name), span);
                                None
                            }
                        }
                    }
                    _ => {
                        self.error(format!("field access on non-struct/vector type {:?}", obj_ty), span);
                        None
                    }
                }
            }

            Expr::Index { array, index, span } => {
                let arr_ty = self.infer_expr(array, env)?;
                let idx_ty = self.infer_expr(index, env);
                if let Some(it) = idx_ty {
                    if !matches!(it, Type::I32 | Type::U32) {
                        self.error(format!("array index must be integer, got {:?}", it), span);
                    }
                }
                if self.threadgroup_written && !self.threadgroup_barrier_seen {
                    if let Expr::Ident(name, _) = array.as_ref() {
                        match env.lookup(name) {
                            Some(VarInfo { ty: Type::Array  { space, .. }, .. }) |
                            Some(VarInfo { ty: Type::ArrayN { space, .. }, .. })
                                if *space == AddressSpace::Threadgroup => {
                                self.error(
                                    format!("reading from threadgroup '{}' after write without barrier()", name),
                                    span,
                                );
                            }
                            _ => {}
                        }
                    }
                }
                match arr_ty {
                    Type::Array  { elem, .. } => Some(*elem),
                    Type::ArrayN { elem, .. } => Some(*elem),
                    _ => {
                        self.error("index on non-array type".to_string(), span);
                        None
                    }
                }
            }

            Expr::BinOp { op, lhs, rhs, .. } => {
                let lt = self.infer_expr(lhs, env);
                let rt = self.infer_expr(rhs, env);
                match op {
                    BinOp::Eq | BinOp::Ne |
                    BinOp::Lt | BinOp::Gt |
                    BinOp::Le | BinOp::Ge |
                    BinOp::And | BinOp::Or => Some(Type::Bool),
                    _ => lt.or(rt),
                }
            }

            Expr::UnaryOp { expr, .. } => self.infer_expr(expr, env),

            Expr::Call { name, args, span } => {
                let arg_tys: Vec<_> = args.iter()
                    .map(|a| self.infer_expr(a, env))
                    .collect();

                match name.as_str() {
                    "barrier" => Some(Type::Void),

                    "simd_sum"     |
                    "simd_min"     |
                    "simd_max"     |
                    "simd_product" => arg_tys.into_iter().next().flatten(),

                    "simd_prefix_sum" |
                    "simd_prefix_min" |
                    "simd_prefix_max" => arg_tys.into_iter().next().flatten(),

                    "simd_broadcast" => {
                        if args.len() != 2 {
                            self.error("simd_broadcast requires 2 arguments", span);
                        }
                        arg_tys.into_iter().next().flatten()
                    }

                    "simd_shuffle_down" |
                    "simd_shuffle_up"   => {
                        if args.len() != 2 {
                            self.error(format!("{} requires 2 arguments", name), span);
                        }
                        arg_tys.into_iter().next().flatten()
                    }

                    "simd_all" | "simd_any" => {
                        if args.len() != 1 {
                            self.error(format!("{} requires 1 argument", name), span);
                        }
                        Some(Type::Bool)
                    }

                    _ => {
                        self.error(format!("unknown function '{}'", name), span);
                        None
                    }
                }
            }
        }
    }

    fn types_compatible(&self, a: &Type, b: &Type) -> bool {
        match (a, b) {
            (Type::F32,  Type::F32)  => true,
            (Type::F16,  Type::F16)  => true,
            (Type::I32,  Type::I32)  => true,
            (Type::U32,  Type::U32)  => true,
            (Type::U32,  Type::I32)  => true,
            (Type::I32,  Type::U32)  => true,
            (Type::Bool, Type::Bool) => true,
            (Type::Named(a),     Type::Named(b))     => a == b,
            (Type::Qualified(ma, ia), Type::Qualified(mb, ib)) => ma == mb && ia == ib,
            (Type::Vec2(a), Type::Vec2(b)) => self.types_compatible(a, b),
            (Type::Vec3(a), Type::Vec3(b)) => self.types_compatible(a, b),
            (Type::Vec4(a), Type::Vec4(b)) => self.types_compatible(a, b),
            (Type::ArrayN { elem: a, size: sa, .. },
             Type::ArrayN { elem: b, size: sb, .. }) => sa == sb && self.types_compatible(a, b),
            _ => false,
        }
    }

    fn buffer_name<'a>(&self, expr: &'a Expr) -> Option<&'a str> {
        match expr {
            Expr::Index { array, .. } => {
                if let Expr::Ident(name, _) = array.as_ref() { Some(name) } else { None }
            }
            _ => None,
        }
    }
}