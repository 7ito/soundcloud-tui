pub fn format_seconds(seconds: u64) -> String {
    let minutes = seconds / 60;
    let remainder = seconds % 60;
    format!("{minutes}:{remainder:02}")
}
