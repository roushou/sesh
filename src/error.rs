use std::{fmt, path::PathBuf};

#[derive(Debug)]
pub enum AppError {
    NotFound {
        resource: &'static str,
        identifier: String,
        hint: Option<String>,
    },
    AlreadyExists {
        resource: &'static str,
        identifier: String,
        hint: Option<String>,
    },
    InvalidInput {
        message: String,
        hint: Option<String>,
    },
    PermissionDenied {
        path: PathBuf,
        action: &'static str,
        hint: Option<String>,
    },
    MissingDependency {
        binary: &'static str,
        hint: Option<String>,
    },
    InvalidConfig {
        path: PathBuf,
        message: String,
        hint: Option<String>,
    },
    Internal(eyre::Report),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound {
                resource,
                identifier,
                hint,
            } => {
                write!(f, "{resource} '{identifier}' not found.")?;
                if let Some(hint) = hint {
                    write!(f, " {hint}")?;
                }
                Ok(())
            }
            Self::AlreadyExists {
                resource,
                identifier,
                hint,
            } => {
                write!(f, "{resource} '{identifier}' already exists.")?;
                if let Some(hint) = hint {
                    write!(f, " {hint}")?;
                }
                Ok(())
            }
            Self::InvalidInput { message, hint } => {
                write!(f, "{message}")?;
                if let Some(hint) = hint {
                    write!(f, " {hint}")?;
                }
                Ok(())
            }
            Self::PermissionDenied { path, action, hint } => {
                write!(
                    f,
                    "Permission denied while trying to {action} {}.",
                    path.display()
                )?;
                if let Some(hint) = hint {
                    write!(f, " {hint}")?;
                }
                Ok(())
            }
            Self::MissingDependency { binary, hint } => {
                write!(f, "Required dependency `{binary}` not found.")?;
                if let Some(hint) = hint {
                    write!(f, " {hint}")?;
                }
                Ok(())
            }
            Self::InvalidConfig {
                path,
                message,
                hint,
            } => {
                write!(f, "Invalid config at {}: {message}", path.display())?;
                if let Some(hint) = hint {
                    write!(f, " {hint}")?;
                }
                Ok(())
            }
            Self::Internal(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<eyre::Report> for AppError {
    fn from(value: eyre::Report) -> Self {
        Self::Internal(value)
    }
}
