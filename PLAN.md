# SoundCloud TUI Plan

## Goal

Build a Linux-first SoundCloud terminal client in Rust using `ratatui`, styled after `spotatui`'s core layout:

- top search row
- left sidebar with `Library` and `Playlists`
- main content pane
- bottom now-playing bar with progress

Playback will use an external `mpv` process controlled over JSON IPC. Desktop media integration will be owned by the app itself via MPRIS so `playerctl`, Waybar, and Quickshell see `soundcloud-tui`, not `mpv`.

## Product Constraints

- Target platform: Linux only for the first version
- UI framework: `ratatui`
- Playback backend: external `mpv`
- Desktop media integration: app-owned MPRIS
- Auth model: local BYO SoundCloud app credentials with OAuth 2.1 + PKCE
- No backend service
- `Recently Played` is local app history, not SoundCloud-provided history

## Core Architecture

### Source of truth

`soundcloud-tui` owns all playback and UI state:

- active route and focused pane
- loaded collections and search results
- queue and current track
- playback state, position, duration, volume, shuffle, repeat
- status, errors, and loading states

### Playback path

1. User or MPRIS sends a playback action
2. App reducer updates intent state and dispatches a player command
3. Player backend sends JSON IPC commands to `mpv`
4. `mpv` emits property changes and playback events
5. App updates state
6. TUI redraws and MPRIS metadata/status are refreshed

### Data path

1. User selects a library route or submits a search
2. App dispatches a SoundCloud request
3. SoundCloud service fetches and normalizes API data
4. App updates route-specific state
5. TUI redraws

## Initial Module Layout

```text
src/
  main.rs
  app/
    mod.rs
    state.rs
    action.rs
    event.rs
    reducer.rs
    route.rs
  soundcloud/
    mod.rs
    auth.rs
    client.rs
    service.rs
    models.rs
    paging.rs
  player/
    mod.rs
    backend.rs
    mpv.rs
    ipc.rs
    command.rs
    event.rs
  integrations/
    mod.rs
    mpris.rs
  ui/
    mod.rs
    layout.rs
    header.rs
    sidebar.rs
    content.rs
    playbar.rs
    widgets.rs
    theme.rs
  input/
    mod.rs
    keys.rs
  config/
    mod.rs
    paths.rs
    settings.rs
    credentials.rs
    tokens.rs
  util/
    mod.rs
    time.rs
    text.rs
```

## Phased Execution Plan

### Phase 0: Project bootstrap

Create the crate, dependency set, and baseline app runtime.

Scope:

- initialize Cargo project
- add core dependencies: `ratatui`, `crossterm`, `tokio`, `reqwest`, `serde`, `serde_json`, `toml`, `dirs`, `anyhow`, `thiserror`, `open`, `url`
- add Linux-only MPRIS dependency behind a feature flag
- define app entrypoint and terminal lifecycle
- define config/state/cache directory helpers
- add logging and basic error reporting

Deliverables:

- app launches into an empty shell screen
- clean shutdown restores terminal state
- base module tree exists

Exit criteria:

- `cargo check` passes
- `cargo run` opens and closes cleanly

### Phase 1: State model and UI shell

Build the spotatui-style layout with mock data and a reducer-driven app model.

Scope:

- define `Route`, `Focus`, `AppState`, `NowPlaying`, `QueueState`
- define `Action` and `Event` enums
- implement reducer for navigation and selection updates
- render static layout:
  - search row
  - sidebar with `Feed`, `Liked Songs`, `Recently Played`, `Albums`, `Following`
  - playlists section
  - content pane
  - now playing bar with progress
- add mock content data so the app is navigable before API work begins

Deliverables:

- keyboard navigation between search, library, playlists, content, playbar
- selection highlighting and route changes
- placeholder content per route

Exit criteria:

- shell matches intended layout closely
- reducer tests cover focus and route transitions

### Phase 2: Input and event loop

Make the UI interactive and structure async work around an app event bus.

Scope:

- implement crossterm event handling
- add keymap for navigation, selection, search mode, playback shortcuts, help, quit
- wire a central action/event loop
- add loading and status messages
- add resize handling

Deliverables:

- app responds to keys consistently
- clear separation between user actions and async result events

Exit criteria:

- no direct business logic inside drawing code
- input behavior feels stable under resize and rapid navigation

### Phase 3: Config and authentication

Support local SoundCloud credentials and desktop OAuth without a backend.

Scope:

- define `credentials.toml` format for `client_id`, `client_secret`, `redirect_uri`
- define token persistence format
- implement PKCE generation and state validation
- open browser for auth
- support redirect handling for desktop flow
- support a manual callback paste fallback if automatic callback capture fails
- implement token refresh and expiry handling

Deliverables:

- first-run auth flow
- persisted access and refresh tokens
- startup auth check and refresh path

Exit criteria:

- authenticated session survives restart
- invalid state, expired tokens, and auth failures produce user-facing errors

### Phase 4: SoundCloud API service layer

Fetch and normalize route data from SoundCloud.

Scope:

- implement typed client wrapper around `reqwest`
- normalize SoundCloud responses into app-facing models
- wire endpoints for:
  - `Feed`
  - `Liked Songs`
  - `Following`
  - `Playlists`
  - `Search`
