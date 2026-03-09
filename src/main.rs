use std::{env, fs, collections::HashMap};
use std::path::Path;
use salvation::metal::{
    parser::Parser, 
    codegen::Codegen, 
    typeck::TypeChecker, 
    module::ModuleLoader
};

fn render_error(src: &str, input_path: &str, e: &salvation::metal::typeck::TypeError) {
    let lines: Vec<&str> = src.lines().collect();
    let line_idx = e.span.line.saturating_sub(1);
    let col = e.span.col.saturating_sub(1);

    eprintln!();
    eprintln!("error: {}", e.message);
    eprintln!(" --> {}:{}:{}", input_path, e.span.line, e.span.col);
    eprintln!("  |");
    if let Some(line) = lines.get(line_idx) {
        eprintln!("{:3} | {}", e.span.line, line);
        eprintln!("  | {}^", " ".repeat(col));
    }
    eprintln!("  |");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: salvation <file.slvt>");
        eprintln!("       salvation <file.slvt> -o <output.metal>");
        std::process::exit(1);
    }

    let input_path = &args[1];

    if !input_path.ends_with(".slvt") {
        eprintln!("Error: input file must have .slvt extension");
        std::process::exit(1);
    }

    let src = match fs::read_to_string(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading '{}': {}", input_path, e);
            std::process::exit(1);
        }
    };

    let output_path = if let Some(pos) = args.iter().position(|a| a == "-o") {
        args.get(pos + 1).cloned().unwrap_or_else(|| {
            eprintln!("Error: -o requires a filename");
            std::process::exit(1);
        })
    } else {
        let stem = Path::new(input_path).file_stem().unwrap().to_str().unwrap();
        let dir = Path::new(input_path).parent().unwrap_or(Path::new("."));
        dir.join(format!("{}.metal", stem)).to_str().unwrap().to_string()
    };

    // 1. 파싱
    let mut parser = Parser::new(&src);
    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error in '{}': {}", input_path, e);
            std::process::exit(1);
        }
    };

    // 2. 모듈 로딩
    let base_dir = Path::new(input_path).parent().unwrap_or(Path::new("."));
    let mut loader = ModuleLoader::new(base_dir);
    for decl in &program.decls {
        if let salvation::metal::parser::Decl::Import { path, span } = decl {
            if let Err(e) = loader.load(path) {
                eprintln!("{}:{}:{} — {}", input_path, span.line, span.col, e);
                std::process::exit(1);
            }
        }
    }

    // 3. 모듈 exports 수집
    let mut module_exports: HashMap<String, HashMap<String, salvation::metal::module::ExportKind>> = HashMap::new();
    for (name, module) in &loader.loaded {
        module_exports.insert(name.clone(), module.exports.clone());
    }

    // 4. 타입 체크
    if let Err(errors) = TypeChecker::new().with_modules(module_exports).check(&program) {
        for e in &errors {
            render_error(&src, input_path, e);
        }
        std::process::exit(1);
    }

    // 5. 코드 생성
    let msl = Codegen::new().generate_with_modules(&program, &loader);

    // 6. 파일 쓰기
    if let Some(parent) = Path::new(&output_path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).ok();
        }
    }

    match fs::write(&output_path, &msl) {
        Ok(_) => println!("✓ {} → {}", input_path, output_path),
        Err(e) => {
            eprintln!("Error writing '{}': {}", output_path, e);
            std::process::exit(1);
        }
    }
}