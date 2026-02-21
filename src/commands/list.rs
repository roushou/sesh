use clap::Args;
use eyre::Result;

use crate::commands::CommandContext;

#[derive(Args, Debug)]
pub struct ListCommand {
    /// Filter by tag
    #[arg(long)]
    pub tag: Option<String>,
    /// Filter by provider
    #[arg(long)]
    pub provider: Option<String>,
}

impl ListCommand {
    pub fn execute(self, ctx: &CommandContext) -> Result<()> {
        let store = ctx.storage.load()?;
        let tag_filter = self.tag.map(|tag| tag.trim().to_ascii_lowercase());
        let provider_filter = self
            .provider
            .map(|provider| provider.trim().to_ascii_lowercase());

        let mut rows: Vec<_> = store
            .hosts
            .iter()
            .filter(|host| host.matches_filter(tag_filter.as_deref(), provider_filter.as_deref()))
            .collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name));

        if rows.is_empty() {
            println!("No host entries found.");
            return Ok(());
        }

        println!(
            "{:<20} {:<30} {:<6} {:<16} {:<16} TAGS",
            "NAME", "HOST", "PORT", "USER", "PROVIDER",
        );
        for row in rows {
            let tags = if row.tags.is_empty() {
                "-".to_string()
            } else {
                row.tags.join(",")
            };
            println!(
                "{:<20} {:<30} {:<6} {:<16} {:<16} {}",
                row.name,
                row.host,
                row.port,
                row.user.as_deref().unwrap_or("-"),
                row.provider.as_deref().unwrap_or("-"),
                tags
            );
        }

        Ok(())
    }
}
