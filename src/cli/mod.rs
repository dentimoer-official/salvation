pub mod helper;
pub mod language_selector;

use std::env;

pub fn run_salvation_cli() {
    let args: Vec<String> = env::args().collect();

    // 언어 결정 로직 (우선순위: --lang → LANG 환경변수 → 기본 en)
    let lang = get_language(&args);

    if args.len() == 1 {
        print_default_help(&lang);
        return;
    }

    let cmd = args[1].as_str();

    match cmd {
        "help" | "--help" | "-h" => print_help(&lang),
        "version" | "--version" | "-V" => println!("cross 0.1.0"),
        "build" => println!("{}", t(&lang, "building", &[])),
        "test" => println!("{}", t(&lang, "testing", &[])),
        _ => {
            eprintln!("{}", t(&lang, "unknown_command", &[cmd]));
            eprintln!();
            print_default_help(&lang);
            std::process::exit(1);
        }
    }
}

// 언어 결정 함수
fn get_language(args: &[String]) -> String {
    // --lang ko 또는 --lang en 찾기
    for i in 1..args.len() {
        if args[i] == "--lang" && i + 1 < args.len() {
            let l = args[i + 1].to_lowercase();
            if l == "ko" || l == "en" {
                return l;
            }
        }
    }

    // 환경변수 LANG 체크
    if let Ok(lang) = env::var("LANG") {
        if lang.to_lowercase().starts_with("ko") {
            return "ko".to_string();
        }
    }

    "en".to_string()
}

// 번역 함수
fn t(lang: &str, key: &str, args: &[&str]) -> String {
    let is_ko = lang == "ko";

    match key {
        "building" => {
            if is_ko {
                "빌드 중...".to_string()
            } else {
                "building...".to_string()
            }
        }
        "testing" => {
            if is_ko {
                "테스트 중...".to_string()
            } else {
                "testing...".to_string()
            }
        }
        "unknown_command" => {
            let cmd_str = args.first().copied().unwrap_or("");
            if is_ko {
                format!("알 수 없는 명령어: {}", cmd_str)
            } else {
                format!("unknown command: {}", cmd_str)
            }
        }
        _ => "[missing translation]".to_string(),
    }
}

fn print_default_help(lang: &str) {
    let is_korean = lang == "korean";

    if is_korean {
        println!("cross: 간단한 크로스 플랫폼 도구 (개발중)");
        println!();
        println!("사용법:");
        println!("  cross <명령어>");
        println!();
        println!("명령어:");
        println!("  help       이 도움말 보기");
        println!("  version    버전 정보");
        println!("  build      빌드");
        println!("  test       테스트");
        println!();
        println!("언어 선택: --lang ko 또는 --lang en");
    } else {
        println!("cross: simple cross-platform tool (WIP)");
        println!();
        println!("Usage:");
        println!("  cross <command>");
        println!();
        println!("Commands:");
        println!("  help       show this help");
        println!("  version    show version");
        println!("  build      build");
        println!("  test       test");
        println!();
        println!("Language: --lang ko or --lang en");
    }
}

fn print_help(lang: &str) {
    let is_ko = lang == "ko";

    if is_ko {
        println!("cross 0.1.0");
        println!("아주 간단한 CLI 예제입니다.");
        println!();
        println!("사용 가능한 명령어:");
        println!("  cross help");
        println!("  cross version");
        println!("  cross build");
        println!("  cross test");
        println!();
        println!("옵션:");
        println!("  --help, -h     이 도움말");
        println!("  --version, -V  버전 정보");
        println!("  --lang <ko|en> 언어 선택 (기본: en 또는 LANG 환경변수)");
    } else {
        println!("cross 0.1.0");
        println!("Very simple CLI example.");
        println!();
        println!("Available commands:");
        println!("  cross help");
        println!("  cross version");
        println!("  cross build");
        println!("  cross test");
        println!();
        println!("Options:");
        println!("  --help, -h     Show this help message");
        println!("  --version, -V  Show version information");
        println!("  --lang <ko|en> Select language (default: en or LANG env)");
    }
}