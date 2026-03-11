use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph, Wrap},
};

use crate::{
    app::AppState,
    visualizer::{SpectrumFrame, VISUALIZER_BANDS, VisualizerStyle},
};

use super::widgets::pane_block;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let title = format!(
        " Visualizer | {} | {} ",
        app.visualizer.style.label(),
        if app.visualizer.capture_active {
            "Capturing"
        } else {
            "Waiting"
        }
    );
    let block = pane_block(title.as_str(), true, app);
    let inner = block.inner(area);

    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    if inner.width < 20 || inner.height < 8 {
        render_compact(frame, inner, app);
        return;
    }

    let sections = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(inner);

    let info = Paragraph::new(info_lines(app))
        .style(Style::default().fg(app.theme().text))
        .wrap(Wrap { trim: true });
    frame.render_widget(info, sections[0]);

    if app.visualizer.capture_active {
        match app.visualizer.style {
            VisualizerStyle::Equalizer => {
                render_equalizer(frame, sections[1], &app.visualizer.spectrum, app)
            }
            VisualizerStyle::BarGraph => {
                render_bar_graph(frame, sections[1], &app.visualizer.spectrum, app)
            }
        }
    } else {
        render_empty_state(frame, sections[1], app);
    }

    let footer = Paragraph::new("v toggle | V cycle style | Esc/q close")
        .style(Style::default().fg(app.theme().inactive));
    frame.render_widget(footer, sections[2]);
}

fn render_compact(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let text = if app.visualizer.capture_active {
        format!(
            "{} | Peak {:>3}%",
            app.visualizer.style.label(),
            (app.visualizer.spectrum.peak * 100.0).round() as u16
        )
    } else {
        app.visualizer.status.clone()
    };

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(app.theme().text))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn info_lines(app: &AppState) -> Vec<Line<'static>> {
    let capture_status = if app.visualizer.capture_active {
        format!(
            "Capture active | Peak {:>3}%",
            (app.visualizer.spectrum.peak * 100.0).round() as u16
        )
    } else {
        "Capture unavailable".to_string()
    };

    let title = match app.now_playing.track.as_ref() {
        Some(track) => format!("Now playing: {} - {}", track.artist, track.title),
        None => "Now playing: idle".to_string(),
    };

    vec![
        Line::from(vec![
            Span::styled(capture_status, Style::default().fg(app.theme().banner)),
            Span::raw(" | "),
            Span::styled(
                app.visualizer.status.clone(),
                Style::default().fg(app.theme().text),
            ),
        ]),
        Line::from(Span::styled(
            title,
            Style::default().fg(app.theme().inactive),
        )),
    ]
}

