use std::{io::ErrorKind, net::IpAddr, process::Command as ProcessCommand};

use clap::Args;

use crate::{
    commands::CommandContext,
    error::AppError,
    storage::{DEFAULT_PORT, HostEntry},
};

#[derive(Args, Debug)]
pub struct ConnectCommand {
    /// Stored entry name, or direct destination (e.g. user@host)
    pub target: String,
    /// Hostname or IP to auto-add and save a missing entry
    #[arg(long)]
    pub host: Option<String>,
    /// SSH user when auto-adding
    #[arg(long)]
    pub user: Option<String>,
    /// SSH port when auto-adding
    #[arg(long)]
    pub port: Option<u16>,
    /// Private key path for -i when auto-adding
    #[arg(long)]
    pub identity_file: Option<String>,
    /// Comma-delimited list of tags when auto-adding
    #[arg(long, value_delimiter = ',')]
    pub tags: Vec<String>,
    /// Optional provider label when auto-adding
    #[arg(long)]
    pub provider: Option<String>,
    /// Name to save an auto-added entry as
    #[arg(long)]
    pub save_as: Option<String>,
    /// Print ssh command without executing
    #[arg(long)]
    pub dry_run: bool,
}

impl ConnectCommand {
    pub fn execute(self, ctx: &CommandContext) -> Result<(), AppError> {
        let mut store = ctx.storage.load()?;
        if let Some(existing_host) = store.get_host(&self.target) {
            self.ensure_no_auto_add_flags_for_existing_entry()?;
            return Self::run_ssh(existing_host.ssh_args(), self.dry_run);
        }

        let host = self.build_auto_add_entry(ctx).ok_or_else(|| AppError::NotFound {
            resource: "Host",
            identifier: self.target.clone(),
            hint: Some(
                "Run `sesh list` to see available names. For new entries use `sesh connect <name> --host <hostname>` or `sesh connect user@host`.".to_string(),
            ),
        })??;

        if store.get_host(&host.name).is_some() {
            return Err(AppError::AlreadyExists {
                resource: "Host",
                identifier: host.name,
                hint: Some("Use a different name or remove the existing entry first.".to_string()),
            });
        }

        let ssh_args = host.ssh_args();
        store.hosts.push(host.clone());
        store.sort_hosts();
        ctx.storage.save(&store)?;
        println!("Host '{}' added.", host.name);

        Self::run_ssh(ssh_args, self.dry_run)
    }

    fn run_ssh(ssh_args: Vec<String>, dry_run: bool) -> Result<(), AppError> {
        if dry_run {
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

    fn ensure_no_auto_add_flags_for_existing_entry(&self) -> Result<(), AppError> {
        if self.uses_auto_add_flags() {
            return Err(AppError::InvalidInput {
                message: "Cannot use auto-add flags when connecting to an existing entry."
                    .to_string(),
                hint: Some("Remove auto-add flags or choose a different entry name.".to_string()),
            });
        }
        Ok(())
    }

    fn uses_auto_add_flags(&self) -> bool {
        self.host.is_some()
            || self.user.is_some()
            || self.port.is_some()
            || self.identity_file.is_some()
            || !self.tags.is_empty()
            || self.provider.is_some()
            || self.save_as.is_some()
    }

    fn build_auto_add_entry(&self, ctx: &CommandContext) -> Option<Result<HostEntry, AppError>> {
        if let Some(host) = self.host.clone() {
            let name = self
                .save_as
                .clone()
                .unwrap_or_else(|| self.target.to_string());
            return Some(self.to_entry(ctx, name, host, self.user.clone()));
        }

        let parsed = match Self::parse_destination_target(&self.target) {
            Ok(parsed) => parsed,
            Err(err) => return Some(Err(err)),
        }?;

        if parsed.0.is_some() && self.user.is_some() {
            return Some(Err(AppError::InvalidInput {
                message: "SSH user is set twice.".to_string(),
                hint: Some("Use either `user@host` or `--user`, not both.".to_string()),
            }));
        }

        let user = self.user.clone().or(parsed.0);
        let name = self.save_as.clone().unwrap_or_else(|| parsed.1.clone());
        Some(self.to_entry(ctx, name, parsed.1, user))
    }

    fn to_entry(
        &self,
        ctx: &CommandContext,
        name: String,
        host: String,
        user: Option<String>,
    ) -> Result<HostEntry, AppError> {
        if name.trim().is_empty() || name.contains(' ') {
            return Err(AppError::InvalidInput {
                message: "Host name cannot be empty or contain spaces.".to_string(),
                hint: Some("Use `--save-as` with a slug like `web-1`.".to_string()),
            });
        }
        if host.trim().is_empty() {
            return Err(AppError::InvalidInput {
                message: "Host cannot be empty.".to_string(),
                hint: Some("Provide `--host` with a hostname or IP.".to_string()),
            });
        }

        let tags = Self::normalize_tags(self.tags.clone());
        let provider = self.provider.as_ref().and_then(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                None
            } else {
                Some(normalized)
            }
        });
        let identity_file = self.identity_file.as_ref().map(|path| {
            ctx.storage
                .expand_tilde(path)
                .to_string_lossy()
                .into_owned()
        });

        Ok(HostEntry {
            name,
            host,
            user,
            port: self.port.unwrap_or(DEFAULT_PORT),
            identity_file,
            tags,
            provider,
            imported_from: None,
        })
    }

