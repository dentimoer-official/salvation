// codegen.rs
// AST (ast_testing) → Metal 소스코드 생성

use salvation_core::compiler::ast::types::{
    Block, BinOpKind, Expr, Item, Param, Program, ShaderStage, Stmt, Type, UnaryOpKind,
};

// runner/mod.rs
// Metal 셰이더 컴파일 + host 코드 컴파일 + 실행을 담당한다.
//
// 순서:
//   1. xcrun metal      → shaders.air
//   2. xcrun metallib   → shaders.metallib
//   3. clang++          → app (실행파일)
//   4. ./app            → 실행

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub struct RunnerError(pub String);

impl std::fmt::Display for RunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn err(msg: impl Into<String>) -> RunnerError {
    RunnerError(msg.into())
}

/// 셰이더 컴파일 + host 코드 빌드 + 실행
pub fn build_and_run(out_dir: &Path) -> Result<(), RunnerError> {
    let metal_path   = out_dir.join("shaders.metal");
    let air_path     = out_dir.join("shaders.air");
    let metallib     = out_dir.join("shaders.metallib");
    let _common_h    = out_dir.join("common.h");
    let main_mm      = out_dir.join("main.mm");
    let app_path     = out_dir.join("salvation_app");

    // ── 1. .metal → .air ────────────────────────────────────
    run_cmd(
        "xcrun",
        &[
            "-sdk", "macosx", "metal",
            "-c", metal_path.to_str().unwrap(),
            "-o", air_path.to_str().unwrap(),
        ],
        "셰이더 컴파일 (metal → air)",
    )?;

    // ── 2. .air → .metallib ─────────────────────────────────
    run_cmd(
        "xcrun",
        &[
            "-sdk", "macosx", "metallib",
            air_path.to_str().unwrap(),
            "-o", metallib.to_str().unwrap(),
        ],
        "셰이더 링크 (air → metallib)",
    )?;

    // ── 3. main.mm → 실행파일 ───────────────────────────────
    run_cmd(
        "clang++",
        &[
            "-std=c++17",
            "-ObjC++",
            "-fobjc-arc",
            // include 경로 — common.h가 있는 디렉터리
            &format!("-I{}", out_dir.to_str().unwrap()),
            main_mm.to_str().unwrap(),
            "-o", app_path.to_str().unwrap(),
            "-framework", "Cocoa",
            "-framework", "Metal",
            "-framework", "MetalKit",
            "-framework", "QuartzCore",
        ],
        "host 코드 컴파일 (main.mm → app)",
    )?;

    // ── 4. 실행 ─────────────────────────────────────────────
    let abs_app = app_path.canonicalize()
        .map_err(|e| err(format!("실행파일 경로 확인 실패: {}\n파일: {}", e, app_path.display())))?;

    eprintln!("  실행: {}", abs_app.display());

    let status = Command::new(&abs_app)
        .current_dir(out_dir.canonicalize().unwrap_or(out_dir.to_path_buf()))
        .status()
        .map_err(|e| err(format!("실행 실패: {}", e)))?;

    if !status.success() {
        return Err(err(format!("앱이 비정상 종료됐습니다 (exit: {})", status)));
    }

    Ok(())
}

/// 셰이더만 컴파일 (metallib 생성까지, 실행 안 함)
pub fn build_only(out_dir: &Path) -> Result<PathBuf, RunnerError> {
    let metal_path = out_dir.join("shaders.metal");
    let air_path   = out_dir.join("shaders.air");
    let metallib   = out_dir.join("shaders.metallib");

    run_cmd(
        "xcrun",
        &[
            "-sdk", "macosx", "metal",
            "-c", metal_path.to_str().unwrap(),
            "-o", air_path.to_str().unwrap(),
        ],
        "셰이더 컴파일 (metal → air)",
    )?;

    run_cmd(
        "xcrun",
        &[
            "-sdk", "macosx", "metallib",
            air_path.to_str().unwrap(),
            "-o", metallib.to_str().unwrap(),
        ],
        "셰이더 링크 (air → metallib)",
    )?;

    Ok(metallib)
}

// ── 헬퍼 ────────────────────────────────────────────────────

fn run_cmd(prog: &str, args: &[&str], label: &str) -> Result<(), RunnerError> {
    eprintln!("  {}...", label);

    let output = Command::new(prog)
        .args(args)
        .output()
        .map_err(|e| err(format!(
            "'{}' 실행 실패: {}\n xcrun/clang++이 설치돼 있는지 확인하세요 (Xcode Command Line Tools 필요)",
            prog, e
        )))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(err(format!("{} 실패:\n{}", label, stderr)));
    }

    Ok(())
}

pub struct Codegen {
    output: String,
    indent: usize,
    /// vertex 함수 내부에서 파라미터 이름 → "in.이름" 으로 변환할 목록
    stage_in_params: Vec<String>,
}

impl Codegen {
    pub fn new() -> Self {
        Codegen { output: String::new(), indent: 0, stage_in_params: Vec::new() }
    }

    // ── 유틸 ───────────────────────────────────────────────

