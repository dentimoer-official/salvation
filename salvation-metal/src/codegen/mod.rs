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
    /// vertex 출력 구조체 이름 — fragment의 [[stage_in]] 타입으로 사용
    /// generate()에서 사전 스캔으로 설정됨
    vert_out_struct: String,
}

impl Codegen {
    pub fn new() -> Self {
        Codegen {
            output: String::new(),
            indent: 0,
            stage_in_params: Vec::new(),
            stage_in_prefix: String::new(),
            vert_out_struct: "VertOut".to_string(), // 기본값
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
            Type::Texture2D => "texture2d<float>".into(),  // Metal requires template arg
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
                // sample(tex, smp, coord) → Metal 메서드 문법: tex.sample(smp, coord)
                // sampleLevel/Bias/Grad 도 동일 패턴
                if (name == "sample" || name == "sampleLevel" || name == "sampleBias" || name == "sampleGrad")
                    && args.len() >= 2
                {
                    self.emit_expr(&args[0]);     // 텍스처 오브젝트
                    self.push(".sample(");
                    for (i, arg) in args[1..].iter().enumerate() {
                        if i > 0 { self.push(", "); }
                        self.emit_expr(arg);
                    }
                    self.push(")");
                } else {
                    self.push(name);
                    self.push("(");
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 { self.push(", "); }
                        self.emit_expr(arg);
                    }
                    self.push(")");
                }
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
            // [Fix 5] shader_types.h에 공유 정의 — Metal 셰이더에서 중복 선언 생략
            Item::StructDecl { name, .. } => {
                self.push(&format!("// struct {} — shader_types.h 참조\n\n", name));
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
                        // [Fix 3-1] 첫 번째 파라미터(position)는 return stmt에서
                        // uniforms 행렬과 함께 처리하므로 여기선 건너뜀.
                        for (i, p) in params.iter().enumerate() {
                            if i == 0 { continue; }
                            self.push_indent();
                            self.push(&format!("out.{} = in.{};\n", p.name, p.name));
                        }

                        // .slvt body 내의 return 구문 처리
                        // [Fix 3-1] return expr → out.position = uniforms.projectionViewModel * expr;
                        // CPU가 매 프레임 계산한 변환 행렬을 GPU에서 실제로 적용함.
                        self.stage_in_prefix = "in".to_string();
                        self.stage_in_params = params.iter().map(|p| p.name.clone()).collect();
                        for stmt in body {
                            match stmt {
                                Stmt::Return(Some(expr)) => {
                                    let pos_name = params.first()
                                        .map(|p| p.name.as_str())
                                        .unwrap_or("position");
                                    self.push_indent();
                                    self.push(&format!(
                                        "out.{} = uniforms.projectionViewModel * ",
                                        pos_name
                                    ));
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

                        // texture2d / sampler 파라미터는 [[texture(n)]] / [[sampler(n)]] 어노테이션.
                        // 나머지 파라미터는 vertex → fragment 전달 경로(VertOut [[stage_in]])를 사용.
                        let mut tex_idx  = 0usize;
                        let mut smp_idx  = 0usize;
                        let mut sig_parts: Vec<String> = Vec::new();
                        let mut stage_in_names: Vec<String> = Vec::new(); // out.xxx 로 접근

                        for p in params.iter() {
                            match &p.ty {
                                Type::Texture2D => {
                                    sig_parts.push(format!(
                                        "texture2d<float> {} [[texture({})]]",
                                        p.name, tex_idx
                                    ));
                                    tex_idx += 1;
                                    // texture2d는 body에서 직접 이름으로 접근 — stage_in 제외
                                }
                                Type::Sampler => {
                                    sig_parts.push(format!(
                                        "sampler {} [[sampler({})]]",
                                        p.name, smp_idx
                                    ));
                                    smp_idx += 1;
                                    // sampler도 직접 접근 — stage_in 제외
                                }
                                _ => {
                                    // 일반 데이터 파라미터 → VertOut [[stage_in]] 으로 전달
                                    stage_in_names.push(p.name.clone());
                                }
                            }
                        }

                        // stage_in 파라미터가 있으면 {VertOut} out [[stage_in]] 추가 (맨 앞)
                        // vert_out_struct는 generate()에서 vertex 함수명으로 미리 결정됨
                        if !stage_in_names.is_empty() {
                            sig_parts.insert(
                                0,
                                format!("{} out [[stage_in]]", self.vert_out_struct),
                            );
                        }

                        let sig = sig_parts.join(", ");
                        self.push(&format!("fragment {} {}({}) ", ret_str, name, sig));

                        // body 내부: 일반 파라미터만 out. 접두사 적용
                        self.stage_in_prefix = "out".to_string();
                        self.stage_in_params = stage_in_names;
                        self.emit_block(body);
                        self.stage_in_params.clear();
                        self.stage_in_prefix.clear();

                        self.newline();
                    }

                    Some(ShaderStage::Kernel) => {
                        // Metal kernel 파라미터는 반드시 어노테이션이 있어야 함.
                        // 휴리스틱:
                        //   첫 번째 uint 파라미터 → [[thread_position_in_grid]] (스레드 인덱스)
                        //   이후 uint 파라미터    → constant uint& name [[buffer(n)]] (상수)
                        //   그 외 타입 파라미터  → device T* name [[buffer(n)]] (read-write 버퍼)
                        let mut buf_idx = 0usize;
                        let mut first_uint_seen = false;
                        let mut param_strs: Vec<String> = Vec::new();

                        for p in params.iter() {
                            let ps = match &p.ty {
                                Type::Uint if !first_uint_seen => {
                                    first_uint_seen = true;
                                    format!("uint {} [[thread_position_in_grid]]", p.name)
                                }
                                Type::Uint => {
                                    let s = format!(
                                        "constant uint& {} [[buffer({})]]",
                                        p.name, buf_idx
                                    );
                                    buf_idx += 1;
                                    s
                                }
                                Type::Int => {
                                    let s = format!(
                                        "constant int& {} [[buffer({})]]",
                                        p.name, buf_idx
                                    );
                                    buf_idx += 1;
                                    s
                                }
                                other => {
                                    let ty_str = self.emit_type(other);
                                    let s = format!(
                                        "device {}* {} [[buffer({})]]",
                                        ty_str, p.name, buf_idx
                                    );
                                    buf_idx += 1;
                                    s
                                }
                            };
                            param_strs.push(ps);
                        }

                        let params_str = param_strs.join(", ");
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
        // 사전 스캔: vertex 함수 이름을 파악해서 vert_out_struct 결정.
        // fragment 함수의 [[stage_in]] 타입이 여기서 결정된 이름을 따른다.
        for item in program {
            if let Item::FnDecl { stage: Some(ShaderStage::Vertex), name, .. } = item {
                self.vert_out_struct = format!("{}Out", capitalize(name));
                break;
            }
        }

        self.push("#include <metal_stdlib>\n");
        self.push("using namespace metal;\n");
        // CPU/GPU 공유 타입 헤더 — FrameUniforms 등을 한 곳에서만 정의.
        self.push("#include \"shader_types.h\"\n\n");

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