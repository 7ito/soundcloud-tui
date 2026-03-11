use ratatui::{
    Frame,
    layout::{Alignment, Position, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    app::{AppState, Focus},
    config::settings::KeyAction,
    ui::widgets::{HIGHLIGHT_SYMBOL, header_style, pane_block},
};

pub fn render_search(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let block = pane_block("Search", app.focus == Focus::Search, app);
    let inner = block.inner(area);
    let available_width = inner
        .width
        .saturating_sub(HIGHLIGHT_SYMBOL.chars().count() as u16) as usize;
    let (offset, visible_query) = visible_search_query(
        app.search_query.as_str(),
        app.search_cursor,
        available_width,
    );
    let search = Paragraph::new(Line::from(vec![
        Span::styled(HIGHLIGHT_SYMBOL, header_style(app)),
        Span::raw(visible_query),
    ]))
    .block(block);

    frame.render_widget(search, area);

    if app.focus == Focus::Search {
        let visible_cursor = app.search_cursor.saturating_sub(offset) as u16;
        let cursor_x = inner
            .x
            .saturating_add(HIGHLIGHT_SYMBOL.chars().count() as u16)
            .saturating_add(visible_cursor)
            .min(inner.x.saturating_add(inner.width.saturating_sub(1)));
        frame.set_cursor_position(Position::new(cursor_x, inner.y));
    }
}

pub fn render_help(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let label = if area.width < 12 {
        "?".to_string()
    } else {
        format!("Type {}", app.settings().keybinding(KeyAction::Help))
    };
    let help = Paragraph::new(label)
        .alignment(Alignment::Left)
        .block(pane_block("Help", app.show_help, app));

    frame.render_widget(help, area);
}

pub fn render_settings(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let label = settings_label(
        area.width,
        app.settings().keybinding(KeyAction::OpenSettings),
    );
    let settings = Paragraph::new(label)
        .alignment(Alignment::Left)
        .block(pane_block("Settings", app.show_settings(), app));

    frame.render_widget(settings, area);
}

fn settings_label(width: u16, binding: &str) -> String {
    if width < 14 {
        "Open".to_string()
    } else {
        format!("Type {binding}")
    }
}

fn visible_search_query(query: &str, cursor: usize, width: usize) -> (usize, String) {
    if width == 0 {
        return (cursor, String::new());
    }

    let chars = query.chars().collect::<Vec<_>>();
    if chars.len() <= width {
        return (0, query.to_string());
    }

    let mut start = cursor.saturating_sub(width.saturating_sub(1));
    if start + width > chars.len() {
        start = chars.len().saturating_sub(width);
    }

    let end = (start + width).min(chars.len());
    (start, chars[start..end].iter().collect())
}

#[cfg(test)]
mod tests {
    use super::{settings_label, visible_search_query};

    #[test]
    fn long_queries_scroll_with_cursor() {
        let (offset, visible) = visible_search_query("abcdefghijklmnop", 12, 6);
        assert_eq!(offset, 7);
        assert_eq!(visible, "hijklm");
    }

    #[test]
    fn narrow_settings_button_uses_open_label() {
        assert_eq!(settings_label(13, "alt-,"), "Open");
        assert_eq!(settings_label(14, "alt-,"), "Type alt-,");
    }
}