- derive `Albums` from playlist-like resources where appropriate
- implement pagination support via `next_href`
- add route-specific loading and error states

Deliverables:

- live data in core routes
- searchable tracks, playlists, and users

Exit criteria:

- route switching loads real data
- empty, loading, and error states are rendered intentionally

### Phase 5: mpv player backend

Add real playback using a managed `mpv` child process over JSON IPC.

Scope:

- define `PlayerBackend` trait
- implement `MpvPlayerBackend`
- spawn `mpv` with `--idle=yes` and `--input-ipc-server=<socket>`
- keep a persistent IPC connection open
- implement commands:
  - load track
  - play/pause/toggle
  - stop
  - seek
  - set volume
- observe properties such as:
  - `pause`
  - `playback-time`
  - `duration`
  - `volume`
  - end-of-track state
- resolve SoundCloud stream URLs before playback
- keep app-owned metadata separate from `mpv`'s inferred metadata

Deliverables:

- play a selected track from the TUI
- playbar updates from live player events
- track end advances queue correctly

Exit criteria:

- playback survives multiple track loads
- seek, pause, resume, and volume all round-trip through the reducer cleanly

### Phase 6: App-owned MPRIS integration

Expose `soundcloud-tui` as a real MPRIS player on Linux.

Scope:

- add Linux-only `integrations/mpris.rs`
- use `mpris-server`'s ready-to-use `Player` interface first
- register as `org.mpris.MediaPlayer2.soundcloud-tui`
- publish:
  - metadata
  - playback status
  - position
  - volume
  - shuffle
  - repeat/loop status
- receive:
  - play
  - pause
  - play/pause
  - next
  - previous
  - stop
  - seek
  - set position
- bridge inbound MPRIS commands into normal app actions
- update MPRIS from the same app state used by the TUI

Deliverables:

- `playerctl --player=soundcloud-tui metadata` works
- media keys work through MPRIS
- Waybar and Quickshell can display and control playback

Exit criteria:

- MPRIS state remains correct after seeking, pausing, track changes, and stop events
- no dependency on `mpv-mpris`

### Phase 7: Local history, queueing, and polish

Fill the main UX gaps needed for an actually usable first release.

Scope:

- persist `Recently Played` locally
- add a queue model and queue navigation behavior
- improve playlist browsing and content tables
- add better empty states and recoverable error messaging
- cache some route results to reduce redundant loading
- add a help screen and first-run guidance

Deliverables:

- `Recently Played` works across restarts
- queue behavior is predictable
- UX feels coherent beyond the happy path

Exit criteria:

- first-run and repeat-run workflows both feel usable
- app remains stable with missing data, failed requests, and unplayable tracks

### Phase 8: Verification and release readiness

Stabilize the project for actual use.

Scope:

- reducer tests
- model parsing tests
- auth tests for PKCE and callback handling
- mpv IPC parser tests with canned messages
- manual Linux verification checklist for MPRIS, `playerctl`, Waybar, and Quickshell
- packaging notes and runtime dependency documentation
- README with setup instructions

Deliverables:

- contributor-friendly setup
- documented runtime requirements: SoundCloud app credentials and `mpv`
- stable first public milestone

Exit criteria:

- `cargo test` passes
- README setup instructions are enough for a clean Linux machine

## Milestone Definition

The first end-to-end milestone is complete when all of the following work:

- app launches into the full ratatui shell
- user authenticates with SoundCloud locally
- `Feed`, `Liked Songs`, and `Playlists` load real data
- selecting a track starts playback in `mpv`
- now-playing bar updates with live progress
- `playerctl --player=soundcloud-tui metadata` shows the current track
- Waybar or Quickshell can display and control playback over MPRIS

## Major Risks and Mitigations

### SoundCloud auth and API inconsistency

Risk:

- SoundCloud docs and endpoint behavior may be inconsistent, especially around stream handling and albums

Mitigation:

- isolate API-specific behavior in `soundcloud/service.rs`
- normalize responses into app-owned models
- treat `Albums` as a derived route from playlist-like resources

### Playback state drift between app and mpv

Risk:

- app state and `mpv` state can diverge during seeks, errors, or manual child exit

Mitigation:

- make app state update from explicit `PlayerEvent`s
- periodically reconcile critical properties when necessary
- detect child exit and surface a recoverable error state

### MPRIS state drift

Risk:

- MPRIS metadata or status may lag behind actual playback state

Mitigation:

- publish MPRIS updates only from reducer-applied state
- treat MPRIS as a projection of app state, not as a second source of truth

### Unsupported or restricted tracks

Risk:

- some SoundCloud tracks may be preview-only, blocked, or otherwise not streamable

Mitigation:

- surface playback restrictions clearly in the UI
- avoid assuming every track is fully playable

## Out of Scope for v1

- macOS Now Playing integration
- Windows media transport integration
- lyrics
- cover art rendering in terminal
- downloads or offline mode
- multiple playback backends
- advanced MPRIS playlist or track list support
- social or collaborative features

## Recommended Build Order

Build in this order:

1. Phase 0
2. Phase 1
3. Phase 2
4. Phase 3
5. Phase 4
6. Phase 5
7. Phase 6
8. Phase 7
9. Phase 8

Do not start with playback or MPRIS before the reducer, route model, and async event flow exist. Those layers should be stable first so player and desktop integration can plug into them cleanly.
