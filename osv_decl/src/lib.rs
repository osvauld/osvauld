pub mod ast;
pub mod diagnostics;
pub mod lexer;
pub mod lowering;
pub mod parser;
pub mod semantic;

use diagnostics::Diagnostic;

pub fn compile_source(source: &str) -> Result<lowering::CompiledArtifacts, Vec<Diagnostic>> {
    let ast = match parser::parse(source) {
        Ok(ast) => ast,
        Err(e) => return Err(vec![e]),
    };

    semantic::check(&ast)?;
    Ok(lowering::compile(&ast))
}
