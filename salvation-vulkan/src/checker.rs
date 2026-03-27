use salvation_core::compiler::ast::types::{Item, Program};

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
        write!(f, "[vulkan checker] {}", self.message)
    }
}

pub struct Checker;

impl Checker {
    pub fn new() -> Self {
        Checker
    }

    pub fn check(self, program: &Program) -> Result<(), Vec<CheckError>> {
        let mut errors = Vec::new();

        // GLSL reserved words that conflict with shader code
        let reserved_glsl = [
            "gl_Position", "gl_FragCoord", "gl_GlobalInvocationID",
            "gl_LocalInvocationID", "gl_WorkGroupID", "uniform", "in", "out",
            "inout", "flat", "smooth", "layout", "location", "binding", "set",
            "mat2", "mat3", "mat4", "vec2", "vec3", "vec4",
            "ivec2", "ivec3", "ivec4", "uvec2", "uvec3", "uvec4",
            "bvec2", "bvec3", "bvec4", "sampler2D", "sampler3D", "samplerCube",
            "texture", "discard", "precision", "lowp", "mediump", "highp",
            "attribute", "varying",
        ];

        // Check for reserved word conflicts and stage restrictions
        for item in program {
            if let Item::FnDecl { name, stage: _, .. } = item {
                // Vulkan supports all three stages
                // But check for reserved names
                if reserved_glsl.contains(&name.as_str()) {
                    errors.push(CheckError::new(format!(
                        "함수명 '{}' 은 GLSL 예약어입니다",
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
