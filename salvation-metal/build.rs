fn main() {
    // macOS일 때만 빌드
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "macos" {
        return;
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
}
