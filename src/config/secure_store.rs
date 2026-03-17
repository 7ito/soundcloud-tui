use std::{
    fs,
    io::{ErrorKind, Write},
    path::Path,
};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use anyhow::{Context, Result, anyhow};
use serde::{Serialize, de::DeserializeOwned};

use crate::config::paths::AppPaths;

pub const CREDENTIALS_ENTRY: &str = "oauth-credentials";
pub const TOKENS_ENTRY: &str = "oauth-tokens";

#[cfg(unix)]
const PRIVATE_FILE_MODE: u32 = 0o600;

pub fn troubleshooting_hint(error: &str) -> Option<&'static str> {
    let error = error.to_ascii_lowercase();
    if error.contains("permission denied")
        || error.contains("access is denied")
        || error.contains("operation not permitted")
        || error.contains("read-only file system")
    {
        Some(
            "Check that your soundcloud-tui config and state directories are writable by your user account.",
        )
    } else {
        None
    }
}

pub fn load_secret<T>(paths: &AppPaths, entry_name: &str, label: &str) -> Result<Option<T>>
where
    T: DeserializeOwned,
{
    let path = path_for(paths, entry_name)?;
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(path)
        .with_context(|| format!("Could not access {label} in local storage"))?;

    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|error| anyhow!("invalid {label} stored in local storage: {error}"))
}

pub fn save_secret<T>(paths: &AppPaths, entry_name: &str, label: &str, value: &T) -> Result<()>
where
    T: Serialize,
{
    let path = path_for(paths, entry_name)?;
    ensure_parent_dir(path)?;
    let raw = serde_json::to_string_pretty(value)?;
    write_private_file(path, &raw)
        .with_context(|| format!("Could not save {label} to local storage"))
}

pub fn delete_secret(paths: &AppPaths, entry_name: &str, label: &str) -> Result<()> {
    let path = path_for(paths, entry_name)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("Could not remove {label} from local storage"))
        }
    }
}

fn path_for<'a>(paths: &'a AppPaths, entry_name: &str) -> Result<&'a Path> {
    match entry_name {
        CREDENTIALS_ENTRY => Ok(paths.credentials_file.as_path()),
        TOKENS_ENTRY => Ok(paths.tokens_file.as_path()),
        _ => Err(anyhow!("unknown local storage entry `{entry_name}`")),
    }
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .context("local storage path is missing a parent directory")?;
    fs::create_dir_all(parent)?;

    #[cfg(unix)]
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;

    Ok(())
}

fn write_private_file(path: &Path, contents: &str) -> Result<()> {
    #[cfg(unix)]
    {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(PRIVATE_FILE_MODE)
            .open(path)?;
        file.write_all(contents.as_bytes())?;
        file.flush()?;
        fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_FILE_MODE))?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        fs::write(path, contents)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use serde::{Deserialize, Serialize};

    use super::{
        CREDENTIALS_ENTRY, TOKENS_ENTRY, delete_secret, load_secret, save_secret,
        troubleshooting_hint,
    };
    use crate::config::paths::AppPaths;

    #[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
    struct SampleSecret {
        value: String,
    }

    #[test]
    fn secret_round_trip_uses_local_files() {
        let (paths, root) = test_paths();
        let sample = SampleSecret {
            value: "secret-value".to_string(),
        };

        save_secret(&paths, CREDENTIALS_ENTRY, "test secret", &sample).expect("save secret");

        let loaded = load_secret::<SampleSecret>(&paths, CREDENTIALS_ENTRY, "test secret")
            .expect("load secret")
            .expect("stored secret");

        assert_eq!(loaded, sample);
        assert!(paths.credentials_file.exists());
        cleanup(root);
    }

    #[test]
    fn delete_secret_removes_existing_value() {
        let (paths, root) = test_paths();
        let sample = SampleSecret {
            value: "secret-value".to_string(),
        };

        save_secret(&paths, TOKENS_ENTRY, "test secret", &sample).expect("save secret");
        delete_secret(&paths, TOKENS_ENTRY, "test secret").expect("delete secret");

        let loaded =
            load_secret::<SampleSecret>(&paths, TOKENS_ENTRY, "test secret").expect("load secret");
        assert!(loaded.is_none());
        cleanup(root);
    }

    #[test]
    fn load_secret_surfaces_local_storage_errors() {
        let (paths, root) = test_paths();
        fs::create_dir_all(&paths.config_dir).expect("create config dir");
        fs::create_dir(&paths.credentials_file).expect("create conflicting directory");

        let error = load_secret::<SampleSecret>(&paths, CREDENTIALS_ENTRY, "test secret")
            .expect_err("expected load failure");

        assert!(error.to_string().contains("local storage"));
        cleanup(root);
    }

    #[test]
    fn troubleshooting_hint_detects_permission_errors() {
        let hint = troubleshooting_hint("Permission denied (os error 13)").expect("hint");

        assert!(hint.contains("writable"));
    }

    #[cfg(unix)]
    #[test]
    fn save_secret_uses_private_file_permissions() {
        let (paths, root) = test_paths();
        let sample = SampleSecret {
            value: "secret-value".to_string(),
        };

        save_secret(&paths, CREDENTIALS_ENTRY, "test secret", &sample).expect("save secret");

        let mode = fs::metadata(&paths.credentials_file)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);

        cleanup(root);
    }

    fn test_paths() -> (AppPaths, PathBuf) {
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "soundcloud-tui-secure-store-test-{}-{unique}",
            std::process::id()
        ));
        let config_dir = root.join("config");
        let state_dir = root.join("state");
        let cache_dir = root.join("cache");
        let paths = AppPaths {
            settings_file: config_dir.join("settings.toml"),
            history_file: state_dir.join("history.json"),
            log_file: state_dir.join("soundcloud-tui.log"),
            credentials_file: config_dir.join("credentials.json"),
            tokens_file: state_dir.join("tokens.json"),
            config_dir,
            state_dir,
            cache_dir,
        };

        (paths, root)
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
