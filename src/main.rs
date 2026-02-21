mod commands;
mod error;
mod storage;

use std::env;

use clap::Parser;

use crate::{
    commands::{Cli, CommandContext},
    error::AppError,
};

fn main() {
    if let Err(err) = run() {
        if is_debug_errors_enabled() {
            eprintln!("Error:\n{err:?}");
        } else {
            eprintln!("Error: {}", err);
            eprintln!("Tip: set SESH_DEBUG_ERRORS=1 for full diagnostics.");
        }
        std::process::exit(1);
    }
}

fn run() -> Result<(), AppError> {
    let cli = Cli::parse();
    let ctx = CommandContext::new()?;
    cli.command.execute(&ctx)
}

fn is_debug_errors_enabled() -> bool {
    matches!(env::var("SESH_DEBUG_ERRORS").as_deref(), Ok("1"))
}
