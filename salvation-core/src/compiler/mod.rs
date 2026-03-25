pub mod ast;
pub mod backend_resolver;
pub mod cache;
pub mod codegen;
pub mod error;
pub mod lexer;
pub mod parser;

/*
.slvt → Lexer → Parser → Checker → Codegen → .metal
         토큰화    AST화    타입검증    MSL생성
*/

pub fn testmain() {
}