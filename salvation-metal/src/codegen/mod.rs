// codegen.rs
// AST (ast_testing) → Metal 소스코드 생성

use salvation_core::compiler::ast::types::{
    Block, BinOpKind, Expr, Item, Param, Program, ShaderStage, Stmt, Type, UnaryOpKind,
};

pub struct Codegen {
    output: String,
    indent: usize,
    /// vertex/fragment 함수 내부에서 파라미터 이름 → "prefix.이름" 으로 변환
    stage_in_params: Vec<String>,
    stage_in_prefix: String,  // vertex: "in", fragment: "out"
}

impl Codegen {
    pub fn new() -> Self {
        Codegen {
            output: String::new(),
            indent: 0,
            stage_in_params: Vec::new(),
            stage_in_prefix: String::new(),
        }
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
            BinOpKind::Add    => "+",
            BinOpKind::Sub    => "-",
            BinOpKind::Mul    => "*",
            BinOpKind::Div    => "/",
            BinOpKind::Mod    => "%",
            BinOpKind::Eq     => "==",
            BinOpKind::NotEq  => "!=",
            BinOpKind::Lt     => "<",
            BinOpKind::Gt     => ">",
            BinOpKind::LtEq   => "<=",
            BinOpKind::GtEq   => ">=",
            BinOpKind::And    => "&&",
            BinOpKind::Or     => "||",
            BinOpKind::Assign => "=",
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
                    self.push(&format!("{}.{}", self.stage_in_prefix, s));
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
                        let in_struct  = format!("{}In",  capitalize(name));
                        let out_struct = format!("{}Out", capitalize(name));

                        // VertIn struct — 입력 속성
                        self.push(&format!("struct {} {{\n", in_struct));
                        for (i, p) in params.iter().enumerate() {
                            let ty_str = self.emit_type(&p.ty);
                            self.push(&format!(
                                "    {} {} [[attribute({})]];\n",
                                ty_str, p.name, i
                            ));
                        }
                        self.push("};\n\n");

                        // VertOut struct — vertex → fragment 전달
                        // 첫 번째 파라미터는 [[position]], 나머지는 일반 필드
                        self.push(&format!("struct {} {{\n", out_struct));
                        for (i, p) in params.iter().enumerate() {
                            let ty_str = self.emit_type(&p.ty);
                            if i == 0 {
                                self.push(&format!(
                                    "    {} {} [[position]];\n",
                                    ty_str, p.name
                                ));
                            } else {
                                self.push(&format!("    {} {};\n", ty_str, p.name));
                            }
                        }
                        self.push("};\n\n");

                        // vertex 함수 — VertOut을 반환
                        self.push(&format!(
                            "vertex {} {}({} in [[stage_in]], constant FrameUniforms& uniforms [[buffer(1)]]) {{\n",
                            out_struct, name, in_struct
                        ));
                        self.indent += 1;

                        // VertOut out; 선언
                        self.push_indent();
                        self.push(&format!("{} out;\n", out_struct));

                        // out.field = in.field; 자동 할당
                        for p in params.iter() {
                            self.push_indent();
                            self.push(&format!("out.{} = in.{};\n", p.name, p.name));
                        }

                        // .slvt body 내의 return 구문 처리
                        // body에 return이 있으면 out.position 덮어쓰기로 변환
                        self.stage_in_prefix = "in".to_string();
                        self.stage_in_params = params.iter().map(|p| p.name.clone()).collect();
                        for stmt in body {
                            match stmt {
                                Stmt::Return(Some(expr)) => {
                                    // return expr → out.position = expr; return out;
                                    self.push_indent();
                                    self.push("out.");
                                    self.push(&params.first().map(|p| p.name.as_str()).unwrap_or("position").to_string());
                                    self.push(" = ");
                                    self.emit_expr(expr);
                                    self.push(";\n");
                                }
                                other => self.emit_stmt(other),
                            }
                        }
                        self.stage_in_params.clear();
                        self.stage_in_prefix.clear();

                        self.push_indent();
                        self.push("return out;\n");
                        self.indent -= 1;
                        self.push("}\n\n");
                    }

                    Some(ShaderStage::Fragment) => {
                        let ret_str = ret_ty
                            .as_ref()
                            .map(|t| self.emit_type(t))
                            .unwrap_or_else(|| "void".into());

                        let frag_param = if params.len() == 1 {
                            "VertOut out [[stage_in]]".to_string()
                        } else {
                            self.emit_params(params)
                        };

                        self.push(&format!("fragment {} {}({}) ", ret_str, name, frag_param));

                        // fragment body에서 파라미터 이름을 out.이름으로 변환
                        self.stage_in_prefix = "out".to_string();
                        self.stage_in_params = params.iter().map(|p| p.name.clone()).collect();
                        self.emit_block(body);
                        self.stage_in_params.clear();
                        self.stage_in_prefix.clear();

                        self.newline();
                    }

                    Some(ShaderStage::Kernel) => {
                        let params_str = self.emit_params(params);
                        self.push(&format!("kernel void {}({}) ", name, params_str));
                        self.emit_block(body);
                        self.newline();
                    }

                    None => {
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