#[cfg(any(target_os = "windows",  target_os = "linux", target_os = "cuda"))]
pub mod cuda;

#[cfg(any(target_os = "macos",  target_os = "ios",  target_os = "tvos",  target_os = "watchos",  target_os = "visionos"))]
pub mod metal;

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub mod rocm;

#[cfg(any(target_os = "windows",  target_os = "linux", target_os = "android",  target_os = "redox"))]
pub mod vulkan;