    fn parse_destination_target(
        target: &str,
    ) -> Result<Option<(Option<String>, String)>, AppError> {
        if target.contains('@') {
            let Some((user, host)) = target.split_once('@') else {
                return Ok(None);
            };
            if user.is_empty() || host.is_empty() || host.contains('@') {
                return Err(AppError::InvalidInput {
                    message: format!("Invalid destination '{target}'."),
                    hint: Some("Use `user@host`.".to_string()),
                });
            }
            return Ok(Some((Some(user.to_string()), host.to_string())));
        }

        if Self::looks_like_host(target) {
            return Ok(Some((None, target.to_string())));
        }

        Ok(None)
    }

    fn looks_like_host(target: &str) -> bool {
        target.parse::<IpAddr>().is_ok() || target.contains('.') || target.contains(':')
    }

    fn normalize_tags(raw_tags: Vec<String>) -> Vec<String> {
        let mut tags: Vec<String> = raw_tags
            .iter()
            .map(|tag| tag.trim().to_ascii_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        tags.sort();
        tags.dedup();
        tags
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{HostStore, Storage};

    fn test_ctx(dir: &std::path::Path) -> CommandContext {
        CommandContext {
            storage: Storage::new_test(dir.to_path_buf()),
        }
    }

    fn base_cmd(target: &str) -> ConnectCommand {
        ConnectCommand {
            target: target.to_string(),
            host: None,
            user: None,
            port: None,
            identity_file: None,
            tags: vec![],
            provider: None,
            save_as: None,
            dry_run: true,
        }
    }

    #[test]
    fn execute_auto_adds_from_host_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        let cmd = ConnectCommand {
            host: Some("203.0.113.10".to_string()),
            user: Some("root".to_string()),
            port: Some(2222),
            tags: vec!["Prod".to_string(), "web".to_string()],
            provider: Some(" Hetzner ".to_string()),
            ..base_cmd("web-1")
        };

        cmd.execute(&ctx).unwrap();
        let store = ctx.storage.load().unwrap();
        assert_eq!(store.hosts.len(), 1);
        assert_eq!(store.hosts[0].name, "web-1");
        assert_eq!(store.hosts[0].host, "203.0.113.10");
        assert_eq!(store.hosts[0].user.as_deref(), Some("root"));
        assert_eq!(store.hosts[0].port, 2222);
        assert_eq!(store.hosts[0].provider.as_deref(), Some("hetzner"));
        assert_eq!(store.hosts[0].tags, vec!["prod", "web"]);
    }

    #[test]
    fn execute_auto_adds_from_user_host_target() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        base_cmd("ubuntu@203.0.113.10").execute(&ctx).unwrap();

        let store = ctx.storage.load().unwrap();
        assert_eq!(store.hosts.len(), 1);
        assert_eq!(store.hosts[0].name, "203.0.113.10");
        assert_eq!(store.hosts[0].host, "203.0.113.10");
        assert_eq!(store.hosts[0].user.as_deref(), Some("ubuntu"));
    }

    #[test]
    fn execute_uses_save_as_when_auto_adding() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        let cmd = ConnectCommand {
            save_as: Some("edge-box".to_string()),
            ..base_cmd("root@198.51.100.5")
        };

        cmd.execute(&ctx).unwrap();
        let store = ctx.storage.load().unwrap();
        assert_eq!(store.hosts[0].name, "edge-box");
        assert_eq!(store.hosts[0].host, "198.51.100.5");
    }

    #[test]
    fn execute_requires_auto_add_details_for_unknown_names() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        let err = base_cmd("web-1").execute(&ctx).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn execute_rejects_auto_add_flags_for_existing_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        let mut store = HostStore::default();
        store.hosts.push(HostEntry {
            name: "web-1".to_string(),
            host: "203.0.113.10".to_string(),
            user: Some("root".to_string()),
            port: DEFAULT_PORT,
            identity_file: None,
            tags: vec![],
            provider: None,
            imported_from: None,
        });
        ctx.storage.save(&store).unwrap();

        let err = ConnectCommand {
            host: Some("198.51.100.1".to_string()),
            ..base_cmd("web-1")
        }
        .execute(&ctx)
        .unwrap_err();
        assert!(err.to_string().contains("auto-add flags"));
    }
}
