use salvation_core::compiler::ast::types::{
    Block, BinOpKind, Expr, Item, Param, Program, ShaderStage, Stmt, Type, UnaryOpKind,
};

pub struct GlslOutput {
    pub vertex: Option<String>,
    pub fragment: Option<String>,
    pub compute: Option<String>,
}

struct Codegen {
    output: String,
    indent: usize,
    stage_in_params: Vec<String>,
    stage_in_prefix: String,
}

impl Codegen {
    fn new() -> Self {
        Codegen {
            output: String::new(),
            indent: 0,
            stage_in_params: Vec::new(),
            stage_in_prefix: String::new(),
        }
    }

    fn push(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn push_indent(&mut self) {
        self.push(&"    ".repeat(self.indent));
    }

    fn newline(&mut self) {
        self.push("\n");
    }

    fn emit_type(&self, ty: &Type) -> String {
        match ty {
            Type::Bool => "bool".into(),
            Type::Int => "int".into(),
            Type::Uint => "uint".into(),
            Type::Float => "float".into(),
            Type::Float2 => "vec2".into(),
            Type::Float3 => "vec3".into(),
            Type::Float4 => "vec4".into(),
            Type::Mat2x2 => "mat2".into(),
            Type::Mat2x3 | Type::Mat2x4 => "mat2".into(),
            Type::Mat3x2 | Type::Mat3x3 | Type::Mat3x4 => "mat3".into(),
            Type::Mat4x2 | Type::Mat4x3 | Type::Mat4x4 => "mat4".into(),
            Type::Texture2D => "sampler2D".into(),
            Type::Sampler => "sampler".into(),
            Type::Array { inner, size } => {
                format!("{}[{}]", self.emit_type(inner), size)
            }
            Type::Named(s) => s.clone(),
            Type::Unit => "void".into(),
        }
    }

    fn emit_binop(&self, op: &BinOpKind) -> &'static str {
        match op {
            BinOpKind::Add => "+",
            BinOpKind::Sub => "-",
            BinOpKind::Mul => "*",
            BinOpKind::Div => "/",
            BinOpKind::Mod => "%",
            BinOpKind::Eq => "==",
            BinOpKind::NotEq => "!=",
            BinOpKind::Lt => "<",
            BinOpKind::Gt => ">",
            BinOpKind::LtEq => "<=",
            BinOpKind::GtEq => ">=",
            BinOpKind::And => "&&",
            BinOpKind::Or => "||",
            BinOpKind::Assign => "=",
            BinOpKind::AddAssign => "+=",
            BinOpKind::SubAssign => "-=",
            BinOpKind::MulAssign => "*=",
            BinOpKind::DivAssign => "/=",
            BinOpKind::ModAssign => "%=",
        }
    }

