use std::fs;
use std::path::Path;

use clap::{Parser as ClapParser, Subcommand};
use colored::Colorize;

use salvation_core::compiler::lexer::Lexer;
use salvation_core::compiler::parser::parser_testing::Parser;
use salvation_core::compiler::backend_resolver::BackendResolver;
use salvation_metal::checker::Checker;
use salvation_metal::codegen::Codegen;
use salvation_metal::host_gen;
use salvation_metal::runtime;

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
    metal_src:  String,
    common_h:   String,
    main_mm:    String,
    has_main:   bool,
}

fn compile(src: &str, verbose: bool, metallib_name: &str) -> Result<CompileOutput, String> {
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

    step(verbose, 4, 5, "의미 검사");
    Checker::new().check(&resolved.program)
        .map_err(|errs| {
            errs.iter()
                .map(|e| format!("{} {}", "체커 오류:".red().bold(), e))
                .collect::<Vec<_>>()
                .join("\n")
        })?;

    step(verbose, 5, 5, "코드 생성");

    // shaders.metal
    let metal_src = Codegen::new().generate(&resolved.program);

    // common.h + main.mm — AST 분석 후 생성
    let info      = host_gen::analyze(&resolved.program);
    let common_h  = host_gen::gen_common_h(&info);
    let main_mm   = host_gen::gen_main_mm(&info, metallib_name);

    Ok(CompileOutput {
        metal_src,
        common_h,
        main_mm,
        has_main: resolved.has_main,
    })
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

fn write_all(out: &CompileOutput, dir: &str, stem: &str) -> Result<(), String> {
    let metal = write(dir, &format!("{}.metal", stem), &out.metal_src)?;
    let common = write(dir, "common.h", &out.common_h)?;
    let main_mm = write(dir, "main.mm", &out.main_mm)?;
    eprintln!("  {} {}", "→".green(), metal);
    eprintln!("  {} {}", "→".green(), common);
    eprintln!("  {} {}", "→".green(), main_mm);
    // shaders.metal을 항상 shaders.metal로 심볼릭하거나 복사
    // (xcrun은 shaders.metal을 직접 읽기 때문에)
    if stem != "shaders" {
        let _ = write(dir, "shaders.metal", &out.metal_src);
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

                    if let Err(e) = runtime::build_and_run(out_path) {
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
                    match runtime::build_only(out_path) {
                        Ok(metallib) => eprintln!(
                            "{} {} {}",
                            "완료".green().bold(), "→".green(),
                            metallib.display()
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