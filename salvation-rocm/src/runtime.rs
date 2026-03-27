use std::path::{Path, PathBuf};
use std::process::Command;

pub fn build_and_run(out_dir: &Path) -> Result<(), String> {
    let hip_file = out_dir.join("salvation.hip.cpp");
    let app_path = out_dir.join("salvation_app");

    run_cmd(
        "hipcc",
        &["-std=c++17", hip_file.to_str().unwrap(), "-o", app_path.to_str().unwrap()],
        "Compile HIP code",
    )?;

    let abs_app = app_path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve app path: {}\nFile: {}", e, app_path.display()))?;

    eprintln!("  Running: {}", abs_app.display());

    let status = Command::new(&abs_app)
        .current_dir(out_dir.canonicalize().unwrap_or(out_dir.to_path_buf()))
        .status()
        .map_err(|e| format!("Failed to run: {}", e))?;

    if !status.success() {
        return Err(format!("App exited with failure: {}", status));
    }

    Ok(())
}

pub fn build_only(out_dir: &Path) -> Result<PathBuf, String> {
    let hip_file = out_dir.join("salvation.hip.cpp");
    let app_path = out_dir.join("salvation_app");

    run_cmd(
        "hipcc",
        &["-std=c++17", hip_file.to_str().unwrap(), "-o", app_path.to_str().unwrap()],
        "Compile HIP code",
    )?;

    Ok(app_path)
}

fn run_cmd(prog: &str, args: &[&str], label: &str) -> Result<(), String> {
    eprintln!("  {}...", label);

    let output = Command::new(prog)
        .args(args)
        .output()
        .map_err(|e| format!("'{}' failed to run: {}\nMake sure ROCm and hipcc are installed", prog, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{} failed:\n{}", label, stderr));
    }

    Ok(())
}
