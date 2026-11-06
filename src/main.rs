mod ast;
mod compiler;
mod rusk_grammar;
#[allow(warnings)]
mod rusk_grammar_trait {
    include!(concat!(env!("OUT_DIR"), "/rusk_grammar_trait.rs"));
}
#[allow(warnings)]
mod rusk_parser {
    include!(concat!(env!("OUT_DIR"), "/rusk_parser.rs"));
}

use parol_runtime::Report;
struct ErrorReporter;
impl Report for ErrorReporter {}

const CORE: &str = include_str!("../lib/core.rusk");
fn parse(text: &str, file: &str) -> Result<Vec<ast::Stmt>, String> {
    let mut grammar = rusk_grammar::RuskGrammar::default();
    rusk_parser::parse(text, file, &mut grammar).map_err(|e| {
        if ErrorReporter::report_error(&e, file).is_err() {
            eprintln!("{e:?}");
        }
        format!("{file}: syntax error")
    })?;
    ast::lower(
        grammar
            .root
            .as_ref()
            .ok_or_else(|| format!("{file}: missing parse result"))?,
    )
}
fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or("usage: rusk compile FILE [-o OUTPUT]")?;
    if command == "--help" || command == "-h" {
        println!("usage: rusk compile FILE [-o OUTPUT]\nGenerate C++20 source (default: a.cpp).");
        return Ok(());
    }
    if command != "compile" {
        return Err(format!("unknown command '{command}'"));
    }
    let file = args.next().ok_or("missing source file")?;
    let mut output = "a.cpp".to_string();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" | "--output" => output = args.next().ok_or("missing output path")?,
            _ => return Err(format!("unknown argument '{arg}'")),
        }
    }
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{file}: {e}"))?;
    let core = parse(CORE, "<stdlib/core.rusk>")?;
    let source = parse(&text, &file)?;
    let cpp = compiler::Compiler::default().compile(&[core, source])?;
    std::fs::write(&output, cpp).map_err(|e| format!("{output}: {e}"))?;
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
