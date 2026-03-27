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
        write!(f, "[cuda checker] {}", self.message)
    }
}

pub struct Checker;

impl Checker {
    pub fn new() -> Self {
        Checker
    }

    pub fn check(self, program: &Program) -> Result<(), Vec<CheckError>> {
        let mut errors = Vec::new();

        // CUDA reserved words
        let reserved_cuda = [
            "threadIdx", "blockIdx", "blockDim", "gridDim", "warpSize",
            "atomicAdd", "atomicSub", "atomicExch", "atomicCAS",
            "__syncthreads", "__syncwarp", "dim3", "cudaMalloc",
            "cudaFree", "cudaMemcpy", "cudaDeviceSynchronize",
            "__device__", "__shared__", "__global__", "__host__",
            "__constant__",
        ];

        for item in program {
            if let Item::FnDecl { name, stage, .. } = item {
                // CUDA only supports @kernel
                if let Some(st) = stage {
                    match st {
                        ShaderStage::Kernel => {
                            // OK
                        }
                        ShaderStage::Vertex | ShaderStage::Fragment => {
                            errors.push(CheckError::new(format!(
                                "CUDA는 @vertex/@fragment를 지원하지 않습니다. @kernel만 사용하세요."
                            )));
                        }
                    }
                }

                // Check reserved names
                if reserved_cuda.contains(&name.as_str()) {
                    errors.push(CheckError::new(format!(
                        "함수명 '{}' 은 CUDA 예약어입니다",
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
