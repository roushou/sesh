use std::process::Command as ProcessCommand;

use clap::Args;
use eyre::{Result, WrapErr, eyre};

use crate::commands::CommandContext;

#[derive(Args, Debug)]
pub struct ConnectCommand {
    /// Entry name
    pub name: String,
    /// Print ssh command without executing
    #[arg(long)]
    pub dry_run: bool,
}

impl ConnectCommand {
    pub fn execute(self, ctx: &CommandContext) -> Result<()> {
        let store = ctx.storage.load()?;
        let host = store
            .get_host(&self.name)
            .ok_or_else(|| eyre!("unknown host '{}'", self.name))?;

        let ssh_args = host.ssh_args();
        if self.dry_run {
            println!("ssh {}", Self::shell_escape_args(&ssh_args));
            return Ok(());
        }

        let status = ProcessCommand::new("ssh")
            .args(&ssh_args)
            .status()
            .wrap_err("failed to execute system ssh")?;
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    }

    fn shell_escape_args(args: &[String]) -> String {
        args.iter()
            .map(|arg| {
                if arg
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_./:@".contains(c))
                {
                    arg.clone()
                } else {
                    format!("'{}'", arg.replace('\'', "'\\''"))
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}
