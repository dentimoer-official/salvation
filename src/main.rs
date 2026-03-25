use std::fs;
use std::path::Path;

use clap::{Parser as ClapParser, Subcommand};
use colored::Colorize;

use salvation_core::compiler::lexer::Lexer;
use salvation_core::compiler::parser::parser_testing::Parser;
use salvation_core::compiler::backend_resolver::BackendResolver;
use salvation_metal::checker::Checker;
use salvation_metal::codegen::Codegen;

// ── CLI 정의 ────────────────────────────────────────────────

#[derive(ClapParser)]
#[command(
    name    = "salvation",
    version = "0.1.0",
    author  = "dentimoer-official <dentimoer@icloud.com>",
    about   = "Salvation GPU language compiler",
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// .slvt 파일을 컴파일하고 결과를 출력합니다 (fn main 필요)
    Run {
        /// 컴파일할 .slvt 파일 경로
        #[arg(value_name = "FILE")]
        file: String,

        /// 출력 디렉터리 (기본값: ./out)
        #[arg(short, long, default_value = "./out")]
        output: String,

        /// 상세 출력 모드
        #[arg(short, long)]
        verbose: bool,
    },

    /// .slvt 파일을 라이브러리로 빌드합니다 (fn main 없어도 됨)
    Build {
        /// 컴파일할 .slvt 파일 경로
        #[arg(value_name = "FILE")]
        file: String,

        /// 출력 디렉터리 (기본값: ./out)
        #[arg(short, long, default_value = "./out")]
        output: String,

        /// 상세 출력 모드
        #[arg(short, long)]
        verbose: bool,
    },

    /// .slvt 파일 문법/백엔드 검사만 수행합니다 (파일 생성 없음)
    Check {
        /// 검사할 .slvt 파일 경로
        #[arg(value_name = "FILE")]
        file: String,
    },
}

// ── 컴파일 파이프라인 ────────────────────────────────────────

struct CompileOutput {
    metal_src: String,
    has_main:  bool,
}

fn compile(src: &str, verbose: bool) -> Result<CompileOutput, String> {
    if verbose { eprintln!("{}", "  [1/4] 렉싱...".dimmed()); }
    let tokens = Lexer::new(src).tokenize()
        .map_err(|e| format!("{} {}", "렉서 오류:".red().bold(), e))?;

    if verbose { eprintln!("{}", "  [2/4] 파싱...".dimmed()); }
    let ast = Parser::new(tokens).parse()
        .map_err(|e| format!("{} {}", "파서 오류:".red().bold(), e))?;

    if verbose { eprintln!("{}", "  [3/4] 백엔드 리졸빙...".dimmed()); }
    let resolved = BackendResolver::new().resolve(ast)
        .map_err(|errs| {
            errs.iter()
                .map(|e| format!("{} {}", "백엔드 오류:".red().bold(), e))
                .collect::<Vec<_>>()
                .join("\n")
        })?;

    if verbose { eprintln!("{}", "  [4/4] 의미 검사...".dimmed()); }
    Checker::new().check(&resolved.program)
        .map_err(|errs| {
            errs.iter()
                .map(|e| format!("{} {}", "체커 오류:".red().bold(), e))
                .collect::<Vec<_>>()
                .join("\n")
        })?;

    let metal_src = Codegen::new().generate(&resolved.program);

    Ok(CompileOutput { metal_src, has_main: resolved.has_main })
}

fn check_only(src: &str) -> Result<bool, String> {
    let tokens = Lexer::new(src).tokenize()
        .map_err(|e| format!("{} {}", "렉서 오류:".red().bold(), e))?;

    let ast = Parser::new(tokens).parse()
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

fn write_output(output_dir: &str, filename: &str, content: &str) -> Result<String, String> {
    let path = Path::new(output_dir).join(filename);
    fs::create_dir_all(output_dir)
        .map_err(|e| format!("출력 디렉터리 생성 실패: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("파일 쓰기 실패: {}", e))?;
    Ok(path.to_string_lossy().into_owned())
}

fn out_filename(file: &str) -> String {
    let stem = Path::new(file)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    format!("{}.metal", stem)
}

// ── 진입점 ──────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    match cli.command {
        // ── salvation run — fn main 필수 ──────────────────
        Commands::Run { file, output, verbose } => {
            let src = read_file(&file);
            eprintln!("{} {}", "컴파일 (실행 모드):".cyan().bold(), file);

            match compile(&src, verbose) {
                Ok(out) => {
                    // fn main 없으면 run 불가
                    if !out.has_main {
                        eprintln!(
                            "\n{} 이 파일은 라이브러리 모드입니다 (fn main 없음).\n\
                             실행하려면 fn main()을 추가하거나, {} 를 사용하세요.",
                            "오류:".red().bold(),
                            "salvation build".yellow()
                        );
                        std::process::exit(1);
                    }
                    print_and_save(&out.metal_src, &output, &out_filename(&file));
                }
                Err(e) => { eprintln!("\n{}", e); std::process::exit(1); }
            }
        }

        // ── salvation build — fn main 없어도 됨 ───────────
        Commands::Build { file, output, verbose } => {
            let src = read_file(&file);
            eprintln!("{} {}", "컴파일 (라이브러리 모드):".cyan().bold(), file);

            match compile(&src, verbose) {
                Ok(out) => {
                    if out.has_main {
                        eprintln!("{}", "  fn main 감지 → 실행 가능 바이너리".dimmed());
                    } else {
                        eprintln!("{}", "  라이브러리 모드 → pub fn만 수출됩니다".dimmed());
                    }
                    print_and_save(&out.metal_src, &output, &out_filename(&file));
                }
                Err(e) => { eprintln!("\n{}", e); std::process::exit(1); }
            }
        }

        // ── salvation check ────────────────────────────────
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

// ── 공통 헬퍼 ───────────────────────────────────────────────

fn read_file(path: &str) -> String {
    match fs::read_to_string(path) {
        Ok(s)  => s,
        Err(e) => {
            eprintln!("{} '{}': {}", "파일을 열 수 없습니다:".red().bold(), path, e);
            std::process::exit(1);
        }
    }
}

fn print_and_save(metal_src: &str, output_dir: &str, filename: &str) {
    println!("{}", metal_src);
    match write_output(output_dir, filename, metal_src) {
        Ok(path) => eprintln!("{} {} {}", "완료".green().bold(), "→".green(), path),
        Err(e)   => { eprintln!("{} {}", "출력 실패:".red().bold(), e); std::process::exit(1); }
    }
}