    fn emit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::IntLit(n) => self.push(&n.to_string()),
            Expr::FloatLit(f) => self.push(&f.to_string()),
            Expr::BoolLit(b) => self.push(if *b { "true" } else { "false" }),
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
            Expr::Call { name, args } => {
                // sample(tex, smp, coord) → texture(tex, coord)  (GLSL merged sampler)
                if (name == "sample" || name == "sampleLevel" || name == "sampleBias"
                    || name == "sampleGrad")
                    && args.len() >= 2
                {
                    self.push("texture(");
                    self.emit_expr(&args[0]); // texture
                    self.push(", ");
                    // Skip sampler (args[1]), use only coord
                    for (i, arg) in args[2..].iter().enumerate() {
                        if i > 0 {
                            self.push(", ");
                        }
                        self.emit_expr(arg);
                    }
                    self.push(")");
                } else {
                    self.push(name);
                    self.push("(");
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            self.push(", ");
                        }
                        self.emit_expr(arg);
                    }
                    self.push(")");
                }
            }
            Expr::Field { object, field } => {
                self.emit_expr(object);
                self.push(".");
                self.push(field);
            }
            Expr::Index { object, index } => {
                self.emit_expr(object);
                self.push("[");
                self.emit_expr(index);
                self.push("]");
            }
        }
    }

    fn expr_to_string(&mut self, expr: &Expr) -> String {
        let start = self.output.len();
        self.emit_expr(expr);
        self.output.split_off(start)
    }

    fn emit_stmt(&mut self, stmt: &Stmt) {
        self.push_indent();
        match stmt {
            Stmt::VarDecl { name, ty, value, .. } => {
                let ty_str = self.emit_type(ty);
                self.push(&format!("{} {}", ty_str, name));
                if let Some(val) = value {
                    self.push(" = ");
                    self.emit_expr(val);
                }
                self.push(";\n");
            }
            Stmt::Return(expr) => {
                self.push("return");
                if let Some(e) = expr {
                    self.push(" ");
                    self.emit_expr(e);
                }
                self.push(";\n");
            }
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
            Stmt::For { var, from, to, body } => {
                let from_str = self.expr_to_string(from);
                let to_str = self.expr_to_string(to);
                self.push(&format!(
                    "for (int {v} = {f}; {v} < {t}; {v}++) ",
                    v = var,
                    f = from_str,
                    t = to_str,
                ));
                self.emit_block(body);
            }
            Stmt::While { cond, body } => {
                self.push("while (");
                self.emit_expr(cond);
                self.push(") ");
                self.emit_block(body);
            }
            Stmt::Break => {
                self.push("break;\n");
            }
            Stmt::Continue => {
                self.push("continue;\n");
            }
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

    fn emit_params(&mut self, params: &[Param]) -> String {
        params
            .iter()
            .map(|p| format!("{} {}", self.emit_type(&p.ty), p.name))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn emit_item(&mut self, item: &Item) {
        match item {
            Item::Import(path) => {
                self.push(&format!("#include \"{}\"\n", path));
            }
            Item::StructDecl { name, .. } => {
                self.push(&format!("// struct {} — defined elsewhere\n\n", name));
            }
            Item::FnDecl {
                is_main,
                stage,
                name,
                params,
                ret_ty,
                body,
                ..
            } => {
                if *is_main {
                    return;
                }

                match stage {
                    Some(ShaderStage::Vertex) => {
                        // GLSL 버텍스 셰이더
                        // 입력: layout(location=n) in TYPE paramName;
                        // 출력: layout(location=n) out TYPE paramName_vert; (이름 충돌 방지)
                        // body에서 paramName은 입력변수 → 직접 접근, _vert 붙인 출력에 대입
                        self.push("#version 450\n\n");

                        // 입력 선언
                        for (i, p) in params.iter().enumerate() {
                            let ty_str = self.emit_type(&p.ty);
                            self.push(&format!("layout(location = {}) in {} {};\n", i, ty_str, p.name));
                        }
                        self.push("\n");

                        // 출력 선언 (첫 번째 파라미터=position은 gl_Position, 나머지만 out)
                        for (i, p) in params.iter().skip(1).enumerate() {
                            let ty_str = self.emit_type(&p.ty);
                            // _vert suffix로 in/out 이름 충돌 방지
                            self.push(&format!("layout(location = {}) out {} {}_vert;\n", i, ty_str, p.name));
                        }
                        self.push("\n");

                        self.push("layout(set = 0, binding = 0) uniform FrameUniforms {\n");
                        self.push("    mat4 projectionViewModel;\n");
                        self.push("} uniforms;\n\n");

                        self.push("void main() {\n");
                        self.indent += 1;

                        // 입력 → 출력 대입 (position은 gl_Position에서, 나머지는 _vert 변수에)
                        for p in params.iter().skip(1) {
                            self.push_indent();
                            self.push(&format!("{0}_vert = {0};\n", p.name));
                        }

                        // body: return expr → gl_Position = uniforms.projectionViewModel * expr
                        // (파라미터는 입력변수로 직접 접근 — stage_in_params 불필요)
                        for stmt in body {
                            match stmt {
                                Stmt::Return(Some(expr)) => {
                                    self.push_indent();
                                    self.push("gl_Position = uniforms.projectionViewModel * ");
                                    self.emit_expr(expr);
                                    self.push(";\n");
                                }
                                other => self.emit_stmt(other),
                            }
                        }

                        self.indent -= 1;
                        self.push("}\n");
                    }

                    Some(ShaderStage::Fragment) => {
                        // GLSL 프래그먼트 셰이더
                        // 버텍스 출력(paramName_vert)을 받아 로컬 변수(paramName)에 bridge
                        self.push("#version 450\n\n");

                        let mut tex_idx = 0usize;
                        for p in params.iter() {
                            if matches!(&p.ty, Type::Texture2D) {
                                self.push(&format!(
                                    "layout(set = 0, binding = {}) uniform sampler2D {};\n",
                                    tex_idx, p.name
                                ));
                                tex_idx += 1;
                            }
                        }

                        let non_tex_params: Vec<_> = params
                            .iter()
                            .filter(|p| !matches!(&p.ty, Type::Texture2D | Type::Sampler))
                            .collect();

                        // 버텍스 출력과 이름 맞춤 (_vert suffix)
                        for (i, p) in non_tex_params.iter().enumerate() {
                            let ty_str = self.emit_type(&p.ty);
                            self.push(&format!("layout(location = {}) in {} {}_vert;\n", i, ty_str, p.name));
                        }

                        self.push("\nlayout(location = 0) out vec4 outColor;\n\n");
                        self.push("void main() {\n");
                        self.indent += 1;

                        // _vert 변수 → body에서 쓰는 파라미터 이름으로 bridge
                        for p in non_tex_params.iter() {
                            let ty_str = self.emit_type(&p.ty);
                            self.push_indent();
                            self.push(&format!("{} {} = {}_vert;\n", ty_str, p.name, p.name));
                        }

                        // body: return expr → outColor = expr
                        for stmt in body {
                            match stmt {
                                Stmt::Return(Some(expr)) => {
                                    self.push_indent();
                                    self.push("outColor = ");
                                    self.emit_expr(expr);
                                    self.push(";\n");
                                }
                                other => self.emit_stmt(other),
                            }
                        }

                        self.indent -= 1;
                        self.push("}\n");
                    }

                    Some(ShaderStage::Kernel) => {
                        self.push("#version 450\n\n");
                        self.push("layout(local_size_x = 256) in;\n\n");

                        let mut binding = 0usize;
                        let first_param_name = params.first().map(|p| p.name.as_str()).unwrap_or("idx");

                        for (i, p) in params.iter().enumerate() {
                            if i == 0 {
                                continue; // Skip first uint (thread index)
                            }
                            match &p.ty {
                                Type::Array { inner, .. } => {
                                    let elem_ty = self.emit_type(inner);
                                    self.push(&format!(
                                        "layout(set = 0, binding = {}) buffer DataBuffer {{ {} data[]; }};\n",
                                        binding, elem_ty
                                    ));
                                    binding += 1;
                                }
                                Type::Uint | Type::Int => {
                                    let ty_str = self.emit_type(&p.ty);
                                    self.push(&format!(
                                        "layout(set = 0, binding = {}) uniform Params {{ {} {}; }};\n",
                                        binding, ty_str, p.name
                                    ));
                                    binding += 1;
                                }
                                _ => {}
                            }
                        }

                        self.push("\nvoid main() {\n");
                        self.indent += 1;
                        self.push_indent();
                        self.push(&format!(
                            "uint {} = gl_GlobalInvocationID.x;\n",
                            first_param_name
                        ));

                        for stmt in body {
                            self.emit_stmt(stmt);
                        }

                        self.indent -= 1;
                        self.push("}\n");
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

    fn generate(&mut self, program: &Program) -> GlslOutput {
        let mut vertex_out = None;
        let mut fragment_out = None;
        let mut compute_out = None;

        for item in program {
            if let Item::FnDecl { stage, .. } = item {
                match stage {
                    Some(ShaderStage::Vertex) => {
                        let mut cg = Codegen::new();
                        cg.emit_item(item);
                        vertex_out = Some(cg.output);
                    }
                    Some(ShaderStage::Fragment) => {
                        let mut cg = Codegen::new();
                        cg.emit_item(item);
                        fragment_out = Some(cg.output);
                    }
                    Some(ShaderStage::Kernel) => {
                        let mut cg = Codegen::new();
                        cg.emit_item(item);
                        compute_out = Some(cg.output);
                    }
                    _ => {}
                }
            }
        }

        GlslOutput {
            vertex: vertex_out,
            fragment: fragment_out,
            compute: compute_out,
        }
    }
}

pub fn generate(program: &Program) -> GlslOutput {
    let mut codegen = Codegen::new();
    codegen.generate(program)
}
