use clap::{Parser, Subcommand};
use std::process::Command;

#[derive(Parser)]
#[command(
    name = "salvation",
    version = "0.1.0",
    about = "간단한 크로스 플랫폼 빌드/실행 도구",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// GPU/백엔드용 파일 빌드
    #[command(
        long_about = "salvation build --[want gpu] -> build gpu file that you want\n\n사용 예시:\n  salvation build --metal\n  salvation build --cuda --target myproject/\n  salvation build --vulkan\n  salvation build --rocm"
    )]
    Build {
        #[arg(long, help = "Metal 백엔드 사용")]
        metal: bool,

        #[arg(long, help = "CUDA 백엔드 사용")]
        cuda: bool,

        #[arg(long, help = "Vulkan 백엔드 사용")]
        vulkan: bool,

        #[arg(long, help = "ROCm 백엔드 사용")]
        rocm: bool,

        /// 빌드할 대상 경로 (기본: 현재 디렉토리)
        #[arg(short, long, default_value = ".")]
        target: String,
    },

    /// 빌드된 GPU 파일 실행
    #[command(
        long_about = "salvation run --[want gpu] -> run gpu file that you want\n\n사용 예시:\n  salvation run --metal\n  salvation run --cuda --file mygpu.bin"
    )]
    Run {
        #[arg(long, help = "Metal 백엔드 사용")]
        metal: bool,

        #[arg(long, help = "CUDA 백엔드 사용")]
        cuda: bool,

        #[arg(long, help = "Vulkan 백엔드 사용")]
        vulkan: bool,

        #[arg(long, help = "ROCm 백엔드 사용")]
        rocm: bool,

        /// 실행할 파일 경로 (기본: out)
        #[arg(short, long, default_value = "out")]
        file: String,
    },

    /// 개발중인 환경의 GPU 백엔드를 자동 탐지하고 가장 적합한 백엔드를 추천
    #[command(about = "개발 환경 GPU 백엔드 자동 탐지 & 추천")]
    Peek,
}

pub fn run_salvation_cli() {
    let cli = Cli::parse();

    match cli.command {
        // ==================== BUILD ====================
        Commands::Build { metal, cuda, vulkan, rocm, target } => {
            let mut backends = Vec::new();
            if metal { backends.push("metal"); }
            if cuda  { backends.push("cuda"); }
            if vulkan { backends.push("vulkan"); }
            if rocm  { backends.push("rocm"); }

            if backends.is_empty() {
                println!("백엔드를 하나 이상 지정해주세요.");
                println!("예시: salvation build --metal");
                println!("전체 사용법: salvation build --help");
                return;
            }

            println!("빌드 시작 → 백엔드: {:?}", backends);
            println!("대상: {}", target);
            // ← 여기에 실제 빌드 로직 넣기
            println!("빌드 완료! (아직 구현 안 됨)");
        }

        // ==================== RUN ====================
        Commands::Run { metal, cuda, vulkan, rocm, file } => {
            let mut backends = Vec::new();
            if metal { backends.push("metal"); }
            if cuda  { backends.push("cuda"); }
            if vulkan { backends.push("vulkan"); }
            if rocm  { backends.push("rocm"); }

            if backends.is_empty() {
                println!("백엔드를 지정해주세요.");
                println!("예시: salvation run --metal");
                println!("전체 사용법: salvation run --help");
                return;
            }

            if backends.len() > 1 {
                println!("한 번에 하나의 백엔드만 실행 가능합니다.");
                return;
            }

            println!("실행 → 백엔드: {:?}, 파일: {}", backends[0], file);
            // ← 여기에 실제 실행 로직 넣기
            println!("실행 완료! (아직 구현 안 됨)");
        }

        // ==================== PEEK (새로 추가) ====================
        Commands::Peek => {
            println!("🔍 salvation peek - 개발 환경 GPU 백엔드 탐지");
            println!();

            let os = std::env::consts::OS;
            println!("운영체제 : {}", os);

            let recommended = detect_recommended_backend();

            println!("추천 백엔드 : --{}", recommended);
            println!();
            println!("바로 사용하기:");
            println!("   salvation build --{}", recommended);
            println!("   salvation run   --{}", recommended);
            println!();
            println!("다른 백엔드도 지원합니다:");
            println!("   --metal  --cuda  --vulkan  --rocm");
            println!();
            println!("전체 도움말 보기: salvation --help");
        }
    }
}

// ==================== GPU 탐지 로직 ====================
fn detect_recommended_backend() -> String {
    // 1. macOS → Metal이 가장 자연스럽고 성능 좋음
    if std::env::consts::OS == "macos" {
        return "metal".to_string();
    }

    // 2. NVIDIA GPU 있는지 확인 (nvidia-smi 존재하면 CUDA 추천)
    if Command::new("nvidia-smi")
        .arg("--version")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return "cuda".to_string();
    }

    // 3. AMD ROCm 있는지 확인
    if Command::new("rocm-smi")
        .arg("--version")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
        || Command::new("rocminfo")
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    {
        return "rocm".to_string();
    }

    // 4. 그 외 모든 환경 → Vulkan (가장 범용적)
    "vulkan".to_string()
}
