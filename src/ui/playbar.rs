use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{LineGauge, Paragraph, Wrap},
};

use crate::{
    app::{AppState, Focus},
    ui::{
        cover_art::CoverArtRenderer,
        theme::Theme,
        widgets::{header_style, pane_block},
    },
};

const CONTROLS: [&str; 8] = [
    "[Prev]",
    "[Play/Pause]",
    "[Next]",
    "[Shuffle]",
    "[Repeat]",
    "[Like]",
    "[Vol-]",
    "[Vol+]",
];

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState, cover_art: &mut CoverArtRenderer) {
    let title = playbar_title(app);
    let block = pane_block(title.as_str(), app.focus == Focus::Playbar);
    let inner = block.inner(area);

    frame.render_widget(block, area);

    cover_art.sync(
        app.now_playing.artwork_url.as_deref(),
        app.cover_art.bytes.as_deref(),
    );

    if inner.width < 24 || inner.height < 4 {
        render_compact(frame, inner, app);
        return;
    }

    let art_width = (inner.height.saturating_mul(2)).clamp(8, inner.width.saturating_sub(12));
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(art_width), Constraint::Min(1)])
        .spacing(1)
        .split(inner);
    let art_area = layout[0];
    let meta_area = layout[1];

    render_cover_art(frame, art_area, app, cover_art);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(meta_area);

    let title = Paragraph::new(Line::from(Span::styled(
        app.now_playing.title.as_str(),
        header_style(),
    )));
    let artist = Paragraph::new(Line::from(app.now_playing.artist.as_str()));
    let controls = Paragraph::new(Line::from(CONTROLS.join(" "))).alignment(Alignment::Center);
    let progress = LineGauge::default()
        .ratio(app.now_playing.progress_ratio)
        .filled_style(header_style())
        .unfilled_style(Style::default().fg(Theme::default().muted))
        .label(progress_label(app));

    frame.render_widget(title, rows[0]);
    frame.render_widget(artist, rows[1]);
    frame.render_widget(controls, rows[2]);
    frame.render_widget(progress, rows[3]);
}

fn render_compact(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let compact = Paragraph::new(vec![
        Line::from(Span::styled(app.now_playing.title.as_str(), header_style())),
        Line::from(progress_label(app)),
    ])
    .wrap(Wrap { trim: true });

    frame.render_widget(compact, area);
}

fn render_cover_art(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &AppState,
    cover_art: &mut CoverArtRenderer,
) {
    if cover_art.render(frame, area) {
        return;
    }

    let placeholder = if app.cover_art.loading {
        "Loading\ncover"
    } else if app.now_playing.track.is_some() {
        "No\ncover"
    } else {
        "No\ntrack"
    };

    let widget = Paragraph::new(placeholder)
        .style(Style::default().fg(Theme::default().muted))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(widget, area);
}

fn playbar_title(app: &AppState) -> String {
    format!(
        " {} | Shuffle: {} | Repeat: {} | Volume: {:.0}% ",
        app.player.status.label(),
        if app.player.shuffle_enabled {
            "On"
        } else {
            "Off"
        },
        app.player.repeat_mode.label(),
        app.player.volume_percent.round()
    )
}

fn progress_label(app: &AppState) -> String {
    let remaining = app
        .player
        .duration_seconds
        .map(|duration| {
            format!(
                "-{}",
                format_seconds_f64(duration - app.player.position_seconds)
            )
        })
        .unwrap_or_else(|| "-0:00".to_string());

    format!(
        "{}/{} ({})",
        app.now_playing.elapsed_label, app.now_playing.duration_label, remaining
    )
}

fn format_seconds_f64(seconds: f64) -> String {
    let seconds = seconds.max(0.0).round() as u64;
    let minutes = seconds / 60;
    let remainder = seconds % 60;
    format!("{minutes}:{remainder:02}")
}
