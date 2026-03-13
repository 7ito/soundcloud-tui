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

On Linux, a Secret Service provider is required, `gnome-keyring` is recommended.

Visualizer notes:

- Linux needs a monitor-style input exposed by PipeWire or PulseAudio
- macOS needs a loopback device such as BlackHole or Loopback
- Windows uses WASAPI loopback on the default output device

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

You still need `mpv` installed separately:

```bash
sudo pacman -S mpv
```

### Windows

Download the Windows release zip from [GitHub Releases](https://github.com/7ito/soundcloud-tui/releases), then extract it.

`mpv` is required. Download it, move the extracted folder to something like `C:\Program Files\mpv`, then add `C:\Program Files\mpv` to your `User` `PATH` environment variable. 

After updating `PATH`, open a new terminal and confirm `mpv` is available:

```powershell
mpv --version
```

Then run `soundcloud-tui` from the extracted release directory:

```powershell
.\soundcloud-tui.exe
```

### macOS

An Apple Silicon macOS binary is available on [GitHub Releases](https://github.com/7ito/soundcloud-tui/releases), but it has not been tested yet.

`mpv` is still a hard requirement:

```bash
brew install mpv
```

For visualizer support on macOS, you also need a loopback device such as BlackHole or Loopback.

### From source

If you would rather build from source:

```bash
git clone https://github.com/7ito/soundcloud-tui.git
cd soundcloud-tui
cargo run --release
```

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
