mod add;
mod connect;
mod doctor;
mod import;
mod list;

pub use add::AddCommand;
use clap::{Parser, Subcommand};
pub use connect::ConnectCommand;
pub use doctor::DoctorCommand;
pub use import::ImportCommand;
pub use list::ListCommand;

use crate::{error::AppError, storage::Storage};

#[derive(Debug)]
pub struct CommandContext {
    pub storage: Storage,
}

impl CommandContext {
    pub fn new() -> Result<Self, AppError> {
        Ok(Self {
            storage: Storage::new_default()?,
        })
    }
}

#[derive(Parser, Debug)]
#[command(name = "sesh", version, about = "SSH connection manager for VPS")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Import host entries from ~/.ssh/config
    Import(ImportCommand),
    /// Add a host entry manually
    Add(AddCommand),
    /// List stored host entries
    List(ListCommand),
    /// Connect to a stored host using system ssh
    Connect(ConnectCommand),
    /// Run environment and host health checks
    Doctor(DoctorCommand),
}

impl Command {
    pub fn execute(self, ctx: &CommandContext) -> Result<(), AppError> {
        match self {
            Self::Import(command) => command.execute(ctx),
            Self::Add(command) => command.execute(ctx),
            Self::List(command) => command.execute(ctx),
            Self::Connect(command) => command.execute(ctx),
            Self::Doctor(command) => command.execute(ctx),
        }
    }
}
