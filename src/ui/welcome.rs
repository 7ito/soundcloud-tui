use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Clear, Paragraph, Wrap},
};

use crate::{
    app::AppState,
    ui::{theme::Theme, widgets::pane_block},
};

const SOUND_CLOUD_BANNER_WIDE: &str = concat!(
    "█████████                                     █████   █████████  ████                          █████\n",
    " ███▒▒▒▒▒███                                   ▒▒███   ███▒▒▒▒▒███▒▒███                         ▒▒███ \n",
    "▒███    ▒▒▒   ██████  █████ ████ ████████    ███████  ███     ▒▒▒  ▒███   ██████  █████ ████  ███████ \n",
    "▒▒█████████  ███▒▒███▒▒███ ▒███ ▒▒███▒▒███  ███▒▒███ ▒███          ▒███  ███▒▒███▒▒███ ▒███  ███▒▒███ \n",
    " ▒▒▒▒▒▒▒▒███▒███ ▒███ ▒███ ▒███  ▒███ ▒███ ▒███ ▒███ ▒███          ▒███ ▒███ ▒███ ▒███ ▒███ ▒███ ▒███ \n",
    " ███    ▒███▒███ ▒███ ▒███ ▒███  ▒███ ▒███ ▒███ ▒███ ▒▒███     ███ ▒███ ▒███ ▒███ ▒███ ▒███ ▒███ ▒███ \n",
    "▒▒█████████ ▒▒██████  ▒▒████████ ████ █████▒▒████████ ▒▒█████████  █████▒▒██████  ▒▒████████▒▒████████\n",
    " ▒▒▒▒▒▒▒▒▒   ▒▒▒▒▒▒    ▒▒▒▒▒▒▒▒ ▒▒▒▒ ▒▒▒▒▒  ▒▒▒▒▒▒▒▒   ▒▒▒▒▒▒▒▒▒  ▒▒▒▒▒  ▒▒▒▒▒▒    ▒▒▒▒▒▒▒▒  ▒▒▒▒▒▒▒▒ ",
);

const SOUND_CLOUD_BANNER_COMPACT: &str = concat!(
    "  ____                      _ _                 _ \n",
    " / ___|  ___  _   _ _ __   __| | | ___  _   _  __| |\n",
    " \\___ \\ / _ \\| | | | '_ \\ / _` | |/ _ \\| | | |/ _` |\n",
    "  ___) | (_) | |_| | | | | (_| | | (_) | |_| | (_| |\n",
    " |____/ \\___/ \\__,_|_| |_|\\__,_|_|\\___/ \\__,_|\\__,_|",
);

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let block = pane_block("Welcome", true, app);
    let inner = block.inner(area);
    let banner = banner_for_width(inner.width);
    let banner_height = banner.lines().count() as u16;
    let content_height = banner_height.saturating_add(4).min(inner.height);
    let centered = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([Constraint::Length(content_height)])
        .flex(Flex::Center)
        .split(inner)[0];
    let sections = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(banner_height.min(centered.height)),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(centered);

    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let banner_widget = Paragraph::new(Text::from(build_banner_gradient_lines(
        &app.theme(),
        banner,
        app.tick_count,
    )))
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: false });

    let footer_widget = Paragraph::new(vec![
        Line::from(Span::styled(
            "soundcloud in your terminal",
            Style::default().fg(app.theme().inactive),
        )),
        Line::from(Span::styled(
            "Press / to search, z to queue, Q to view queue, Tab to move panes, Enter to open",
            Style::default().fg(app.theme().inactive),
        )),
        Line::from(Span::styled(
            "Press any key to continue",
            Style::default()
                .fg(app.theme().hovered)
                .add_modifier(Modifier::BOLD),
        )),
    ])
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true });

    frame.render_widget(banner_widget, sections[0]);
    frame.render_widget(footer_widget, sections[2]);
}

fn banner_for_width(width: u16) -> &'static str {
    if width >= 118 {
        SOUND_CLOUD_BANNER_WIDE
    } else {
        SOUND_CLOUD_BANNER_COMPACT
    }
}

fn build_banner_gradient_lines(theme: &Theme, banner: &str, tick_count: u64) -> Vec<Line<'static>> {
    let phase = (tick_count as f32 * 0.06) % 1.0;

    banner
        .lines()
        .enumerate()
        .map(|(row, line)| {
            let line_len = line.chars().count().max(1) as f32;
            let spans = line
                .chars()
                .enumerate()
                .map(|(col, ch)| {
                    let t = ((col as f32 / line_len) + (row as f32 * 0.08) + phase) % 1.0;
                    Span::styled(
                        ch.to_string(),
                        Style::default().fg(gradient_color(theme, t)),
                    )
                })
                .collect::<Vec<_>>();
            Line::from(spans)
        })
        .collect()
}

fn gradient_color(theme: &Theme, t: f32) -> Color {
    let palette = [theme.active, theme.banner, theme.hint, theme.active];
    let segment_count = (palette.len() - 1) as f32;
    let scaled = (t.clamp(0.0, 0.9999)) * segment_count;
    let index = scaled.floor() as usize;
    let local_t = scaled.fract();

    lerp_color(palette[index], palette[index + 1], local_t)
}

fn lerp_color(start: Color, end: Color, t: f32) -> Color {
    let (sr, sg, sb) = to_rgb(start);
    let (er, eg, eb) = to_rgb(end);

    Color::Rgb(
        lerp_channel(sr, er, t),
        lerp_channel(sg, eg, t),
        lerp_channel(sb, eb, t),
    )
}

fn lerp_channel(start: u8, end: u8, t: f32) -> u8 {
    let start = start as f32;
    let end = end as f32;
    (start + ((end - start) * t)).round() as u8
}

fn to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (255, 0, 0),
        Color::Green => (0, 255, 0),
        Color::Yellow => (255, 255, 0),
        Color::Blue => (0, 0, 255),
        Color::Magenta => (255, 0, 255),
        Color::Cyan => (0, 255, 255),
        Color::Gray => (128, 128, 128),
        Color::DarkGray => (64, 64, 64),
        Color::LightRed => (255, 128, 128),
        Color::LightGreen => (128, 255, 128),
        Color::LightYellow => (255, 255, 128),
        Color::LightBlue => (128, 128, 255),
        Color::LightMagenta => (255, 128, 255),
        Color::LightCyan => (128, 255, 255),
        Color::White | Color::Reset => (255, 255, 255),
        _ => (255, 255, 255),
    }
}
