use crate::metal::parser::{Program, Decl, Stmt, Expr, Type, AddressSpace, BinOp, UnaryOp, Param, StructField};
use crate::metal::module::ModuleLoader;

pub struct Codegen {
    output: String,
    indent: usize,
}

impl Codegen {
    pub fn new() -> Self {
        Self { output: String::new(), indent: 0 }
    }
    
    pub fn generate_with_modules(mut self, program: &Program, loader: &ModuleLoader) -> String {
        self.emit_line("#include <metal_stdlib>");
        self.emit_line("using namespace metal;");
        self.emit_line("");
    
        // import된 모듈의 struct/const 먼저 출력
        for decl in &program.decls {
            if let Decl::Import { path, .. } = decl {
                if let Some(module) = loader.get(path) {
                    for mdecl in &module.program.decls {
                        match mdecl {
                            Decl::Struct { is_pub: true, .. } => self.gen_decl(mdecl),
                            Decl::Const  { is_pub: true, .. } => self.gen_decl(mdecl),
                            _ => {}
                        }
                    }
                }
            }
        }
    
        // 메인 파일
        for decl in &program.decls {
            if !matches!(decl, Decl::Import { .. }) {
                self.gen_decl(decl);
            }
        }
    
        self.output
    }

    pub fn generate(mut self, program: &Program) -> String {
        self.emit_line("#include <metal_stdlib>");
        self.emit_line("using namespace metal;");
        self.emit_line("");
        for decl in &program.decls {
            self.gen_decl(decl);
        }
        self.output
    }

    fn emit(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn emit_line(&mut self, s: &str) {
        let indent = "    ".repeat(self.indent);
        self.output.push_str(&indent);
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn emit_indent(&mut self) {
        self.output.push_str(&"    ".repeat(self.indent));
    }

    fn gen_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::KernelFn { name, params, body, .. } => self.gen_kernel_fn(name, params, body),
            Decl::Struct { name, fields, .. } => self.gen_struct(name, fields),
            Decl::Const { name, ty, value, .. } => self.gen_const(name, ty, value),
             Decl::Import { .. } => {},
        }
    }

    fn gen_struct(&mut self, name: &str, fields: &[StructField]) {
        self.emit_line(&format!("struct {} {{", name));
        self.indent += 1;
        for field in fields {
            self.emit_line(&format!("{} {};", self.msl_type(&field.ty), field.name));
        }
        self.indent -= 1;
        self.emit_line("};");
        self.emit_line("");
    }

    fn gen_const(&mut self, name: &str, ty: &Type, value: &Expr) {
        self.emit(&format!("constant {} {} = ", self.msl_type(ty), name));
        self.gen_expr(value);
        self.emit_line(";");
        self.emit_line("");
    }

    fn gen_kernel_fn(&mut self, name: &str, params: &[Param], body: &[Stmt]) {
        self.emit("kernel void ");
        self.emit(name);
        self.emit_line("(");
        self.indent += 1;

        let mut buf_idx = 0u32;
        for param in params.iter() {
            self.emit_indent();
            let (type_str, attr_str) = self.msl_param_parts(&param.ty, buf_idx);
            self.emit(&format!("{} {} {},\n", type_str, param.name, attr_str));
            buf_idx += 1;
        }

        self.emit_line("uint3 thread_pos [[thread_position_in_grid]],");
        self.emit_line("uint3 threadgroup_pos [[threadgroup_position_in_grid]],");
        self.emit_line("uint3 threads_per_threadgroup [[threads_per_threadgroup]]");

        self.indent -= 1;
        self.emit_line(") {");
        self.indent += 1;

        self.emit_line("// built-ins");
        self.emit_line("uint3 thread = thread_pos;");
        self.emit_line("uint3 threadgroup_id = threadgroup_pos;");
        self.emit_line("");

        for stmt in body { self.gen_stmt(stmt); }

        self.indent -= 1;
        self.emit_line("}");
        self.emit_line("");
    }

