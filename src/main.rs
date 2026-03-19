use salvation_core::compiler::lexer::Lexer;
use salvation_core::compiler::lexer::tokens::Token;
use salvation_core::compiler::lexer::Spanned;
use std::fs;
use std::path::Path;
use salvation_core::compiler::checker::Checker;

use salvation_core::compiler::parser::parser_testing::Parser;
use salvation_core::compiler::codegen::Codegen;

fn write_file(output_dir: &str, filename: &str, content: &str) {
    let path = Path::new(output_dir).join(filename);
    fs::create_dir_all(output_dir).unwrap();
    fs::write(&path, content).unwrap();
    println!("Generated: {}", path.display());
}

fn compile(src: &str) -> Result<String, String> {
    let tokens = Lexer::new(src).tokenize()?;
    let ast    = Parser::new(tokens).parse()?;

    // checker 통과 못 하면 에러 출력하고 중단
    Checker::new().check(&ast).map_err(|errs| {
        errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")
    })?;

    Ok(Codegen::new().generate(&ast))
}


// ── Translator ──────────────────────────────────────────────

/*pub struct Translator {
    tokens: Vec<Spanned<Token>>,
    pos: usize,
    output: String,
    indent: usize,
}

impl Translator {
    pub fn new(tokens: Vec<Spanned<Token>>) -> Self {
        Translator { tokens, pos: 0, output: String::new(), indent: 0 }
    }

    fn peek(&self) -> Token {
        self.tokens.get(self.pos)
            .map(|s| s.node.clone())
            .unwrap_or(Token::Eof)
    }

    fn peek2(&self) -> Token {
        self.tokens.get(self.pos + 1)
            .map(|s| s.node.clone())
            .unwrap_or(Token::Eof)
    }

    // ✅ clone() 반환으로 lifetime 문제 해결
    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos)
            .map(|s| s.node.clone())
            .unwrap_or(Token::Eof);
        if self.pos < self.tokens.len() { self.pos += 1; }
        tok
    }

    fn push(&mut self, s: &str) { self.output.push_str(s); }

    fn push_indent(&mut self) {
        for _ in 0..self.indent { self.output.push_str("    "); }
    }

    fn token_to_metal(tok: &Token) -> Option<&'static str> {
        Some(match tok {
            Token::Fn          => "",
            Token::Struct      => "struct",
            Token::Let         => "",
            Token::Mut         => "",
            Token::Return      => "return",
            Token::If          => "if",
            Token::Else        => "else",
            Token::For         => "for",
            Token::In          => "",
            Token::Bool        => "bool",
            Token::Int         => "int",
            Token::Uint        => "uint",
            Token::Float       => "float",
            Token::Float2      => "float2",
            Token::Float3      => "float3",
            Token::Float4      => "float4",
            Token::Mat2x2      => "float2x2",
            Token::Mat2x3      => "float2x3",
            Token::Mat2x4      => "float2x4",
            Token::Mat3x2      => "float3x2",
            Token::Mat3x3      => "float3x3",
            Token::Mat3x4      => "float3x4",
            Token::Mat4x2      => "float4x2",
            Token::Mat4x3      => "float4x3",
            Token::Mat4x4      => "float4x4",
            Token::Device      => "device",
            Token::Constant    => "constant",
            Token::Threadgroup => "threadgroup",
            Token::Thread      => "thread",
            Token::Plus        => "+",
            Token::Minus       => "-",
            Token::Star        => "*",
            Token::Slash       => "/",
            Token::Percent     => "%",
            Token::Eq          => "=",
            Token::EqEq        => "==",
            Token::BangEq      => "!=",
            Token::Lt          => "<",
            Token::Gt          => ">",
            Token::LtEq        => "<=",
            Token::GtEq        => ">=",
            Token::And         => "&&",
            Token::Or          => "||",
            Token::Bang        => "!",
            Token::Dot         => ".",
            Token::Arrow       => ".",
            Token::ColonColon  => "::",
            Token::At          => "",
            Token::LParen      => "(",
            Token::RParen      => ")",
            Token::LBrace      => "{",
            Token::RBrace      => "}",
            Token::LBracket    => "[",
            Token::RBracket    => "]",
            Token::Semicolon   => ";",
            Token::Colon       => ":",
            Token::Comma       => ",",
            _                  => return None,
        })
    }

    fn collect_type(&mut self) -> String {
        let tok = self.advance();
        Self::token_to_metal(&tok)
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                if let Token::Ident(s) = tok { s } else { "auto".into() }
            })
    }

    fn collect_parens(&mut self) -> String {
        if !matches!(self.peek(), Token::LParen) { return String::new(); }
        self.advance();

        let mut args: Vec<String> = Vec::new();
        let mut current = String::new();

        loop {
            match self.peek() {
                Token::RParen | Token::Eof => { self.advance(); break; }
                Token::Comma => {
                    self.advance();
                    args.push(current.trim().to_string());
                    current = String::new();
                }
                Token::Ident(_) => {
                    let name = if let Token::Ident(s) = self.advance() { s } else { String::new() };
                    if matches!(self.peek(), Token::Colon) {
                        self.advance();
                        let ty = self.collect_type();
                        current.push_str(&format!("{} {}", ty, name));
                    } else {
                        current.push_str(&name);
                    }
                }
                tok => {
                    if let Some(s) = Self::token_to_metal(&tok) { current.push_str(s); }
                    self.advance();
                }
            }
        }

        if !current.trim().is_empty() { args.push(current.trim().to_string()); }
        args.join(", ")
    }

    fn translate_let(&mut self) {
        if matches!(self.peek(), Token::Mut) { self.advance(); }

        let var_name = if let Token::Ident(s) = self.advance() { s } else { "unknown".into() };
        if matches!(self.peek(), Token::Colon) { self.advance(); }
        let type_str = self.collect_type();

        self.push(&format!("{} {}", type_str, var_name));
    }

    fn translate_fn(&mut self, qualifier: Option<&str>) {
        let name = if let Token::Ident(s) = self.advance() { s } else { "unknown".into() };
        let args = self.collect_parens();
        let ret_type = if matches!(self.peek(), Token::Arrow) {
            self.advance();
            self.collect_type()
        } else {
            "void".to_string()
        };

        match qualifier {
            Some(q) => self.push(&format!("{} {} {}({})", q, ret_type, name, args)),
            None    => self.push(&format!("{} {}({})", ret_type, name, args)),
        }
    }

    fn translate_struct(&mut self) {
        let name = if let Token::Ident(s) = self.advance() { s } else { "Unknown".into() };
        self.push(&format!("struct {} ", name));
    }

    pub fn translate(&mut self) -> String {
        self.push("#include <metal_stdlib>\n");
        self.push("using namespace metal;\n\n");

        let mut qualifier: Option<String> = None;

        loop {
            match self.peek() {
                Token::Eof => break,

                Token::At => {
                    self.advance();
                    qualifier = match self.peek() {
                        Token::Vertex   => { self.advance(); Some("vertex".into()) }
                        Token::Fragment => { self.advance(); Some("fragment".into()) }
                        Token::Kernel   => { self.advance(); Some("kernel".into()) }
                        _ => None,
                    };
                }

                Token::Fn => {
                    self.advance();
                    let q = qualifier.take();
                    self.push_indent();
                    self.translate_fn(q.as_deref());
                    self.push(" ");
                }

                Token::Let => {
                    self.advance();
                    self.push_indent();
                    self.translate_let();
                }

                Token::Struct => {
                    self.advance();
                    self.push_indent();
                    self.translate_struct();
                }

                Token::Import => {
                    self.advance();
                    if let Token::StrLit(path) = self.advance() {
                        self.push(&format!("#include \"{}\"\n", path));
                    }
                }

                Token::LBrace => {
                    self.advance();
                    self.push("{\n");
                    self.indent += 1;
                }

                Token::RBrace => {
                    self.advance();
                    self.indent = self.indent.saturating_sub(1);
                    self.push_indent();
                    self.push("}\n");
                }

                Token::Semicolon => {
                    self.advance();
                    self.push(";\n");
                }

                tok => {
                    let out = match &tok {
                        Token::Ident(s)    => s.clone(),
                        Token::IntLit(n)   => n.to_string(),
                        Token::FloatLit(f) => f.to_string(),
                        Token::BoolLit(b)  => b.to_string(),
                        Token::StrLit(s)   => format!("\"{}\"", s),
                        other => Self::token_to_metal(other).unwrap_or("").to_string(),
                    };
                    if !out.is_empty() {
                        self.push(" ");
                        self.push(&out);
                    }
                    self.advance();
                }
            }
        }

        self.output.clone()
    }
}*/

// ── 파일 I/O ───────────────────────────────────────────────



// ── main ───────────────────────────────────────────────────

/*fn main() {
    // .slvt 파일 읽기
    let src = fs::read_to_string("examples/test.slvt")
        .unwrap_or_else(|_| {
            // 파일 없으면 인라인 테스트 소스 사용
            r#"
@vertex
fn vs_main(pos: float4, uv: float2) -> float4 {
    let result: float4 = pos;
    return result;
}

@fragment
fn fs_main(color: float4) -> float4 {
    return color;
}
            "#.to_string()
        });

    // Lexer → Token
    let tokens = match Lexer::new(&src).tokenize() {
        Ok(t)  => t,
        Err(e) => { eprintln!("Lex error: {}", e); return; }
    };

    // Token → Metal
    let metal_src = Translator::new(tokens).translate();

    // 출력
    println!("{}", metal_src);
    write_file("./out", "output.metal", &metal_src);
}*/

fn main() {
    // .slvt 파일 읽기 (없으면 인라인 테스트)
    let src = fs::read_to_string("examples/test1.slvt")
        .unwrap();

    match compile(&src) {
        Ok(metal) => {
            println!("{}", metal);
            write_file("./out", "output.metal", &metal);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}

