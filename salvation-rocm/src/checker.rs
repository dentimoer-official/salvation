use salvation_core::compiler::ast::types::{Item, Program, ShaderStage};

#[derive(Debug, Clone)]
pub struct CheckError {
    pub message: String,
}

impl CheckError {
    pub fn new(msg: impl Into<String>) -> Self {
        CheckError { message: msg.into() }
    }
}

impl std::fmt::Display for CheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[rocm checker] {}", self.message)
    }
}

pub struct Checker;

impl Checker {
    pub fn new() -> Self {
        Checker
    }

    pub fn check(self, program: &Program) -> Result<(), Vec<CheckError>> {
        let mut errors = Vec::new();

        // ROCm/HIP reserved words
        let reserved_hip = [
            "hipMalloc", "hipFree", "hipMemcpy", "hipDeviceSynchronize",
            "hipLaunchKernelGGL", "hipBlockIdx_x", "hipThreadIdx_x",
            "hipBlockDim_x", "__hip_device__", "atomicAdd",
        ];

        for item in program {
            if let Item::FnDecl { name, stage, .. } = item {
                // ROCm/HIP only supports @kernel
                if let Some(st) = stage {
                    match st {
                        ShaderStage::Kernel => {
                            // OK
                        }
                        ShaderStage::Vertex | ShaderStage::Fragment => {
                            errors.push(CheckError::new(format!(
                                "ROCm은 @vertex/@fragment를 지원하지 않습니다. @kernel만 사용하세요."
                            )));
                        }
                    }
                }

                // Check reserved names
                if reserved_hip.contains(&name.as_str()) {
                    errors.push(CheckError::new(format!(
                        "함수명 '{}' 은 ROCm 예약어입니다",
                        name
                    )));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
