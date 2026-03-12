use colored::Colorize;
use crate::backend::types::BackendTarget;

#[derive(Debug, Clone)]
pub struct BackendInfo {
    pub kind: BackendTarget,
    pub available: bool,
    pub version: Option<String>,
    pub devices: Vec<String>,
    pub compiler_path: Option<String>,
}

/// Probe all backends and return information about each
pub fn probe_all() -> Vec<BackendInfo> {
    vec![probe_cuda(), probe_rocm(), probe_metal(), probe_vulkan()]
}

/// Auto-detect the best available backend
pub fn auto_detect() -> BackendTarget {
    if probe_cuda().available   { return BackendTarget::Cuda; }
    if probe_rocm().available   { return BackendTarget::Rocm; }
    if probe_metal().available  { return BackendTarget::Metal; }
    if probe_vulkan().available { return BackendTarget::Vulkan; }
    eprintln!("{}", "Warning: No GPU backend detected. Defaulting to Vulkan.".yellow());
    BackendTarget::Vulkan
}

/// Print a formatted detection report to stdout
pub fn detect_and_report() {
    println!("{}\n", "── Backend Detection ─────────────────────────".cyan().bold());
    for info in &probe_all() {
        let status = if info.available {
            "✓  AVAILABLE".green().bold()
        } else {
            "✗  not found".red()
        };
        println!("  {:10} {}", format!("{:?}", info.kind).cyan(), status);
        if let Some(ver)  = &info.version       { println!("             version  : {ver}"); }
        if let Some(path) = &info.compiler_path { println!("             compiler : {path}"); }
        for dev in &info.devices                { println!("             device   : {dev}"); }
        println!();
    }
    println!("  {} {:?}\n", "Recommended backend:".yellow().bold(), auto_detect());
}

// ─────────────────────────────────────────────────────────────────────────────

fn probe_cuda() -> BackendInfo {
    let mut info = BackendInfo {
        kind: BackendTarget::Cuda,
        available: false, version: None, devices: Vec::new(), compiler_path: None,
    };
    if let Ok(nvcc) = which::which("nvcc") {
        info.compiler_path = Some(nvcc.display().to_string());
        info.available = true;
        if let Ok(out) = std::process::Command::new("nvcc").arg("--version").output() {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Some(line) = s.lines().find(|l| l.contains("release")) {
                info.version = Some(line.trim().to_string());
            }
        }
    }
    if let Ok(out) = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name,memory.total", "--format=csv,noheader"])
        .output()
    {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if !line.trim().is_empty() {
                info.devices.push(line.trim().to_string());
                info.available = true;
            }
        }
    }
    info
}

fn probe_rocm() -> BackendInfo {
    let mut info = BackendInfo {
        kind: BackendTarget::Rocm,
        available: false, version: None, devices: Vec::new(), compiler_path: None,
    };
    if let Ok(hipcc) = which::which("hipcc") {
        info.compiler_path = Some(hipcc.display().to_string());
        info.available = true;
        if let Ok(out) = std::process::Command::new("hipcc").arg("--version").output() {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Some(line) = s.lines().find(|l| l.contains("HIP")) {
                info.version = Some(line.trim().to_string());
            }
        }
    }
    if let Ok(out) = std::process::Command::new("rocm-smi").arg("--showproductname").output() {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let line = line.trim();
            if line.contains("GPU") && !line.is_empty() {
                info.devices.push(line.to_string());
                info.available = true;
            }
        }
    }
    info
}

fn probe_metal() -> BackendInfo {
    let mut info = BackendInfo {
        kind: BackendTarget::Metal,
        available: false, version: None, devices: Vec::new(), compiler_path: None,
    };
    #[cfg(target_os = "macos")]
    {
        if let Ok(metal) = which::which("metal") {
            info.compiler_path = Some(metal.display().to_string());
            info.available = true;
        } else if let Ok(out) = std::process::Command::new("xcrun")
            .args(["--sdk", "macosx", "--find", "metal"])
            .output()
        {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !path.is_empty() {
                info.compiler_path = Some(path);
                info.available = true;
            }
        }
        if let Ok(out) = std::process::Command::new("sw_vers").arg("-productVersion").output() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            info.version = Some(format!("Metal on macOS {s}"));
        }
        if let Ok(out) = std::process::Command::new("system_profiler")
            .args(["SPDisplaysDataType"])
            .output()
        {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let line = line.trim();
                if line.starts_with("Chipset Model:") || line.starts_with("Type:") {
                    let val = line.splitn(2, ':').nth(1).unwrap_or("").trim().to_string();
                    if !val.is_empty() { info.devices.push(val); }
                }
            }
        }
    }
    info
}

fn probe_vulkan() -> BackendInfo {
    let mut info = BackendInfo {
        kind: BackendTarget::Vulkan,
        available: false, version: None, devices: Vec::new(), compiler_path: None,
    };
    if let Ok(glslc) = which::which("glslc") {
        info.compiler_path = Some(glslc.display().to_string());
        info.available = true;
        if let Ok(out) = std::process::Command::new("glslc").arg("--version").output() {
            let s = String::from_utf8_lossy(&out.stdout);
            info.version = Some(s.lines().next().unwrap_or("").trim().to_string());
        }
    } else if let Ok(glslang) = which::which("glslangValidator") {
        info.compiler_path = Some(glslang.display().to_string());
        info.available = true;
    }
    if let Ok(out) = std::process::Command::new("vulkaninfo").args(["--summary"]).output() {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if line.trim_start().starts_with("deviceName") {
                let name = line.split('=').nth(1).unwrap_or("").trim().to_string();
                if !name.is_empty() { info.devices.push(name); info.available = true; }
            }
        }
    }
    // macOS: MoltenVK via Vulkan SDK
    #[cfg(target_os = "macos")]
    {
        let vk_paths = [
            "/usr/local/lib/libvulkan.dylib",
            "$HOME/VulkanSDK/macOS/lib/libvulkan.dylib",
        ];
        for path in &vk_paths {
            if std::path::Path::new(path).exists() { info.available = true; break; }
        }
    }
    #[cfg(target_os = "linux")]
    {
        for path in &["/usr/lib/libvulkan.so.1", "/usr/local/lib/libvulkan.so.1"] {
            if std::path::Path::new(path).exists() { info.available = true; break; }
        }
    }
    info
}
