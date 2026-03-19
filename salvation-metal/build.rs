use std::{
    env, fs, path::Path,
};

fn main() {
    metal_manager().expect("Failed to build salvation to metal");
    codegen_manager().expect("Failed to make metal code from salvation")
}

fn metal_manager() -> std::io::Result<()> {
    // macOS일 때만 빌드
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "macos" {
        return Ok(());
    }

    cc::Build::new()
        .file("src/ffi/metal_info.m")
        .flag("-fobjc-arc")   // ARC 활성화
        .compile("metal_info"); // libmetal_info.a 생성

    // Metal 프레임워크 링크
    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=framework=Foundation");

    // .m 파일이 바뀌면 rebuild
    println!("cargo:rerun-if-changed=src/ffi/metal_info.m");
    
    // 여기서 부터 codegen
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

    // 예: 구조체 자동 생성
    code.push_str("\n#[derive(Debug)]\n");
    code.push_str("pub struct AutoConfig {\n");
    for (name, _) in &config_items {
        let field = name.to_lowercase();
        code.push_str(&format!("    pub {}: &'static str,\n", field));
    }
    code.push_str("}\n");

    fs::write(&dest_path, code).unwrap();

    // 이 파일이 변경될 때만 재실행
    println!("cargo:rerun-if-changed=build.rs");
    
    Ok(())
}

fn codegen_manager() -> std::io::Result<()> {
    Ok(())
}
