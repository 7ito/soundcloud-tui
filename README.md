# soundcloud-tui

A SoundCloud client for the terminal written in Rust, powered by Ratatui, heavily inspired by spotatui (spotify-tui).

This project is still very early. Expect rough edges, missing features, and breaking changes while the core experience is still taking shape.

![Demo](https://github.com/7ito/soundcloud-tui/blob/main/docs/soundcloud-tui-demo.gif)

## Current capabilities

- Search for tracks, users, and playlists
- Browse your feed, likes, and playlists
- Play audio with bundled native streaming
- Manage a queue and add tracks to playlists or liked songs
- Use a fullscreen visualizer and customize settings and keybindings

## Requirements

- An interactive terminal
- Rust if you are building from source
- FFmpeg development libraries if you are building from source
- Your own SoundCloud app credentials for OAuth

Credentials and session tokens are stored locally for the current user.

Visualizer notes:

- Linux and Windows analyze the decoded playback stream directly
- macOS playback is still present in releases, but native streaming has only been validated on Linux and Windows so far

## Installation

### Arch Linux (AUR)

The AUR package is available as `soundcloud-tui`.

```bash
yay -S soundcloud-tui
```

or:

```bash
paru -S soundcloud-tui
```

The AUR package links against system FFmpeg libraries at build and runtime:

```bash
sudo pacman -S ffmpeg
```

### Windows

Download the Windows release zip from [GitHub Releases](https://github.com/7ito/soundcloud-tui/releases), then extract it.

Windows release archives bundle the native FFmpeg DLLs needed for playback.

Then run `soundcloud-tui` from the extracted release directory:

```powershell
.\soundcloud-tui.exe
```

### macOS

An Apple Silicon macOS binary is available on [GitHub Releases](https://github.com/7ito/soundcloud-tui/releases), but it has not been tested yet.

macOS native playback is still behind Linux and Windows in testing coverage. If you build from source, you will also need FFmpeg development libraries installed:

```bash
brew install ffmpeg
```

The visualizer now reads directly from the playback stream instead of requiring loopback capture.

### From source

If you would rather build from source:

```bash
git clone https://github.com/7ito/soundcloud-tui.git
cd soundcloud-tui
cargo run --release
```

Native playback currently depends on FFmpeg development libraries when building from source. On Debian or Ubuntu, install `libavcodec-dev`, `libavformat-dev`, `libavutil-dev`, and `libswresample-dev` before building.

On first launch, the app walks you through entering your SoundCloud app credentials and authorizing in your browser.

## Release status

- AUR package available as `soundcloud-tui`
- Windows binaries available on GitHub Releases
- Apple Silicon macOS binary available on GitHub Releases, but currently untested

Packaging notes for the initial release live in [`docs/packaging.md`](docs/packaging.md).

## Inspiration

This project is heavily inspired by [spotatui](https://github.com/lfabianh/spotatui), a maintained fork of the original [spotify-tui](https://github.com/Rigellute/spotify-tui).

## License

`soundcloud-tui` is available under the MIT License. See [`LICENSE`](LICENSE).
