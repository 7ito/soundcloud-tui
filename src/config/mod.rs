#![allow(dead_code)]

pub mod credentials;
pub mod paths;
pub mod settings;
pub mod tokens;

use std::fs::OpenOptions;

use anyhow::{Result, anyhow};
use chrono::Local;
use fern::Dispatch;
use log::LevelFilter;

use crate::config::paths::AppPaths;

pub fn init_logging(paths: &AppPaths) -> Result<()> {
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_file)?;

    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                message
            ));
        })
        .level(LevelFilter::Info)
        .chain(log_file)
        .apply()
        .map_err(|error| anyhow!("failed to initialize logging: {error}"))?;

    Ok(())
}
