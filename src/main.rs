use std::{io, io::IsTerminal, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        size as terminal_size,
    },
};
use log::{info, warn};
use ratatui::{Terminal, backend::CrosstermBackend};
use soundcloud_tui::{
    app::{AppEvent, AppState},
    config::{self, paths::AppPaths},
    input::events::EventHandler,
    integrations::media_controls::MediaControlsIntegration,
    player::runtime::PlayerHandle,
    runtime::CommandExecutor,
    soundcloud::auth,
    ui::{self, cover_art::CoverArtRenderer},
    visualizer::VisualizerHandle,
};
use tokio::{sync::mpsc, task::LocalSet};

#[tokio::main]
async fn main() {
    let local = LocalSet::new();

    if let Err(error) = local.run_until(run()).await {
        eprintln!("soundcloud-tui failed: {error:?}");
    }
}

async fn run() -> Result<()> {
    if !io::stdout().is_terminal() {
        anyhow::bail!("soundcloud-tui must be run in an interactive terminal");
    }

    let paths = AppPaths::discover()?;
    paths.ensure_dirs()?;
    config::settings::ensure_default_file(&paths)?;
    config::init_logging(&paths)?;
    let settings = config::settings::Settings::load(&paths)?;
    let tick_rate_ms = settings.tick_rate_ms;
    let recent_history = config::history::RecentlyPlayedStore::load(&paths)?;

    info!("starting soundcloud-tui auth onboarding scaffold");

    let bootstrap = auth::bootstrap();

    let mut terminal = TerminalHandle::new()?;
    let mut app = AppState::new_onboarding_with_persistence(
        bootstrap.credentials.clone(),
        settings,
        recent_history,
    );
    let (width, height) = terminal_size()?;
    app.viewport.width = width;
    app.viewport.height = height;
    if let Some(warning) = bootstrap.warning {
        app.auth.set_error(warning.clone());
        if let Some(hint) = config::secure_store::troubleshooting_hint(&warning) {
            app.auth.set_info(hint);
        }
        app.status = warning;
    }

    if let Some(tokens) = bootstrap.tokens {
        let credentials = app.auth.credentials();
        if credentials.validate().is_ok() {
            app.begin_saved_session_validation(credentials, tokens);
        }
    }

    let mut events = EventHandler::new(Duration::from_millis(tick_rate_ms));
    let (async_tx, mut async_rx) = mpsc::unbounded_channel::<AppEvent>();
    let player = PlayerHandle::spawn(paths.clone(), async_tx.clone());
    let visualizer = VisualizerHandle::spawn(async_tx.clone());
    let executor = CommandExecutor::new(
        paths.clone(),
        async_tx.clone(),
        player.clone(),
        visualizer.clone(),
    );

    let mut media_controls = match MediaControlsIntegration::new(async_tx.clone()).await {
        Ok(Some(mut integration)) => {
            if let Err(error) = integration.sync_from_app(&app).await {
                warn!("disabling media controls integration after initial sync failure: {error}");
                None
            } else {
                Some(integration)
            }
        }
        Ok(None) => None,
        Err(error) => {
            warn!("media controls integration unavailable: {error}");
            None
        }
    };

    loop {
        if let Some(integration) = media_controls.as_mut() {
            if let Err(error) = integration.pump_main_thread() {
                warn!(
                    "disabling media controls integration after main-thread pump failure: {error}"
                );
                media_controls = None;
            }
        }

        executor.drain(&mut app);
        terminal.draw(&app)?;

        let sync_media_controls = tokio::select! {
            maybe_event = events.next() => {
                let Some(event) = maybe_event else { break; };
                let sync_media_controls = !matches!(event, AppEvent::VisualizerFrame(_));
                app.dispatch_event(event);
                sync_media_controls
            }
            maybe_async = async_rx.recv() => {
                let Some(event) = maybe_async else { break; };
                let sync_media_controls = !matches!(event, AppEvent::VisualizerFrame(_));
                app.dispatch_event(event);
                sync_media_controls
            }
        };

        if sync_media_controls {
            if let Some(integration) = media_controls.as_mut() {
                if let Err(error) = integration.sync_from_app(&app).await {
                    warn!("disabling media controls integration after sync failure: {error}");
                    media_controls = None;
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    info!("shutting down soundcloud-tui");

    Ok(())
}

struct TerminalHandle {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    cover_art: CoverArtRenderer,
}

impl TerminalHandle {
    fn new() -> Result<Self> {
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableBracketedPaste,
            EnableMouseCapture
        )?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        let cover_art = CoverArtRenderer::new();

        Ok(Self {
            terminal,
            cover_art,
        })
    }

    fn draw(&mut self, app: &AppState) -> Result<()> {
        self.terminal
            .draw(|frame| ui::layout::render_app(frame, app, &mut self.cover_art))?;
        Ok(())
    }
}

impl Drop for TerminalHandle {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            DisableBracketedPaste,
            DisableMouseCapture,
            LeaveAlternateScreen
        );
        let _ = self.terminal.show_cursor();
    }
}
