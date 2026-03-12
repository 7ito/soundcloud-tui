# soundcloud-tui

A SoundCloud client for the terminal written in Rust, powered by Ratatui, heavily inspired by spotatui (spotify-tui).

This project is still very early. Expect rough edges, missing features, and breaking changes while the core experience is still taking shape.

## Current capabilities

- Search for tracks, users, and playlists
- Browse your feed, likes, and playlists
- Play audio through `mpv`
- Manage a queue and add tracks to playlists or liked songs
- Use a fullscreen visualizer and customize settings and keybindings

## Requirements

- An interactive terminal
- Rust if you are building from source
- `mpv` installed and available on `PATH`
- Your own SoundCloud app credentials for OAuth
- An OS keyring for secure credential and token storage

On Linux, a Secret Service provider such as `gnome-keyring` is recommended.

Visualizer notes:

- Linux needs a monitor-style input exposed by PipeWire or PulseAudio
- macOS needs a loopback device such as BlackHole or Loopback
- Windows uses WASAPI loopback on the default output device

## Getting started

For now, the easiest way to try `soundcloud-tui` is to run it from source:

```bash
git clone https://github.com/7ito/soundcloud-tui.git
cd soundcloud-tui
cargo run --release
```

On first launch, the app walks you through entering your SoundCloud app credentials and authorizing in your browser.

If `mpv` is not already installed:

- Arch Linux: `sudo pacman -S mpv`
- macOS: `brew install mpv`
- Windows: install `mpv` and make sure `mpv.exe` is on `PATH`

## Packaging

AUR, Homebrew, and WinGet packages are planned for the initial release. Until then, building from source is the supported installation path.

Packaging notes for the initial release live in [`docs/packaging.md`](docs/packaging.md).

## Inspiration

This project is heavily inspired by [spotatui](https://github.com/lfabianh/spotatui), a maintained fork of the original [spotify-tui](https://github.com/Rigellute/spotify-tui).

## License

`soundcloud-tui` is available under the MIT License. See [`LICENSE`](LICENSE).
