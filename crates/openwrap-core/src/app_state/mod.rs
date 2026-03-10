use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::AppError;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub base_dir: PathBuf,
    pub profiles_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub database_path: PathBuf,
}

impl AppPaths {
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        let base_dir = base_dir.as_ref().to_path_buf();
        Self {
            profiles_dir: base_dir.join("profiles"),
            runtime_dir: base_dir.join("runtime"),
            database_path: base_dir.join("openwrap.sqlite3"),
            base_dir,
        }
    }

    pub fn ensure(&self) -> Result<(), AppError> {
        fs::create_dir_all(&self.base_dir)?;
        fs::create_dir_all(&self.profiles_dir)?;
        fs::create_dir_all(&self.runtime_dir)?;
        Ok(())
    }
}
