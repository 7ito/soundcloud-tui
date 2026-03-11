use std::{
    env, fmt, fs, io,
    path::{Path, PathBuf},
};

const MPV_NOT_FOUND_PREFIX: &str = "soundcloud-tui could not find the `mpv` executable.";
const MPV_INSTALL_URL: &str = "https://mpv.io/installation/";

#[derive(Debug)]
pub enum MpvLocatorError {
    NotFound,
    LaunchFailed { path: PathBuf, source: io::Error },
}

pub fn discover() -> Result<PathBuf, MpvLocatorError> {
    find_on_path()
        .or_else(find_next_to_current_exe)
        .or_else(find_in_common_locations)
        .ok_or(MpvLocatorError::NotFound)
}

pub fn launch_failed(path: PathBuf, source: io::Error) -> MpvLocatorError {
    MpvLocatorError::LaunchFailed { path, source }
}

pub fn is_missing_error_message(message: &str) -> bool {
    message.starts_with(MPV_NOT_FOUND_PREFIX)
}

impl fmt::Display for MpvLocatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write_missing_message(f),
            Self::LaunchFailed { path, source } => write!(
                f,
                "soundcloud-tui found `mpv` at `{}` but could not start it.\n\n{}\n\nIf you recently installed mpv, reopen your terminal so PATH changes take effect.\nMore help: {}",
                path.display(),
                source,
                MPV_INSTALL_URL,
            ),
        }
    }
}

impl std::error::Error for MpvLocatorError {}

fn write_missing_message(f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
        f,
        "{MPV_NOT_FOUND_PREFIX}\n\n{}\n\nMore help: {MPV_INSTALL_URL}",
        install_guidance()
    )
}

#[cfg(target_os = "windows")]
fn install_guidance() -> &'static str {
    "Install mpv, then make sure `mpv.exe` is available on PATH.\n\nRecommended Windows options:\n- `scoop install mpv`\n- `choco install mpvio`\n- or download a build from mpv.io and add its folder to PATH"
}

#[cfg(target_os = "macos")]
fn install_guidance() -> &'static str {
    "Install mpv, then make sure the `mpv` binary is available on PATH.\n\nRecommended macOS options:\n- `brew install mpv`\n- `port install mpv`\n- or install a build from mpv.io"
}

#[cfg(all(unix, not(target_os = "macos")))]
fn install_guidance() -> &'static str {
    "Install mpv with your package manager, then make sure the `mpv` binary is available on PATH."
}

#[cfg(not(any(unix, target_os = "windows")))]
fn install_guidance() -> &'static str {
    "Install mpv and make sure the executable is available on PATH."
}

fn find_on_path() -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    let directories = env::split_paths(&path).collect::<Vec<_>>();
    find_in_directories(&directories)
}

fn find_next_to_current_exe() -> Option<PathBuf> {
    let current_exe = env::current_exe().ok()?;
    let parent = current_exe.parent()?;

    #[cfg(target_os = "macos")]
    let mut directories = vec![parent.to_path_buf()];

    #[cfg(not(target_os = "macos"))]
    let directories = vec![parent.to_path_buf()];

    #[cfg(target_os = "macos")]
    {
        if parent.file_name().and_then(|name| name.to_str()) == Some("MacOS") {
            if let Some(contents) = parent.parent() {
                directories.push(contents.join("Resources"));
                directories.push(contents.join("Resources").join("bin"));
            }
        }
    }

    find_in_directories(&directories)
}

fn find_in_common_locations() -> Option<PathBuf> {
    find_in_directories(&common_directories())
}

fn find_in_directories(directories: &[PathBuf]) -> Option<PathBuf> {
    let mut seen = Vec::new();

    for directory in directories {
        for path in command_candidates_in(directory) {
            if seen.contains(&path) {
                continue;
            }
            seen.push(path.clone());

            if is_executable_file(&path) {
                return Some(path);
            }
        }
    }

    None
}

fn command_candidates_in(directory: &Path) -> Vec<PathBuf> {
    command_file_names()
        .into_iter()
        .map(|name| directory.join(name))
        .collect()
}

fn command_file_names() -> Vec<String> {
    #[cfg(windows)]
    {
        let pathext = env::var_os("PATHEXT")
            .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".into())
            .to_string_lossy()
            .split(';')
            .map(str::trim)
            .filter(|ext| !ext.is_empty())
            .map(|ext| ext.trim_start_matches('.').to_ascii_lowercase())
            .collect::<Vec<_>>();

        let mut names = vec!["mpv.exe".to_string()];
        for ext in pathext {
            let candidate = format!("mpv.{ext}");
            if !names.contains(&candidate) {
                names.push(candidate);
            }
        }
        names
    }

    #[cfg(not(windows))]
    {
        vec!["mpv".to_string()]
    }
}

fn common_directories() -> Vec<PathBuf> {
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let mut directories = Vec::new();

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let directories = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Some(home) = dirs::home_dir() {
            directories.push(home.join("scoop").join("shims"));
            directories.push(home.join("scoop").join("apps").join("mpv").join("current"));
        }

        if let Some(local_data) = dirs::data_local_dir() {
            directories.push(local_data.join("Microsoft").join("WinGet").join("Links"));
        }

        if let Some(program_files) = env::var_os("ProgramFiles") {
            directories.push(PathBuf::from(program_files).join("mpv"));
        }

        if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
            directories.push(PathBuf::from(program_files_x86).join("mpv"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        directories.push(PathBuf::from("/opt/homebrew/bin"));
        directories.push(PathBuf::from("/usr/local/bin"));
        directories.push(PathBuf::from("/opt/local/bin"));
        directories.push(PathBuf::from("/Applications/mpv.app/Contents/MacOS"));

        if let Some(home) = dirs::home_dir() {
            directories.push(
                home.join("Applications")
                    .join("mpv.app")
                    .join("Contents")
                    .join("MacOS"),
            );
        }
    }

    directories
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };

    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_error_message_is_detectable() {
        assert!(is_missing_error_message(
            &MpvLocatorError::NotFound.to_string()
        ));
        assert!(!is_missing_error_message("some other error"));
    }

    #[cfg(unix)]
    #[test]
    fn finds_executable_file_in_directory_list() {
        use std::{
            os::unix::fs::PermissionsExt,
            time::{SystemTime, UNIX_EPOCH},
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let dir = env::temp_dir().join(format!("soundcloud-tui-mpv-test-{unique}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        let binary = dir.join("mpv");
        fs::write(&binary, b"#!/bin/sh\n").expect("write temp binary");

        let mut permissions = fs::metadata(&binary)
            .expect("read temp binary metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary, permissions).expect("set temp binary executable");

        let found = find_in_directories(&[dir.clone()]);
        assert_eq!(found.as_deref(), Some(binary.as_path()));

        let _ = fs::remove_file(binary);
        let _ = fs::remove_dir(dir);
    }
}
