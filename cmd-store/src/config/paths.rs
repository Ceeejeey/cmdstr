use anyhow::{Context, Result};
use std::path::PathBuf;

const APP_NAME: &str = "cmdstr";

pub fn data_dir() -> Result<PathBuf> {
    let dir = dirs::data_dir()
        .context("could not find XDG data directory")?
        .join(APP_NAME);
    std::fs::create_dir_all(&dir).context("failed to create data directory")?;
    Ok(dir)
}

pub fn config_dir() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .context("could not find XDG config directory")?
        .join(APP_NAME);
    std::fs::create_dir_all(&dir).ok();
    Ok(dir)
}

pub fn db_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("commands.db"))
}

pub fn config_path() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"));
    dir.join(APP_NAME).join("config.toml")
}
