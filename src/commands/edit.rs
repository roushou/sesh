use clap::Args;

use crate::{commands::CommandContext, error::AppError};

#[derive(Args, Debug)]
pub struct EditCommand {
    /// Existing entry name
    pub name: String,
    /// Rename entry
    #[arg(long)]
    pub rename: Option<String>,
    /// Hostname or IP
    #[arg(long)]
    pub host: Option<String>,
    /// SSH user
    #[arg(long, conflicts_with = "clear_user")]
    pub user: Option<String>,
    /// SSH port
    #[arg(long)]
    pub port: Option<u16>,
    /// Private key path for -i
    #[arg(long, conflicts_with = "clear_identity_file")]
    pub identity_file: Option<String>,
    /// Comma-delimited tags (replaces all tags)
    #[arg(long, value_delimiter = ',', conflicts_with = "clear_tags")]
    pub tags: Option<Vec<String>>,
    /// Provider label
    #[arg(long, conflicts_with = "clear_provider")]
    pub provider: Option<String>,
    /// Clear SSH user
    #[arg(long)]
    pub clear_user: bool,
    /// Clear private key path
    #[arg(long)]
    pub clear_identity_file: bool,
    /// Clear all tags
    #[arg(long)]
    pub clear_tags: bool,
    /// Clear provider label
    #[arg(long)]
    pub clear_provider: bool,
}

impl EditCommand {
    pub fn execute(self, ctx: &CommandContext) -> Result<(), AppError> {
        self.validate()?;

        let mut store = ctx.storage.load()?;
        let index = store
            .hosts
            .iter()
            .position(|host| host.name == self.name)
            .ok_or_else(|| AppError::NotFound {
                resource: "Host",
                identifier: self.name.clone(),
                hint: Some("Run `sesh list` to see available names.".to_string()),
            })?;

        if let Some(rename) = &self.rename
            && rename != &self.name
            && store.get_host(rename).is_some()
        {
            return Err(AppError::AlreadyExists {
                resource: "Host",
                identifier: rename.clone(),
                hint: Some("Use a different name or remove the existing entry first.".to_string()),
            });
        }

        {
            let host = &mut store.hosts[index];
            if let Some(rename) = &self.rename {
                host.name = rename.clone();
            }
            if let Some(new_host) = &self.host {
                host.host = new_host.clone();
            }
            if let Some(user) = &self.user {
                host.user = Some(user.clone());
            } else if self.clear_user {
                host.user = None;
            }
            if let Some(port) = self.port {
                host.port = port;
            }
            if let Some(identity_file) = &self.identity_file {
                host.identity_file = Some(
                    ctx.storage
                        .expand_tilde(identity_file)
                        .to_string_lossy()
                        .into_owned(),
                );
            } else if self.clear_identity_file {
                host.identity_file = None;
            }
            if let Some(tags) = &self.tags {
                host.tags = Self::normalize_tags(tags.clone());
            } else if self.clear_tags {
                host.tags = Vec::new();
            }
            if let Some(provider) = &self.provider {
                host.provider = Some(provider.trim().to_ascii_lowercase());
            } else if self.clear_provider {
                host.provider = None;
            }
        }

        store.sort_hosts();
        ctx.storage.save(&store)?;
        println!("Host updated.");
        Ok(())
    }

    fn validate(&self) -> Result<(), AppError> {
        if !self.has_changes() {
            return Err(AppError::InvalidInput {
                message: "No changes provided.".to_string(),
                hint: Some(
                    "Pass at least one edit flag, e.g. `--host`, `--user`, or `--rename`."
                        .to_string(),
                ),
            });
        }
        if let Some(rename) = &self.rename
            && (rename.trim().is_empty() || rename.contains(' '))
        {
            return Err(AppError::InvalidInput {
                message: "Host name cannot be empty or contain spaces.".to_string(),
                hint: Some("Use a slug like `web-1`.".to_string()),
            });
        }
        if let Some(host) = &self.host
            && host.trim().is_empty()
        {
            return Err(AppError::InvalidInput {
                message: "Host cannot be empty.".to_string(),
                hint: Some("Provide a hostname or IP for `--host`.".to_string()),
            });
        }
        if let Some(provider) = &self.provider
            && provider.trim().is_empty()
        {
            return Err(AppError::InvalidInput {
                message: "Provider cannot be empty.".to_string(),
                hint: Some("Use `--clear-provider` to remove provider.".to_string()),
            });
        }
        Ok(())
    }

