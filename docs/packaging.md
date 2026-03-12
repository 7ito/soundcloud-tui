# Packaging Notes

This project is being prepared for distribution through AUR, Homebrew, and WinGet.

## Release process

- Tag releases as `vX.Y.Z`
- Pushing a version tag triggers `.github/workflows/release.yml`
- The workflow builds release archives for Linux, macOS, and Windows
- Each archive includes the binary, `README.md`, and `LICENSE`
- A `SHA256SUMS` file is attached to the GitHub release for packagers

The generated asset names follow this pattern:

- `soundcloud-tui-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`
- `soundcloud-tui-vX.Y.Z-x86_64-apple-darwin.tar.gz`
- `soundcloud-tui-vX.Y.Z-aarch64-apple-darwin.tar.gz`
- `soundcloud-tui-vX.Y.Z-x86_64-pc-windows-msvc.zip`

## AUR

Recommended first package:

- `soundcloud-tui` as a source-built package using the GitHub tag tarball

Expected package notes:

- Runtime dependency: `mpv`
- Build dependencies: `cargo`, `rust`
- The app stores credentials in the OS keyring; on Linux, a Secret Service provider such as `gnome-keyring` is recommended for runtime use

## Homebrew

Recommended first package:

- A custom tap formula that builds from source instead of targeting `homebrew/core`

Expected formula notes:

- `depends_on "rust" => :build`
- `depends_on "mpv"`
- Use `soundcloud-tui --version` as the formula test command

## WinGet

Recommended first package:

- A portable manifest that points to the Windows GitHub release archive

Expected manifest notes:

- Command alias: `soundcloud-tui`
- Installer type: `portable`
- The package should document `mpv` as a separate prerequisite until a suitable WinGet dependency path is chosen

## Runtime caveats

- `mpv` must be installed separately and available on `PATH`
- Users need their own SoundCloud app credentials for OAuth
- macOS visualizer support needs a loopback device such as BlackHole or Loopback
- Linux keyring support expects Secret Service via D-Bus