    fn push(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn push_indent(&mut self) {
        self.push(&"    ".repeat(self.indent));
    }

    fn newline(&mut self) {
        self.push("\n");
    }

    // ── 타입 → Metal 문자열 ────────────────────────────────

    fn emit_type(&self, ty: &Type) -> String {
        match ty {
            Type::Bool    => "bool".into(),
            Type::Int     => "int".into(),
            Type::Uint    => "uint".into(),
            Type::Float   => "float".into(),
            Type::Float2  => "float2".into(),
            Type::Float3  => "float3".into(),
            Type::Float4  => "float4".into(),
            Type::Mat2x2  => "float2x2".into(),
            Type::Mat2x3  => "float2x3".into(),
            Type::Mat2x4  => "float2x4".into(),
            Type::Mat3x2  => "float3x2".into(),
            Type::Mat3x3  => "float3x3".into(),
            Type::Mat3x4  => "float3x4".into(),
            Type::Mat4x2  => "float4x2".into(),
            Type::Mat4x3  => "float4x3".into(),
            Type::Mat4x4  => "float4x4".into(),
            Type::Texture2D => "texture2d".into(),
            Type::Sampler => "sampler".into(),
            Type::Array { inner, size } => {
                format!("array<{}, {}>", self.emit_type(inner), size)
            }
            Type::Named(s) => s.clone(),
            Type::Unit => "void".into(),
        }
    }

    // ── 연산자 → Metal 문자열 ──────────────────────────────

    fn emit_binop(&self, op: &BinOpKind) -> &'static str {
        match op {
            BinOpKind::Add       => "+",
            BinOpKind::Sub       => "-",
            BinOpKind::Mul       => "*",
            BinOpKind::Div       => "/",
            BinOpKind::Mod       => "%",
            BinOpKind::Eq        => "==",
            BinOpKind::NotEq     => "!=",
            BinOpKind::Lt        => "<",
            BinOpKind::Gt        => ">",
            BinOpKind::LtEq      => "<=",
            BinOpKind::GtEq      => ">=",
            BinOpKind::And       => "&&",
            BinOpKind::Or        => "||",
            BinOpKind::Assign    => "=",
            BinOpKind::AddAssign => "+=",
            BinOpKind::SubAssign => "-=",
            BinOpKind::MulAssign => "*=",
            BinOpKind::DivAssign => "/=",
            BinOpKind::ModAssign => "%=",
        }
    }

    // ── 표현식 ─────────────────────────────────────────────

    fn emit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::IntLit(n)   => self.push(&n.to_string()),
            Expr::FloatLit(f) => self.push(&f.to_string()),
            Expr::BoolLit(b)  => self.push(if *b { "true" } else { "false" }),
            Expr::Ident(s) => {
                if self.stage_in_params.contains(s) {
                    self.push(&format!("in.{}", s));
                } else {
                    self.push(s);
                }
            }

            Expr::BinOp { op, lhs, rhs } => {
                self.push("(");
                self.emit_expr(lhs);
                self.push(&format!(" {} ", self.emit_binop(op)));
                self.emit_expr(rhs);
                self.push(")");
            }

            Expr::UnaryOp { op, expr } => {
                let op_str = match op {
                    UnaryOpKind::Neg => "-",
                    UnaryOpKind::Not => "!",
                };
                self.push(op_str);
                self.emit_expr(expr);
            }

            // foo(a, b)
            Expr::Call { name, args } => {
                self.push(name);
                self.push("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { self.push(", "); }
                    self.emit_expr(arg);
                }
                self.push(")");
            }

            // v.x
            Expr::Field { object, field } => {
                self.emit_expr(object);
                self.push(".");
                self.push(field);
            }

