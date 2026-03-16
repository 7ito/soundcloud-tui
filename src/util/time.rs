pub fn format_seconds(seconds: u64) -> String {
    let minutes = seconds / 60;
    let remainder = seconds % 60;
    format!("{minutes}:{remainder:02}")
}

pub fn format_seconds_f64(seconds: f64) -> String {
    format_seconds(seconds.max(0.0).round() as u64)
}