    fn msl_param_parts(&self, ty: &Type, buf_idx: u32) -> (String, String) {
        match ty {
            Type::Array { space, elem, mutable } => {
                let space_str = match space {
                    AddressSpace::Device      => "device",
                    AddressSpace::Threadgroup => "threadgroup",
                    AddressSpace::Constant    => "constant",
                    AddressSpace::Private     => "thread",
                };
                let const_str = if *mutable { "" } else { "const " };
                let type_str = format!("{} {}{}*", space_str, const_str, self.msl_type(elem));
                let attr_str = format!("[[buffer({})]]", buf_idx);
                (type_str, attr_str)
            },
            Type::ArrayN { space, elem, mutable, size } => {
                let space_str = match space {
                    AddressSpace::Device      => "device",
                    AddressSpace::Threadgroup => "threadgroup",
                    AddressSpace::Constant    => "constant",
                    AddressSpace::Private     => "thread",
                };
                let const_str = if *mutable { "" } else { "const " };
                let type_str = format!("{} {}{}[{}]", space_str, const_str, self.msl_type(elem), size);
                let attr_str = format!("[[buffer({})]]", buf_idx);
                (type_str, attr_str)
            }
            _ => (self.msl_type(ty), String::new()),
        }
    }

    fn msl_type(&self, ty: &Type) -> String {
        match ty {
            Type::F32        => "float".to_string(),
            Type::F16        => "half".to_string(),
            Type::I32        => "int".to_string(),
            Type::U32        => "uint".to_string(),
            Type::Bool       => "bool".to_string(),
            Type::Void       => "void".to_string(),
            Type::Mat4       => "float4x4".to_string(),
            Type::Named(n)   => n.clone(),
            Type::Vec2(t)    => format!("{}2", self.msl_base_vec(t)),
            Type::Vec3(t)    => format!("{}3", self.msl_base_vec(t)),
            Type::Vec4(t)    => format!("{}4", self.msl_base_vec(t)),
            Type::Array { elem, .. } => format!("{}*", self.msl_type(elem)),
            Type::ArrayN { elem, size, .. } => format!("{}[{}]", self.msl_type(elem), size),
            Type::Qualified(_, item) => item.clone(),
        }
    }

    fn msl_base_vec(&self, ty: &Type) -> &str {
        match ty {
            Type::F32  => "float",
            Type::F16  => "half",
            Type::I32  => "int",
            Type::U32  => "uint",
            Type::Bool => "bool",
            _ => "float",
        }
    }