    fn has_changes(&self) -> bool {
        self.rename.is_some()
            || self.host.is_some()
            || self.user.is_some()
            || self.port.is_some()
            || self.identity_file.is_some()
            || self.tags.is_some()
            || self.provider.is_some()
            || self.clear_user
            || self.clear_identity_file
            || self.clear_tags
            || self.clear_provider
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{DEFAULT_PORT, HostEntry, HostStore, Storage};

    fn test_ctx(dir: &std::path::Path) -> CommandContext {
        CommandContext {
            storage: Storage::new_test(dir.to_path_buf()),
        }
    }

    fn seed_store(ctx: &CommandContext) {
        let mut store = HostStore::default();
        store.hosts.push(HostEntry {
            name: "web-1".to_string(),
            host: "203.0.113.10".to_string(),
            user: Some("root".to_string()),
            port: DEFAULT_PORT,
            identity_file: Some("/tmp/key".to_string()),
            tags: vec!["prod".to_string()],
            provider: Some("hetzner".to_string()),
            imported_from: None,
        });
        ctx.storage.save(&store).unwrap();
    }

    fn base_cmd(name: &str) -> EditCommand {
        EditCommand {
            name: name.to_string(),
            rename: None,
            host: None,
            user: None,
            port: None,
            identity_file: None,
            tags: None,
            provider: None,
            clear_user: false,
            clear_identity_file: false,
            clear_tags: false,
            clear_provider: false,
        }
    }

    #[test]
    fn execute_updates_selected_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        seed_store(&ctx);
        let cmd = EditCommand {
            host: Some("198.51.100.5".to_string()),
            user: Some("ubuntu".to_string()),
            port: Some(2201),
            tags: Some(vec!["Web".to_string(), "prod".to_string()]),
            provider: Some("  Akamai ".to_string()),
            ..base_cmd("web-1")
        };

        cmd.execute(&ctx).unwrap();
        let store = ctx.storage.load().unwrap();
        let host = store.get_host("web-1").unwrap();
        assert_eq!(host.host, "198.51.100.5");
        assert_eq!(host.user.as_deref(), Some("ubuntu"));
        assert_eq!(host.port, 2201);
        assert_eq!(host.tags, vec!["prod", "web"]);
        assert_eq!(host.provider.as_deref(), Some("akamai"));
    }

    #[test]
    fn execute_renames_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        seed_store(&ctx);
        EditCommand {
            rename: Some("edge-1".to_string()),
            ..base_cmd("web-1")
        }
        .execute(&ctx)
        .unwrap();

        let store = ctx.storage.load().unwrap();
        assert!(store.get_host("web-1").is_none());
        assert!(store.get_host("edge-1").is_some());
    }

    #[test]
    fn execute_rejects_duplicate_rename() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        let mut store = HostStore::default();
        store.hosts.push(HostEntry {
            name: "web-1".to_string(),
            host: "203.0.113.10".to_string(),
            user: None,
            port: DEFAULT_PORT,
            identity_file: None,
            tags: vec![],
            provider: None,
            imported_from: None,
        });
        store.hosts.push(HostEntry {
            name: "db-1".to_string(),
            host: "203.0.113.11".to_string(),
            user: None,
            port: DEFAULT_PORT,
            identity_file: None,
            tags: vec![],
            provider: None,
            imported_from: None,
        });
        ctx.storage.save(&store).unwrap();

        let err = EditCommand {
            rename: Some("db-1".to_string()),
            ..base_cmd("web-1")
        }
        .execute(&ctx)
        .unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn execute_clears_optional_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        seed_store(&ctx);

        EditCommand {
            clear_user: true,
            clear_identity_file: true,
            clear_tags: true,
            clear_provider: true,
            ..base_cmd("web-1")
        }
        .execute(&ctx)
        .unwrap();

        let store = ctx.storage.load().unwrap();
        let host = store.get_host("web-1").unwrap();
        assert!(host.user.is_none());
        assert!(host.identity_file.is_none());
        assert!(host.tags.is_empty());
        assert!(host.provider.is_none());
    }

    #[test]
    fn execute_rejects_no_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        seed_store(&ctx);

        let err = base_cmd("web-1").execute(&ctx).unwrap_err();
        assert!(err.to_string().contains("No changes"));
    }

    #[test]
    fn execute_rejects_empty_provider() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        seed_store(&ctx);

        let err = EditCommand {
            provider: Some("  ".to_string()),
            ..base_cmd("web-1")
        }
        .execute(&ctx)
        .unwrap_err();
        assert!(err.to_string().contains("Provider cannot be empty"));
    }
}
