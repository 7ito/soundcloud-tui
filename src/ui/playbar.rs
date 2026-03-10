use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
};

use crate::{
    app::{AppState, Focus},
    ui::widgets::{header_style, pane_block},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let block = pane_block("Now Playing", app.focus == Focus::Playbar);
    let inner = block.inner(area);

    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let title = Paragraph::new(Line::from(vec![
        Span::styled(app.now_playing.title.as_str(), header_style()),
        Span::raw(format!(
            "  |  {}  |  Vol {:.0}%",
            app.player.status.label(),
            app.player.volume_percent.round()
        )),
    ]));
    let meta = Paragraph::new(Line::from(format!(
        "{}  |  {}  |  {}",
        app.now_playing.artist,
        app.now_playing.context,
        app.queue_status_label()
    )));
    let gauge = Gauge::default()
        .ratio(app.now_playing.progress_ratio)
        .label(format!(
            "{} / {}  |  {}",
            app.now_playing.elapsed_label, app.now_playing.duration_label, app.status
        ));

    frame.render_widget(title, rows[0]);
    frame.render_widget(meta, rows[1]);
    frame.render_widget(gauge, rows[2]);
}
