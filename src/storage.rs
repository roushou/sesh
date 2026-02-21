use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::error::AppError;

pub const DEFAULT_PORT: u16 = 22;
const STORAGE_RELATIVE_PATH: &str = ".config/sesh/hosts.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostEntry {
    pub name: String,
    pub host: String,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub identity_file: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub imported_from: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HostStore {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub hosts: Vec<HostEntry>,
}

impl Default for HostStore {
    fn default() -> Self {
        Self {
            version: default_version(),
            hosts: Vec::new(),
        }
    }
}

fn default_version() -> u32 {
    1
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

impl HostEntry {
    pub fn matches_filter(&self, tag: Option<&str>, provider: Option<&str>) -> bool {
        let tag_ok = match tag {
            Some(tag) => self.tags.iter().any(|t| t == tag),
            None => true,
        };
        let provider_ok = match provider {
            Some(provider) => self.provider.as_deref() == Some(provider),
            None => true,
        };
        tag_ok && provider_ok
    }

    pub fn ssh_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if self.port != DEFAULT_PORT {
            args.push("-p".to_string());
            args.push(self.port.to_string());
        }
        if let Some(user) = &self.user {
            args.push("-l".to_string());
            args.push(user.clone());
        }
        if let Some(identity_file) = &self.identity_file {
            args.push("-i".to_string());
            args.push(identity_file.clone());
        }
        args.push(self.host.clone());
        args
    }
}

impl HostStore {
    pub fn sort_hosts(&mut self) {
        self.hosts.sort_by(|a, b| a.name.cmp(&b.name));
    }

    pub fn get_host(&self, name: &str) -> Option<&HostEntry> {
        self.hosts.iter().find(|h| h.name == name)
    }

    pub fn get_host_mut(&mut self, name: &str) -> Option<&mut HostEntry> {
        self.hosts.iter_mut().find(|h| h.name == name)
    }
}

#[derive(Debug, Clone)]
pub struct Storage {
    home: PathBuf,
    path: PathBuf,
}

impl Storage {
    #[cfg(test)]
    pub fn new_test(home: PathBuf) -> Self {
        let path = home.join(STORAGE_RELATIVE_PATH);
        Self { home, path }
    }

    pub fn new_default() -> Result<Self, AppError> {
        let home = find_home_dir()?;
        Ok(Self {
            home: home.clone(),
            path: home.join(STORAGE_RELATIVE_PATH),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn home_dir(&self) -> &Path {
        &self.home
    }

    pub fn load(&self) -> Result<HostStore, AppError> {
        if !self.path.exists() {
            return Ok(HostStore::default());
        }

        let content = match fs::read_to_string(&self.path) {
            Ok(content) => content,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                return Err(AppError::PermissionDenied {
                    path: self.path.clone(),
                    action: "read",
                    hint: Some("Check file permissions.".to_string()),
                });
            }
            Err(err) => {
                return Err(AppError::Internal(eyre::eyre!(
                    "failed reading {}: {}",
                    self.path.display(),
                    err
                )));
            }
        };
        let store: HostStore = toml::from_str(&content).map_err(|err| AppError::InvalidConfig {
            path: self.path.clone(),
            message: err.to_string(),
            hint: Some("Fix the file or move it aside and retry.".to_string()),
        })?;
        Ok(store)
    }

    pub fn save(&self, store: &HostStore) -> Result<(), AppError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                if err.kind() == ErrorKind::PermissionDenied {
                    AppError::PermissionDenied {
                        path: parent.to_path_buf(),
                        action: "create directory",
                        hint: Some("Check directory permissions.".to_string()),
                    }
                } else {
                    AppError::Internal(eyre::eyre!("failed creating {}: {}", parent.display(), err))
                }
            })?;
        }
        let body = toml::to_string_pretty(store).map_err(|err| {
            AppError::Internal(eyre::eyre!("failed serializing host store: {}", err))
        })?;
        match fs::write(&self.path, body) {
            Ok(()) => {}
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                return Err(AppError::PermissionDenied {
                    path: self.path.clone(),
                    action: "write",
                    hint: Some("Check directory permissions.".to_string()),
                });
            }
            Err(err) => {
                return Err(AppError::Internal(eyre::eyre!(
                    "failed writing {}: {}",
                    self.path.display(),
                    err
                )));
            }
        }
        Ok(())
    }

    pub fn expand_tilde(&self, path: &str) -> PathBuf {
        if path == "~" {
            return self.home.clone();
        }
        if let Some(remainder) = path.strip_prefix("~/") {
            return self.home.join(remainder);
        }
        PathBuf::from(path)
    }
}

fn find_home_dir() -> Result<PathBuf, AppError> {
    if let Some(home) = env::var_os("HOME") {
        return Ok(PathBuf::from(home));
    }
    if let Some(user_profile) = env::var_os("USERPROFILE") {
        return Ok(PathBuf::from(user_profile));
    }
    let home_drive = env::var_os("HOMEDRIVE");
    let home_path = env::var_os("HOMEPATH");
    if let (Some(drive), Some(path)) = (home_drive, home_path) {
        let mut buf = PathBuf::from(drive);
        buf.push(path);
        return Ok(buf);
    }
    Err(AppError::InvalidInput {
        message: "Could not determine home directory.".to_string(),
        hint: Some("Set HOME (or USERPROFILE on Windows).".to_string()),
    })
}
