use std::path::{Path, PathBuf};
use std::process::Command;

pub fn build_and_run(out_dir: &Path) -> Result<(), String> {
    let comp_path = out_dir.join("shader.comp");
    let vert_path = out_dir.join("shader.vert");
    let frag_path = out_dir.join("shader.frag");
    let main_cpp = out_dir.join("main.cpp");
    let app_path = out_dir.join("salvation_app");

    // Check which shaders exist
    let has_vert = vert_path.exists();
    let has_frag = frag_path.exists();
    let has_comp = comp_path.exists();

    // Compile shaders
    if has_comp {
        run_cmd(
            "glslc",
            &[
                comp_path.to_str().unwrap(),
                "-o",
                out_dir.join("shader.comp.spv").to_str().unwrap(),
            ],
            "Compile compute shader",
        )?;
    }

    if has_vert {
        run_cmd(
            "glslc",
            &[
                vert_path.to_str().unwrap(),
                "-o",
                out_dir.join("shader.vert.spv").to_str().unwrap(),
            ],
            "Compile vertex shader",
        )?;
    }

    if has_frag {
        run_cmd(
            "glslc",
            &[
                frag_path.to_str().unwrap(),
                "-o",
                out_dir.join("shader.frag.spv").to_str().unwrap(),
            ],
            "Compile fragment shader",
        )?;
    }

    // Build host code
    let mut cmd_args = vec!["-std=c++17"];
    cmd_args.push(main_cpp.to_str().unwrap());
    cmd_args.push("-o");
    cmd_args.push(app_path.to_str().unwrap());
    cmd_args.push("-lvulkan");

    if has_vert || has_frag {
        cmd_args.push("-lglfw");
    }

    run_cmd("g++", &cmd_args, "Compile C++ host code")?;

    // Run
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
    let comp_path = out_dir.join("shader.comp");
    let vert_path = out_dir.join("shader.vert");
    let frag_path = out_dir.join("shader.frag");

    let has_comp = comp_path.exists();
    let has_vert = vert_path.exists();
    let has_frag = frag_path.exists();

    if has_comp {
        run_cmd(
            "glslc",
            &[
                comp_path.to_str().unwrap(),
                "-o",
                out_dir.join("shader.comp.spv").to_str().unwrap(),
            ],
            "Compile compute shader",
        )?;
    }

    if has_vert {
        run_cmd(
            "glslc",
            &[
                vert_path.to_str().unwrap(),
                "-o",
                out_dir.join("shader.vert.spv").to_str().unwrap(),
            ],
            "Compile vertex shader",
        )?;
    }

    if has_frag {
        run_cmd(
            "glslc",
            &[
                frag_path.to_str().unwrap(),
                "-o",
                out_dir.join("shader.frag.spv").to_str().unwrap(),
            ],
            "Compile fragment shader",
        )?;
    }

    Ok(out_dir.join("shader.spv"))
}

fn run_cmd(prog: &str, args: &[&str], label: &str) -> Result<(), String> {
    eprintln!("  {}...", label);

    let output = Command::new(prog)
        .args(args)
        .output()
        .map_err(|e| format!("'{}' failed to run: {}\nMake sure glslc and g++ are installed", prog, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{} failed:\n{}", label, stderr));
    }

    Ok(())
}
