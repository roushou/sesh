use std::{collections::BTreeMap, fs, path::PathBuf};

use clap::Args;
use eyre::{Result, WrapErr};

use crate::{
    commands::CommandContext,
    storage::{DEFAULT_PORT, HostEntry, HostStore},
};

#[derive(Args, Debug)]
pub struct ImportCommand {
    /// Path to SSH config file
    #[arg(long, default_value = "~/.ssh/config")]
    pub file: String,
    /// Overwrite existing hosts with imported values
    #[arg(long)]
    pub overwrite: bool,
    /// Optional provider label for imported entries
    #[arg(long)]
    pub provider: Option<String>,
    /// Comma-delimited list of tags to apply to imported entries
    #[arg(long, value_delimiter = ',')]
    pub tags: Vec<String>,
}

impl ImportCommand {
    pub fn execute(self, ctx: &CommandContext) -> Result<()> {
        let config_path = ctx.storage.expand_tilde(&self.file);
        let content = fs::read_to_string(&config_path)
            .wrap_err_with(|| format!("failed reading {}", config_path.display()))?;

        let importer = SshConfigImporter::new(
            ctx.storage.home_dir().to_path_buf(),
            config_path.clone(),
            self.normalized_tags(),
            self.provider.map(|value| value.trim().to_ascii_lowercase()),
        );
        let imported_hosts = importer.parse(&content);

        if imported_hosts.is_empty() {
            println!(
                "No importable host entries found in {}",
                config_path.display()
            );
            return Ok(());
        }

        let mut store = ctx.storage.load()?;
        let summary = ImportMergeSummary::apply(&mut store, imported_hosts, self.overwrite);
        store.sort_hosts();
        ctx.storage.save(&store)?;

        println!(
            "Import complete: added {}, replaced {}, skipped {} (use --overwrite to replace duplicates).",
            summary.added, summary.replaced, summary.skipped
        );
        Ok(())
    }

    fn normalized_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .tags
            .iter()
            .map(|tag| tag.trim().to_ascii_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        tags.sort();
        tags.dedup();
        tags
    }
}

#[derive(Debug, Default)]
struct ImportMergeSummary {
    added: usize,
    replaced: usize,
    skipped: usize,
}

impl ImportMergeSummary {
    fn apply(store: &mut HostStore, imported_hosts: Vec<HostEntry>, overwrite: bool) -> Self {
        let mut summary = Self::default();

        for incoming in imported_hosts {
            if let Some(existing) = store.get_host_mut(&incoming.name) {
                if overwrite {
                    *existing = incoming;
                    summary.replaced += 1;
                } else {
                    summary.skipped += 1;
                }
            } else {
                store.hosts.push(incoming);
                summary.added += 1;
            }
        }

        summary
    }
}

struct SshConfigImporter {
    home_dir: PathBuf,
    source_path: PathBuf,
    tags: Vec<String>,
    provider: Option<String>,
}

impl SshConfigImporter {
    fn new(
        home_dir: PathBuf,
        source_path: PathBuf,
        tags: Vec<String>,
        provider: Option<String>,
    ) -> Self {
        Self {
            home_dir,
            source_path,
            tags,
            provider,
        }
    }

    fn parse(&self, content: &str) -> Vec<HostEntry> {
        let mut entries: BTreeMap<String, HostEntry> = BTreeMap::new();
        let mut current: Option<RawHostBlock> = None;

        for raw_line in content.lines() {
            let line = raw_line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }

            let mut parts = line.split_whitespace();
            let key = parts.next().unwrap_or("").to_ascii_lowercase();
            let value = parts.collect::<Vec<_>>().join(" ");
            if value.is_empty() {
                continue;
            }

            if key == "host" {
                if let Some(block) = current.take() {
                    self.flush_host_block(block, &mut entries);
                }
                current = Some(RawHostBlock::new(
                    value.split_whitespace().map(ToOwned::to_owned).collect(),
                ));
                continue;
            }

            if let Some(block) = &mut current {
                match key.as_str() {
                    "hostname" => block.hostname = Some(value),
                    "user" => block.user = Some(value),
                    "port" => block.port = value.parse::<u16>().ok(),
                    "identityfile" => block.identity_file = Some(value),
                    _ => {}
                }
            }
        }

        if let Some(block) = current {
            self.flush_host_block(block, &mut entries);
        }

        entries.into_values().collect()
    }

    fn flush_host_block(&self, block: RawHostBlock, out: &mut BTreeMap<String, HostEntry>) {
        for alias in block.aliases {
            if !Self::is_importable_alias(&alias) {
                continue;
            }

            let host = block.hostname.clone().unwrap_or_else(|| alias.clone());
            let entry = HostEntry {
                name: alias.clone(),
                host,
                user: block.user.clone(),
                port: block.port.unwrap_or(DEFAULT_PORT),
                identity_file: block
                    .identity_file
                    .clone()
                    .map(|path| self.expand_tilde(&path).to_string_lossy().into_owned()),
                tags: self.tags.clone(),
                provider: self.provider.clone(),
                imported_from: Some(self.source_path.to_string_lossy().into_owned()),
            };
            out.insert(alias, entry);
        }
    }

    fn is_importable_alias(alias: &str) -> bool {
        if alias == "*" {
            return false;
        }
        !(alias.contains('*') || alias.contains('?') || alias.contains('!'))
    }

    fn expand_tilde(&self, path: &str) -> PathBuf {
        if path == "~" {
            return self.home_dir.clone();
        }
        if let Some(remainder) = path.strip_prefix("~/") {
            return self.home_dir.join(remainder);
        }
        PathBuf::from(path)
    }
}

#[derive(Debug, Clone)]
struct RawHostBlock {
    aliases: Vec<String>,
    hostname: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_file: Option<String>,
}

impl RawHostBlock {
    fn new(aliases: Vec<String>) -> Self {
        Self {
            aliases,
            hostname: None,
            user: None,
            port: None,
            identity_file: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::SshConfigImporter;

    #[test]
    fn parses_basic_ssh_config() {
        let content = r#"
Host web-1
  HostName 203.0.113.10
  User root
  Port 2222
  IdentityFile ~/.ssh/id_ed25519

Host *
  ForwardAgent no

Host db-1 db-2
  HostName 10.0.0.8
"#;
        let importer = SshConfigImporter::new(
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp/.ssh/config"),
            vec![],
            Some("hetzner".to_string()),
        );
        let entries = importer.parse(content);
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().any(|e| e.name == "web-1" && e.port == 2222));
        assert!(entries.iter().any(|e| e.name == "db-1"));
        assert!(entries.iter().any(|e| e.name == "db-2"));
        assert!(
            entries
                .iter()
                .all(|e| e.provider.as_deref() == Some("hetzner"))
        );
    }

    #[test]
    fn skips_wildcard_aliases() {
        let content = r#"
Host web-*
  HostName 1.2.3.4
Host exact
  HostName 5.6.7.8
"#;
        let importer = SshConfigImporter::new(
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp/.ssh/config"),
            vec![],
            None,
        );
        let entries = importer.parse(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "exact");
    }
}
