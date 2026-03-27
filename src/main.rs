use std::fs;
use std::path::Path;

use clap::{Parser as ClapParser, Subcommand};
use colored::Colorize;

use salvation_core::compiler::lexer::Lexer;
use salvation_core::compiler::parser::parser_testing::Parser;
use salvation_core::compiler::backend_resolver::BackendResolver;
use salvation_core::compiler::ast::types::Backend;

// ── CLI ─────────────────────────────────────────────────────

#[derive(ClapParser)]
#[command(
    name    = "salvation",
    version = "0.1.0",
    author  = "dentimoer-official <dentimoer@icloud.com>",
    about   = "Salvation GPU language compiler & runner",
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// .slvt 컴파일 → shaders.metal + common.h + main.mm 생성 후 즉시 실행
    /// fn main() 이 있어야 합니다
    Run {
        #[arg(value_name = "FILE")]
        file: String,

        /// 출력 디렉터리 (기본값: ./out)
        #[arg(short, long, default_value = "./out")]
        output: String,

        /// 상세 로그
        #[arg(short, long)]
        verbose: bool,
    },

    /// .slvt 컴파일 → 파일 생성만 (실행 안 함)
    Build {
        #[arg(value_name = "FILE")]
        file: String,

        #[arg(short, long, default_value = "./out")]
        output: String,

        #[arg(short, long)]
        verbose: bool,
    },

    /// 문법/백엔드 검사만 (파일 생성 없음)
    Check {
        #[arg(value_name = "FILE")]
        file: String,
    },
}

// ── 컴파일 파이프라인 ────────────────────────────────────────

struct CompileOutput {
    backend:        Backend,
    metal_src:      Option<String>,
    glsl_vert:      Option<String>,
    glsl_frag:      Option<String>,
    glsl_comp:      Option<String>,
    cuda_src:       Option<String>,
    rocm_src:       Option<String>,
    shader_types_h: Option<String>,   // Metal: CPU/GPU 공유 타입 헤더
    common_h:       Option<String>,
    main_mm:        Option<String>,
    main_cpp:       Option<String>,   // Vulkan
    main_hip:       Option<String>,   // ROCm
    has_main:       bool,
}

fn compile(src: &str, verbose: bool, _metallib_name: &str) -> Result<CompileOutput, String> {
    step(verbose, 1, 5, "렉싱");
    let tokens = Lexer::new(src).tokenize()
        .map_err(|e| format!("{} {}", "렉서 오류:".red().bold(), e))?;

    step(verbose, 2, 5, "파싱");
    let ast = Parser::new(tokens).parse()
        .map_err(|e| format!("{} {}", "파서 오류:".red().bold(), e))?;

    step(verbose, 3, 5, "백엔드 리졸빙");
    let resolved = BackendResolver::new().resolve(ast)
        .map_err(|errs| {
            errs.iter()
                .map(|e| format!("{} {}", "백엔드 오류:".red().bold(), e))
                .collect::<Vec<_>>()
                .join("\n")
        })?;

    // Determine backend from program
    let backend = get_backend(&resolved.program)?;

    step(verbose, 4, 5, "의미 검사");
    check_backend(&backend, &resolved.program)?;

    step(verbose, 5, 5, "코드 생성");

    // Generate code for the appropriate backend
    // (체커는 step 4에서 이미 실행됨 — 여기선 codegen만)
    let (metal_src, glsl_vert, glsl_frag, glsl_comp, cuda_src, rocm_src,
         shader_types_h, common_h, main_mm, main_cpp, main_hip) = match backend {
        Backend::Metal => {
            use salvation_metal::codegen::Codegen as MetalCodegen;
            use salvation_metal::host_gen;

            let metal_src = MetalCodegen::new().generate(&resolved.program);
            let info = host_gen::analyze(&resolved.program);
            let shader_types_h = host_gen::gen_shader_types_h(&info);
            let common_h = host_gen::gen_common_h(&info);
            let main_mm = host_gen::gen_main_mm(&info, "shaders.metallib");

            (Some(metal_src), None, None, None, None, None,
             Some(shader_types_h), Some(common_h), Some(main_mm), None, None)
        }
        Backend::Vulkan => {
            use salvation_vulkan::codegen;
            use salvation_vulkan::host_gen;

            let output = codegen::generate(&resolved.program);
            let info = host_gen::analyze(&resolved.program);
            let main_cpp = host_gen::gen_main_cpp(&info);

            (None, output.vertex, output.fragment, output.compute, None, None,
             None, None, None, Some(main_cpp), None)
        }
        Backend::Cuda => {
            use salvation_cuda::codegen;
            use salvation_cuda::host_gen;

            let cuda_src = codegen::generate(&resolved.program);
            let info = host_gen::analyze(&resolved.program);
            let main_cu = host_gen::gen_main_cu(&info);
            let full_cu = format!("{}{}", cuda_src, main_cu);

            (None, None, None, None, Some(full_cu), None,
             None, None, None, None, None)
        }
        Backend::Rocm => {
            use salvation_rocm::codegen;
            use salvation_rocm::host_gen;

            let rocm_src = codegen::generate(&resolved.program);
            let info = host_gen::analyze(&resolved.program);
            let main_hip = host_gen::gen_main_hip(&info);
            let full_hip = format!("{}{}", rocm_src, main_hip);

            (None, None, None, None, None, Some(full_hip),
             None, None, None, None, None)
        }
    };

    Ok(CompileOutput {
        backend,
        metal_src,
        glsl_vert,
        glsl_frag,
        glsl_comp,
        cuda_src,
        rocm_src,
        shader_types_h,
        common_h,
        main_mm,
        main_cpp,
        main_hip,
        has_main: resolved.has_main,
    })
}

