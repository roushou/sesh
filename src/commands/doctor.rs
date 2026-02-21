use std::{
    io::Read,
    net::{TcpStream, ToSocketAddrs},
    process::Command as ProcessCommand,
    time::Duration,
};

use clap::Args;
use eyre::{Result, WrapErr, eyre};

use crate::{commands::CommandContext, storage::HostEntry};

#[derive(Args, Debug)]
pub struct DoctorCommand {
    /// Optional entry name to check
    pub name: Option<String>,
    /// Timeout in milliseconds for TCP and banner checks
    #[arg(long, default_value_t = 1_500)]
    pub timeout_ms: u64,
}

impl DoctorCommand {
    pub fn execute(self, ctx: &CommandContext) -> Result<()> {
        println!("Environment checks:");
        for check in Self::environment_checks(ctx) {
            println!("- {}: {}", check.name, check.status.render());
        }

        let store = ctx.storage.load()?;
        if store.hosts.is_empty() {
            println!("No hosts to check.");
            return Ok(());
        }

        let timeout = Duration::from_millis(self.timeout_ms);
        let hosts: Vec<&HostEntry> = if let Some(name) = self.name {
            let host = store
                .get_host(&name)
                .ok_or_else(|| eyre!("unknown host '{name}'"))?;
            vec![host]
        } else {
            store.hosts.iter().collect()
        };

        println!("\nHost checks:");
        for result in hosts
            .into_iter()
            .map(|host| Self::host_check_result(host, timeout))
        {
            println!("- {}: {}", result.name, result.status.render());
        }

        Ok(())
    }

    fn environment_checks(ctx: &CommandContext) -> Vec<CheckResult> {
        let ssh_available = ProcessCommand::new("ssh").arg("-V").status().is_ok();
        let ssh_check = if ssh_available {
            CheckResult::ok("ssh binary", "OK")
        } else {
            CheckResult::fail("ssh binary", "system ssh not found")
        };

        let store_check = if ctx.storage.path().exists() {
            CheckResult::ok(
                "store file",
                format!("OK ({})", ctx.storage.path().display()),
            )
        } else {
            CheckResult::warn(
                "store file",
                format!("MISSING ({})", ctx.storage.path().display()),
            )
        };

        vec![ssh_check, store_check]
    }

    fn host_check_result(host: &HostEntry, timeout: Duration) -> CheckResult {
        match Self::tcp_connect(host, timeout) {
            Ok(stream) => match Self::read_banner(&stream, timeout) {
                Ok(Some(line)) if line.starts_with("SSH-") => {
                    CheckResult::ok(host.name.clone(), format!("OK ({})", line))
                }
                Ok(Some(line)) => CheckResult::warn(
                    host.name.clone(),
                    format!("connected, non-SSH banner: {}", line),
                ),
                Ok(None) => CheckResult::warn(host.name.clone(), "connected, no banner"),
                Err(err) => CheckResult::warn(
                    host.name.clone(),
                    format!("connected, banner read failed: {}", err),
                ),
            },
            Err(err) => CheckResult::fail(host.name.clone(), err.to_string()),
        }
    }

    fn tcp_connect(host: &HostEntry, timeout: Duration) -> Result<TcpStream> {
        let target = format!("{}:{}", host.host, host.port);
        let mut addrs = target
            .to_socket_addrs()
            .wrap_err_with(|| format!("cannot resolve {}", host.host))?;
        let addr = addrs
            .next()
            .ok_or_else(|| eyre!("no resolved addresses for {}", host.host))?;
        let stream = TcpStream::connect_timeout(&addr, timeout)
            .wrap_err_with(|| format!("tcp connect to {} timed out/failed", target))?;
        Ok(stream)
    }

    fn read_banner(stream: &TcpStream, timeout: Duration) -> Result<Option<String>> {
        let mut stream = stream.try_clone().wrap_err("failed to clone tcp stream")?;
        stream
            .set_read_timeout(Some(timeout))
            .wrap_err("failed setting read timeout")?;

        let mut buf = [0u8; 256];
        let n = stream.read(&mut buf).wrap_err("failed reading banner")?;
        if n == 0 {
            return Ok(None);
        }
        Ok(Some(String::from_utf8_lossy(&buf[..n]).trim().to_string()))
    }
}

#[derive(Debug)]
struct CheckResult {
    name: String,
    status: CheckStatus,
}

impl CheckResult {
    fn ok(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Ok(detail.into()),
        }
    }

    fn warn(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Warn(detail.into()),
        }
    }

    fn fail(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Fail(detail.into()),
        }
    }
}

#[derive(Debug)]
enum CheckStatus {
    Ok(String),
    Warn(String),
    Fail(String),
}

impl CheckStatus {
    fn render(&self) -> String {
        match self {
            Self::Ok(detail) => detail.clone(),
            Self::Warn(detail) => format!("WARN ({})", detail),
            Self::Fail(detail) => format!("FAIL ({})", detail),
        }
    }
}
