//! lex — compiler library.
//!
//! Pipeline: lex -> parse -> sema -> codegen (docs/03-architecture.md).
//! The front end (everything before codegen) owns ALL user-facing
//! correctness and every diagnostic. The Rust backend is a verifier and
//! optimizer, never a source of user-facing errors.

pub mod ast;
pub mod codegen;
pub mod diag;
pub mod lexer;
pub mod parser;
pub mod sema;
pub mod syntax;

use diag::{Diagnostic, Severity};

/// Result of a successful compile: generated Rust plus any lint warnings.
#[derive(Debug)]
pub struct CompileOutput {
    pub rust: String,
    pub lints: Vec<Diagnostic>,
}

/// Run the full front end on source text.
pub fn compile(src: &str) -> Result<CompileOutput, Vec<Diagnostic>> {
    let toks = lexer::lex(src).map_err(|d| vec![d])?;
    let mut prog = parser::parse(&toks).map_err(|d| vec![d])?;
    let diags = sema::check(&mut prog);
    let mut errors = Vec::new();
    let mut lints = Vec::new();
    for d in diags {
        match d.severity {
            Severity::Error => errors.push(d),
            Severity::Lint => lints.push(d),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(CompileOutput {
        rust: codegen::emit(&prog),
        lints,
    })
}

/// Back-compat: compile and return only Rust (drops lints).
pub fn compile_rust(src: &str) -> Result<String, Vec<Diagnostic>> {
    compile(src).map(|o| o.rust)
}

pub use diag::render_all as render_diagnostics;