            // arr[i]
            Expr::Index { object, index } => {
                self.emit_expr(object);
                self.push("[");
                self.emit_expr(index);
                self.push("]");
            }
        }
    }

    // 표현식을 String으로 추출 (for문 같은 곳에서 필요)
    fn expr_to_string(&mut self, expr: &Expr) -> String {
        let start = self.output.len();
        self.emit_expr(expr);
        self.output.split_off(start)
    }

    // ── 구문 ───────────────────────────────────────────────

    fn emit_stmt(&mut self, stmt: &Stmt) {
        self.push_indent();
        match stmt {
            // let [mut] x: Type = expr;
            // → Metal: Type x = expr;  (mut는 Metal에 없음)
            Stmt::VarDecl { name, ty, value, .. } => {
                let ty_str = self.emit_type(ty);
                self.push(&format!("{} {}", ty_str, name));
                if let Some(val) = value {
                    self.push(" = ");
                    self.emit_expr(val);
                }
                self.push(";\n");
            }

            // return expr;
            Stmt::Return(expr) => {
                self.push("return");
                if let Some(e) = expr {
                    self.push(" ");
                    self.emit_expr(e);
                }
                self.push(";\n");
            }

            // if (cond) { } else { }
            Stmt::If { cond, then_block, else_block } => {
                self.push("if (");
                self.emit_expr(cond);
                self.push(") ");
                self.emit_block(then_block);
                if let Some(else_b) = else_block {
                    self.push_indent();
                    self.push("else ");
                    self.emit_block(else_b);
                }
            }

            // for i in 0..n { }
            // → Metal: for (int i = from; i < to; i++) { }
            Stmt::For { var, from, to, body } => {
                let from_str = self.expr_to_string(from);
                let to_str   = self.expr_to_string(to);
                self.push(&format!(
                    "for (int {v} = {f}; {v} < {t}; {v}++) ",
                    v = var,
                    f = from_str,
                    t = to_str,
                ));
                self.emit_block(body);
            }

            // while cond { }
            Stmt::While { cond, body } => {
                self.push("while (");
                self.emit_expr(cond);
                self.push(") ");
                self.emit_block(body);
            }

            // break;
            Stmt::Break => {
                self.push("break;\n");
            }

            // continue;
            Stmt::Continue => {
                self.push("continue;\n");
            }

            // foo(x);
            Stmt::ExprStmt(expr) => {
                self.emit_expr(expr);
                self.push(";\n");
            }
        }
    }

    fn emit_block(&mut self, block: &Block) {
        self.push("{\n");
        self.indent += 1;
        for stmt in block {
            self.emit_stmt(stmt);
        }
        self.indent -= 1;
        self.push_indent();
        self.push("}\n");
    }

    // ── 파라미터 ───────────────────────────────────────────

    fn emit_params(&mut self, params: &[Param]) -> String {
        params
            .iter()
            .map(|p| format!("{} {}", self.emit_type(&p.ty), p.name))
            .collect::<Vec<_>>()
            .join(", ")
    }

    // ── 최상위 선언 ────────────────────────────────────────

    fn emit_item(&mut self, item: &Item) {
        match item {
            // import "path" → #include "path"
            Item::Import(path) => {
                self.push(&format!("#include \"{}\"\n", path));
            }

            // struct Name { fields }
            Item::StructDecl { name, fields } => {
                self.push(&format!("struct {} {{\n", name));
                self.indent += 1;
                for f in fields {
                    self.push_indent();
                    let ty_str = self.emit_type(&f.ty);
                    self.push(&format!("{} {};\n", ty_str, f.name));
                }
                self.indent -= 1;
                self.push("};\n\n");
            }

            // fn / @vertex fn / @fragment fn / @kernel fn
            Item::FnDecl { is_main, stage, name, params, ret_ty, body, .. } => {
                // fn main()은 host-side 진입점 — Metal 셰이더에 출력하지 않음
                if *is_main {
                    return;
                }

                match stage {
                    Some(ShaderStage::Vertex) => {
                        // vertex 함수는 파라미터를 [[stage_in]] struct로 감싸야 함
                        let struct_name = format!("{}In", capitalize(name));
                        self.push(&format!("struct {} {{\n", struct_name));
                        for (i, p) in params.iter().enumerate() {
                            let ty_str = self.emit_type(&p.ty);
                            self.push(&format!(
                                "    {} {} [[attribute({})]];\n",
                                ty_str, p.name, i
                            ));
                        }
                        self.push("};\n\n");

                        let ret_str = ret_ty
                            .as_ref()
                            .map(|t| self.emit_type(t))
                            .unwrap_or_else(|| "void".into());

                        self.push(&format!(
                            "vertex {} {}({} in [[stage_in]], constant FrameUniforms& uniforms [[buffer(1)]]) ",
                            ret_str, name, struct_name
                        ));

                        // body 내에서 파라미터 이름을 in.이름으로 자동 변환
                        self.stage_in_params = params.iter().map(|p| p.name.clone()).collect();
                        self.emit_block(body);
                        self.stage_in_params.clear();

                        self.newline();
                    }

                    Some(ShaderStage::Fragment) => {
                        let ret_str = ret_ty
                            .as_ref()
                            .map(|t| self.emit_type(t))
                            .unwrap_or_else(|| "void".into());
                        let params_str = self.emit_params(params);
                        self.push(&format!("fragment {} {}({}) ", ret_str, name, params_str));
                        self.emit_block(body);
                        self.newline();
                    }

                    Some(ShaderStage::Kernel) => {
                        let params_str = self.emit_params(params);
                        self.push(&format!("kernel void {}({}) ", name, params_str));
                        self.emit_block(body);
                        self.newline();
                    }

                    None => {
                        // 일반 헬퍼 함수
                        let ret_str = ret_ty
                            .as_ref()
                            .map(|t| self.emit_type(t))
                            .unwrap_or_else(|| "void".into());
                        let params_str = self.emit_params(params);
                        self.push(&format!("{} {}({}) ", ret_str, name, params_str));
                        self.emit_block(body);
                        self.newline();
                    }
                }
            }
        }
    }

    // ── 진입점 ─────────────────────────────────────────────

    pub fn generate(&mut self, program: &Program) -> String {
        // Metal 필수 헤더
        self.push("#include <metal_stdlib>\n");
        self.push("using namespace metal;\n\n");

        for item in program {
            self.emit_item(item);
        }

        self.output.clone()
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None    => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}