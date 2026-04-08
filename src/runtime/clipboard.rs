#[cfg(target_os = "linux")]
use std::{
    io::Write,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::Result;
use arboard::Clipboard;

pub(super) fn copy_text(text: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let mut errors = Vec::new();

        for candidate in linux_clipboard_candidates() {
            match run_linux_clipboard_command(candidate, text) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }
        }

        use arboard::{LinuxClipboardKind, SetExtLinux};

        let mut clipboard = Clipboard::new().map_err(|error| {
            errors.push(format!("arboard init: {error}"));
            errors.join("; ")
        })?;

        return clipboard
            .set()
            .clipboard(LinuxClipboardKind::Clipboard)
            .wait_until(Instant::now() + Duration::from_millis(250))
            .text(text.to_string())
            .map_err(|error| {
                errors.push(format!("arboard: {error}"));
                errors.join("; ")
            });
    }

    #[cfg(not(target_os = "linux"))]
    {
        let mut clipboard = Clipboard::new().map_err(|error| error.to_string())?;
        clipboard
            .set_text(text.to_string())
            .map_err(|error| error.to_string())
    }
}

#[cfg(target_os = "linux")]
fn linux_clipboard_candidates() -> Vec<(&'static str, &'static [&'static str])> {
    let mut candidates = Vec::new();

    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        candidates.push(("wl-copy", &["--type", "text/plain"][..]));
    }
    if std::env::var_os("DISPLAY").is_some() {
        candidates.push(("xclip", &["-selection", "clipboard", "-in"][..]));
        candidates.push(("xsel", &["--clipboard", "--input"][..]));
    }
    if candidates.is_empty() {
        candidates.push(("wl-copy", &["--type", "text/plain"][..]));
        candidates.push(("xclip", &["-selection", "clipboard", "-in"][..]));
        candidates.push(("xsel", &["--clipboard", "--input"][..]));
    }

    candidates
}

#[cfg(target_os = "linux")]
fn run_linux_clipboard_command(
    candidate: (&'static str, &'static [&'static str]),
    text: &str,
) -> Result<(), String> {
    let (program, args) = candidate;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("{program}: {error}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|error| format!("{program}: failed writing stdin: {error}"))?;
    }

    let status = child
        .wait()
        .map_err(|error| format!("{program}: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("{program}: exited with status {status}"))
    }
}