fn render_empty_state(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let lines = vec![
        Line::from(app.visualizer.status.clone()),
        Line::from(""),
        Line::from(platform_hint()),
    ];
    let paragraph = Paragraph::new(lines)
        .style(Style::default().fg(app.theme().inactive))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_equalizer(frame: &mut Frame<'_>, area: Rect, spectrum: &SpectrumFrame, app: &AppState) {
    let band_count = VISUALIZER_BANDS as u16;
    let gap: u16 = if area.width >= band_count * 2 { 1 } else { 0 };
    let usable_width = area
        .width
        .saturating_sub(gap.saturating_mul(band_count.saturating_sub(1)));
    let bar_width = (usable_width / band_count).max(1);
    let total_width = bar_width * band_count + gap * band_count.saturating_sub(1);
    let start_x = area.x + area.width.saturating_sub(total_width) / 2;

    for (index, band) in spectrum.bands.iter().copied().enumerate() {
        let x = start_x + index as u16 * (bar_width + gap);
        let filled_rows = ((band.clamp(0.0, 1.0)) * area.height as f32).round() as u16;
        for row in 0..filled_rows.min(area.height) {
            let y = area.y + area.height.saturating_sub(row + 1);
            let intensity = row as f32 / area.height.max(1) as f32;
            let color = vertical_gradient(app, intensity);
            for dx in 0..bar_width {
                write_cell(frame, x + dx, y, '█', color);
            }
        }
    }
}

fn render_bar_graph(frame: &mut Frame<'_>, area: Rect, spectrum: &SpectrumFrame, app: &AppState) {
    let columns = interpolate_bands(&spectrum.bands, area.width as usize);
    let total_steps = area.height.saturating_mul(8);

    for (index, value) in columns.iter().copied().enumerate() {
        let filled_steps = (value.clamp(0.0, 1.0) * total_steps as f32).round() as u16;
        let x = area.x + index as u16;
        let color = horizontal_gradient(app, index as f32 / area.width.max(1) as f32, value);

        for row in 0..area.height {
            let base = row.saturating_mul(8);
            let remainder = filled_steps.saturating_sub(base).min(8);
            if remainder == 0 {
                continue;
            }

            let y = area.y + area.height.saturating_sub(row + 1);
            write_cell(frame, x, y, block_for_steps(remainder), color);
        }
    }
}

fn interpolate_bands(bands: &[f32; VISUALIZER_BANDS], target_width: usize) -> Vec<f32> {
    if target_width == 0 {
        return Vec::new();
    }

    if target_width <= bands.len() {
        return bands[..target_width].to_vec();
    }

    let mut values = Vec::with_capacity(target_width);
    let scale = (bands.len() - 1) as f32 / (target_width - 1) as f32;

    for index in 0..target_width {
        let position = index as f32 * scale;
        let left = position.floor() as usize;
        let right = (left + 1).min(bands.len() - 1);
        let fraction = position - left as f32;
        let value = bands[left] * (1.0 - fraction) + bands[right] * fraction;
        values.push(value);
    }

    values
}

fn write_cell(frame: &mut Frame<'_>, x: u16, y: u16, ch: char, color: Color) {
    let cell = &mut frame.buffer_mut()[(x, y)];
    cell.set_char(ch);
    cell.set_fg(color);
}

fn block_for_steps(steps: u16) -> char {
    match steps {
        0 => ' ',
        1 => '▁',
        2 => '▂',
        3 => '▃',
        4 => '▄',
        5 => '▅',
        6 => '▆',
        7 => '▇',
        _ => '█',
    }
}

fn vertical_gradient(app: &AppState, t: f32) -> Color {
    mix_colors(app.theme().active, app.theme().hint, t)
}

fn horizontal_gradient(app: &AppState, position: f32, value: f32) -> Color {
    let base = mix_colors(
        app.theme().active,
        app.theme().banner,
        position.clamp(0.0, 1.0),
    );
    mix_colors(base, app.theme().hint, value.clamp(0.0, 1.0) * 0.35)
}

fn mix_colors(left: Color, right: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (lr, lg, lb) = rgb_triplet(left);
    let (rr, rg, rb) = rgb_triplet(right);
    Color::Rgb(
        lerp_channel(lr, rr, t),
        lerp_channel(lg, rg, t),
        lerp_channel(lb, rb, t),
    )
}

fn lerp_channel(left: u8, right: u8, t: f32) -> u8 {
    (left as f32 + (right as f32 - left as f32) * t).round() as u8
}

fn rgb_triplet(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (220, 50, 47),
        Color::Green => (133, 153, 0),
        Color::Yellow => (181, 137, 0),
        Color::Blue => (38, 139, 210),
        Color::Magenta => (211, 54, 130),
        Color::Cyan => (42, 161, 152),
        Color::Gray => (128, 128, 128),
        Color::DarkGray => (96, 96, 96),
        Color::LightRed => (255, 85, 85),
        Color::LightGreen => (80, 250, 123),
        Color::LightYellow => (241, 250, 140),
        Color::LightBlue => (139, 233, 253),
        Color::LightMagenta => (255, 121, 198),
        Color::LightCyan => (102, 217, 239),
        Color::White | Color::Reset => (240, 240, 240),
        _ => (240, 240, 240),
    }
}

fn platform_hint() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "Linux needs a monitor-style input exposed by PipeWire or PulseAudio."
    }

    #[cfg(target_os = "windows")]
    {
        "Windows uses WASAPI loopback on the default output device."
    }

    #[cfg(target_os = "macos")]
    {
        "macOS needs a loopback device such as BlackHole or Loopback."
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        "This platform may require a manual loopback capture device."
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::{app::AppState, visualizer::SpectrumFrame};

    #[test]
    fn render_handles_small_viewports() {
        let backend = TestBackend::new(18, 6);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = AppState::new();
        app.visualizer.visible = true;

        terminal
            .draw(|frame| render(frame, frame.area(), &app))
            .expect("visualizer should render on tiny viewports");
    }

    #[test]
    fn render_handles_active_capture() {
        let backend = TestBackend::new(60, 18);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = AppState::new();
        app.visualizer.visible = true;
        app.visualizer.capture_active = true;
        app.visualizer.spectrum = SpectrumFrame {
            bands: [
                0.1, 0.15, 0.2, 0.35, 0.5, 0.65, 0.75, 0.7, 0.55, 0.4, 0.25, 0.15,
            ],
            peak: 0.75,
        };

        terminal
            .draw(|frame| render(frame, frame.area(), &app))
            .expect("visualizer should render active frames");
    }
}
