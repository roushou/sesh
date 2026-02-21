use clap::Args;

use crate::{commands::CommandContext, error::AppError};

#[derive(Args, Debug)]
pub struct RemoveCommand {
    /// Entry name to remove
    pub name: String,
}

impl RemoveCommand {
    pub fn execute(self, ctx: &CommandContext) -> Result<(), AppError> {
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

        store.hosts.remove(index);
        ctx.storage.save(&store)?;
        println!("Host removed.");
        Ok(())
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
    }

    #[test]
    fn execute_removes_existing_host() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        seed_store(&ctx);

        RemoveCommand {
            name: "web-1".to_string(),
        }
        .execute(&ctx)
        .unwrap();

        let store = ctx.storage.load().unwrap();
        assert_eq!(store.hosts.len(), 1);
        assert!(store.get_host("web-1").is_none());
        assert!(store.get_host("db-1").is_some());
    }

    #[test]
    fn execute_rejects_missing_host() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(tmp.path());
        seed_store(&ctx);

        let err = RemoveCommand {
            name: "missing".to_string(),
        }
        .execute(&ctx)
        .unwrap_err();
        assert!(err.to_string().contains("not found"));
    }
}
