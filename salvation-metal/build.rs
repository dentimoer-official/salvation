use std::{
    env, 
    fs, 
    path::Path,
};

type BuildResult = Result<(), Box<dyn std::error::Error>>;

fn main() {
    metal_manager().expect("Failed to build metal");
    ffi_manager().expect("Failed to build ffi");
}

fn metal_manager() -> std::io::Result<()> {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "macos" {
        return Ok(());
    }

    cc::Build::new()
        .file("src/ffi/metal_info.m")
        .flag("-fobjc-arc")
        .compile("metal_info");

    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rerun-if-changed=src/ffi/metal_info.m");

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated.rs");
    let mut code = String::new();

    let config_items = vec![
        ("MAX_CONNECTIONS", "100u32"),
        ("TIMEOUT_MS", "5000u64"),
        ("APP_NAME", r#""my_app""#),
    ];

    for (name, value) in &config_items {
        code.push_str(&format!(
            "pub const {}: {} = {};\n",
            name,
            if value.contains('"') { "&str" } else { value.split_once('u').map_or("i32", |_| "u64") },
            value
        ));
    }

    code.push_str("\n#[derive(Debug)]\n");
    code.push_str("pub struct AutoConfig {\n");
    for (name, _) in &config_items {
        code.push_str(&format!("    pub {}: &'static str,\n", name.to_lowercase()));
    }
    code.push_str("}\n");

    fs::write(&dest_path, code).unwrap();
    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}

fn ffi_manager() -> BuildResult {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "macos" {
        return Ok(());
    }

    // Objective-C 빌드
    cc::Build::new()
        .file("src/ffi/ffi_manager.m")
        .flag("-fobjc-arc")
        .flag("-fmodules")
        .compile("ffi_manager");

    // Objective-C++ 빌드
    cc::Build::new()
        .file("src/ffi/ffi_manager.mm")
        .flag("-fobjc-arc")
        .flag("-fmodules")
        .flag("-std=c++17")     // C++ 표준 지정
        .cpp(true)              // C++ 모드 활성화
        .compile("ffi_manager_cpp");

    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=dylib=c++"); // libc++ 링크 (C++ 런타임)

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let header_path = format!("{}/src/ffi/ffi_manager.h", manifest_dir);

    let target = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let clang_target = match target.as_str() {
        "aarch64" => "--target=aarch64-apple-macosx",
        "x86_64"  => "--target=x86_64-apple-macosx",
        _         => "--target=aarch64-apple-macosx",
    };

    let bindings = bindgen::Builder::default()
        .header(&header_path)
        .clang_arg(clang_target)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .map_err(|e| e.to_string())?;

    let out_dir = env::var("OUT_DIR")?;
    bindings.write_to_file(Path::new(&out_dir).join("ffi_manager_bindings.rs"))?;

    println!("cargo:rerun-if-changed=src/ffi/ffi_manager.m");
    println!("cargo:rerun-if-changed=src/ffi/ffi_manager.mm");
    println!("cargo:rerun-if-changed=src/ffi/ffi_manager.h");

    Ok(())
}