fn get_backend(program: &salvation_core::compiler::ast::types::Program) -> Result<Backend, String> {
    use salvation_core::compiler::ast::types::Item;

    // Find the first function with an explicit backend
    for item in program {
        if let Item::FnDecl { backend: Some(b), .. } = item {
            return Ok(b.clone());
        }
    }

    // Default to Metal
    Ok(Backend::Metal)
}

fn check_backend(backend: &Backend, program: &salvation_core::compiler::ast::types::Program) -> Result<(), String> {
    match backend {
        Backend::Metal => {
            use salvation_metal::checker::Checker;
            Checker::new().check(program)
                .map_err(|errs| {
                    errs.iter()
                        .map(|e| format!("{} {}", "체커 오류:".red().bold(), e))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
        }
        Backend::Vulkan => {
            use salvation_vulkan::checker::Checker;
            Checker::new().check(program)
                .map_err(|errs| {
                    errs.iter()
                        .map(|e| format!("{} {}", "체커 오류:".red().bold(), e))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
        }
        Backend::Cuda => {
            use salvation_cuda::checker::Checker;
            Checker::new().check(program)
                .map_err(|errs| {
                    errs.iter()
                        .map(|e| format!("{} {}", "체커 오류:".red().bold(), e))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
        }
        Backend::Rocm => {
            use salvation_rocm::checker::Checker;
            Checker::new().check(program)
                .map_err(|errs| {
                    errs.iter()
                        .map(|e| format!("{} {}", "체커 오류:".red().bold(), e))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
        }
    }
}

fn check_only(src: &str) -> Result<bool, String> {
    let tokens   = Lexer::new(src).tokenize()
        .map_err(|e| format!("{} {}", "렉서 오류:".red().bold(), e))?;
    let ast      = Parser::new(tokens).parse()
        .map_err(|e| format!("{} {}", "파서 오류:".red().bold(), e))?;
    let resolved = BackendResolver::new().resolve(ast)
        .map_err(|errs| {
            errs.iter()
                .map(|e| format!("{} {}", "백엔드 오류:".red().bold(), e))
                .collect::<Vec<_>>()
                .join("\n")
        })?;

    // Determine backend and check
    let backend = get_backend(&resolved.program)?;
    check_backend(&backend, &resolved.program)?;

    Ok(resolved.has_main)
}

// ── 파일 출력 ───────────────────────────────────────────────

fn write(dir: &str, name: &str, content: &str) -> Result<String, String> {
    let path = Path::new(dir).join(name);
    fs::create_dir_all(dir)
        .map_err(|e| format!("디렉터리 생성 실패: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("파일 쓰기 실패: {}", e))?;
    Ok(path.to_string_lossy().into_owned())
}

fn write_all(out: &CompileOutput, dir: &str, _stem: &str) -> Result<(), String> {
    match out.backend {
        Backend::Metal => {
            if let Some(ref src) = out.shader_types_h {
                let path = write(dir, "shader_types.h", src)?;
                eprintln!("  {} {}", "→".green(), path);
            }
            if let Some(ref src) = out.metal_src {
                let path = write(dir, "shaders.metal", src)?;
                eprintln!("  {} {}", "→".green(), path);
            }
            if let Some(ref src) = out.common_h {
                let path = write(dir, "common.h", src)?;
                eprintln!("  {} {}", "→".green(), path);
            }
            if let Some(ref src) = out.main_mm {
                let path = write(dir, "main.mm", src)?;
                eprintln!("  {} {}", "→".green(), path);
            }
        }
        Backend::Vulkan => {
            if let Some(ref src) = out.glsl_vert {
                let path = write(dir, "shader.vert", src)?;
                eprintln!("  {} {}", "→".green(), path);
            }
            if let Some(ref src) = out.glsl_frag {
                let path = write(dir, "shader.frag", src)?;
                eprintln!("  {} {}", "→".green(), path);
            }
            if let Some(ref src) = out.glsl_comp {
                let path = write(dir, "shader.comp", src)?;
                eprintln!("  {} {}", "→".green(), path);
            }
            if let Some(ref src) = out.main_cpp {
                let path = write(dir, "main.cpp", src)?;
                eprintln!("  {} {}", "→".green(), path);
            }
        }
        Backend::Cuda => {
            if let Some(ref src) = out.cuda_src {
                let path = write(dir, "salvation.cu", src)?;
                eprintln!("  {} {}", "→".green(), path);
            }
        }
        Backend::Rocm => {
            if let Some(ref src) = out.rocm_src {
                let path = write(dir, "salvation.hip.cpp", src)?;
                eprintln!("  {} {}", "→".green(), path);
            }
        }
    }
    Ok(())
}

// ── 진입점 ──────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { file, output, verbose } => {
            let src  = read_file(&file);
            let stem = file_stem(&file);

            eprintln!("{} {}", "컴파일:".cyan().bold(), file);

            match compile(&src, verbose, "shaders.metallib") {
                Ok(out) => {
                    if !out.has_main {
                        eprintln!(
                            "\n{} fn main()이 없습니다. \
                             라이브러리 모드 파일은 {} 를 사용하세요.",
                            "오류:".red().bold(),
                            "salvation build".yellow()
                        );
                        std::process::exit(1);
                    }

                    if let Err(e) = write_all(&out, &output, &stem) {
                        eprintln!("{} {}", "파일 출력 실패:".red().bold(), e);
                        std::process::exit(1);
                    }

                    eprintln!("{}", "빌드 & 실행:".cyan().bold());
                    let out_path = Path::new(&output);

                    // Metal은 RunnerError, 나머지는 String — 통일해서 처리
                    let runtime_result: Result<(), String> = match out.backend {
                        Backend::Metal  => salvation_metal::runtime::build_and_run(out_path).map_err(|e| e.to_string()),
                        Backend::Vulkan => salvation_vulkan::runtime::build_and_run(out_path),
                        Backend::Cuda   => salvation_cuda::runtime::build_and_run(out_path),
                        Backend::Rocm   => salvation_rocm::runtime::build_and_run(out_path),
                    };

                    if let Err(e) = runtime_result {
                        eprintln!("{} {}", "실행 오류:".red().bold(), e);
                        std::process::exit(1);
                    }
                }
                Err(e) => { eprintln!("\n{}", e); std::process::exit(1); }
            }
        }

        Commands::Build { file, output, verbose } => {
            let src  = read_file(&file);
            let stem = file_stem(&file);

            eprintln!("{} {}", "빌드:".cyan().bold(), file);

            match compile(&src, verbose, "shaders.metallib") {
                Ok(out) => {
                    if let Err(e) = write_all(&out, &output, &stem) {
                        eprintln!("{} {}", "파일 출력 실패:".red().bold(), e);
                        std::process::exit(1);
                    }

                    let out_path = Path::new(&output);
                    let build_result: Result<std::path::PathBuf, String> = match out.backend {
                        Backend::Metal  => salvation_metal::runtime::build_only(out_path).map_err(|e| e.to_string()),
                        Backend::Vulkan => salvation_vulkan::runtime::build_only(out_path),
                        Backend::Cuda   => salvation_cuda::runtime::build_only(out_path),
                        Backend::Rocm   => salvation_rocm::runtime::build_only(out_path),
                    };

                    match build_result {
                        Ok(artifact) => eprintln!(
                            "{} {} {}",
                            "완료".green().bold(), "→".green(),
                            artifact.display()
                        ),
                        Err(e) => {
                            eprintln!("{} {}", "셰이더 빌드 실패:".red().bold(), e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => { eprintln!("\n{}", e); std::process::exit(1); }
            }
        }

        Commands::Check { file } => {
            let src = read_file(&file);
            eprintln!("{} {}", "검사:".cyan().bold(), file);

            match check_only(&src) {
                Ok(has_main) => {
                    let mode = if has_main {
                        "실행 모드 (fn main 있음)".green().to_string()
                    } else {
                        "라이브러리 모드 (fn main 없음)".yellow().to_string()
                    };
                    eprintln!("{} — {}", "이상 없음".green().bold(), mode);
                }
                Err(e) => { eprintln!("\n{}", e); std::process::exit(1); }
            }
        }
    }
}

// ── 헬퍼 ────────────────────────────────────────────────────

fn read_file(path: &str) -> String {
    match fs::read_to_string(path) {
        Ok(s)  => s,
        Err(e) => {
            eprintln!("{} '{}': {}", "파일을 열 수 없습니다:".red().bold(), path, e);
            std::process::exit(1);
        }
    }
}

fn file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn step(verbose: bool, n: u8, total: u8, label: &str) {
    if verbose {
        eprintln!("{}", format!("  [{}/{}] {}...", n, total, label).dimmed());
    }
}