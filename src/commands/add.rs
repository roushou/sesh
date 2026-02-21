use clap::Args;

use crate::{commands::CommandContext, error::AppError, storage::HostEntry};

#[derive(Args, Debug)]
pub struct AddCommand {
    /// Unique entry name
    pub name: String,
    /// Hostname or IP
    #[arg(long)]
    pub host: String,
    /// SSH user
    #[arg(long)]
    pub user: Option<String>,
    /// SSH port
    #[arg(long, default_value_t = crate::storage::DEFAULT_PORT)]
    pub port: u16,
    /// Private key path for -i
    #[arg(long)]
    pub identity_file: Option<String>,
    /// Comma-delimited list of tags
    #[arg(long, value_delimiter = ',')]
    pub tags: Vec<String>,
    /// Optional provider label (e.g. hetzner)
    #[arg(long)]
    pub provider: Option<String>,
}

impl AddCommand {
    pub fn execute(self, ctx: &CommandContext) -> Result<(), AppError> {
        self.validate()?;

        let mut store = ctx.storage.load()?;
        if store.get_host(&self.name).is_some() {
            return Err(AppError::AlreadyExists {
                resource: "Host",
                identifier: self.name,
                hint: Some("Use a different name or remove the existing entry first.".to_string()),
            });
        }

        store.hosts.push(self.into_entry(ctx));
        store.sort_hosts();
        ctx.storage.save(&store)?;
        println!("Host added.");
        Ok(())
    }

    fn validate(&self) -> Result<(), AppError> {
        if self.name.contains(' ') {
            return Err(AppError::InvalidInput {
                message: "Host name cannot contain spaces.".to_string(),
                hint: Some("Use a slug like `web-1`.".to_string()),
            });
        }
        Ok(())
    }

    fn into_entry(self, ctx: &CommandContext) -> HostEntry {
        let tags = Self::normalize_tags(self.tags);
        let provider = self.provider.map(|value| value.trim().to_ascii_lowercase());
        let identity_file = self.identity_file.map(|path| {
            ctx.storage
                .expand_tilde(&path)
                .to_string_lossy()
                .into_owned()
        });

        HostEntry {
            name: self.name,
            host: self.host,
            user: self.user,
            port: self.port,
            identity_file,
            tags,
            provider,
            imported_from: None,
        }
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
    use crate::storage::{DEFAULT_PORT, Storage};

    fn test_ctx(dir: &std::path::Path) -> CommandContext {
        CommandContext {
            storage: Storage::new_test(dir.to_path_buf()),
        }
    }

    fn base_cmd(name: &str, host: &str) -> AddCommand {
        AddCommand {
            name: name.to_string(),
            host: host.to_string(),
            user: None,
            port: DEFAULT_PORT,
            identity_file: None,
            tags: vec![],
            provider: None,
        }
    }

    // ---- normalize_tags ----

    #[test]
    fn normalize_tags_empty_input() {
        assert!(AddCommand::normalize_tags(vec![]).is_empty());
    }

    #[test]
    fn normalize_tags_trims_and_lowercases() {
        let tags = vec!["  Prod ".to_string(), "DEV".to_string()];
        assert_eq!(AddCommand::normalize_tags(tags), vec!["dev", "prod"]);
    }

    #[test]
    fn normalize_tags_deduplicates() {
        let tags = vec!["web".to_string(), "web".to_string(), "db".to_string()];
        assert_eq!(AddCommand::normalize_tags(tags), vec!["db", "web"]);
    }

    #[test]
    fn normalize_tags_filters_blank_entries() {
        let tags = vec!["".to_string(), "  ".to_string(), "ok".to_string()];
        assert_eq!(AddCommand::normalize_tags(tags), vec!["ok"]);
    }

    // ---- validate ----

    #[test]
    fn validate_rejects_name_with_spaces() {
        let cmd = base_cmd("my server", "1.2.3.4");
        assert!(cmd.validate().is_err());
    }

    #[test]
    fn validate_accepts_valid_name() {
        let cmd = base_cmd("web-1", "1.2.3.4");
        assert!(cmd.validate().is_ok());
    }

    // ---- into_entry ----

    #[test]
    fn into_entry_basic_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        let cmd = AddCommand {
            user: Some("root".to_string()),
            port: 2222,
            provider: Some("  Hetzner ".to_string()),
            tags: vec!["prod".to_string(), "web".to_string()],
            ..base_cmd("web-1", "10.0.0.1")
        };
        let entry = cmd.into_entry(&ctx);
        assert_eq!(entry.name, "web-1");
        assert_eq!(entry.host, "10.0.0.1");
        assert_eq!(entry.user.as_deref(), Some("root"));
        assert_eq!(entry.port, 2222);
        assert_eq!(entry.provider.as_deref(), Some("hetzner"));
        assert_eq!(entry.tags, vec!["prod", "web"]);
        assert!(entry.imported_from.is_none());
    }

    #[test]
    fn into_entry_expands_tilde_in_identity_file() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        let cmd = AddCommand {
            identity_file: Some("~/.ssh/id_ed25519".to_string()),
            ..base_cmd("box", "1.2.3.4")
        };
        let entry = cmd.into_entry(&ctx);
        let expected = tmp.path().join(".ssh/id_ed25519");
        assert_eq!(
            entry.identity_file.as_deref(),
            Some(expected.to_str().unwrap())
        );
    }

    #[test]
    fn into_entry_absolute_identity_file_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        let cmd = AddCommand {
            identity_file: Some("/etc/ssh/key".to_string()),
            ..base_cmd("box", "1.2.3.4")
        };
        let entry = cmd.into_entry(&ctx);
        assert_eq!(entry.identity_file.as_deref(), Some("/etc/ssh/key"));
    }

    // ---- execute (integration) ----

    #[test]
    fn execute_adds_host_to_store() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        let cmd = base_cmd("web-1", "10.0.0.1");
        cmd.execute(&ctx).unwrap();

        let store = ctx.storage.load().unwrap();
        assert_eq!(store.hosts.len(), 1);
        assert_eq!(store.hosts[0].name, "web-1");
    }

    #[test]
    fn execute_rejects_duplicate_name() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        base_cmd("dup", "1.1.1.1").execute(&ctx).unwrap();

        let err = base_cmd("dup", "2.2.2.2").execute(&ctx).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn execute_sorts_hosts_alphabetically() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        base_cmd("zeta", "1.1.1.1").execute(&ctx).unwrap();
        base_cmd("alpha", "2.2.2.2").execute(&ctx).unwrap();

        let store = ctx.storage.load().unwrap();
        let names: Vec<&str> = store.hosts.iter().map(|h| h.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }

    #[test]
    fn execute_rejects_name_with_space() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        let err = base_cmd("bad name", "1.1.1.1").execute(&ctx).unwrap_err();
        assert!(err.to_string().contains("spaces"));
    }
}
