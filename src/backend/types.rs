/// Shared types used across the compiler and runtime.
/// BackendTarget lives here so both lib crate and binary crate can use it.

use clap::ValueEnum;

#[derive(Clone, ValueEnum, Debug, PartialEq)]
pub enum BackendTarget {
    /// Vulkan SPIR-V (cross-platform)
    Vulkan,
    /// Apple Metal (macOS/iOS)
    Metal,
    /// NVIDIA CUDA
    Cuda,
    /// AMD ROCm/HIP
    Rocm,
    /// Auto-detect best available backend
    Auto,
}

impl BackendTarget {
    pub fn extension(&self) -> &'static str {
        match self {
            BackendTarget::Vulkan => "spv",
            BackendTarget::Metal  => "metallib",
            BackendTarget::Cuda   => "ptx",
            BackendTarget::Rocm   => "hsaco",
            BackendTarget::Auto   => "bin",
        }
    }
}
