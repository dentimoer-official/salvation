use salvation_core::compiler::ast::types::{Item, Param, Program, ShaderStage};

#[derive(Debug, Default)]
pub struct ShaderInfo {
    pub kernel_fn: Option<String>,
    pub kernel_params: Vec<Param>,
}

pub fn analyze(program: &Program) -> ShaderInfo {
    let mut info = ShaderInfo::default();

    for item in program {
        if let Item::FnDecl { stage, name, params, .. } = item {
            if let Some(ShaderStage::Kernel) = stage {
                info.kernel_fn = Some(name.clone());
                info.kernel_params = params.clone();
            }
        }
    }

    info
}

pub fn gen_main_cu(info: &ShaderInfo) -> String {
    let kernel_name = info.kernel_fn.as_ref().map(|s| s.as_str()).unwrap_or("kernel");

    let mut code = String::new();

    code.push_str("int main() {\n");
    code.push_str("    const uint32_t count = 8;\n");
    code.push_str("    float h_data[] = {1.0f, 2.0f, 3.0f, 4.0f, 5.0f, 6.0f, 7.0f, 8.0f};\n");
    code.push_str("    \n");
    code.push_str("    float* d_data;\n");
    code.push_str("    cudaMalloc((void**)&d_data, count * sizeof(float));\n");
    code.push_str("    cudaMemcpy(d_data, h_data, count * sizeof(float), cudaMemcpyHostToDevice);\n");
    code.push_str("    \n");
    code.push_str("    dim3 block(256);\n");
    code.push_str("    dim3 grid((count + 255) / 256);\n");
    code.push_str(&format!("    {}<<<grid, block>>>(d_data, count);\n", kernel_name));
    code.push_str("    cudaDeviceSynchronize();\n");
    code.push_str("    \n");
    code.push_str("    cudaMemcpy(h_data, d_data, count * sizeof(float), cudaMemcpyDeviceToHost);\n");
    code.push_str("    \n");
    code.push_str(&format!(
        "    printf(\"[Salvation] {} -> data:\\n\");\n",
        kernel_name
    ));
    code.push_str("    for (uint32_t i = 0; i < count; i++) {\n");
    code.push_str("        printf(\"  [%u] = %.1f\\n\", i, h_data[i]);\n");
    code.push_str("    }\n");
    code.push_str("    \n");
    code.push_str("    cudaFree(d_data);\n");
    code.push_str("    return 0;\n");
    code.push_str("}\n");

    code
}
