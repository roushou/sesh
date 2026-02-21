use std::{io::ErrorKind, process::Command as ProcessCommand};

use clap::Args;

use crate::{commands::CommandContext, error::AppError};

#[derive(Args, Debug)]
pub struct ConnectCommand {
    /// Entry name
    pub name: String,
    /// Print ssh command without executing
    #[arg(long)]
    pub dry_run: bool,
}

impl ConnectCommand {
    pub fn execute(self, ctx: &CommandContext) -> Result<(), AppError> {
        let store = ctx.storage.load()?;
        let host = store
            .get_host(&self.name)
            .ok_or_else(|| AppError::NotFound {
                resource: "Host",
                identifier: self.name.clone(),
                hint: Some("Run `sesh list` to see available names.".to_string()),
            })?;

        let ssh_args = host.ssh_args();
        if self.dry_run {
            println!("ssh {}", Self::shell_escape_args(&ssh_args));
            return Ok(());
        }

        let status = match ProcessCommand::new("ssh").args(&ssh_args).status() {
            Ok(status) => status,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return Err(AppError::MissingDependency {
                    binary: "ssh",
                    hint: Some("Install OpenSSH client and retry.".to_string()),
                });
            }
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                return Err(AppError::PermissionDenied {
                    path: "ssh".into(),
                    action: "execute",
                    hint: Some("Check your environment or PATH permissions.".to_string()),
                });
            }
            Err(err) => {
                return Err(AppError::Internal(eyre::eyre!(
                    "failed to execute system ssh: {}",
                    err
                )));
            }
        };
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
