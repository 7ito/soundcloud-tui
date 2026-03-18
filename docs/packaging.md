# Packaging Notes

This project is being prepared for distribution through AUR, Homebrew, and WinGet.

## Release process

- Tag releases as `vX.Y.Z`
- Pushing a version tag triggers `.github/workflows/release.yml`
- The workflow builds release archives for Linux, Apple Silicon macOS, and Windows
- Linux and Windows release archives should include the binary, the native FFmpeg runtime libraries required for playback, `README.md`, and `LICENSE`
- A `SHA256SUMS` file is attached to the GitHub release for packagers

The generated asset names follow this pattern:

- `soundcloud-tui-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`
- `soundcloud-tui-vX.Y.Z-aarch64-apple-darwin.tar.gz`
- `soundcloud-tui-vX.Y.Z-x86_64-pc-windows-msvc.zip`

## AUR

Recommended first package:

- `soundcloud-tui` as a source-built package using the GitHub tag tarball

Expected package notes:

- Runtime dependency: `ffmpeg`
- Build dependencies: `cargo`, `rust`
- The app stores credentials and session tokens in user-owned local files

## Homebrew

Recommended first package:

- A custom tap formula that builds from source instead of targeting `homebrew/core`

Expected formula notes:

- `depends_on "rust" => :build`
- `depends_on "ffmpeg"`
- Use `soundcloud-tui --version` as the formula test command

## WinGet

Recommended first package:

- A portable manifest that points to the Windows GitHub release archive

Expected manifest notes:

- Command alias: `soundcloud-tui`
- Installer type: `portable`
- The release archive should ship the FFmpeg DLLs beside `soundcloud-tui.exe`

## Runtime caveats

- Release archives for Linux and Windows should bundle the FFmpeg runtime used by native playback
- Source and distro builds should link against system FFmpeg libraries
- Users need their own SoundCloud app credentials for OAuth
- The visualizer reads directly from the decoded playback stream
