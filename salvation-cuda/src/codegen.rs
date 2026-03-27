use salvation_core::compiler::ast::types::{
    Block, BinOpKind, Expr, Item, Param, Program, ShaderStage, Stmt, Type, UnaryOpKind,
};

struct Codegen {
    output: String,
    indent: usize,
}

impl Codegen {
    fn new() -> Self {
        Codegen {
            output: String::new(),
            indent: 0,
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
            Type::Int => "int32_t".into(),
            Type::Uint => "uint32_t".into(),
            Type::Float => "float".into(),
            Type::Float2 => "float2".into(),
            Type::Float3 => "float3".into(),
            Type::Float4 => "float4".into(),
            Type::Mat2x2 | Type::Mat2x3 | Type::Mat2x4 => "float2".into(),
            Type::Mat3x2 | Type::Mat3x3 | Type::Mat3x4 => "float3".into(),
            Type::Mat4x2 | Type::Mat4x3 | Type::Mat4x4 => "float4".into(),
            Type::Texture2D => "cudaTextureObject_t".into(),
            Type::Sampler => "cudaTextureObject_t".into(),
            Type::Array { inner, .. } => {
                format!("{}*", self.emit_type(inner))
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
            Expr::Ident(s) => self.push(s),
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
                    Some(ShaderStage::Kernel) => {
                        let ret_str = ret_ty
                            .as_ref()
                            .map(|t| self.emit_type(t))
                            .unwrap_or_else(|| "void".into());

                        // Skip first param (idx) in signature
                        // Note: emit_type already returns "float*" for Array types
                        let kernel_params: Vec<_> = params.iter().skip(1).collect();
                        let params_str = kernel_params
                            .iter()
                            .map(|p| format!("{} {}", self.emit_type(&p.ty), p.name))
                            .collect::<Vec<_>>()
                            .join(", ");

                        self.push(&format!("__global__ {} {}({}) ", ret_str, name, params_str));

                        // Create block with idx variable already declared
                        self.push("{\n");
                        self.indent += 1;

                        let idx_name = params
                            .first()
                            .map(|p| p.name.as_str())
                            .unwrap_or("idx");
                        self.push_indent();
                        self.push(&format!(
                            "uint32_t {} = blockIdx.x * blockDim.x + threadIdx.x;\n",
                            idx_name
                        ));

                        for stmt in body {
                            self.emit_stmt(stmt);
                        }

                        self.indent -= 1;
                        self.push("}\n\n");
                    }

                    None => {
                        let ret_str = ret_ty
                            .as_ref()
                            .map(|t| self.emit_type(t))
                            .unwrap_or_else(|| "void".into());
                        let params_str = self.emit_params(params);
                        self.push(&format!("__device__ {} {}({}) ", ret_str, name, params_str));
                        self.emit_block(body);
                        self.newline();
                    }

                    _ => {} // Vertex/Fragment not supported in CUDA
                }
            }
        }
    }

    fn generate(&mut self, program: &Program) -> String {
        self.push("#include <cuda_runtime.h>\n");
        self.push("#include <device_launch_parameters.h>\n");
        self.push("#include <cstdint>\n");
        self.push("#include <cmath>\n\n");

        for item in program {
            self.emit_item(item);
        }

        self.output.clone()
    }
}

pub fn generate(program: &Program) -> String {
    let mut codegen = Codegen::new();
    codegen.generate(program)
}
