use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, LineGauge, Padding, Paragraph, Wrap},
};
use ratatui_image::picker::ProtocolType;

use crate::app::state::PlaybackStatus;
use crate::{
    app::{AppState, Focus},
    ui::{
        cover_art::CoverArtRenderer,
        widgets::{header_style, pane_block},
    },
};

const CONTROLS: [&str; 11] = [
    "[Prev]",
    "[Play/Pause]",
    "[Next]",
    "[Q Queue]",
    "[z Add]",
    "[Shuffle]",
    "[Repeat]",
    "[W Playlist]",
    "[L Like]",
    "[Vol-]",
    "[Vol+]",
];

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState, cover_art: &mut CoverArtRenderer) {
    let title = playbar_title(app);
    let block = pane_block(title.as_str(), app.focus == Focus::Playbar, app);
    let inner = block.inner(area);

    frame.render_widget(block, area);

    if should_draw_cover_art(app, cover_art) {
        cover_art.sync(
            app.now_playing.artwork_url.as_deref(),
            app.cover_art.bytes.as_deref(),
        );
    } else {
        cover_art.sync(None, None);
    }

    if inner.width < 24 || inner.height < 4 {
        render_compact(frame, inner, app);
        return;
    }

    let show_cover_art = should_draw_cover_art(app, cover_art) && inner.width >= 32;
    let meta_area = if show_cover_art {
        let art_width = (inner.height.saturating_mul(2)).clamp(8, inner.width.saturating_sub(12));
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(art_width), Constraint::Min(1)])
            .spacing(1)
            .split(inner);
        render_cover_art(frame, layout[0], app, cover_art);
        layout[1]
    } else {
        inner
    };

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
        header_style(app),
    )));
    let artist = Paragraph::new(Line::from(app.now_playing.artist.as_str()))
        .style(Style::default().fg(app.theme().text));
    let controls = Paragraph::new(Line::from(CONTROLS.join(" "))).alignment(Alignment::Center);
    let progress = LineGauge::default()
        .ratio(app.now_playing.progress_ratio)
        .filled_style(Style::default().fg(app.theme().playbar_progress))
        .unfilled_style(Style::default().fg(app.theme().inactive))
        .label(progress_label(app));

    if app.theme().playbar_background != ratatui::style::Color::Reset {
        frame.render_widget(
            Block::default().style(Style::default().bg(app.theme().playbar_background)),
            meta_area,
        );
    }

    frame.render_widget(title, rows[0]);
    frame.render_widget(artist, rows[1]);
    frame.render_widget(controls, rows[2]);
    frame.render_widget(progress, rows[3]);

    render_toast(frame, meta_area, app);
}

fn render_compact(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let compact = Paragraph::new(vec![
        Line::from(Span::styled(
            app.now_playing.title.as_str(),
            header_style(app),
        )),
        Line::from(progress_label(app)),
    ])
    .style(Style::default().fg(app.theme().text))
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
        .style(Style::default().fg(app.theme().inactive))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(widget, area);
}

fn render_toast(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let Some(toast) = app.toast.as_ref() else {
        return;
    };

    if area.width < 24 || area.height < 3 {
        return;
    }

    let width = toast
        .message
        .chars()
        .count()
        .saturating_add(4)
        .clamp(18, 34) as u16;
    let width = width.min(area.width);
    let toast_area = Rect {
        x: area.x.saturating_add(area.width.saturating_sub(width)),
        y: area.y,
        width,
        height: 3.min(area.height),
    };
    let theme = app.theme();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(Padding::horizontal(1))
        .border_style(Style::default().fg(theme.hovered));
    let paragraph = Paragraph::new(toast.message.as_str())
        .style(Style::default().fg(theme.hovered))
        .alignment(Alignment::Center)
        .block(block);

    frame.render_widget(Clear, toast_area);
    frame.render_widget(paragraph, toast_area);
}

fn playbar_title(app: &AppState) -> String {
    let status = match app.player.status {
        PlaybackStatus::Playing => format!("{} Playing", app.settings().playing_icon),
        PlaybackStatus::Paused => format!("{} Paused", app.settings().paused_icon),
        PlaybackStatus::Buffering => "Loading".to_string(),
        PlaybackStatus::Stopped => "Stopped".to_string(),
    };

    format!(
        " {} | {} | Volume: {:.0}% ",
        status,
        app.queue_status_label(),
        app.player.volume_percent.round()
    )
}

fn should_draw_cover_art(app: &AppState, cover_art: &CoverArtRenderer) -> bool {
    if !app.settings().draw_cover_art {
        return false;
    }

    app.settings().force_draw_cover_art || cover_art.protocol_type() != ProtocolType::Halfblocks
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
