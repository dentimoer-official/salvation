use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n]+")]
#[logos(skip r"//[^\\n]*")]
pub enum Token {
    // keywords (키워)
    #[token("kernel")] Kernel,
    #[token("fn")] Fn,
    #[token("let")] Let,
    #[token("mut")] Mut,
    #[token("if")] If,
    #[token("else")] Else,
    #[token("for")] For,
    #[token("in")] In,
    #[token("return")] Return,
    #[token("true")] True,
    #[token("false")] False,
    
    // memory qualifier (메모리 한정자)
    #[token("device")] Device,
    #[token("threadgroup")] Threadgroup,
    #[token("private")] Private,
    
    // type (타입)
    #[token("f32")] F32,
    #[token("f16")] F16,
    #[token("i32")] I32,
    #[token("u32")] U32,
    #[token("bool")] Bool,
    #[token("void")] Void,
    
    // vec type (백터 타입)
    #[token("vec2")] Vec2,
    #[token("vec3")] Vec3,
    #[token("vec4")] Vec4,
    
    // matrix (행령)
    #[token("mat4")] Mat4,
    
    // literal (리터럴)
    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse::<f64>().unwrap())]
    FloatLit(f64),
    #[regex(r"[0-9]+", |lex| lex.slice().parse::<i64>().unwrap())]
    IntLit(i64),
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),
    
    // operator (연산자)
    #[token("+")] Plus,
    #[token("-")] Minus,
    #[token("*")] Star,
    #[token("/")] Slash,
    #[token("%")] Percent,
    #[token("=")] Eq,
    #[token("==")] EqEq,
    #[token("!=")] Ne,
    #[token("<")]  Lt,
    #[token(">")]  Gt,
    #[token("<=")] Le,
    #[token(">=")] Ge,
    #[token("&&")] And,
    #[token("||")] Or,
    #[token("!")]  Bang,

    // separator (구분자)
    #[token("(")] LParen,
    #[token(")")] RParen,
    #[token("{")] LBrace,
    #[token("}")] RBrace,
    #[token("[")] LBracket,
    #[token("]")] RBracket,
    #[token(",")] Comma,
    #[token(";")] Semi,
    #[token(":")] Colon,
    #[token("import")] Import,
    #[token("pub")] Pub,
    #[token("::")] ColonColon,
    #[token(".")] Dot,
    #[token("->")] Arrow,
    #[token("@")]  At,
    
    #[token("const")] Const,
    #[token("struct")] Struct,
    #[token("..")] DotDot,
}

pub struct Lexer<'a> {
    inner: logos::Lexer<'a, Token>
}

impl <'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self { inner: Token::lexer(src)}
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<(Token, std::ops::Range<usize>), ()>;
    
    fn next(&mut self) -> Option<Self::Item> {
        let token = self.inner.next()?;
        let span = self.inner.span();
        Some(token.map(|t| (t, span)).map_err(|_| ()))
    }
}