    fn gen_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, ty, value, .. } => {
                self.emit_indent();
                match ty {
                    // threadgroup [f32; 32] → threadgroup float shared[32];
                    Some(Type::ArrayN { space: AddressSpace::Threadgroup, elem, mutable: _, size }) => {
                        self.emit(&format!(
                            "threadgroup {} {}[{}];\n",
                            self.msl_type(elem), name, size
                        ));
                        return;
                    }
                    Some(t) => self.emit(&self.msl_type(t)),
                    None    => self.emit("auto"),
                }
                self.emit(&format!(" {} = ", name));
                self.gen_expr(value);
                self.emit(";\n");
            }

            Stmt::Assign { target, value, .. } => {
                self.emit_indent();
                self.gen_expr(target);
                self.emit(" = ");
                self.gen_expr(value);
                self.emit(";\n");
            }

            Stmt::Return(expr, _) => {
                self.emit_indent();
                if let Some(e) = expr {
                    self.emit("return ");
                    self.gen_expr(e);
                    self.emit(";\n");
                } else {
                    self.emit("return;\n");
                }
            }

            Stmt::If { cond, then, else_, .. } => {
                self.emit_indent();
                self.emit("if (");
                self.gen_expr(cond);
                self.emit(") {\n");
                self.indent += 1;
                for s in then { self.gen_stmt(s); }
                self.indent -= 1;
                if let Some(eb) = else_ {
                    self.emit_line("} else {");
                    self.indent += 1;
                    for s in eb { self.gen_stmt(s); }
                    self.indent -= 1;
                }
                self.emit_line("}");
            }

            Stmt::For { var, from, to, body, .. } => {
                self.emit_indent();
                self.emit(&format!("for (uint {} = ", var));
                self.gen_expr(from);
                self.emit(&format!("; {} < ", var));
                self.gen_expr(to);
                self.emit(&format!("; {}++) {{\n", var));
                self.indent += 1;
                for s in body { self.gen_stmt(s); }
                self.indent -= 1;
                self.emit_line("}");
            }

            Stmt::Expr(e, _) => {
                self.emit_indent();
                self.gen_expr(e);
                self.emit(";\n");
            }
        }
    }

    fn gen_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::IntLit(n, _)   => self.emit(&n.to_string()),
            Expr::FloatLit(f, _) => self.emit(&format!("{}f", f)),
            Expr::Bool(b, _)     => self.emit(if *b { "true" } else { "false" }),
            //Expr::Ident(name, _) => self.emit(name),
            Expr::Ident(name, _) => {
                // math::PI → PI (MSL에서는 네임스페이스 없음)
                let msl_name = if name.contains("::") {
                    name.splitn(2, "::").nth(1).unwrap_or(name)
                } else {
                    name
                };
                self.emit(msl_name);
            }

            Expr::Index { array, index, .. } => {
                self.gen_expr(array);
                self.emit("[");
                self.gen_expr(index);
                self.emit("]");
            }

            Expr::Field { object, field, .. } => {
                self.gen_expr(object);
                self.emit(".");
                self.emit(field);
            }

            Expr::BinOp { op, lhs, rhs, .. } => {
                self.emit("(");
                self.gen_expr(lhs);
                self.emit(&format!(" {} ", self.msl_binop(op)));
                self.gen_expr(rhs);
                self.emit(")");
            }

            Expr::UnaryOp { op, expr, .. } => {
                let op_str = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                };
                self.emit(op_str);
                self.gen_expr(expr);
            }

            Expr::Call { name, args, .. } => {
                let msl_name = match name.as_str() {
                    "barrier"            => "threadgroup_barrier(mem_flags::mem_threadgroup)",
                    "simd_sum"           => "simd_sum",
                    "simd_min"           => "simd_min",
                    "simd_max"           => "simd_max",
                    "simd_product"       => "simd_product",
                    "simd_prefix_sum"    => "simd_prefix_inclusive_sum",
                    "simd_prefix_min"    => "simd_prefix_inclusive_min",
                    "simd_prefix_max"    => "simd_prefix_inclusive_max",
                    "simd_broadcast"     => "simd_broadcast",
                    "simd_shuffle_down"  => "simd_shuffle_down",
                    "simd_shuffle_up"    => "simd_shuffle_up",
                    "simd_all"           => "simd_all",
                    "simd_any"           => "simd_any",
                    other                => other,
                };
                if name == "barrier" {
                    self.emit(msl_name);
                    return;
                }
                self.emit(msl_name);
                self.emit("(");
                for (i, arg) in args.iter().enumerate() {
                    self.gen_expr(arg);
                    if i < args.len() - 1 { self.emit(", "); }
                }
                self.emit(")");
            }
        }
    }

    fn msl_binop(&self, op: &BinOp) -> &str {
        match op {
            BinOp::Add => "+",  BinOp::Sub => "-",
            BinOp::Mul => "*",  BinOp::Div => "/",
            BinOp::Mod => "%",  BinOp::Eq  => "==",
            BinOp::Ne  => "!=", BinOp::Lt  => "<",
            BinOp::Gt  => ">",  BinOp::Le  => "<=",
            BinOp::Ge  => ">=", BinOp::And => "&&",
            BinOp::Or  => "||",
        }
    }
}