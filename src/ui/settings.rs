use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};

use crate::{
    app::{AppState, SettingsTab, SettingsValue},
    config::settings::KeyAction,
    ui::widgets::{header_style, pane_block, selected_row_style, HIGHLIGHT_SYMBOL},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let Some(_menu) = app.settings_menu.as_ref() else {
        return;
    };

    let overlay = Layout::default()
        .margin(1)
        .constraints([Constraint::Min(1)])
        .split(area)[0];
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(overlay);

    frame.render_widget(Clear, area);
    render_tabs(frame, sections[0], app);
    render_list(frame, sections[1], app);
    render_footer(frame, sections[2], app);
}

fn render_tabs(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let menu = app.settings_menu.as_ref().expect("settings menu state");
    let titles = SettingsTab::ALL
        .iter()
        .map(|tab| Line::from(tab.label()))
        .collect::<Vec<_>>();
    let tabs = Tabs::new(titles)
        .select(menu.tab.index())
        .block(pane_block("Settings (←/→ to switch tabs)", true, app))
        .highlight_style(header_style(app))
        .style(Style::default().fg(app.theme().text));

    frame.render_widget(tabs, area);
}

fn render_list(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let menu = app.settings_menu.as_ref().expect("settings menu state");
    let items = menu.items();
    let title = format!("{} Settings ({} items)", menu.tab.label(), items.len());
    let list_items = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let editing_selected = menu.editing && index == menu.selected_index();
            let value = if editing_selected {
                format!("{}|", menu.edit_buffer)
            } else {
                item.display_value()
            };
            let label_style = if index == menu.selected_index() {
                header_style(app)
            } else {
                Style::default().fg(app.theme().text)
            };
            let value_style = if editing_selected {
                Style::default().fg(app.theme().hint)
            } else if index == menu.selected_index() {
                Style::default().fg(app.theme().selected)
            } else {
                Style::default().fg(app.theme().inactive)
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!("{}: ", item.name), label_style),
                Span::styled(value, value_style),
            ]))
        })
        .collect::<Vec<_>>();
    let list = List::new(list_items)
        .block(pane_block(title.as_str(), true, app))
        .highlight_style(selected_row_style(app))
        .highlight_symbol(HIGHLIGHT_SYMBOL);
    let mut state = ListState::default();
    state.select((!items.is_empty()).then_some(menu.selected_index()));
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let menu = app.settings_menu.as_ref().expect("settings menu state");
    let controls = if menu.editing {
        match menu
            .items()
            .get(menu.selected_index())
            .map(|item| &item.value)
        {
            Some(SettingsValue::Key(_)) => "Press any key to set binding | Esc: Cancel".to_string(),
            Some(SettingsValue::Number(_))
            | Some(SettingsValue::Text(_))
            | Some(SettingsValue::Color(_)) => {
                "Type to edit | Enter: Confirm | Esc: Cancel".to_string()
            }
            _ => "Enter: Confirm | Esc: Cancel".to_string(),
        }
    } else {
        return_controls(app)
    };
    let hint = if menu.has_unsaved_changes(app.settings()) {
        "Unsaved changes"
    } else {
        "Saved settings match current runtime state"
    };

    let footer = Paragraph::new(vec![
        Line::from(controls),
        Line::from(Span::styled(hint, Style::default().fg(app.theme().hint))),
    ])
    .block(pane_block("Controls", false, app));

    frame.render_widget(footer, area);
}

fn return_controls(app: &AppState) -> String {
    format!(
        "↑/↓: Select | ←/→: Switch Tab | Enter: Toggle/Edit | Mouse: Click/Scroll | {}: Save | Esc/{}: Exit",
        format_key_hint(app.settings().keybinding(KeyAction::SaveSettings)),
        format_key_hint(app.settings().keybinding(KeyAction::Back)),
    )
}

fn format_key_hint(binding: &str) -> String {
    if let Some(key) = binding.strip_prefix("alt-") {
        return format!("<Alt+{}>", key);
    }

    binding.to_string()
}
