mod commands;
mod storage;

use clap::Parser;
use eyre::Result;

use crate::commands::{Cli, CommandContext};

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let ctx = CommandContext::new()?;
    cli.command.execute(&ctx)
}
